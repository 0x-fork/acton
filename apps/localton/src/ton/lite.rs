use std::{
    fs,
    net::Ipv4Addr,
    path::Path,
    time::{Duration, Instant},
};

use crate::ton::tools::types::parse_account_address;
use anyhow::{Context, Result, anyhow, ensure};
use crc::{CRC_16_XMODEM, Crc};
use num_bigint::BigInt;
use rston::{
    boc::Boc,
    cell::{Cell, CellFamily, LoadCell},
    merkle::MerkleProof,
    models::{Block, ShardIdent, ShardStateUnsplit, config::BlockchainConfigParams},
    prelude::HashBytes,
};
use rston::{
    liteclient::{ConfigGlobal, ConfigLiteServer, ConfigPublicKey, LiteClient, LiteClientOptions},
    models::{AccountState, OptionalAccount},
    tl::{
        Int256,
        common::{AccountId, BlockId, BlockIdExt},
        request::{
            GetAccountState, GetBlock, GetConfigParams, GetState, ListBlockTransactions,
            LookupBlock, Request, RunSmcMethod, SendMessage,
        },
        response::{
            AccountState as AccountStateResponse, BlockData, BlockHeader, BlockState,
            BlockTransactions, ConfigInfo, MasterchainInfo, RunMethodResult, SendMsgStatus,
            TransactionId,
        },
    },
};
use serde::{Deserialize, Serialize};
use ton_hardfork::{HardforkPrevBlock, HardforkSources, ShardSource};
use tracing::info;
use tvm_ffi::{
    serde::serialize_tuple,
    stack::{Tuple, TupleItem},
};

use crate::ton::{global_config::GlobalConfig, tools::types::TonPublicKey};

/// Requests the result and proof material returned by official `runmethod`.
///
/// `mode.2` carries the result stack. The remaining bits retain the shard and
/// state proofs plus library extras, so this adapter can add local verification
/// later without changing its wire contract.
const RUN_METHOD_MODE: u32 = 0x17;

/// Computes the selector used by TON to dispatch a named get method.
///
/// Named methods use CRC-16/XMODEM with bit 16 set, as in the official lite-client.
fn ton_method_id(name: &str) -> u64 {
    const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_XMODEM);
    u64::from(CRC16.checksum(name.as_bytes())) | 0x1_0000
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockRef {
    pub workchain: i32,
    pub shard: String,
    pub seqno: u32,
    pub root_hash: String,
    pub file_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionRef {
    pub account: Option<String>,
    pub lt: Option<u64>,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountInfo {
    pub address: String,
    pub state: String,
    pub balance_nano: String,
    pub last_transaction_lt: Option<u64>,
    pub last_transaction_hash: Option<String>,
    pub block: BlockRef,
    pub shard_block: BlockRef,
}

pub struct LocalLiteClient {
    inner: LiteClient,
}

impl LocalLiteClient {
    /// Connects to the first reachable liteserver in configured priority order.
    ///
    /// Every endpoint has a bounded connection attempt so one unavailable server
    /// cannot prevent the remaining authenticated endpoints from being tried.
    pub async fn connect(global_config: &Path) -> Result<Self> {
        let config = GlobalConfig::load(global_config)?;
        Self::connect_config(liteapi_config(&config)).await
    }

    /// Connects directly to one host-local liteserver from its typed endpoint identity.
    ///
    /// The liteserver client only needs an address and authentication key, so this
    /// path neither reads nor rewrites the durable network configuration.
    pub async fn connect_node(port: u16, public_key: TonPublicKey) -> Result<Self> {
        let config = ConfigGlobal {
            liteservers: vec![liteapi_liteserver(Ipv4Addr::LOCALHOST, port, public_key)],
        };

        Self::connect_config(config).await
    }

    async fn connect_config(config: ConfigGlobal) -> Result<Self> {
        const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

        if config.liteservers.is_empty() {
            return Err(anyhow!("global config has no liteservers"));
        }

        let mut failures = Vec::with_capacity(config.liteservers.len());
        for liteserver in &config.liteservers {
            let endpoint = liteserver.socket_addr();
            match LiteClient::connect_liteserver(
                liteserver,
                LiteClientOptions {
                    connect_timeout: CONNECT_TIMEOUT,
                    ..Default::default()
                },
            )
            .await
            {
                Ok(inner) => return Ok(Self { inner }),
                Err(error) => failures.push(format!("{endpoint}: {error}")),
            }
        }

        Err(anyhow!(
            "failed to connect to any configured liteserver: {}",
            failures.join("; ")
        ))
    }

    /// Downloads both chain states at one paused head and authenticates their roots.
    /// The caller owns suspending block production; a changed head rejects the snapshot.
    pub async fn hardfork_sources(&mut self, cache_dir: &Path) -> Result<HardforkSources> {
        let head = self
            .inner
            .query_typed::<MasterchainInfo>(Request::GetMasterchainInfo)
            .await?
            .last;
        let mc = self.complete_state(head.clone(), cache_dir).await?;
        let state = mc.parse::<ShardStateUnsplit>()?;
        let shards = state
            .custom
            .as_ref()
            .context("Missing masterchain config")?
            .load()?
            .shards;
        let mut basechain = None;
        for entry in shards.iter() {
            let (shard, descr) = entry?;
            ensure!(
                shard == ShardIdent::BASECHAIN,
                "Administrative edits currently require one unsplit basechain shard"
            );
            let id = BlockIdExt {
                workchain: 0,
                shard: shard.prefix() as i64,
                seqno: descr.seqno as i32,
                root_hash: Int256(descr.root_hash.0),
                file_hash: Int256(descr.file_hash.0),
            };
            let cell = self.complete_state(id, cache_dir).await?;
            basechain = Some(ShardSource {
                shard,
                state: cell,
                prev: HardforkPrevBlock {
                    seqno: descr.seqno,
                    root_hash: descr.root_hash,
                    file_hash: descr.file_hash,
                },
            });
        }
        ensure!(
            self.inner
                .query_typed::<MasterchainInfo>(Request::GetMasterchainInfo)
                .await?
                .last
                == head,
            "Chain advanced while reading hardfork sources"
        );
        Ok(HardforkSources {
            masterchain_state: mc,
            masterchain_prev: HardforkPrevBlock {
                seqno: head.seqno as u32,
                root_hash: HashBytes(head.root_hash.0),
                file_hash: HashBytes(head.file_hash.0),
            },
            basechain,
        })
    }

    async fn checked_block(&mut self, id: BlockIdExt) -> Result<Block> {
        let bytes = self
            .inner
            .query_typed::<BlockData>(Request::GetBlock(GetBlock { id: id.clone() }))
            .await?
            .data;
        let root = Boc::decode(&bytes)?;
        ensure!(
            root.repr_hash().0 == id.root_hash.0 && Boc::file_hash(&bytes).0 == id.file_hash.0,
            "Block hash mismatch"
        );
        Ok(root.parse()?)
    }

    /// Stock TON refuses getState for seqno > 1000. Bootstrap from an allowed
    /// state, then authenticate and apply block updates. The reusable cache is
    /// verified against the current chain, including after a snapshot restore.
    async fn complete_state(&mut self, id: BlockIdExt, cache_dir: &Path) -> Result<Cell> {
        let started = Instant::now();
        let shard = format!("{:016x}", id.shard as u64);
        let path = cache_dir.join(format!("{}-{shard}.boc", id.workchain));
        let mut cached = None;
        if let Ok(bytes) = fs::read(&path)
            && let Ok(cell) = Boc::decode(bytes)
            && let Ok(state) = cell.parse::<ShardStateUnsplit>()
            && state.seqno <= id.seqno as u32
            && state.shard_ident.workchain() == id.workchain
            && state.shard_ident.prefix() == id.shard as u64
        {
            let block_id = self.lookup(id.workchain, &shard, state.seqno).await?;
            if state.seqno > 0
                && self
                    .checked_block(block_id)
                    .await?
                    .state_update
                    .load()?
                    .new_hash
                    == *cell.repr_hash()
            {
                cached = Some((state.seqno, cell));
            }
        }
        let (mut seqno, mut cell) = match cached {
            Some(value) => value,
            None => {
                let seqno = (id.seqno as u32).min(1000);
                let seed = if seqno == id.seqno as u32 {
                    id.clone()
                } else {
                    self.lookup(id.workchain, &shard, seqno).await?
                };
                let expected = if seqno == 0 {
                    HashBytes(seed.root_hash.0)
                } else {
                    self.checked_block(seed.clone())
                        .await?
                        .state_update
                        .load()?
                        .new_hash
                };
                let cell = Boc::decode(
                    self.inner
                        .query_typed::<BlockState>(Request::GetState(GetState { id: seed }))
                        .await?
                        .data,
                )?;
                ensure!(
                    *cell.repr_hash() == expected,
                    "Initial state does not match its block"
                );
                (seqno, cell)
            }
        };
        info!(operation = "reconstruct_state", workchain = id.workchain, %shard,
            target = id.seqno, from = seqno, "Reconstructing paused chain state");

        fs::create_dir_all(cache_dir)?;
        while seqno < id.seqno as u32 {
            seqno += 1;
            let next = if seqno == id.seqno as u32 {
                id.clone()
            } else {
                self.lookup(id.workchain, &shard, seqno).await?
            };
            let block = self.checked_block(next).await?;
            let info = block.info.load()?;
            ensure!(
                !info.after_merge && !info.after_split,
                "Split and merged shard histories are not supported"
            );
            cell = block
                .state_update
                .load()?
                .apply(&cell)
                .context("State update does not extend the previous state")?;
            if seqno % 128 == 0 {
                save_state_cache(&path, &cell)?;
                info!(operation = "reconstruct_state", workchain = id.workchain, %shard,
                    target = id.seqno, current = seqno, duration_ms = started.elapsed().as_millis(),
                    "Saved state reconstruction checkpoint");
            }
        }

        save_state_cache(&path, &cell)?;
        info!(operation = "reconstruct_state", workchain = id.workchain, %shard,
            target = id.seqno, duration_ms = started.elapsed().as_millis(), outcome = "completed",
            "Paused chain state reconstructed");

        Ok(cell)
    }

    pub async fn last(&mut self) -> Result<BlockRef> {
        let info = self
            .inner
            .query_typed::<MasterchainInfo>(Request::GetMasterchainInfo)
            .await
            .context("getMasterchainInfo failed")?;
        Ok(info.last.into())
    }

    pub async fn account(&mut self, address: &str) -> Result<AccountInfo> {
        let parsed = parse_account_address(address)
            .with_context(|| format!("invalid TON address `{address}`"))?;
        let block = self
            .inner
            .query_typed::<MasterchainInfo>(Request::GetMasterchainInfo)
            .await?
            .last;
        let response = self
            .inner
            .query_typed::<AccountStateResponse>(Request::GetAccountState(GetAccountState {
                id: block,
                account: AccountId {
                    workchain: i32::from(parsed.workchain),
                    id: Int256(parsed.address.0),
                },
            }))
            .await
            .with_context(|| format!("getAccountState failed for {address}"))?;
        account_info(address, response)
    }

    pub async fn block(
        &mut self,
        workchain: i32,
        shard: &str,
        seqno: u32,
    ) -> Result<(BlockRef, Vec<u8>)> {
        let id = self.lookup(workchain, shard, seqno).await?;
        self.download_block(id).await
    }

    pub async fn transactions(
        &mut self,
        workchain: i32,
        shard: &str,
        seqno: u32,
        count: u32,
    ) -> Result<(BlockRef, Vec<TransactionRef>, bool)> {
        let id = self.lookup(workchain, shard, seqno).await?;
        // `ShardStateUnsplit` validates its complete structural envelope while
        // leaving large dictionaries lazy. Ask the liteserver to retain every
        // state/config root needed for that parse; otherwise those references are
        // pruned exotic cells even though the four requested config values exist.
        let response = self
            .inner
            .query_typed::<BlockTransactions>(Request::ListBlockTransactions(
                ListBlockTransactions {
                    id,
                    mode: (),
                    count,
                    after: None,
                    reverse_order: None,
                    want_proof: None,
                },
            ))
            .await
            .context("listBlockTransactions failed")?;
        Ok((
            response.id.into(),
            response.ids.into_iter().map(Into::into).collect(),
            response.incomplete,
        ))
    }

    pub async fn send_boc(&mut self, body: Vec<u8>) -> Result<u32> {
        self.inner
            .query_typed::<SendMsgStatus>(Request::SendMessage(SendMessage { body }))
            .await
            .map(|response| response.status)
            .context("sendMessage failed")
    }

    /// Runs one read-only get method against the latest masterchain state.
    ///
    /// Inputs are integers because Localton's wallet and elector workflows only
    /// need integer stack arguments. Keeping the stack typed prevents them from
    /// constructing release-specific lite-client command strings.
    /// Returns the serialized `VmStack` root so response adapters can preserve
    /// opaque values without normalizing their wire representation.
    pub async fn run_method(
        &mut self,
        address: &str,
        method: &str,
        arguments: Vec<BigInt>,
    ) -> Result<Cell> {
        let account = parse_account_address(address)
            .with_context(|| format!("invalid TON address `{address}`"))?;
        let block = self
            .inner
            .query_typed::<MasterchainInfo>(Request::GetMasterchainInfo)
            .await
            .context("getMasterchainInfo failed before run method")?
            .last;
        let mut stack = Tuple::empty();
        let max_exclusive: BigInt = BigInt::from(1_u8) << 256_usize;
        let min_inclusive = -max_exclusive.clone();
        for argument in arguments {
            anyhow::ensure!(
                argument >= min_inclusive && argument < max_exclusive,
                "get method argument does not fit into a signed TVM int257"
            );
            stack.push(TupleItem::Int(argument));
        }
        let result = self
            .inner
            .query_typed::<RunMethodResult>(Request::RunSmcMethod(RunSmcMethod {
                mode: RUN_METHOD_MODE,
                id: block,
                account: AccountId {
                    workchain: i32::from(account.workchain),
                    id: Int256(account.address.0),
                },
                method_id: ton_method_id(method),
                params: Boc::encode(
                    serialize_tuple(&stack)
                        .context("failed to serialize canonical TVM argument stack")?,
                ),
            }))
            .await
            .with_context(|| format!("run get method `{method}` failed for {address}"))?;
        anyhow::ensure!(
            result.exit_code == 0,
            "run get method `{method}` exited with code {}",
            result.exit_code
        );
        Boc::decode(
            result
                .result
                .context("liteserver omitted the requested TVM result stack")?,
        )
        .context("liteserver returned an invalid canonical TVM result stack")
    }

    /// Reads selected on-chain configuration parameters at the latest block.
    ///
    /// The response contains a Merkle proof of the masterchain state, not a bare
    /// config dictionary. We virtualize that proof and load `McStateExtra.config`
    /// through canonical TON types so application code never guesses at proof-cell
    /// references or relies on the presentation format of the official CLI.
    pub async fn config_params(&mut self, params: Vec<i32>) -> Result<BlockchainConfigParams> {
        let block = self
            .inner
            .query_typed::<MasterchainInfo>(Request::GetMasterchainInfo)
            .await
            .context("getMasterchainInfo failed before config query")?
            .last;
        let response = self
            .inner
            .query_typed::<ConfigInfo>(Request::GetConfigParams(GetConfigParams {
                mode: (),
                id: block,
                param_list: params,
                with_state_root: Some(()),
                with_libraries: Some(()),
                with_state_extra_root: Some(()),
                with_shard_hashes: Some(()),
                with_validator_set: Some(()),
                with_special_smc: Some(()),
                with_accounts_root: Some(()),
                with_prev_blocks: Some(()),
                with_workchain_info: Some(()),
                with_capabilities: Some(()),
                extract_from_key_block: None,
            }))
            .await
            .context("getConfigParams failed")?;
        let proof_root = Boc::decode(&response.config_proof)
            .context("getConfigParams returned an invalid config proof BoC")?;
        let proof = MerkleProof::load_from_cell(proof_root.as_ref())
            .context("getConfigParams config proof is not a Merkle proof")?;
        let state_root = Cell::virtualize(proof.cell);
        let state = state_root.parse::<ShardStateUnsplit>().with_context(|| {
            format!(
                "config proof does not contain a masterchain shard state (root type: {:?})",
                state_root.cell_type()
            )
        })?;
        let extra = state
            .custom
            .context("config proof masterchain state has no McStateExtra")?
            .load()
            .context("config proof contains invalid McStateExtra")?;
        Ok(extra.config.params)
    }

    async fn lookup(&mut self, workchain: i32, shard: &str, seqno: u32) -> Result<BlockIdExt> {
        let shard = parse_shard(shard)?;
        let seqno = i32::try_from(seqno).context("block seqno exceeds i32")?;
        let header = self
            .inner
            .query_typed::<BlockHeader>(Request::LookupBlock(LookupBlock {
                mode: (),
                id: BlockId {
                    workchain,
                    shard,
                    seqno,
                },
                seqno: Some(()),
                lt: None,
                utime: None,
                with_state_update: None,
                with_value_flow: None,
                with_extra: None,
                with_shard_hashes: None,
                with_prev_blk_signatures: None,
            }))
            .await
            .context("lookupBlock failed")?;
        Ok(header.id)
    }

    async fn download_block(&mut self, id: BlockIdExt) -> Result<(BlockRef, Vec<u8>)> {
        let bytes = self
            .inner
            .query_typed::<BlockData>(Request::GetBlock(GetBlock { id: id.clone() }))
            .await
            .context("getBlock failed")?;
        Ok((id.into(), bytes.data))
    }
}

/// Converts Localton's complete network config to the subset consumed by the LiteAPI client.
fn liteapi_config(config: &GlobalConfig) -> ConfigGlobal {
    ConfigGlobal {
        liteservers: config
            .liteserver_endpoints()
            .map(|(ip, port, public_key)| liteapi_liteserver(ip, port, public_key))
            .collect(),
    }
}

fn liteapi_liteserver(ip: Ipv4Addr, port: u16, public_key: TonPublicKey) -> ConfigLiteServer {
    ConfigLiteServer {
        ip: i32::from_be_bytes(ip.octets()),
        port,
        id: ConfigPublicKey::Ed25519 {
            key: *public_key.as_bytes(),
        },
    }
}

pub fn parse_shard(value: &str) -> Result<i64> {
    let value = value.trim();
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        let raw = u64::from_str_radix(hex, 16)
            .with_context(|| format!("invalid hexadecimal shard `{value}`"))?;
        return Ok(raw as i64);
    }
    if value.len() == 16 && !value.starts_with('-') && value.chars().all(|c| c.is_ascii_hexdigit())
        || value
            .chars()
            .any(|c| c.is_ascii_hexdigit() && c.is_ascii_alphabetic())
    {
        let raw = u64::from_str_radix(value, 16)
            .with_context(|| format!("invalid hexadecimal shard `{value}`"))?;
        return Ok(raw as i64);
    }
    value
        .parse::<i64>()
        .with_context(|| format!("invalid shard `{value}`"))
}

impl From<BlockIdExt> for BlockRef {
    fn from(value: BlockIdExt) -> Self {
        Self {
            workchain: value.workchain,
            shard: format!("{:016x}", value.shard as u64),
            seqno: u32::try_from(value.seqno).unwrap_or_default(),
            root_hash: value.root_hash.to_hex(),
            file_hash: value.file_hash.to_hex(),
        }
    }
}

impl From<TransactionId> for TransactionRef {
    fn from(value: TransactionId) -> Self {
        Self {
            account: value.account.map(|value| value.to_hex()),
            lt: value.lt,
            hash: value.hash.map(|value| value.to_hex()),
        }
    }
}

/// Decode account storage without treating the response as a verified state proof.
/// The transaction hash is unavailable without extracting an authenticated shard account.
fn account_info(address: &str, response: AccountStateResponse) -> Result<AccountInfo> {
    let account = if response.state.is_empty() {
        None
    } else {
        Boc::decode(&response.state)?.parse::<OptionalAccount>()?.0
    };
    let state = match account.as_ref().map(|account| &account.state) {
        None => "nonexist",
        Some(AccountState::Uninit) => "uninit",
        Some(AccountState::Frozen(_)) => "frozen",
        Some(AccountState::Active(_)) => "active",
    };

    Ok(AccountInfo {
        address: address.to_owned(),
        state: state.to_owned(),
        balance_nano: account
            .as_ref()
            .map(|account| account.balance.tokens.into_inner().to_string())
            .unwrap_or_else(|| "0".to_owned()),
        last_transaction_lt: account.as_ref().map(|account| account.last_trans_lt),
        last_transaction_hash: None,
        block: response.id.into(),
        shard_block: response.shardblk.into(),
    })
}

pub fn require_existing_config(path: &Path) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        Err(anyhow!(
            "global config does not exist at {}; start the network first",
            path.display()
        ))
    }
}

fn save_state_cache(path: &Path, cell: &Cell) -> Result<()> {
    let temp = path.with_extension("boc.tmp");
    fs::write(&temp, Boc::encode(cell))?;
    fs::rename(temp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};

    use rston::liteclient::ConfigGlobal;

    use super::{LocalLiteClient, liteapi_liteserver, parse_shard, ton_method_id};
    use crate::ton::tools::types::TonPublicKey;

    #[test]
    fn account_view_decodes_canonical_states_and_balances() {
        use rston::cell::CellBuilder;
        use rston::models::{
            Account, AccountState, CurrencyCollection, IntAddr, OptionalAccount, StdAddr,
        };

        let address = StdAddr::new(0, rston::cell::HashBytes([3; 32]));
        let states = [
            (AccountState::Uninit, "uninit"),
            (
                AccountState::Frozen(rston::cell::HashBytes([4; 32])),
                "frozen",
            ),
            (AccountState::Active(Default::default()), "active"),
        ];
        let block = rston::tl::BlockIdExt {
            workchain: -1,
            shard: i64::MIN,
            seqno: 7,
            root_hash: rston::tl::Int256([1; 32]),
            file_hash: rston::tl::Int256([2; 32]),
        };

        for (state, expected) in states {
            let account = OptionalAccount(Some(Account {
                address: IntAddr::Std(address.clone()),
                storage_stat: Default::default(),
                last_trans_lt: 42,
                balance: CurrencyCollection::new(123_456_789),
                state,
            }));
            let response = super::AccountStateResponse {
                id: block.clone(),
                shardblk: block.clone(),
                shard_proof: Vec::new(),
                proof: Vec::new(),
                state: super::Boc::encode(CellBuilder::build_from(account).unwrap()),
            };

            let view = super::account_info(&address.to_string(), response).unwrap();

            assert_eq!(view.state, expected);
            assert_eq!(view.balance_nano, "123456789");
            assert_eq!(view.last_transaction_lt, Some(42));
            assert_eq!(view.last_transaction_hash, None);
            assert_eq!(view.block.seqno, 7);
        }
    }

    #[tokio::test]
    async fn connection_failover_attempts_every_configured_liteserver() {
        let first = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let first_port = first.local_addr().unwrap().port();
        let second = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let second_port = second.local_addr().unwrap().port();
        drop((first, second));

        let config = ConfigGlobal {
            liteservers: vec![
                liteapi_liteserver(
                    Ipv4Addr::LOCALHOST,
                    first_port,
                    TonPublicKey::from_bytes([1; 32]),
                ),
                liteapi_liteserver(
                    Ipv4Addr::LOCALHOST,
                    second_port,
                    TonPublicKey::from_bytes([2; 32]),
                ),
            ],
        };
        let error = LocalLiteClient::connect_config(config)
            .await
            .err()
            .expect("both unavailable liteservers must fail");
        let message = error.to_string();

        assert!(message.contains(&format!("127.0.0.1:{first_port}")));
        assert!(message.contains(&format!("127.0.0.1:{second_port}")));
    }

    #[test]
    fn parses_signed_and_hex_shards() {
        assert_eq!(parse_shard("-9223372036854775808").unwrap(), i64::MIN);
        assert_eq!(parse_shard("8000000000000000").unwrap(), i64::MIN);
        assert_eq!(parse_shard("0x4000000000000000").unwrap(), 1_i64 << 62);
    }

    #[test]
    fn get_method_ids_match_official_ton_crc16() {
        assert_eq!(ton_method_id("seqno"), 85_143);
        assert_eq!(ton_method_id("active_election_id"), 86_535);
    }

    #[test]
    fn liteapi_liteserver_preserves_typed_endpoint() {
        let config = liteapi_liteserver(
            Ipv4Addr::new(192, 168, 27, 4),
            18_004,
            TonPublicKey::from_bytes([7; 32]),
        );

        assert_eq!(
            config.socket_addr(),
            SocketAddrV4::new(Ipv4Addr::new(192, 168, 27, 4), 18_004)
        );
        assert_eq!(config.public_key(), &[7; 32]);
    }
}
