use std::sync::{Arc, Mutex};

use expect_test::expect;
use rston::boc::Boc;
use rston::cell::CellBuilder;
use rston::liteclient::{BlockLookup, BlockProofOptions, LiteClientPool, VerifiedBlock};
use rston::merkle::MerkleProof;
use rston::models::{Block, BlockId, ShardIdent};
use rston::tl::common::{BlockIdExt, Int256};
use rston::tl::{Request, Response, response};

use super::TestServer;

const MASTERCHAIN: &[u8] = include_bytes!("../../src/models/block/tests/mc_simple_block.boc");
const SHARD: &[u8] = include_bytes!("../../src/models/block/tests/simple_shard_block.boc");

fn fixture(bytes: &[u8]) -> (VerifiedBlock, response::BlockData, response::BlockHeader) {
    let root = Boc::decode(bytes).unwrap();
    let info = root.parse::<Block>().unwrap().load_info().unwrap();
    let id = BlockIdExt {
        workchain: info.shard.workchain(),
        shard: info.shard.prefix() as i64,
        seqno: info.seqno as i32,
        root_hash: Int256(root.repr_hash().0),
        file_hash: Int256(Boc::file_hash(bytes).0),
    };
    let block = VerifiedBlock::from_trusted(BlockId {
        shard: info.shard,
        seqno: info.seqno,
        root_hash: *root.repr_hash(),
        file_hash: Boc::file_hash(bytes),
    });
    // Real header responses retain BlockInfo and prune unrelated block subtrees.
    let mut included = std::collections::HashSet::from([*root.repr_hash()]);
    let mut pending = vec![root.reference(0).unwrap()];
    while let Some(cell) = pending.pop() {
        if included.insert(*cell.repr_hash()) {
            pending.extend(cell.references());
        }
    }
    let proof = MerkleProof::create(root.as_ref(), included)
        .build()
        .unwrap();
    let header = response::BlockHeader {
        id: id.clone(),
        mode: (),
        with_state_update: None,
        with_value_flow: None,
        with_extra: None,
        with_shard_hashes: None,
        with_prev_blk_signatures: None,
        header_proof: Boc::encode(CellBuilder::build_from(proof).unwrap()),
    };
    (
        block,
        response::BlockData {
            id,
            data: bytes.to_vec(),
        },
        header,
    )
}

#[test]
fn authenticates_full_blocks_and_headers_in_both_chains() {
    let mut decoded = Vec::new();
    for bytes in [MASTERCHAIN, SHARD] {
        let (trusted, data, header) = fixture(bytes);
        let block = data.verify(&trusted).unwrap();
        let info = header.verify(&trusted).unwrap();
        decoded.push((
            info == block.load_info().unwrap(),
            info.shard.workchain(),
            info.seqno,
            info.gen_utime,
            info.start_lt,
            info.end_lt,
        ));
    }
    expect![[r#"
        [
            (
                true,
                -1,
                24721433,
                1674507085,
                34671157000000,
                34671157000005,
            ),
            (
                true,
                0,
                31806148,
                1674506665,
                34671006000000,
                34671006000012,
            ),
        ]
    "#]]
    .assert_debug_eq(&decoded);
}

#[test]
fn rejects_forged_block_bytes_identities_and_header_proofs() {
    let (trusted, data, header) = fixture(MASTERCHAIN);
    let mut errors = Vec::new();

    let mut changed = data.clone();
    changed.id.file_hash.0[0] ^= 1;
    errors.push((
        "response identity",
        changed.verify(&trusted).unwrap_err().to_string(),
    ));

    for field in ["file hash", "root hash", "seqno", "shard"] {
        let mut expected = *trusted.id();
        let mut changed = data.clone();
        match field {
            "file hash" => {
                expected.file_hash.0[0] ^= 1;
                changed.id.file_hash.0[0] ^= 1;
            }
            "root hash" => {
                expected.root_hash.0[0] ^= 1;
                changed.id.root_hash.0[0] ^= 1;
            }
            "seqno" => {
                expected.seqno += 1;
                changed.id.seqno += 1;
            }
            "shard" => {
                expected.shard = ShardIdent::BASECHAIN;
                changed.id.workchain = 0;
            }
            _ => unreachable!(),
        }
        let expected = VerifiedBlock::from_trusted(expected);
        errors.push((field, changed.verify(&expected).unwrap_err().to_string()));
        let changed_header = response::BlockHeader {
            id: changed.id,
            ..header.clone()
        };
        if field != "file hash" {
            errors.push((
                field,
                changed_header.verify(&expected).unwrap_err().to_string(),
            ));
        }
    }

    let mut changed = header.clone();
    changed.id.file_hash.0[0] ^= 1;
    errors.push((
        "header identity",
        changed.verify(&trusted).unwrap_err().to_string(),
    ));

    let root = Boc::decode(MASTERCHAIN).unwrap();
    let proof = MerkleProof::create(
        root.as_ref(),
        std::collections::HashSet::from([*root.repr_hash()]),
    )
    .build()
    .unwrap();
    let changed = response::BlockHeader {
        header_proof: Boc::encode(CellBuilder::build_from(proof).unwrap()),
        ..header.clone()
    };
    errors.push((
        "pruned info",
        changed.verify(&trusted).unwrap_err().to_string(),
    ));

    let changed = response::BlockHeader {
        header_proof: Vec::new(),
        ..header.clone()
    };
    errors.push((
        "missing proof",
        changed.verify(&trusted).unwrap_err().to_string(),
    ));
    let changed = response::BlockHeader {
        header_proof: Boc::encode(root),
        ..header.clone()
    };
    errors.push((
        "ordinary proof cell",
        changed.verify(&trusted).unwrap_err().to_string(),
    ));
    let changed = response::BlockHeader {
        header_proof: vec![0; 8 * 1024 * 1024 + 1],
        ..header
    };
    errors.push((
        "oversized proof",
        changed.verify(&trusted).unwrap_err().to_string(),
    ));
    let changed = response::BlockData {
        data: Vec::new(),
        ..data
    };
    errors.push((
        "missing block",
        changed.verify(&trusted).unwrap_err().to_string(),
    ));
    expect![[r#"
        [
            (
                "response identity",
                "block response has a different block id",
            ),
            (
                "file hash",
                "block file hash differs from its id",
            ),
            (
                "root hash",
                "block root hash differs from its id",
            ),
            (
                "root hash",
                "Merkle proof root hash does not match the expected object",
            ),
            (
                "seqno",
                "block header coordinates differ from its id",
            ),
            (
                "seqno",
                "block header coordinates differ from its id",
            ),
            (
                "shard",
                "block header coordinates differ from its id",
            ),
            (
                "shard",
                "block header coordinates differ from its id",
            ),
            (
                "header identity",
                "header response has a different block id",
            ),
            (
                "pruned info",
                "requested payload contains pruned data",
            ),
            (
                "missing proof",
                "invalid proof BoC: unexpected EOF",
            ),
            (
                "ordinary proof cell",
                "invalid proof cell: unexpected exotic cell",
            ),
            (
                "oversized proof",
                "proof payload exceeds 8 MiB",
            ),
            (
                "missing block",
                "invalid proof BoC: unexpected EOF",
            ),
        ]
    "#]]
    .assert_debug_eq(&errors);
}

#[test]
fn lookup_checks_coordinates_and_time_bounds_without_granting_trust() {
    let (trusted, _, header) = fixture(MASTERCHAIN);
    let info = header.verify(&trusted).unwrap();
    let shard = trusted.id().shard;
    let mut results = Vec::new();
    for lookup in [
        BlockLookup::Seqno(info.seqno),
        BlockLookup::Seqno(info.seqno + 1),
        BlockLookup::LogicalTime(info.end_lt - 1),
        BlockLookup::LogicalTime(info.end_lt),
        BlockLookup::UnixTime(info.gen_utime),
        BlockLookup::UnixTime(info.gen_utime + 1),
    ] {
        results.push(
            header
                .verify_lookup(shard, lookup)
                .map(|block| block.id == *trusted.id())
                .map_err(|error| error.to_string()),
        );
    }
    results.push(
        header
            .verify_lookup(ShardIdent::BASECHAIN, BlockLookup::Seqno(info.seqno))
            .map(|block| block.id == *trusted.id())
            .map_err(|error| error.to_string()),
    );
    expect![[r#"
        [
            Ok(
                true,
            ),
            Err(
                "lookup header does not satisfy the requested criterion",
            ),
            Ok(
                true,
            ),
            Err(
                "lookup header does not satisfy the requested criterion",
            ),
            Ok(
                true,
            ),
            Err(
                "lookup header does not satisfy the requested criterion",
            ),
            Err(
                "lookup response is outside the requested shard",
            ),
        ]
    "#]]
    .assert_debug_eq(&results);
}

#[tokio::test]
async fn client_and_pool_encode_block_requests_and_proof_modes() {
    let (trusted, data, header) = fixture(MASTERCHAIN);
    let info = header.verify(&trusted).unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    let server = TestServer::start(move |bytes| {
        let request: Request = tl_proto::deserialize(&bytes).unwrap();
        let response = match request {
            Request::GetBlock(request) => {
                recorded
                    .lock()
                    .unwrap()
                    .push(format!("block: exact_id={}", request.id == data.id));
                Response::BlockData(data.clone())
            }
            Request::GetBlockHeader(request) => {
                recorded.lock().unwrap().push(format!(
                    "header: exact_id={} flags={:?}",
                    request.id == data.id,
                    (
                        request.with_state_update,
                        request.with_value_flow,
                        request.with_extra,
                        request.with_shard_hashes,
                        request.with_prev_blk_signatures
                    )
                ));
                Response::BlockHeader(header.clone())
            }
            Request::LookupBlock(request) => {
                recorded.lock().unwrap().push(format!(
                    "lookup: shard={} seqno={} seqno_flag={:?} lt={:?} utime={:?}",
                    request.id.workchain == data.id.workchain && request.id.shard == data.id.shard,
                    request.id.seqno,
                    request.seqno,
                    request.lt,
                    request.utime
                ));
                Response::BlockHeader(header.clone())
            }
            Request::GetBlockProof(request) => {
                let mode = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
                recorded.lock().unwrap().push(format!(
                    "proof: mode={mode:#x} exact_from={} exact_to={}",
                    request.known_block == data.id,
                    request.target_block.as_ref() == Some(&data.id)
                ));
                Response::PartialBlockProof(response::PartialBlockProof {
                    complete: true,
                    from: request.known_block,
                    to: request.target_block.unwrap(),
                    steps: Vec::new(),
                })
            }
            _ => panic!("unexpected block request"),
        };
        tl_proto::serialize(response)
    })
    .await;

    let mut client = server.client().await;
    client.get_block(&trusted).await.unwrap();
    client.get_block_header(&trusted).await.unwrap();
    for lookup in [
        BlockLookup::Seqno(info.seqno),
        BlockLookup::LogicalTime(info.end_lt - 1),
        BlockLookup::UnixTime(info.gen_utime),
    ] {
        client
            .lookup_block(trusted.id().shard, lookup)
            .await
            .unwrap();
    }
    for options in [
        BlockProofOptions::default(),
        BlockProofOptions {
            allow_weak_target: true,
            base_block_from_request: false,
        },
        BlockProofOptions {
            allow_weak_target: false,
            base_block_from_request: true,
        },
        BlockProofOptions {
            allow_weak_target: true,
            base_block_from_request: true,
        },
    ] {
        client
            .get_block_proof(&trusted, trusted.id(), options)
            .await
            .unwrap();
    }

    let config = rston::GlobalConfig {
        liteservers: vec![server.config.clone()],
        ..Default::default()
    };
    let mut pool = LiteClientPool::new(config, Default::default()).unwrap();
    pool.get_block(&trusted).await.unwrap();
    pool.get_block_header(&trusted).await.unwrap();
    pool.lookup_block(trusted.id().shard, BlockLookup::Seqno(info.seqno))
        .await
        .unwrap();
    pool.get_block_proof(
        &trusted,
        trusted.id(),
        BlockProofOptions {
            base_block_from_request: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    expect![[r#"
        [
            "block: exact_id=true",
            "header: exact_id=true flags=(None, None, None, None, None)",
            "lookup: shard=true seqno=24721433 seqno_flag=Some(()) lt=None utime=None",
            "lookup: shard=true seqno=0 seqno_flag=None lt=Some(34671157000004) utime=None",
            "lookup: shard=true seqno=0 seqno_flag=None lt=None utime=Some(1674507085)",
            "proof: mode=0x1 exact_from=true exact_to=true",
            "proof: mode=0x3 exact_from=true exact_to=true",
            "proof: mode=0x1001 exact_from=true exact_to=true",
            "proof: mode=0x1003 exact_from=true exact_to=true",
            "block: exact_id=true",
            "header: exact_id=true flags=(None, None, None, None, None)",
            "lookup: shard=true seqno=24721433 seqno_flag=Some(()) lt=None utime=None",
            "proof: mode=0x1001 exact_from=true exact_to=true",
        ]
    "#]]
    .assert_debug_eq(&*requests.lock().unwrap());
}
