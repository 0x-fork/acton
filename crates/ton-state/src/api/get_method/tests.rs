use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use expect_test::expect_file;
use http_body_util::BodyExt;
use rocksdb::DB;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{Semaphore, watch};
use ton_node_db::{BlockIndex, StateStore};
use tower::ServiceExt;
use tycho_types::cell::{CellBuilder, Lazy};
use tycho_types::models::{
    Account, BlockRef, BlockchainConfig, CurrencyCollection, DepthBalanceInfo, IntAddr,
    KeyBlockRef, KeyMaxLt, LibDescr, McStateExtra, OptionalAccount, ShardAccount, ShardAccounts,
    ShardDescription, ShardHashes, ShardIdent, ShardStateUnsplit, StateInit, ValidatorInfo,
};

use super::*;

/// A validator-format database with real account cells and synthetic block IDs.
/// Requests go through the production router, snapshot reader, and native TVM.
struct Fixture {
    _directory: tempfile::TempDir,
    store: StateStore,
    api: Api,
}

impl Fixture {
    fn new(state: AccountState, libraries: Dict<HashBytes, LibDescr>) -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let snapshot = directory.path().join("snapshot");
        std::fs::create_dir(&snapshot)?;
        let wallet: Value = serde_json::from_str(include_str!("fixtures/wallet-v5.json"))?;
        let address: StdAddr = wallet["address"].as_str().unwrap().parse()?;
        let mut balance = CurrencyCollection::new(109_388_167_086);
        balance
            .other
            .as_dict_mut()
            .set(42, tycho_types::num::VarUint248::from(123_u32))?;
        let mut accounts = ShardAccounts::new();
        accounts.set(
            address.address,
            DepthBalanceInfo {
                split_depth: 0,
                balance: balance.clone(),
            },
            ShardAccount {
                account: Lazy::new(&OptionalAccount(Some(Account {
                    address: IntAddr::Std(address),
                    storage_stat: Default::default(),
                    last_trans_lt: 900,
                    balance,
                    state,
                })))?,
                last_trans_hash: HashBytes([9; 32]),
                last_trans_lt: 899,
            },
        )?;
        let shard = CellBuilder::build_from(ShardStateUnsplit {
            shard_ident: ShardIdent::BASECHAIN,
            seqno: 12,
            gen_utime: 1_700_000_123,
            gen_lt: 123456,
            accounts: Lazy::new(&accounts)?,
            ..Default::default()
        })?;
        let shard_id = state_id(&shard)?;
        let config = BlockchainConfig {
            address: HashBytes::ZERO,
            params: tycho_types::models::BlockchainConfigParams::from_raw(Boc::decode_base64(
                include_str!("../../../../ton-executor/src/default_config.boc64"),
            )?),
        };
        let description = ShardDescription {
            seqno: shard_id.seqno,
            reg_mc_seqno: 100,
            start_lt: 0,
            end_lt: 123456,
            root_hash: shard_id.root_hash,
            file_hash: shard_id.file_hash,
            before_split: false,
            before_merge: false,
            want_split: false,
            want_merge: false,
            nx_cc_updated: false,
            next_catchain_seqno: 0,
            next_validator_shard: shard_id.shard.prefix(),
            min_ref_mc_seqno: 0,
            gen_utime: 1_700_000_123,
            split_merge_at: None,
            fees_collected: CurrencyCollection::ZERO,
            funds_created: CurrencyCollection::ZERO,
        };
        let mut extra = McStateExtra {
            shards: ShardHashes::from_shards([(&shard_id.shard, &description)])?,
            config,
            validator_info: ValidatorInfo {
                validator_list_hash_short: 0,
                catchain_seqno: 0,
                nx_cc_updated: false,
            },
            prev_blocks: Default::default(),
            after_key_block: false,
            last_key_block: Some(previous_block(90)),
            block_create_stats: None,
            global_balance: CurrencyCollection::ZERO,
        };
        for seqno in 0..100 {
            extra.prev_blocks.set(
                seqno,
                KeyMaxLt {
                    has_key_block: seqno == 90,
                    max_end_lt: u64::from(seqno),
                },
                KeyBlockRef {
                    is_key_block: seqno == 90,
                    block_ref: previous_block(seqno),
                },
            )?;
        }
        let master = CellBuilder::build_from(ShardStateUnsplit {
            shard_ident: ShardIdent::MASTERCHAIN,
            seqno: 100,
            gen_utime: 1_700_000_124,
            gen_lt: 123457,
            libraries,
            custom: Some(Lazy::new(&extra)?),
            ..Default::default()
        })?;
        let master_id = state_id(&master)?;
        let cells = DB::open_default(snapshot.join("celldb"))?;
        let db_state = DB::open_default(snapshot.join("state"))?;
        for (id, root) in [(master_id, master), (shard_id, shard)] {
            let bare = bare_id(id);
            let mut boxed = constructor(
                "tonNode.blockIdExt workchain:int shard:long seqno:int root_hash:int256 file_hash:int256 = tonNode.BlockIdExt",
            );
            boxed.extend(&bare);
            let key = format!("desc{}", STANDARD.encode(Sha256::digest(boxed)));
            let mut record = constructor(
                "db.celldb.value block_id:tonNode.blockIdExt prev:int256 next:int256 root_hash:int256 = db.celldb.Value",
            );
            record.extend(bare);
            record.extend([0; 64]);
            record.extend(root.repr_hash().as_slice());
            cells.put(key, record)?;
            let mut record = (-1_i32).to_le_bytes().to_vec();
            record.extend(1_i32.to_le_bytes());
            record.extend(Boc::encode(&root));
            cells.put(root.repr_hash().as_slice(), record)?;
        }
        let key = constructor("db.state.key.shardClient = db.state.Key");
        let mut record =
            constructor("db.state.shardClient block:tonNode.blockIdExt = db.state.ShardClient");
        record.extend(bare_id(master_id));
        db_state.put(Sha256::digest(key), record)?;
        drop((cells, db_state));
        let store = StateStore::open(&snapshot, &directory.path().join("updates"), 10_000)?;
        let (_, state) = watch::channel(store.snapshot());
        let api = Api {
            state,
            zero_state: master_id,
            history: Arc::new(BlockIndex::open(&directory.path().join("history"))?),
            get_method_slot: Arc::new(Semaphore::new(1)),
        };
        Ok(Self {
            _directory: directory,
            store,
            api,
        })
    }

    async fn request(&self, body: Value) -> Result<Value> {
        self.raw(&serde_json::to_vec(&body)?).await
    }

    async fn raw(&self, body: &[u8]) -> Result<Value> {
        let app = axum::Router::new()
            .route("/api/v2/runGetMethod", axum::routing::post(run_get_method))
            .with_state(self.api.clone());
        let response = app
            .oneshot(
                Request::post("/api/v2/runGetMethod")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_vec()))?,
            )
            .await?;
        let status = response.status().as_u16();
        let value: Value =
            serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
        Ok(json!({ "status": status, "body": value }))
    }
}

fn constructor(schema: &str) -> Vec<u8> {
    crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC)
        .checksum(schema.as_bytes())
        .to_le_bytes()
        .to_vec()
}

fn bare_id(id: BlockId) -> Vec<u8> {
    let mut bytes = id.shard.workchain().to_le_bytes().to_vec();
    bytes.extend(id.shard.prefix().to_le_bytes());
    bytes.extend(id.seqno.to_le_bytes());
    bytes.extend(id.root_hash.0);
    bytes.extend(id.file_hash.0);
    bytes
}

fn previous_block(seqno: u32) -> BlockRef {
    BlockRef {
        end_lt: u64::from(seqno),
        seqno,
        root_hash: HashBytes([seqno as u8; 32]),
        file_hash: HashBytes([1; 32]),
    }
}

fn state_id(root: &Cell) -> Result<BlockId> {
    let state: ShardStateUnsplit = root.parse()?;
    Ok(BlockId {
        shard: state.shard_ident,
        seqno: state.seqno,
        root_hash: *root.repr_hash(),
        file_hash: Boc::file_hash(Boc::encode(root)),
    })
}

fn active(code: Cell, data: Cell) -> AccountState {
    AccountState::Active(StateInit {
        code: Some(code),
        data: Some(data),
        ..Default::default()
    })
}

fn result(response: &Value) -> Value {
    let body = &response["body"];
    if let Some(result) = body.get("result") {
        json!({ "status": response["status"], "ok": body["ok"], "gas_used": result["gas_used"], "exit_code": result["exit_code"], "stack": result["stack"], "mc_seqno": result["block_id"]["seqno"], "lt": result["last_transaction_id"]["lt"] })
    } else {
        response.clone()
    }
}

#[tokio::test]
async fn run_get_method_matches_toncenter_and_preserves_the_committed_state() -> Result<()> {
    let wallet: Value = serde_json::from_str(include_str!("fixtures/wallet-v5.json"))?;
    let code = Boc::decode_base64(wallet["account"]["code"].as_str().unwrap())?;
    let data = Boc::decode_base64(wallet["account"]["data"].as_str().unwrap())?;
    let fixture = Fixture::new(active(code, data.clone()), Dict::new())?;
    let base = json!({ "address": wallet["address"], "method": "seqno", "stack": [] });
    let mut rows = Vec::new();
    for method in [
        json!("seqno"),
        json!(85143),
        json!("0x14c97"),
        json!("get_extensions"),
        json!("missing_method"),
    ] {
        let mut request = base.clone();
        request["method"] = method.clone();
        let response = fixture.request(request).await?;
        let compact = result(&response);
        let oracle = match method.as_str() {
            Some("get_extensions") => &wallet["get_extensions"],
            Some("missing_method") => &wallet["missing_method"],
            _ => &wallet["seqno"],
        };
        rows.push(json!({
            "method": method,
            "response": compact,
            "matches_toncenter": compact["gas_used"] == oracle["gas_used"]
                && compact["exit_code"] == oracle["exit_code"]
                && compact["stack"] == oracle["stack"],
        }));
    }
    expect_file!["snapshots/wallet.json"].assert_eq(&serde_json::to_string_pretty(&rows)?);

    let contracts: Value = serde_json::from_str(include_str!("fixtures/contracts.json"))?;
    let mut rows = Vec::new();
    for name in [
        "identity",
        "context",
        "out_of_gas",
        "throw",
        "write_data",
        "alternative_return",
        "nan",
        "slice_offset",
        "continuation",
        "builder",
        "deep_result",
        "shared_tuple_result",
    ] {
        let code = Boc::decode_base64(contracts[name].as_str().unwrap())?;
        let fixture = Fixture::new(active(code, data.clone()), Dict::new())?;
        let address: StdAddr = wallet["address"].as_str().unwrap().parse()?;
        let before = fixture
            .store
            .get_account(&address)?
            .account
            .unwrap()
            .account
            .inner()
            .repr_hash()
            .to_string();
        let mut request = base.clone();
        if name == "slice_offset" {
            request["stack"] = json!([[
                "tvm.Slice",
                Boc::encode_base64(CellBuilder::build_from(0x1234_u16)?)
            ]]);
        }
        rows.push(json!({ "contract": name, "response": result(&fixture.request(request).await?),
            "state_unchanged": before == fixture.store.get_account(&address)?.account.unwrap().account.inner().repr_hash().to_string() }));
    }
    expect_file!["snapshots/execution.json"].assert_eq(&serde_json::to_string_pretty(&rows)?);

    let echo = Fixture::new(
        active(
            Boc::decode_base64(contracts["identity"].as_str().unwrap())?,
            data.clone(),
        ),
        Dict::new(),
    )?;
    let cell = Boc::encode_base64(CellBuilder::build_from(0x1234_u16)?);
    let maximum = (num_bigint::BigInt::from(1) << 256_usize) - 1_u32;
    let minimum = -(num_bigint::BigInt::from(1) << 256_usize);
    let typed_number = json!({ "@type": "tvm.stackEntryNumber", "number": { "@type": "tvm.numberDecimal", "number": "123" } });
    let mut rows = Vec::new();
    for (name, input) in [
        (
            "integer_aliases",
            json!([
                ["int", -1],
                ["integer", "-0x2"],
                ["number", "123"],
                ["num", maximum.to_string()],
                ["num", minimum.to_string()]
            ]),
        ),
        (
            "cell_and_slice",
            json!([["tvm.Cell", cell], ["cell", {"bytes": cell}], ["tvm.Slice", cell], ["slice", {"bytes": cell}]]),
        ),
        (
            "empty_tuple",
            json!([["tuple", {"@type": "tvm.tuple", "elements": []}]]),
        ),
        (
            "empty_list",
            json!([["list", {"@type": "tvm.list", "elements": []}]]),
        ),
        (
            "list",
            json!([["tvm.List", {"@type": "tvm.list", "elements": [typed_number, typed_number]}]]),
        ),
        (
            "nested",
            json!([["tvm.Tuple", {"@type": "tvm.tuple", "elements": [typed_number, {"@type":"tvm.stackEntryTuple", "tuple": {"@type":"tvm.tuple", "elements":[typed_number]}}]}]]),
        ),
        (
            "int_overflow",
            json!([["num", (-minimum.clone()).to_string()]]),
        ),
        ("invalid_sign", json!([["num", "--1"]])),
        ("too_many_arguments", json!(vec![json!(["num", 1]); 257])),
        (
            "deep_list",
            json!([["list", {"@type":"tvm.list", "elements":vec![typed_number.clone(); 65]}]]),
        ),
        (
            "oversized_boc",
            json!([["tvm.Cell", "A".repeat(1024 * 1024 + 1)]]),
        ),
    ] {
        let mut request = base.clone();
        request["stack"] = input;
        rows.push(json!({ "case": name, "response": result(&echo.request(request).await?) }));
    }
    expect_file!["snapshots/stack.json"].assert_eq(&serde_json::to_string_pretty(&rows)?);

    let identity = Boc::decode_base64(contracts["identity"].as_str().unwrap())?;
    let mut library_reference = CellBuilder::new();
    library_reference.set_exotic(true);
    library_reference.store_u8(2)?;
    library_reference.store_u256(identity.repr_hash())?;
    let library_reference = library_reference.build()?;
    let mut libraries = Dict::new();
    let mut publishers = Dict::new();
    publishers.set(HashBytes([1; 32]), ())?;
    libraries.set(
        *identity.repr_hash(),
        LibDescr {
            lib: identity,
            publishers,
        },
    )?;
    let mut rows = Vec::new();
    for libraries in [Dict::new(), libraries] {
        let fixture = Fixture::new(active(library_reference.clone(), data.clone()), libraries)?;
        rows.push(result(&fixture.request(base.clone()).await?));
    }
    for state in [
        AccountState::Uninit,
        AccountState::Frozen(HashBytes([8; 32])),
        AccountState::Active(StateInit::default()),
    ] {
        let fixture = Fixture::new(state, Dict::new())?;
        let mut request = base.clone();
        request["stack"] = json!([["num", 5]]);
        rows.push(result(&fixture.request(request.clone()).await?));
        request["address"] = json!(StdAddr::new(0, HashBytes([2; 32])).to_string());
        rows.push(result(&fixture.request(request).await?));
    }
    expect_file!["snapshots/lifecycle.json"].assert_eq(&serde_json::to_string_pretty(&rows)?);

    let mut rows = Vec::new();
    for (key, value) in [
        ("address", json!("invalid")),
        ("seqno", json!(-1)),
        ("seqno", json!(99)),
        ("seqno", json!(100)),
        ("method", json!("")),
        ("method", json!("4294967296")),
        ("stack", json!([["num", "broken"]])),
        ("stack", json!([["cell", {"bytes": "broken"}]])),
        ("stack", json!([["unsupported", ""]])),
        ("extra", json!(true)),
    ] {
        let mut request = base.clone();
        request[key] = value.clone();
        rows.push(
            json!({ "input": [key, value], "response": result(&fixture.request(request).await?) }),
        );
    }
    rows.push(fixture.raw(b"{").await?);
    rows.push(fixture.raw(&vec![b' '; 2 * 1024 * 1024 + 1]).await?);
    let permit = fixture.api.get_method_slot.acquire().await?;
    rows.push(fixture.request(base).await?);
    drop(permit);
    expect_file!["snapshots/errors.json"].assert_eq(&serde_json::to_string_pretty(&rows)?);
    Ok(())
}
