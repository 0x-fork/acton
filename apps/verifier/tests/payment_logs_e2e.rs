mod support;

use std::sync::Arc;

use support::{
    PAYMENT_ADDRESS, PAYMENT_TX_HASH, StaticPaymentBlockchainClient, payment_transaction,
};
use tracing::instrument::WithSubscriber;
use verifier::payment::{OnchainPaymentVerifier, PaymentError, PaymentLedger, PaymentVerifier};

const AMOUNT_NANO: u64 = 1_000_000;

// Keep log capture in its own test process: tracing callsite interest is global,
// while most verifier tests deliberately run without a subscriber in parallel.
#[tokio::test]
async fn first_payment_claim_reports_the_new_payment() {
    let code_hash = "ab".repeat(32);
    let client = Arc::new(StaticPaymentBlockchainClient::new(
        Some(payment_transaction(PAYMENT_TX_HASH, &code_hash)),
        Vec::new(),
    ));
    let verifier = OnchainPaymentVerifier::new(
        client,
        PaymentLedger::in_memory().expect("in-memory payment ledger should open"),
        PAYMENT_ADDRESS.to_owned(),
        AMOUNT_NANO,
    );
    verifier
        .recover(&[])
        .await
        .expect("empty payment history recovery should succeed");

    let (log, subscriber) = log_capture();

    let claim = verifier
        .claim(PAYMENT_TX_HASH, &code_hash)
        .with_subscriber(subscriber)
        .await
        .expect("new payment should be reserved");
    assert_eq!(claim.claim_version, 1);

    let content = std::fs::read_to_string(log.path()).expect("logs should be readable");
    let event = content
        .lines()
        .find(|line| line.contains("new payment found and reserved"))
        .expect("new payment event should be logged");
    for expected in [
        &format!("transaction_hash={PAYMENT_TX_HASH}"),
        &format!("code_hash={code_hash}"),
        &format!("amount_nano={AMOUNT_NANO}"),
        &format!("payment_address={PAYMENT_ADDRESS}"),
        "network=testnet",
    ] {
        assert!(event.contains(expected), "{event}");
    }
}

#[tokio::test]
async fn recovery_reports_only_newly_saved_payments_across_restarts() {
    let code_hash = "ab".repeat(32);
    let payment = payment_transaction(PAYMENT_TX_HASH, &code_hash);
    let new_hash = "cd".repeat(32);
    let new_payment = payment_transaction(&new_hash, &code_hash);
    let mut dust = payment_transaction(&"ef".repeat(32), &code_hash);
    dust.incoming.as_mut().expect("payment has a message").value = Some(1);
    let directory = tempfile::tempdir().expect("ledger directory should be created");
    let ledger_path = directory.path().join("payments.sqlite3");
    let (log, subscriber) = log_capture();

    for (history, expected_new_count) in [
        (vec![payment.clone(), dust.clone()], 1),
        (vec![payment.clone(), new_payment.clone(), dust.clone()], 1),
        (vec![payment, new_payment, dust], 0),
    ] {
        let verifier = OnchainPaymentVerifier::new(
            Arc::new(StaticPaymentBlockchainClient::new(None, history)),
            PaymentLedger::open(&ledger_path).expect("payment ledger should open"),
            PAYMENT_ADDRESS.to_owned(),
            AMOUNT_NANO,
        );
        verifier
            .recover(&[PAYMENT_TX_HASH.to_owned()])
            .with_subscriber(subscriber.clone())
            .await
            .expect("payment history recovery should succeed");

        let content = std::fs::read_to_string(log.path()).expect("logs should be readable");
        let summary = content.lines().last().expect("recovery should be logged");
        assert!(
            summary.contains(&format!("new_payment_count={expected_new_count}")),
            "{summary}"
        );
    }

    let content = std::fs::read_to_string(log.path()).expect("logs should be readable");
    let events = content
        .lines()
        .filter(|line| line.contains("new payment found in blockchain history"))
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2, "{content}");
    for transaction_hash in [PAYMENT_TX_HASH, &new_hash] {
        let event = events
            .iter()
            .find(|line| line.contains(&format!("transaction_hash={transaction_hash}")))
            .expect("each saved payment should be logged once");
        for expected in [
            format!("code_hash={code_hash}"),
            format!("amount_nano={AMOUNT_NANO}"),
            format!("payment_address={PAYMENT_ADDRESS}"),
            "transaction_time=1700000000".to_owned(),
            "network=testnet".to_owned(),
        ] {
            assert!(event.contains(&expected), "{event}");
        }
    }
}

#[tokio::test]
async fn failed_recovery_does_not_report_rolled_back_payments() {
    let code_hash = "ab".repeat(32);
    let payment = payment_transaction(PAYMENT_TX_HASH, &code_hash);
    let mut overflow = payment_transaction(&"ff".repeat(32), &code_hash);
    overflow.lt = u64::MAX;
    let verifier = OnchainPaymentVerifier::new(
        Arc::new(StaticPaymentBlockchainClient::new(
            None,
            vec![payment, overflow],
        )),
        PaymentLedger::in_memory().expect("in-memory payment ledger should open"),
        PAYMENT_ADDRESS.to_owned(),
        AMOUNT_NANO,
    );
    let (log, subscriber) = log_capture();

    let result = verifier.recover(&[]).with_subscriber(subscriber).await;
    assert!(matches!(
        result,
        Err(PaymentError::IntegerOverflow { field: "lt", .. })
    ));
    assert!(!verifier.is_ready());
    let content = std::fs::read_to_string(log.path()).expect("logs should be readable");
    assert!(!content.contains("new payment found"), "{content}");
    assert!(!content.contains("payment ledger recovered"), "{content}");
}

fn log_capture() -> (tempfile::NamedTempFile, tracing::Dispatch) {
    let log = tempfile::NamedTempFile::new().expect("log file should be created");
    let writer = log.reopen().expect("log writer should open");
    let subscriber = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_writer(move || writer.try_clone().expect("log writer should clone"))
            .finish(),
    );
    (log, subscriber)
}
