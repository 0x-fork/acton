//! Typed queries anchored to an explicit trusted or verified block.

use std::time::Instant;

use super::{LiteClient, LiteClientPool, LiteError, ProofError, proof};
use crate::GlobalConfig;
use crate::boc::Boc;
use crate::cell::{Cell, CellBuilder, HashBytes};
use crate::models::vm::Tuple;
use crate::models::{
    Account, Block, BlockId, BlockInfo, BlockchainConfig, MsgInfo, OwnedMessage, ShardIdent,
    StdAddr, Transaction,
};
use crate::tl::common::{AccountId, Int256};
use crate::tl::request::{self, Request};
use crate::tl::response;

/// A block accepted as a trust anchor or authenticated by a proof chain.
///
/// The identifier is immutable. It authenticates data at that block, without
/// asserting that the block is recent or that the checkpoint belongs to a
/// particular network. The caller owns checkpoint selection and persistence.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct VerifiedBlock(BlockId);

impl VerifiedBlock {
    /// Establish an independently trusted checkpoint without contacting a server.
    ///
    /// This is an explicit trust decision, not cryptographic verification.
    /// Obtain the complete identifier from a trusted source. Never use an
    /// arbitrary server response here to bootstrap a trustless client.
    pub const fn from_trusted(id: BlockId) -> Self {
        Self(id)
    }

    /// Trust the init block from an independently trusted global configuration.
    ///
    /// The caller owns trust in the file and its network identity. This performs
    /// no cryptographic verification and never selects a hardfork automatically.
    /// Missing validator/init sections and sequence-zero init blocks are errors:
    /// masterchain proof-chain verification requires a nonzero block checkpoint.
    pub fn from_trusted_config(config: &GlobalConfig) -> Result<Self, ProofError> {
        let validator = config.validator.as_ref().ok_or(ProofError::Invalid(
            "global config has no validator section",
        ))?;
        let block = validator.init_block.ok_or(ProofError::Invalid(
            "global config has no validator.init_block",
        ))?;
        proof::require_masterchain(&block)?;
        Ok(Self::from_trusted(block))
    }

    /// Read the complete identifier without allowing its hashes to be modified.
    pub const fn id(&self) -> &BlockId {
        &self.0
    }

    pub(super) const fn verified(id: BlockId) -> Self {
        Self(id)
    }
}

/// Account state and transaction cursor authenticated at a requested block.
#[derive(Debug, Clone)]
pub struct AccountInfo {
    /// Block against which the account's inclusion or absence was checked.
    pub block: BlockId,
    /// Shard block bound to the requested masterchain block, or that block itself.
    pub shard_block: VerifiedBlock,
    /// Decoded state. `None` means authenticated absence from the accounts dictionary.
    pub account: Option<Account>,
    /// Logical time from the authenticated shard-account entry; zero for absence.
    pub last_transaction_lt: u64,
    /// Hash from the authenticated shard-account entry; zero for absence.
    pub last_transaction_hash: HashBytes,
}

impl AccountInfo {
    /// Start history traversal at this account's authenticated last transaction.
    /// Accounts without history return `None`. Use an unmodified result of
    /// [`LiteClient::get_account`]; this does not authenticate caller-created data.
    pub fn transaction_cursor(&self) -> Result<Option<TransactionCursor>, ProofError> {
        if self.last_transaction_lt == 0 && self.last_transaction_hash == HashBytes::ZERO {
            return Ok(None);
        }
        let address = match self.account.as_ref().map(|account| &account.address) {
            Some(crate::models::IntAddr::Std(address)) => address.clone(),
            _ => {
                return Err(ProofError::Invalid(
                    "history cursor has no standard account address",
                ));
            }
        };
        TransactionCursor::from_trusted(
            address,
            self.last_transaction_lt,
            self.last_transaction_hash,
        )
        .map(Some)
    }
}

/// One authenticated starting point in an account's transaction hash chain.
/// Obtain it from [`AccountInfo::transaction_cursor`] or a verified page's `next`.
/// A cursor proves history relative to its hash, without asserting freshness or
/// authenticating the server's reported block identifiers.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TransactionCursor {
    address: StdAddr,
    lt: u64,
    hash: HashBytes,
}

impl TransactionCursor {
    /// Restore a cursor whose address, logical time, and hash are independently
    /// trusted. This is a trust decision, not a cryptographic check. A zero
    /// cursor denotes no transaction and is rejected; use `None` for history end.
    pub fn from_trusted(address: StdAddr, lt: u64, hash: HashBytes) -> Result<Self, ProofError> {
        if address.anycast.is_some() || lt == 0 || hash == HashBytes::ZERO {
            return Err(ProofError::Invalid("invalid transaction history cursor"));
        }
        Ok(Self { address, lt, hash })
    }

    /// Account to which every transaction in a returned page must belong.
    pub const fn address(&self) -> &StdAddr {
        &self.address
    }

    /// Logical time of the first transaction requested from the server.
    pub const fn lt(&self) -> u64 {
        self.lt
    }

    /// Representation hash against which the first returned cell is checked.
    pub const fn hash(&self) -> &HashBytes {
        &self.hash
    }
}

/// A transaction authenticated through the history cursor's hash chain.
#[derive(Debug, Clone)]
pub struct TransactionRecord {
    /// Original complete cell, retaining the exact authenticated representation hash.
    pub cell: Cell,
    /// Transaction decoded from `cell`, with its address and logical time checked.
    pub transaction: Transaction,
}

/// One history page, inclusive of the supplied cursor and ordered oldest first.
/// A server may return fewer records than requested. Only `next == None` proves
/// the chain ended; a short page with a cursor must still be paginated.
#[derive(Debug, Clone)]
pub struct TransactionPage {
    /// Authenticated records in ascending logical-time order.
    pub transactions: Vec<TransactionRecord>,
    /// Predecessor of the oldest returned transaction, for the next older page.
    pub next: Option<TransactionCursor>,
}

/// Server execution output whose source account state has been authenticated.
///
/// The stack and exit code remain server claims: Merkle proofs authenticate
/// input state, not execution of TVM. Recompute locally to authenticate results.
#[derive(Debug, Clone)]
pub struct GetMethodResult {
    /// Block used for the account-state proof.
    pub block: BlockId,
    /// Authenticated shard block containing the account.
    pub shard_block: VerifiedBlock,
    /// Account state hash matched by the server's state proof.
    pub account_state_hash: HashBytes,
    /// Server-reported successful TVM exit code, either zero or one.
    pub exit_code: i32,
    /// Decoded server result in bottom-to-top stack order.
    pub stack: Tuple,
}

/// TVM method selector, including numeric selectors for nonstandard methods.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MethodId(u64);

impl MethodId {
    /// Use the exact LiteAPI method identifier supplied by the caller.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Compute TON's get-method selector from the method name's UTF-8 bytes.
    pub fn from_name(name: &str) -> Self {
        Self(u64::from(crate::crc::crc_16(name.as_bytes())) | 0x10000)
    }

    /// Return the selector encoded in the LiteAPI request.
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl From<&str> for MethodId {
    fn from(name: &str) -> Self {
        Self::from_name(name)
    }
}

impl From<u64> for MethodId {
    fn from(id: u64) -> Self {
        Self::new(id)
    }
}

/// Select one block lookup criterion. The server supplies the block's hashes;
/// a lookup does not authenticate its membership in the canonical chain.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockLookup {
    /// Request an exact sequence number within the selected shard.
    Seqno(u32),
    /// Request the first block whose ending logical time exceeds this value.
    /// The header alone cannot prove that the server chose the first such block.
    LogicalTime(u64),
    /// Request the first block generated at or after these Unix seconds.
    /// The header alone cannot prove that the server chose the first such block.
    UnixTime(u32),
}

/// A lookup result whose header matches its identifier and lookup criterion.
///
/// Both hashes originate from the server. Authenticate the identifier against
/// a checkpoint or shard ancestry before treating this as a trusted block.
#[derive(Debug, Clone)]
pub struct BlockHeader {
    /// Server-supplied identity; header proofs cannot authenticate the file hash.
    pub id: BlockId,
    /// Decoded header committed by the server-supplied root hash.
    pub info: BlockInfo,
}

/// Server-side proof construction choices. These flags do not relax local
/// verification or promote a returned destination to a verified block.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct BlockProofOptions {
    /// Set LiteAPI mode bit 1, permitting the protocol's weak target selection.
    pub allow_weak_target: bool,
    /// Set mode bit 12: build using the newer requested endpoint's state instead
    /// of the server's current head. With an explicit target, this selects `0x1001`.
    pub base_block_from_request: bool,
}

impl LiteClient {
    /// Download a block bound to an independently trusted or verified identity.
    /// Checks the exact serialized file hash, root hash, and header coordinates.
    /// Referenced model structures remain lazy; state updates may contain pruned cells.
    pub async fn get_block(&mut self, block: &VerifiedBlock) -> Result<Block, LiteError> {
        let response: response::BlockData = self
            .query_typed(Request::GetBlock(request::GetBlock {
                id: proof::wire_id(block.id()),
            }))
            .await?;
        Ok(response.verify(block)?)
    }

    /// Authenticate complete BlockInfo against a trusted block's root hash.
    /// The response identifier must match, including the file hash; the header
    /// proof itself commits only the root hash. Other block subtrees may be pruned.
    pub async fn get_block_header(
        &mut self,
        block: &VerifiedBlock,
    ) -> Result<BlockInfo, LiteError> {
        let response: response::BlockHeader = self
            .query_typed(Request::GetBlockHeader(request::GetBlockHeader {
                id: proof::wire_id(block.id()),
                mode: (),
                with_state_update: None,
                with_value_flow: None,
                with_extra: None,
                with_shard_hashes: None,
                with_prev_blk_signatures: None,
            }))
            .await?;
        Ok(response.verify(block)?)
    }

    /// Resolve an identity and decode its header without establishing chain trust.
    /// Checks the proof hash, coordinates, shard containment, and the selected
    /// sequence or time bound. Time lookup cannot prove nearest-block selection.
    pub async fn lookup_block(
        &mut self,
        shard: ShardIdent,
        lookup: BlockLookup,
    ) -> Result<BlockHeader, LiteError> {
        let response: response::BlockHeader = self
            .query_typed(Request::LookupBlock(request::LookupBlock {
                mode: (),
                id: crate::tl::common::BlockId {
                    workchain: shard.workchain(),
                    shard: shard.prefix() as i64,
                    seqno: match lookup {
                        BlockLookup::Seqno(seqno) => seqno as i32,
                        _ => 0,
                    },
                },
                seqno: matches!(lookup, BlockLookup::Seqno(_)).then_some(()),
                lt: match lookup {
                    BlockLookup::LogicalTime(lt) => Some(lt),
                    _ => None,
                },
                utime: match lookup {
                    BlockLookup::UnixTime(utime) => Some(utime),
                    _ => None,
                },
                with_state_update: None,
                with_value_flow: None,
                with_extra: None,
                with_shard_hashes: None,
                with_prev_blk_signatures: None,
            }))
            .await?;
        Ok(response.verify_lookup(shard, lookup)?)
    }

    /// Authenticate the server's current masterchain head from a checkpoint.
    /// Freshness is a server claim; the caller decides acceptable age and seqno.
    pub async fn get_masterchain_info(
        &mut self,
        checkpoint: &VerifiedBlock,
    ) -> Result<VerifiedBlock, LiteError> {
        let response: response::MasterchainInfo =
            self.query_typed(Request::GetMasterchainInfo).await?;
        let target = proof::block_id(&response.last)?;
        self.verify_proof_chain(checkpoint, &target).await
    }

    /// Decode and authenticate account state or absence at this exact block.
    /// Anycast addresses are rejected; proofs must identify the exact account.
    pub async fn get_account(
        &mut self,
        block: &VerifiedBlock,
        address: &StdAddr,
    ) -> Result<AccountInfo, LiteError> {
        let account = account_id(address)?;
        let response: response::AccountState = self
            .query_typed(Request::GetAccountState(request::GetAccountState {
                id: proof::wire_id(block.id()),
                account,
            }))
            .await?;
        Ok(response.verify(block, address)?)
    }

    /// Authenticate configuration at a masterchain block. An empty list requests
    /// the complete config; otherwise only requested parameters are returned.
    /// Missing requested parameters and pruned requested data are errors.
    pub async fn get_config(
        &mut self,
        block: &VerifiedBlock,
        parameters: &[i32],
    ) -> Result<BlockchainConfig, LiteError> {
        proof::require_masterchain(block.id())?;
        let id = proof::wire_id(block.id());
        let query = if parameters.is_empty() {
            Request::GetConfigAll(request::GetConfigAll {
                mode: (),
                id,
                with_state_root: None,
                with_libraries: None,
                with_state_extra_root: None,
                with_shard_hashes: None,
                with_validator_set: None,
                with_special_smc: None,
                with_accounts_root: None,
                with_prev_blocks: None,
                with_workchain_info: None,
                with_capabilities: None,
                extract_from_key_block: None,
            })
        } else {
            Request::GetConfigParams(request::GetConfigParams {
                mode: (),
                id,
                param_list: parameters.to_vec(),
                with_state_root: None,
                with_libraries: None,
                with_state_extra_root: None,
                with_shard_hashes: None,
                with_validator_set: None,
                with_special_smc: None,
                with_accounts_root: None,
                with_prev_blocks: None,
                with_workchain_info: None,
                with_capabilities: None,
                extract_from_key_block: None,
            })
        };
        let response: response::ConfigInfo = self.query_typed(query).await?;
        Ok(response.verify(block, parameters)?)
    }

    /// Authenticate one transaction's address, logical time, and inclusion.
    /// `block` must identify its containing shard block, not a masterchain head.
    pub async fn get_transaction(
        &mut self,
        block: &VerifiedBlock,
        address: &StdAddr,
        lt: u64,
    ) -> Result<Transaction, LiteError> {
        let account = account_id(address)?;
        if !block.id().shard.contains_address(address) {
            return Err(
                ProofError::Invalid("transaction address is outside the requested shard").into(),
            );
        }
        let response: response::TransactionInfo = self
            .query_typed(Request::GetOneTransaction(request::GetOneTransaction {
                id: proof::wire_id(block.id()),
                account,
                lt,
            }))
            .await?;
        Ok(response.verify(block, address, lt)?)
    }

    /// Read up to `count` history records, including the cursor transaction.
    /// `count` must be 1..=256. Each cell's hash, account, logical time, and
    /// predecessor link are checked against the cursor. Records are oldest first.
    /// Empty responses for a nonempty cursor are errors, not history end.
    /// This does not authenticate block identifiers supplied with the response.
    pub async fn get_transactions(
        &mut self,
        cursor: &TransactionCursor,
        count: u32,
    ) -> Result<TransactionPage, LiteError> {
        proof::check_transaction_count(count)?;
        let response: response::TransactionList = self
            .query_typed(Request::GetTransactions(request::GetTransactions {
                count,
                account: account_id(cursor.address())?,
                lt: cursor.lt(),
                hash: Int256(cursor.hash().0),
            }))
            .await?;
        Ok(response.verify(cursor, count)?)
    }

    /// Retrieve a partial masterchain proof without authenticating its links.
    /// Use [`response::PartialBlockProof::verify`] or [`Self::verify_proof_chain`]
    /// before promoting its destination to a verified block.
    pub async fn get_block_proof(
        &mut self,
        known: &VerifiedBlock,
        target: &BlockId,
        options: BlockProofOptions,
    ) -> Result<response::PartialBlockProof, LiteError> {
        proof::require_masterchain(known.id())?;
        proof::require_masterchain(target)?;
        let response: response::PartialBlockProof = self
            .query_typed(Request::GetBlockProof(request::GetBlockProof {
                mode: (),
                known_block: proof::wire_id(known.id()),
                target_block: Some(proof::wire_id(target)),
                allow_weak_target: options.allow_weak_target.then_some(()),
                base_block_from_request: options.base_block_from_request.then_some(()),
            }))
            .await?;
        if proof::block_id(&response.from)? != *known.id() {
            return Err(ProofError::Invalid("proof starts from a different block").into());
        }
        Ok(response)
    }

    /// Authenticate a masterchain block through forward and backward proof links.
    ///
    /// The request deadline covers the whole operation, including all queries.
    /// Synchronous proof batches finish before an expired deadline is reported.
    /// At most 64 partial responses and 1024 links are accepted. The checkpoint
    /// is never changed on failure or cancellation; persist the returned block
    /// yourself. Requests are not retried and no first-response trust is used.
    pub async fn verify_proof_chain(
        &mut self,
        checkpoint: &VerifiedBlock,
        target: &BlockId,
    ) -> Result<VerifiedBlock, LiteError> {
        proof::require_masterchain(checkpoint.id())?;
        proof::require_masterchain(target)?;
        let started = Instant::now();
        let timeout = self.request_timeout;
        log::debug!(
            "operation=verify_proof_chain node={} target={target} outcome=started",
            self.target
        );

        let result = tokio::time::timeout(timeout, async {
            let mut current = *checkpoint;
            let mut links = 0usize;
            for _ in 0..64 {
                if current.id() == target {
                    return Ok(current);
                }
                let part = self
                    .get_block_proof(&current, target, BlockProofOptions::default())
                    .await?;
                links += part.steps.len();
                if links > 1024 {
                    return Err(ProofError::Invalid("proof chain exceeds 1024 links").into());
                }
                current = part.verify(&current, target)?;
                if started.elapsed() >= timeout {
                    return Err(LiteError::Timeout {
                        operation: "verify_proof_chain",
                        timeout,
                    });
                }
                log::debug!(
                    "operation=verify_proof_chain node={} target={target} seqno={} links={links} duration_ms={} outcome=progress",
                    self.target,
                    current.id().seqno,
                    started.elapsed().as_millis(),
                );
            }
            Err(LiteError::from(ProofError::Invalid(
                "proof chain exceeds 64 responses",
            )))
        })
        .await
        .map_err(|_| LiteError::Timeout {
            operation: "verify_proof_chain",
            timeout,
        })
        .and_then(|result| result);

        log::debug!(
            "operation=verify_proof_chain node={} target={target} duration_ms={} outcome={}",
            self.target,
            started.elapsed().as_millis(),
            if result.is_ok() { "success" } else { "failure" }
        );
        result
    }

    /// Execute a get method remotely, authenticating its source account state.
    /// The returned stack and exit code remain unverified server computation.
    /// Arguments and results use bottom-to-top stack order. Nonzero/non-one
    /// exit codes are returned as [`LiteError::ContractExecution`].
    pub async fn run_get_method(
        &mut self,
        block: &VerifiedBlock,
        address: &StdAddr,
        method: impl Into<MethodId>,
        arguments: &Tuple,
    ) -> Result<GetMethodResult, LiteError> {
        let account = account_id(address)?;
        let params = CellBuilder::build_from(arguments).map_err(ProofError::from)?;
        let response: response::RunMethodResult = self
            .query_typed(Request::RunSmcMethod(request::RunSmcMethod {
                mode: 7,
                id: proof::wire_id(block.id()),
                account,
                method_id: method.into().value(),
                params: Boc::encode(params),
            }))
            .await
            .map_err(|error| match error {
                LiteError::ServerError(error) if error.code == -256 => {
                    LiteError::ContractExecution { exit_code: -256 }
                }
                error => error,
            })?;
        Ok(response.verify(block, address)?)
    }

    /// Submit one external incoming message, returning its representation hash
    /// after the server acknowledges receipt. This proves neither propagation
    /// nor on-chain inclusion. Cancellation and timeout leave acceptance unknown.
    /// The exact message is sent once, with no automatic retry or re-signing.
    pub async fn send_message(&mut self, message: &Cell) -> Result<HashBytes, LiteError> {
        let decoded = message.parse::<OwnedMessage>().map_err(ProofError::from)?;
        if !matches!(decoded.info, MsgInfo::ExtIn(_)) {
            return Err(
                ProofError::Invalid("only external incoming messages can be submitted").into(),
            );
        }
        let response: response::SendMsgStatus = self
            .query_typed(Request::SendMessage(request::SendMessage {
                body: Boc::encode(message),
            }))
            .await?;
        if response.status != 1 {
            return Err(LiteError::MessageRejected {
                status: response.status,
            });
        }
        Ok(*message.repr_hash())
    }
}

impl LiteClientPool {
    /// Read and authenticate one history page through the next reachable server.
    /// The ordering, bounds, and cursor rules of [`LiteClient::get_transactions`] apply.
    pub async fn get_transactions(
        &mut self,
        cursor: &TransactionCursor,
        count: u32,
    ) -> Result<TransactionPage, LiteError> {
        proof::check_transaction_count(count)?;
        self.next_client()
            .await?
            .get_transactions(cursor, count)
            .await
    }

    /// Download and authenticate a block through the next reachable server.
    /// The file, root, and coordinate checks of [`LiteClient::get_block`] apply.
    pub async fn get_block(&mut self, block: &VerifiedBlock) -> Result<Block, LiteError> {
        self.next_client().await?.get_block(block).await
    }

    /// Authenticate BlockInfo through the next reachable server.
    pub async fn get_block_header(
        &mut self,
        block: &VerifiedBlock,
    ) -> Result<BlockInfo, LiteError> {
        self.next_client().await?.get_block_header(block).await
    }

    /// Resolve a block through the next reachable server. The consistency checks
    /// of [`LiteClient::lookup_block`] apply; the identity remains untrusted.
    pub async fn lookup_block(
        &mut self,
        shard: ShardIdent,
        lookup: BlockLookup,
    ) -> Result<BlockHeader, LiteError> {
        self.next_client().await?.lookup_block(shard, lookup).await
    }

    /// Authenticate the selected server's head from an explicit checkpoint.
    /// Connection setup is bounded separately from proof-chain verification.
    pub async fn get_masterchain_info(
        &mut self,
        checkpoint: &VerifiedBlock,
    ) -> Result<VerifiedBlock, LiteError> {
        self.next_client()
            .await?
            .get_masterchain_info(checkpoint)
            .await
    }

    /// Authenticate account state at an exact block through the next reachable server.
    pub async fn get_account(
        &mut self,
        block: &VerifiedBlock,
        address: &StdAddr,
    ) -> Result<AccountInfo, LiteError> {
        self.next_client().await?.get_account(block, address).await
    }

    /// Authenticate requested masterchain configuration through the next reachable server.
    pub async fn get_config(
        &mut self,
        block: &VerifiedBlock,
        parameters: &[i32],
    ) -> Result<BlockchainConfig, LiteError> {
        self.next_client()
            .await?
            .get_config(block, parameters)
            .await
    }

    /// Authenticate transaction inclusion in an exact shard block.
    pub async fn get_transaction(
        &mut self,
        block: &VerifiedBlock,
        address: &StdAddr,
        lt: u64,
    ) -> Result<Transaction, LiteError> {
        self.next_client()
            .await?
            .get_transaction(block, address, lt)
            .await
    }

    /// Fetch unverified proof links; receiving them does not authenticate a block.
    pub async fn get_block_proof(
        &mut self,
        known: &VerifiedBlock,
        target: &BlockId,
        options: BlockProofOptions,
    ) -> Result<response::PartialBlockProof, LiteError> {
        self.next_client()
            .await?
            .get_block_proof(known, target, options)
            .await
    }

    /// Authenticate all links through one selected connection, without retries.
    /// The limits and timeout of [`LiteClient::verify_proof_chain`] apply.
    pub async fn verify_proof_chain(
        &mut self,
        checkpoint: &VerifiedBlock,
        target: &BlockId,
    ) -> Result<VerifiedBlock, LiteError> {
        self.next_client()
            .await?
            .verify_proof_chain(checkpoint, target)
            .await
    }

    /// Authenticate source state for a remote get-method result; execution remains a server claim.
    pub async fn run_get_method(
        &mut self,
        block: &VerifiedBlock,
        address: &StdAddr,
        method: impl Into<MethodId>,
        arguments: &Tuple,
    ) -> Result<GetMethodResult, LiteError> {
        self.next_client()
            .await?
            .run_get_method(block, address, method, arguments)
            .await
    }

    /// Submit once through the next reachable server. Receipt does not confirm inclusion.
    pub async fn send_message(&mut self, message: &Cell) -> Result<HashBytes, LiteError> {
        self.next_client().await?.send_message(message).await
    }
}

fn account_id(address: &StdAddr) -> Result<AccountId, ProofError> {
    if address.anycast.is_some() {
        return Err(ProofError::Invalid(
            "anycast addresses are not supported by account proofs",
        ));
    }
    Ok(AccountId {
        workchain: i32::from(address.workchain),
        id: Int256(address.address.0),
    })
}
