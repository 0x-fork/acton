use std::collections::HashSet;
use std::slice;
use std::sync::{Arc, Mutex};

use expect_test::expect;
use rston::boc::{Boc, ser::BocHeader};
use rston::cell::{Cell, CellBuilder, CellFamily, CellType, HashBytes};
use rston::liteclient::{
    FromResponse, LiteClientOptions, LiteClientPool, TransactionCursor, VerifiedBlock,
};
use rston::merkle::MerkleProof;
use rston::models::{BlockId, ExtInMsgInfo, IntAddr, Message, MsgInfo, StdAddr, Transaction};
use rston::tl::{Request, Response, response};

use super::TestServer;

fn address() -> StdAddr {
    "-1:3333333333333333333333333333333333333333333333333333333333333333"
        .parse()
        .unwrap()
}

fn transaction_cell() -> Cell {
    let response = response::TransactionInfo::from_response(
        tl_proto::deserialize(include_bytes!("../fixtures/liteclient/transaction.tl")).unwrap(),
    )
    .unwrap();
    Boc::decode(response.transaction).unwrap()
}

// Retain the real transaction's body and lazy fields while constructing a small
// deterministic account history with an explicit trusted starting hash.
fn history() -> Vec<Cell> {
    let mut transaction: Transaction = transaction_cell().parse().unwrap();
    transaction.account = address().address;
    transaction.prev_trans_lt = 0;
    transaction.prev_trans_hash = HashBytes::ZERO;
    let mut cells: Vec<Cell> = Vec::new();
    for lt in [100, 200, 300] {
        if let Some(previous) = cells.last() {
            transaction.prev_trans_lt = transaction.lt;
            transaction.prev_trans_hash = *previous.repr_hash();
        }
        transaction.lt = lt;
        cells.push(CellBuilder::build_from(&transaction).unwrap());
    }
    cells
}

fn cursor(cell: &Cell) -> TransactionCursor {
    let transaction: Transaction = cell.parse().unwrap();
    TransactionCursor::from_trusted(address(), transaction.lt, *cell.repr_hash()).unwrap()
}

fn reply(cells: &[Cell]) -> response::TransactionList {
    let mut header = BocHeader::<ahash::RandomState>::default();
    for cell in cells {
        header.add_root(cell.as_ref());
    }
    let mut bytes = Vec::new();
    header.encode(&mut bytes);
    response::TransactionList {
        // Block identifiers are server claims, independent of the account hash chain.
        ids: Vec::new(),
        transactions: bytes,
    }
}

#[test]
fn authenticates_pages_and_preserves_original_cells_and_older_cursors() {
    let cells = history();
    let start = cursor(&cells[2]);
    // A server can return a short page even when older transactions remain.
    let first = reply(&[cells[2].clone(), cells[1].clone()])
        .verify(&start, 16)
        .unwrap();
    let next = first.next.unwrap();
    let second = reply(&[cells[0].clone()]).verify(&next, 16).unwrap();
    let result = (
        first
            .transactions
            .iter()
            .map(|item| item.transaction.lt)
            .collect::<Vec<_>>(),
        first.transactions[0].cell == cells[1],
        first.transactions[1].cell == cells[2],
        next.lt(),
        next.hash() == cells[0].repr_hash(),
        next.address() == &address(),
        second.transactions[0].transaction.lt,
        second.next.is_none(),
    );
    expect![[r#"
        (
            [
                200,
                300,
            ],
            true,
            true,
            100,
            true,
            true,
            100,
            true,
        )
    "#]]
    .assert_debug_eq(&result);
}

#[test]
fn reads_a_real_transaction_and_derives_a_cursor_from_a_proven_account() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/liteclient/manifest.json")).unwrap();
    let block: BlockId = manifest["head"].as_str().unwrap().parse().unwrap();
    let account = response::AccountState::from_response(
        tl_proto::deserialize(include_bytes!("../fixtures/liteclient/account.tl")).unwrap(),
    )
    .unwrap()
    .verify(&VerifiedBlock::from_trusted(block), &address())
    .unwrap();
    let authenticated = account.transaction_cursor().unwrap().unwrap();
    let cell = transaction_cell();
    let page = reply(slice::from_ref(&cell))
        .verify(&authenticated, 1)
        .unwrap();
    let record = &page.transactions[0];
    let result = (
        authenticated.address() == &address(),
        authenticated.lt() == account.last_transaction_lt,
        *authenticated.hash() == account.last_transaction_hash,
        record.cell == cell,
        record.cell.repr_hash().to_string() == manifest["transaction_hash"].as_str().unwrap(),
        record.transaction.lt == manifest["transaction_lt"].as_u64().unwrap(),
        page.next.as_ref().unwrap().lt() == record.transaction.prev_trans_lt,
    );
    expect![[r#"
        (
            true,
            true,
            true,
            true,
            true,
            true,
            true,
        )
    "#]]
    .assert_debug_eq(&result);
}

#[test]
fn preserves_embedded_merkle_proofs_but_rejects_pruned_transaction_data() {
    let mut builder = CellBuilder::new();
    builder.store_reference(Cell::empty_cell()).unwrap();
    let child = builder.build().unwrap();
    let mut builder = CellBuilder::new();
    builder.store_reference(child).unwrap();
    let root = builder.build().unwrap();
    let proof = MerkleProof::create(root.as_ref(), HashSet::from([*root.repr_hash()]))
        .build()
        .unwrap();
    let proof_cell = CellBuilder::build_from(&proof).unwrap();
    let mut builder = CellBuilder::new();
    builder.store_reference(proof_cell.clone()).unwrap();
    let body = builder.build().unwrap();
    let message = Message {
        info: MsgInfo::ExtIn(ExtInMsgInfo {
            dst: IntAddr::Std(address()),
            ..Default::default()
        }),
        init: None,
        body: body.as_slice().unwrap(),
        layout: None,
    };
    let mut transaction: Transaction = history()[0].parse().unwrap();
    transaction.in_msg = Some(CellBuilder::build_from(message).unwrap());
    let cell = CellBuilder::build_from(&transaction).unwrap();
    let start = cursor(&cell);
    let page = reply(slice::from_ref(&cell)).verify(&start, 1).unwrap();
    let record = &page.transactions[0];
    let mut message: Message<'_> = record.transaction.in_msg.as_ref().unwrap().parse().unwrap();
    let embedded = message.body.load_reference().unwrap();

    // A server-pruned transaction is different from an original message carrying
    // a Merkle proof: required transaction fields must remain available.
    let partial = MerkleProof::create(cell.as_ref(), HashSet::from([*cell.repr_hash()]))
        .build()
        .unwrap()
        .cell;
    let result = (
        record.cell == cell,
        embedded == proof_cell.as_ref(),
        embedded.cell_type() == CellType::MerkleProof,
        proof.cell.references().next().unwrap().cell_type() == CellType::PrunedBranch,
        reply(&[partial]).verify(&start, 1).unwrap_err().to_string(),
    );
    expect![[r#"
        (
            true,
            true,
            true,
            true,
            "requested payload contains pruned data",
        )
    "#]]
    .assert_debug_eq(&result);
}

#[test]
fn absent_accounts_have_no_cursor_and_zero_cursors_are_rejected() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/liteclient/manifest.json")).unwrap();
    let block: BlockId = manifest["head"].as_str().unwrap().parse().unwrap();
    let absent: StdAddr = manifest["absent"].as_str().unwrap().parse().unwrap();
    let account = response::AccountState::from_response(
        tl_proto::deserialize(include_bytes!("../fixtures/liteclient/absent-account.tl")).unwrap(),
    )
    .unwrap()
    .verify(&VerifiedBlock::from_trusted(block), &absent)
    .unwrap();
    let errors = [
        (0, HashBytes::ZERO),
        (0, HashBytes([1; 32])),
        (100, HashBytes::ZERO),
    ]
    .map(|(lt, hash)| {
        TransactionCursor::from_trusted(address(), lt, hash)
            .unwrap_err()
            .to_string()
    });
    expect![[r#"
        (
            true,
            [
                "invalid transaction history cursor",
                "invalid transaction history cursor",
                "invalid transaction history cursor",
            ],
        )
    "#]]
    .assert_debug_eq(&(account.transaction_cursor().unwrap().is_none(), errors));
}

#[test]
fn rejects_tampered_reordered_and_wrong_account_history() {
    let cells = history();
    let start = cursor(&cells[2]);
    let mut cases = Vec::new();
    cases.push((
        "wrong first hash",
        reply(&[cells[1].clone()]),
        start.clone(),
    ));
    cases.push((
        "missing middle",
        reply(&[cells[2].clone(), cells[0].clone()]),
        start.clone(),
    ));
    cases.push((
        "duplicate transaction",
        reply(&[cells[2].clone(), cells[2].clone()]),
        start.clone(),
    ));

    let mut changed: Transaction = cells[1].parse().unwrap();
    changed.now += 1;
    cases.push((
        "tampered predecessor",
        reply(&[cells[2].clone(), CellBuilder::build_from(&changed).unwrap()]),
        start.clone(),
    ));

    let mut wrong_address = address();
    wrong_address.address.0[0] ^= 1;
    let wrong_account =
        TransactionCursor::from_trusted(wrong_address, start.lt(), *start.hash()).unwrap();
    cases.push(("wrong account", reply(&[cells[2].clone()]), wrong_account));
    let wrong_lt = TransactionCursor::from_trusted(address(), 301, *start.hash()).unwrap();
    cases.push(("wrong logical time", reply(&[cells[2].clone()]), wrong_lt));

    for (label, lt, hash) in [
        ("nondecreasing predecessor", 100, HashBytes([1; 32])),
        ("zero time with nonzero hash", 0, HashBytes([1; 32])),
        ("nonzero time with zero hash", 99, HashBytes::ZERO),
    ] {
        let mut transaction: Transaction = cells[0].parse().unwrap();
        transaction.prev_trans_lt = lt;
        transaction.prev_trans_hash = hash;
        let cell = CellBuilder::build_from(&transaction).unwrap();
        cases.push((label, reply(slice::from_ref(&cell)), cursor(&cell)));
    }
    cases.push((
        "past history end",
        reply(&[cells[0].clone(), cells[0].clone()]),
        cursor(&cells[0]),
    ));

    let errors = cases
        .into_iter()
        .map(|(label, reply, cursor)| (label, reply.verify(&cursor, 16).unwrap_err().to_string()))
        .collect::<Vec<_>>();
    expect![[r#"
        [
            (
                "wrong first hash",
                "transaction hash does not match history cursor",
            ),
            (
                "missing middle",
                "transaction hash does not match history cursor",
            ),
            (
                "duplicate transaction",
                "transaction hash does not match history cursor",
            ),
            (
                "tampered predecessor",
                "transaction hash does not match history cursor",
            ),
            (
                "wrong account",
                "transaction account or logical time differs from history cursor",
            ),
            (
                "wrong logical time",
                "transaction account or logical time differs from history cursor",
            ),
            (
                "nondecreasing predecessor",
                "invalid transaction predecessor cursor",
            ),
            (
                "zero time with nonzero hash",
                "invalid transaction predecessor cursor",
            ),
            (
                "nonzero time with zero hash",
                "invalid transaction predecessor cursor",
            ),
            (
                "past history end",
                "history continues past chain end",
            ),
        ]
    "#]]
    .assert_debug_eq(&errors);
}

#[test]
fn rejects_empty_oversized_and_over_limit_pages() {
    let cells = history();
    let start = cursor(&cells[2]);
    let mut errors = Vec::new();
    for (label, response, count) in [
        ("zero count", reply(&[cells[2].clone()]), 0),
        ("excessive count", reply(&[cells[2].clone()]), 257),
        (
            "too many roots",
            reply(&[cells[2].clone(), cells[1].clone()]),
            1,
        ),
        (
            "zero roots",
            response::TransactionList {
                ids: vec![],
                transactions: vec![0xb5, 0xee, 0x9c, 0x72, 1, 1, 0, 0, 0, 0],
            },
            16,
        ),
        (
            "empty payload",
            response::TransactionList {
                ids: vec![],
                transactions: vec![],
            },
            16,
        ),
        (
            "oversized payload",
            response::TransactionList {
                ids: vec![],
                transactions: vec![0; 8 * 1024 * 1024 + 1],
            },
            16,
        ),
    ] {
        errors.push((
            label,
            response.verify(&start, count).unwrap_err().to_string(),
        ));
    }
    expect![[r#"
        [
            (
                "zero count",
                "history page count must be 1..=256",
            ),
            (
                "excessive count",
                "history page count must be 1..=256",
            ),
            (
                "too many roots",
                "invalid proof BoC: too many root cells",
            ),
            (
                "zero roots",
                "invalid proof BoC: root cell not found",
            ),
            (
                "empty payload",
                "empty history response for a nonempty cursor",
            ),
            (
                "oversized payload",
                "history payload exceeds 8 MiB",
            ),
        ]
    "#]]
    .assert_debug_eq(&errors);
}

#[tokio::test]
async fn client_and_pool_authenticate_pages_and_validate_requests_before_sending() {
    let cells = history();
    let start = cursor(&cells[2]);
    let first = reply(&[cells[2].clone(), cells[1].clone()]);
    let second = reply(&[cells[0].clone()]);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let server = TestServer::start(move |bytes| {
        let Request::GetTransactions(request) = tl_proto::deserialize(&bytes).unwrap() else {
            panic!("expected a transaction-history request");
        };
        captured.lock().unwrap().push((
            request.count,
            request.lt,
            request.account.workchain,
            request.account.id.0 == address().address.0,
        ));
        let response = if request.lt == 300 {
            first.clone()
        } else {
            second.clone()
        };
        tl_proto::serialize(Response::TransactionList(response))
    })
    .await;
    let mut client = server.client().await;
    let first = client.get_transactions(&start, 2).await.unwrap();
    let second = client
        .get_transactions(first.next.as_ref().unwrap(), 2)
        .await
        .unwrap();
    let config = rston::GlobalConfig {
        liteservers: vec![server.config.clone()],
        ..Default::default()
    };
    let mut pool = LiteClientPool::new(config, LiteClientOptions::default()).unwrap();
    let pooled = pool.get_transactions(&start, 2).await.unwrap();
    let errors = [
        client
            .get_transactions(&start, 0)
            .await
            .unwrap_err()
            .to_string(),
        pool.get_transactions(&start, 257)
            .await
            .unwrap_err()
            .to_string(),
    ];
    let result = (
        first
            .transactions
            .iter()
            .map(|item| item.transaction.lt)
            .collect::<Vec<_>>(),
        second.transactions[0].transaction.lt,
        second.next.is_none(),
        pooled
            .transactions
            .iter()
            .map(|item| item.transaction.lt)
            .collect::<Vec<_>>(),
        requests.lock().unwrap().clone(),
        errors,
    );
    expect![[r#"
        (
            [
                200,
                300,
            ],
            100,
            true,
            [
                200,
                300,
            ],
            [
                (
                    2,
                    300,
                    -1,
                    true,
                ),
                (
                    2,
                    100,
                    -1,
                    true,
                ),
                (
                    2,
                    300,
                    -1,
                    true,
                ),
            ],
            [
                "proof verification failed: history page count must be 1..=256",
                "proof verification failed: history page count must be 1..=256",
            ],
        )
    "#]]
    .assert_debug_eq(&result);
}
