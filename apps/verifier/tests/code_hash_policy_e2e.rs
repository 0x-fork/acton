mod support;

use std::{io::Write, sync::Arc};

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use verifier::{
    app,
    code_hash_policy::CodeHashPolicy,
    compilers::CompileGeneratedSource,
    config::{Config, ConfigError},
    payment::{OnchainPaymentVerifier, PaymentLedger},
    registry::SourceVerificationRegistry,
    registry_index::{SqliteVerificationIndex, VerificationIndex},
    state::{AppState, StateError},
};

use support::{
    MultipartPart, PAYMENT_ADDRESS, PAYMENT_TX_HASH, StaticPaymentBlockchainClient, file_part, get,
    head, mock_blockchain::MockBlockchainClient, mock_compiler::MockCompilerService,
    mock_source_storage::MockSourceStorage, owned_text_part, post_verify, post_verify_with_api_key,
    post_verify_with_user_agent, post_verify_without_payment, response_json, text_part,
};

const BLOCKED: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BLOCKED_BASE64: &str = "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqo=";
const ALLOWED: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ADDRESS: &str = "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c";
const API_KEY: &str = "test-api-key";

fn config(contents: &str) -> Result<Config, ConfigError> {
    let mut file = tempfile::NamedTempFile::new().expect("config file");
    file.write_all(contents.as_bytes()).expect("write config");
    Config::load_from_path(file.path())
}

fn policy(entries: &[&str]) -> CodeHashPolicy {
    CodeHashPolicy::from_blocked(
        &entries
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
    )
    .expect("valid policy")
}

#[test]
fn config_stores_raw_hashes_and_policy_normalizes_blocked_entries() {
    for config in [
        Config::default(),
        config("").expect("defaults"),
        config("[server]\nblocked_code_hashes = []").expect("empty list"),
    ] {
        assert!(config.blocked_code_hashes().is_empty());
        let policy = CodeHashPolicy::from_blocked(config.blocked_code_hashes())
            .expect("empty code hash policy");
        assert!(!policy.is_blocked(BLOCKED));
    }
    let config = config(&format!(
        "[server]\nblocked_code_hashes = [\" {} \", \"{BLOCKED_BASE64}\", \"{BLOCKED}\"]",
        BLOCKED.to_ascii_uppercase()
    ))
    .expect("raw entries");
    assert_eq!(
        config.blocked_code_hashes(),
        [
            format!(" {} ", BLOCKED.to_ascii_uppercase()),
            BLOCKED_BASE64.to_owned(),
            BLOCKED.to_owned(),
        ]
    );
    let policy =
        CodeHashPolicy::from_blocked(config.blocked_code_hashes()).expect("code hash policy");
    for hash in [
        BLOCKED.to_owned(),
        BLOCKED.to_uppercase(),
        BLOCKED_BASE64.to_owned(),
    ] {
        assert!(policy.is_blocked(&hash));
    }
    assert!(!policy.is_blocked(ALLOWED));
}

#[test]
fn policy_accepts_all_supported_hash_encodings_and_rejects_invalid_lookups() {
    let hash = "fb".repeat(32);
    let encodings = [
        hash.clone(),
        hash.to_uppercase(),
        format!(" \t{}\n", hash.to_uppercase()),
        STANDARD.encode([0xfb; 32]),
        STANDARD_NO_PAD.encode([0xfb; 32]),
        URL_SAFE.encode([0xfb; 32]),
        URL_SAFE_NO_PAD.encode([0xfb; 32]),
    ];
    for entry in &encodings {
        let policy = CodeHashPolicy::from_blocked(std::slice::from_ref(entry))
            .expect("supported blocked hash encoding");
        for query in &encodings {
            assert!(
                policy.is_blocked(query),
                "entry: {entry:?}, query: {query:?}"
            );
        }
        for query in ["", " \t\n", "not-a-hash", "fb", ALLOWED] {
            assert!(
                !policy.is_blocked(query),
                "entry: {entry:?}, query: {query:?}"
            );
        }
    }
}

#[test]
fn invalid_blocked_entries_are_rejected_by_policy_and_application_initialization() {
    for entry in [
        String::new(),
        " ".to_owned(),
        "not-a-hash".to_owned(),
        " not-a-hash ".to_owned(),
        "a".repeat(63),
        "a".repeat(65),
        "g".repeat(64),
        "é".repeat(32),
        STANDARD.encode([0; 31]),
        STANDARD.encode([0; 33]),
    ] {
        let config = config(&format!(
            "[server]\nblocked_code_hashes = [\"{BLOCKED}\", \"{entry}\"]"
        ))
        .expect("config stores code hashes as raw strings");
        assert_eq!(config.blocked_code_hashes(), [BLOCKED, &entry]);
        let error = CodeHashPolicy::from_blocked(config.blocked_code_hashes()).expect_err(&entry);
        assert_eq!(error.entry, entry);
        assert!(matches!(
            AppState::from_config(&config),
            Err(StateError::CodeHashPolicy(_))
        ));
    }
}

#[test]
fn blocked_code_hashes_requires_a_list_of_strings() {
    for value in ["true", "42", "[42]", "\"hash\""] {
        assert!(matches!(
            config(&format!("[server]\nblocked_code_hashes = {value}")),
            Err(ConfigError::Parse { .. })
        ));
    }
}

fn state(
    storage: Arc<MockSourceStorage>,
    compiler: Arc<MockCompilerService>,
    index: SqliteVerificationIndex,
    code_hash_policy: CodeHashPolicy,
) -> AppState {
    AppState::new(
        Arc::new(MockBlockchainClient::new(&[(ADDRESS, BLOCKED)])),
        compiler,
        Arc::new(
            SourceVerificationRegistry::new(storage, Arc::new(index))
                .with_code_hash_policy(code_hash_policy),
        ),
        Arc::new(OnchainPaymentVerifier::new(
            Arc::new(StaticPaymentBlockchainClient::new(None, Vec::new())),
            PaymentLedger::in_memory().expect("payment ledger"),
            PAYMENT_ADDRESS.to_owned(),
            1,
        )),
    )
    .with_api_key(Some(API_KEY))
}

fn parts(field: &'static str, target: &str) -> Vec<MultipartPart> {
    vec![
        owned_text_part(field, target),
        text_part("language", "tolk"),
        text_part("compile_params", r#"{"compiler_version":"1.4.1"}"#),
        text_part("sources", r#"[{"path":"main.tolk","is_entrypoint":true}]"#),
        file_part("files", "main.tolk", "text/plain", "fun main() {}"),
    ]
}

async fn assert_blocked(response: axum::response::Response, expected_status: StatusCode) {
    assert_eq!(response.status(), expected_status);
    let body = response_json::<Value>(response).await;
    assert_eq!(
        body["error"],
        format!("code_hash_blocked: access to code_hash {BLOCKED} is disabled")
    );
}

async fn assert_blocked_reads(state: &AppState) {
    for hash in [
        BLOCKED.to_owned(),
        BLOCKED.to_ascii_uppercase(),
        BLOCKED_BASE64.to_owned(),
    ] {
        for endpoint in ["verification/source", "abi"] {
            let path = format!("/api/v1/{endpoint}?code_hash={hash}");
            assert_blocked(get(state.clone(), &path).await, StatusCode::NOT_FOUND).await;
            assert_eq!(
                head(state.clone(), &path).await.status(),
                StatusCode::NOT_FOUND
            );
        }
    }
    for query in [
        format!("address={ADDRESS}"),
        format!("address={ADDRESS}&code_hash={BLOCKED}"),
    ] {
        let path = format!("/api/v1/verification/source?{query}");
        assert_blocked(get(state.clone(), &path).await, StatusCode::NOT_FOUND).await;
        assert_eq!(
            head(state.clone(), &path).await.status(),
            StatusCode::NOT_FOUND
        );
    }
}

async fn assert_status_for_blocked_hash(state: &AppState, verified: bool) {
    for query in [
        format!("code_hash={BLOCKED}"),
        format!("code_hash={}", BLOCKED.to_ascii_uppercase()),
        format!("code_hash={BLOCKED_BASE64}"),
        format!("address={ADDRESS}"),
        format!("address={ADDRESS}&code_hash={BLOCKED}"),
    ] {
        let path = format!("/api/v1/verification/status?{query}");
        let response = get(state.clone(), &path).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response_json::<Value>(response).await,
            json!({
                "code_hash": BLOCKED,
                "verified": verified,
                "status": if verified { "verified" } else { "unverified" },
            })
        );
        assert_eq!(head(state.clone(), &path).await.status(), StatusCode::OK);
    }
}

async fn assert_blocked_submissions(state: &AppState) {
    for (field, target) in [
        ("code_hash", BLOCKED.to_owned()),
        ("code_hash", BLOCKED.to_ascii_uppercase()),
        ("code_hash", BLOCKED_BASE64.to_owned()),
        ("address", ADDRESS.to_owned()),
    ] {
        assert_blocked(
            post_verify(state.clone(), parts(field, &target)).await,
            StatusCode::FORBIDDEN,
        )
        .await;
        assert_blocked(
            post_verify_without_payment(state.clone(), parts(field, &target)).await,
            StatusCode::FORBIDDEN,
        )
        .await;
        assert_blocked(
            post_verify_with_api_key(state.clone(), parts(field, &target), API_KEY).await,
            StatusCode::FORBIDDEN,
        )
        .await;
        assert_blocked(
            post_verify_with_user_agent(state.clone(), parts(field, &target), "acton/1.2.0").await,
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    for user_agent in ["acton/1.2.0", "acton/1.3.0"] {
        for hash in [
            BLOCKED.to_owned(),
            BLOCKED.to_ascii_uppercase(),
            BLOCKED_BASE64.to_owned(),
        ] {
            let request = Request::builder()
                .method("POST")
                .uri("/api/v1/take_ticket")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::USER_AGENT, user_agent)
                .body(Body::from(
                    json!({"code_hash": hash, "compiler": "tolk", "compiler_version": "1.4.1"})
                        .to_string(),
                ))
                .expect("ticket request");
            assert_blocked(
                app::router_with_state(state.clone())
                    .oneshot(request)
                    .await
                    .expect("ticket response"),
                StatusCode::FORBIDDEN,
            )
            .await;
        }
    }
}

#[tokio::test]
async fn verification_status_ignores_blocklist_for_unverified_hashes() {
    let state = state(
        Arc::new(MockSourceStorage::confirmed()),
        compiler(BLOCKED),
        SqliteVerificationIndex::in_memory().expect("index"),
        policy(&[BLOCKED]),
    );
    assert_status_for_blocked_hash(&state, false).await;
}

#[tokio::test]
async fn blocked_hashes_are_rejected_before_payment_compilation_or_storage() {
    let compiler = Arc::new(MockCompilerService::new(BLOCKED));
    let storage = Arc::new(MockSourceStorage::failing("storage must not be accessed"));
    let state = state(
        storage.clone(),
        compiler.clone(),
        SqliteVerificationIndex::in_memory().expect("index"),
        policy(&[BLOCKED]),
    );
    assert_blocked_reads(&state).await;
    assert_blocked_submissions(&state).await;
    assert!(
        compiler
            .recorded_requests()
            .lock()
            .expect("compiler requests")
            .is_empty()
    );
    assert!(
        storage
            .recorded_requests()
            .lock()
            .expect("storage requests")
            .is_empty()
    );
}

fn compiler(code_hash: &str) -> Arc<MockCompilerService> {
    Arc::new(MockCompilerService::with_generated_sources(
        code_hash,
        vec![CompileGeneratedSource {
            path: "output/main.abi.json".to_owned(),
            content: r#"{"contract_name":"Example","abi_schema_version":"1.0"}"#.to_owned(),
        }],
    ))
}

#[tokio::test]
async fn policy_blocks_direct_access_without_changing_indexed_collections() {
    let directory = tempfile::tempdir().expect("index directory");
    let path = directory.path().join("index.sqlite3");
    let storage = Arc::new(MockSourceStorage::confirmed());
    for (hash, timestamp) in [(ALLOWED, "1000000"), (BLOCKED, "2000000")] {
        let state = state(
            storage.clone(),
            compiler(hash),
            SqliteVerificationIndex::open(&path).expect("index"),
            CodeHashPolicy::default(),
        );
        let mut parts = parts("code_hash", hash);
        parts.push(text_part("verified_at", timestamp));
        let response = post_verify_with_api_key(state, parts, API_KEY).await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let index = SqliteVerificationIndex::open(&path).expect("existing index");
    let mut bundle = index
        .bundle(BLOCKED)
        .await
        .expect("bundle lookup")
        .expect("stored bundle");
    bundle.manifest.payment_tx_hash = Some(PAYMENT_TX_HASH.to_owned());
    index
        .upsert_bundle(&bundle, Some("mock-revision"))
        .await
        .expect("record published payment");
    let state = state(
        storage.clone(),
        compiler(BLOCKED),
        index,
        policy(&[BLOCKED]),
    );

    assert_status_for_blocked_hash(&state, true).await;
    assert_blocked_reads(&state).await;
    assert_blocked_submissions(&state).await;
    assert_eq!(
        state
            .published_payment_transaction_hashes()
            .await
            .expect("payment history"),
        [PAYMENT_TX_HASH]
    );
    assert_eq!(
        get(
            state.clone(),
            &format!("/api/v1/verification/source?code_hash={ALLOWED}")
        )
        .await
        .status(),
        StatusCode::OK
    );

    for endpoint in ["last_verified", "abi"] {
        let response = get(state.clone(), &format!("/api/v1/{endpoint}?limit=1")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::LAST_MODIFIED],
            "Thu, 01 Jan 1970 00:33:20 GMT"
        );
        let body = response_json::<Value>(response).await;
        assert_eq!(body["total"], 2);
        assert_eq!(body["items"].as_array().expect("items").len(), 1);
        assert_eq!(body["items"][0]["code_hash"], BLOCKED);
        let response = head(state.clone(), &format!("/api/v1/{endpoint}?offset=99")).await;
        assert_eq!(
            response.headers()[header::LAST_MODIFIED],
            "Thu, 01 Jan 1970 00:33:20 GMT"
        );
        let body = response_json::<Value>(
            get(
                state.clone(),
                &format!("/api/v1/{endpoint}?limit=1&offset=1"),
            )
            .await,
        )
        .await;
        assert_eq!(body["total"], 2);
        assert_eq!(body["items"].as_array().expect("items").len(), 1);
        assert_eq!(body["items"][0]["code_hash"], ALLOWED);
    }
    let statistics = response_json::<Value>(get(state.clone(), "/api/v1/statistics").await).await;
    assert_eq!(statistics["total"], 2);
    let history = response_json::<Value>(get(state, "/api/v1/statistics/history").await).await;
    assert_eq!(history["items"].as_array().expect("history").len(), 2);
    assert_eq!(history["items"][0]["timestamp"], 1000);
    assert_eq!(history["items"][1]["timestamp"], 2000);

    // Reopening the same index with no restrictions restores access without reindexing.
    let restored = self::state(
        storage,
        compiler(BLOCKED),
        SqliteVerificationIndex::open(&path).expect("reopen index"),
        CodeHashPolicy::default(),
    );
    assert_eq!(
        get(
            restored.clone(),
            &format!("/api/v1/verification/source?code_hash={BLOCKED}")
        )
        .await
        .status(),
        StatusCode::OK
    );
    for endpoint in ["last_verified", "abi", "statistics"] {
        let body =
            response_json::<Value>(get(restored.clone(), &format!("/api/v1/{endpoint}")).await)
                .await;
        assert_eq!(body["total"], 2);
    }
}

#[tokio::test]
async fn runtime_config_applies_policy() {
    let directory = tempfile::tempdir().expect("state directory");
    let config = config(&format!(
        r#"
[server]
blocked_code_hashes = ["{BLOCKED}"]
[registry_index]
path = "{}"
[payment]
address = "{PAYMENT_ADDRESS}"
min_amount_nano = 1
ledger_path = "{}"
"#,
        directory.path().join("index.sqlite3").display(),
        directory.path().join("payment.sqlite3").display()
    ))
    .expect("runtime config");
    let configured = AppState::from_config(&config).expect("configured state");
    assert_blocked(
        get(
            configured,
            &format!("/api/v1/verification/source?code_hash={BLOCKED}"),
        )
        .await,
        StatusCode::NOT_FOUND,
    )
    .await;
}

#[tokio::test]
async fn collections_and_status_follow_source_removal_after_restart() {
    let directory = tempfile::tempdir().expect("index directory");
    let path = directory.path().join("index.sqlite3");
    let storage = Arc::new(MockSourceStorage::confirmed());
    let initial = state(
        storage.clone(),
        compiler(BLOCKED),
        SqliteVerificationIndex::in_memory().expect("index"),
        CodeHashPolicy::default(),
    );
    assert_eq!(
        post_verify_with_api_key(initial, parts("code_hash", BLOCKED), API_KEY)
            .await
            .status(),
        StatusCode::OK
    );
    // A fresh index includes all source bundles, even with an active blocklist.
    let blocked = state(
        storage,
        compiler(BLOCKED),
        SqliteVerificationIndex::open(&path).expect("index"),
        policy(&[BLOCKED]),
    );
    assert_status_for_blocked_hash(&blocked, true).await;
    assert_blocked_reads(&blocked).await;
    for endpoint in ["last_verified", "abi"] {
        let response = get(blocked.clone(), &format!("/api/v1/{endpoint}")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key(header::LAST_MODIFIED));
        let body = response_json::<Value>(response).await;
        assert_eq!(body["total"], 1);
        assert_eq!(body["items"].as_array().expect("items").len(), 1);
        assert_eq!(body["items"][0]["code_hash"], BLOCKED);
    }
    assert_eq!(
        response_json::<Value>(get(blocked.clone(), "/api/v1/statistics").await).await["total"],
        1
    );
    drop(blocked);

    // A committed deletion changes the source revision. Restarting with the same
    // persisted index must rebuild it and discard records absent from the repository.
    let restarted = state(
        Arc::new(MockSourceStorage::snapshot(
            "revision-after-removal",
            Vec::new(),
        )),
        compiler(BLOCKED),
        SqliteVerificationIndex::open(&path).expect("reopen index"),
        policy(&[BLOCKED]),
    );
    assert_status_for_blocked_hash(&restarted, false).await;
    assert_blocked_reads(&restarted).await;
    assert_blocked_submissions(&restarted).await;
    for endpoint in ["last_verified", "abi"] {
        let response = get(restarted.clone(), &format!("/api/v1/{endpoint}")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!response.headers().contains_key(header::LAST_MODIFIED));
        assert_eq!(
            response_json::<Value>(response).await,
            json!({"items": [], "total": 0})
        );
        assert!(
            !head(restarted.clone(), &format!("/api/v1/{endpoint}"))
                .await
                .headers()
                .contains_key(header::LAST_MODIFIED)
        );
    }
    assert_eq!(
        response_json::<Value>(get(restarted.clone(), "/api/v1/statistics").await).await["total"],
        0
    );
    assert_eq!(
        response_json::<Value>(get(restarted, "/api/v1/statistics/history").await).await["items"],
        json!([])
    );
}
