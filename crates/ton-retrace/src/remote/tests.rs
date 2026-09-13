use super::*;
use crate::BaseTxInfo;
use crate::methods::find_all_transactions_between;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use expect_test::expect;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use ton_api::toncenter::v3;
use tycho_types::cell::{CellBuilder, CellFamily, HashBytes, Lazy};
use tycho_types::models::{
    AccountStatus, ComputePhase, ComputePhaseSkipReason, HashUpdate, OrdinaryTxInfo,
    SkippedComputePhase, StdAddr, Transaction, TxInfo,
};

async fn serve_response(status: &str, response: Value) -> (TonCenterClient, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}/api/v3", listener.local_addr().unwrap());
    let body = response.to_string();
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let count = stream.read(&mut buffer).await.unwrap();
            assert_ne!(count, 0, "request closed before headers");
            request.extend_from_slice(&buffer[..count]);
        }
        stream.write_all(response.as_bytes()).await.unwrap();
        String::from_utf8(request).unwrap()
    });
    let client = TonCenterClient {
        client: Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
        api_key: Some("fixture-key".to_owned()),
        base_url,
    };
    (client, server)
}

fn transaction_cell(lt: u64, previous: Option<&Cell>) -> anyhow::Result<Cell> {
    let previous_tx = previous
        .map(|cell| cell.parse::<Transaction>())
        .transpose()?;
    Ok(CellBuilder::build_from(Transaction {
        account: HashBytes([0x11; 32]),
        lt,
        prev_trans_lt: previous_tx.map_or(0, |tx| tx.lt),
        prev_trans_hash: previous.map_or(HashBytes::ZERO, |cell| *cell.repr_hash()),
        now: 1,
        out_msg_count: Default::default(),
        orig_status: AccountStatus::Uninit,
        end_status: AccountStatus::Uninit,
        in_msg: None,
        out_msgs: Default::default(),
        total_fees: Default::default(),
        state_update: Lazy::new(&HashUpdate {
            old: HashBytes::ZERO,
            new: HashBytes::ZERO,
        })?,
        info: Lazy::new(&TxInfo::Ordinary(OrdinaryTxInfo {
            credit_first: false,
            storage_phase: None,
            credit_phase: None,
            compute_phase: ComputePhase::Skipped(SkippedComputePhase {
                reason: ComputePhaseSkipReason::NoState,
            }),
            action_phase: None,
            aborted: true,
            bounce_phase: None,
            destroyed: false,
        }))?,
    })?)
}

fn transaction_response(cell: &Cell) -> Value {
    let tx: Transaction = cell.parse().unwrap();
    let account = StdAddr::new(0, tx.account).to_string();
    json!({
        "@type": "raw.transaction",
        "address": { "@type": "accountAddress", "account_address": account },
        "account": account,
        "utime": tx.now,
        "data": Boc::encode_base64(cell),
        "transaction_id": { "@type": "internal.transactionId", "lt": tx.lt.to_string(), "hash": STANDARD.encode(cell.repr_hash().as_slice()) },
        "fee": "0", "storage_fee": "0", "other_fee": "0", "out_msgs": []
    })
}

#[tokio::test]
async fn archive_history_preserves_order_and_excludes_snapshot_transaction() -> anyhow::Result<()> {
    let snapshot = transaction_cell(10, None)?;
    let predecessor = transaction_cell(20, Some(&snapshot))?;
    let target = transaction_cell(30, Some(&predecessor))?;
    let base_tx = BaseTxInfo {
        lt: 30,
        hash: target.repr_hash().0,
        address: StdAddr::new(0, HashBytes([0x11; 32])),
        block: v3::BlockId {
            workchain: 0,
            shard: "8000000000000000".to_owned(),
            seqno: 1,
        },
    };
    let (client, server) = serve_response("200 OK", json!({
        "ok": true, "@extra": "", "result": [transaction_response(&target), transaction_response(&predecessor)]
    })).await;

    let transactions = find_all_transactions_between(&client, &base_tx, 10).await?;
    let request = server.await?;
    let uri = request.split_whitespace().nth(1).unwrap();
    let url = reqwest::Url::parse(&format!("http://localhost{uri}"))?;
    let query = url
        .query_pairs()
        .into_owned()
        .collect::<std::collections::BTreeMap<_, _>>();
    expect![[r#"
        (
            "/api/v2/getTransactions",
            Some(
                "10",
            ),
            Some(
                "true",
            ),
            [
                30,
                20,
            ],
        )
    "#]]
    .assert_debug_eq(&(
        url.path(),
        query.get("to_lt"),
        query.get("archival"),
        transactions.iter().map(|tx| tx.lt).collect::<Vec<_>>(),
    ));

    // The first-block approximation uses an arbitrary LT cutoff just before the
    // target because a true zerostate snapshot is unavailable.
    let (client, server) = serve_response(
        "200 OK",
        json!({
            "ok": true, "@extra": "", "result": [transaction_response(&target)]
        }),
    )
    .await;
    let transactions = find_all_transactions_between(&client, &base_tx, 29).await?;
    server.await?;
    expect![[r"
        [
            30,
        ]
    "]]
    .assert_debug_eq(&transactions.iter().map(|tx| tx.lt).collect::<Vec<_>>());
    Ok(())
}

#[tokio::test]
async fn archive_history_rejects_incomplete_or_inconsistent_responses() -> anyhow::Result<()> {
    let first = transaction_cell(10, None)?;
    let second = transaction_cell(20, Some(&first))?;
    let target = transaction_cell(30, Some(&second))?;
    let base_tx = BaseTxInfo {
        lt: 30,
        hash: target.repr_hash().0,
        address: StdAddr::new(0, HashBytes([0x11; 32])),
        block: v3::BlockId {
            workchain: 0,
            shard: "8000000000000000".to_owned(),
            seqno: 1,
        },
    };
    let wrong_hash = transaction_cell(20, None)?;
    let mut malformed = transaction_response(&target);
    malformed["data"] = json!(Boc::encode_base64(Cell::empty_cell()));
    let mut results = Vec::new();
    for (name, rows) in [
        ("empty", vec![]),
        ("short page", vec![transaction_response(&target)]),
        (
            "gap",
            vec![transaction_response(&target), transaction_response(&first)],
        ),
        (
            "duplicate",
            vec![transaction_response(&target), transaction_response(&target)],
        ),
        (
            "wrong hash",
            vec![
                transaction_response(&target),
                transaction_response(&wrong_hash),
            ],
        ),
        (
            "snapshot included",
            vec![
                transaction_response(&target),
                transaction_response(&second),
                transaction_response(&first),
            ],
        ),
        ("malformed BOC", vec![malformed]),
    ] {
        let (client, server) = serve_response(
            "200 OK",
            json!({ "ok": true, "@extra": "", "result": rows }),
        )
        .await;
        let result = find_all_transactions_between(&client, &base_tx, 10).await;
        server.await?;
        results.push((name, result.unwrap_err().to_string()));
    }
    expect![[r#"
        [
            (
                "empty",
                "Incomplete TON Center history: missing transaction at LT 30",
            ),
            (
                "short page",
                "Incomplete TON Center history: missing transaction at LT 20",
            ),
            (
                "gap",
                "TON Center history does not match expected transaction at LT 20",
            ),
            (
                "duplicate",
                "TON Center history does not match expected transaction at LT 20",
            ),
            (
                "wrong hash",
                "TON Center history does not match expected transaction at LT 20",
            ),
            (
                "snapshot included",
                "TON Center history contains a transaction outside the requested account or LT range",
            ),
            (
                "malformed BOC",
                "Failed to parse TON Center transaction at LT 30",
            ),
        ]
    "#]].assert_debug_eq(&results);
    Ok(())
}

#[tokio::test]
async fn cell_requests_preserve_api_errors_and_reject_missing_results() -> anyhow::Result<()> {
    let mut results = Vec::new();
    for (status, response) in [
        (
            "200 OK",
            json!({"ok": false, "error": "archive unavailable", "code": 500}),
        ),
        (
            "500 Internal Server Error",
            json!({"ok": false, "error": "archive unavailable", "code": 500}),
        ),
        ("200 OK", json!({"ok": true, "@extra": ""})),
        (
            "200 OK",
            json!({"ok": true, "@extra": "", "result": {"@type": "tvm.cell", "bytes": "invalid"}}),
        ),
    ] {
        let (client, server) = serve_response(status, response).await;
        let error = client.get_shard_account_cell(1, "0:11").await.unwrap_err();
        server.await?;
        results.push(format!("{error:#}"));
    }
    expect![[r#"
        [
            "TON Center getShardAccountCell error (200 OK): \"archive unavailable\"",
            "TON Center getShardAccountCell error (500 Internal Server Error): \"archive unavailable\"",
            "Failed to decode TON Center getShardAccountCell response: missing field `result`",
            "Failed to decode shard account cell BOC data: unknown BOC tag",
        ]
    "#]].assert_debug_eq(&results);
    Ok(())
}

#[test]
fn deserializes_toncenter_v3_tick_tock_without_credit_phase() {
    // Tick-tock has neither credit_first nor credit_ph in TonCenter v3.
    let value = serde_json::json!({
        "type": "tick_tock",
        "aborted": false,
        "destroyed": false,
        "is_tock": false,
        "storage_ph": {
            "storage_fees_collected": "0",
            "status_change": "unchanged"
        },
        "compute_ph": {
            "skipped": false,
            "success": true,
            "exit_code": 0
        }
    });

    let description: v3::TransactionDescr = serde_json::from_value(value).unwrap();
    let compute = description.compute_ph.unwrap();
    expect_test::expect![[r#"
        (
            "tick_tock",
            None,
            Some(
                true,
            ),
            Some(
                0,
            ),
        )
    "#]]
    .assert_debug_eq(&(
        description.kind,
        description.credit_first,
        compute.success,
        compute.exit_code,
    ));
}

#[test]
fn deserializes_toncenter_v3_transaction_with_skipped_compute_phase() {
    let value = serde_json::json!({
        "transactions": [{
            "account": "0:F7E97472D4849F481F339A5490281B1AE5B99E8B1016C03EAF51484E5D7BABF1",
            "hash": "6BOT/kLF43JNLlC5hACgFtni2TjHC9s2Beaig0EDe0w=",
            "lt": "66023973000007",
            "now": 1777378799,
            "mc_block_seqno": 123,
            "trace_id": "HVJuGGDRxhB6vGFpyLDfa7o0qlHIrSd3RRuLYG5falo=",
            "prev_trans_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            "prev_trans_lt": "0",
            "orig_status": "active",
            "end_status": "active",
            "total_fees": "0",
            "total_fees_extra_currencies": {},
            "description": {
                "type": "ord",
                "aborted": true,
                "destroyed": false,
                "credit_first": true,
                "storage_ph": {
                    "storage_fees_collected": "0",
                    "status_change": "unchanged"
                },
                "credit_ph": {
                    "credit": "1"
                },
                "compute_ph": {
                    "skipped": true,
                    "reason": "no_gas"
                }
            },
            "block_ref": {
                "workchain": 0,
                "shard": "8000000000000000",
                "seqno": 1
            },
            "account_state_before": {
                "hash": "before"
            },
            "account_state_after": {
                "hash": "after"
            },
            "emulated": false,
            "finality": "finalized"
        }],
        "address_book": {}
    });

    let data: v3::TransactionsResponse =
        serde_json::from_value(value).expect("skipped compute phase response should deserialize");
    let compute = data.transactions[0]
        .description
        .compute_ph
        .as_ref()
        .expect("compute phase should be present");

    assert_eq!(compute.skipped, Some(true));
    assert_eq!(compute.reason.as_deref(), Some("no_gas"));
    assert_eq!(compute.success, None);
    assert_eq!(compute.exit_code, None);
}
