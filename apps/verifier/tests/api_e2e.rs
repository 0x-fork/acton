mod support;

use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use support::{
    app_state, failing_compiler_app_state, failing_registry_app_state,
    failing_source_storage_recording_registry_app_state, file_part, get, post_verify,
    recording_app_state, recording_registry_app_state, recording_source_storage_app_state,
    response_json, text_part, unverified_registry_app_state,
};

const ADDRESS_ONE: &str = "EQD0000000000000000000000000000000000000000000000";
const ADDRESS_TWO: &str = "EQD1111111111111111111111111111111111111111111111";
const CODE_HASH_ONE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CODE_HASH_ONE_BASE64: &str = "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqo=";
const CODE_HASH_TWO: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const COMPILE_PARAMS_TOLK: &str = r#"{"compiler_version":"1.4.1"}"#;
const COMPILE_PARAMS_TOLK_WITH_IMPORT_MAPPINGS: &str =
    r#"{"compiler_version":"1.4.1","import_mappings":{"@contracts":"contracts"}}"#;
const SOURCES_MAIN: &str = r#"[{"path":"main.tolk","is_entrypoint":true}]"#;
const SOURCES_TWO_FILES: &str = r#"[
  {"path":"main.tolk","is_entrypoint":true},
  {"path":"imports/lib.tolk","is_entrypoint":false}
]"#;
const SOURCES_ALIASED_FILES: &str = r#"[
  {"path":"main.tolk","is_entrypoint":true},
  {"path":"contracts/lib.tolk","is_entrypoint":false}
]"#;

#[tokio::test]
async fn healthz_returns_ok() {
    let response = get(app_state(&[], CODE_HASH_ONE), "/healthz").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<Value>(response).await;
    assert_eq!(body, json!({"ok": true}));
}

#[tokio::test]
async fn verification_status_reports_verified_for_code_hash() {
    let response = get(
        app_state(&[], CODE_HASH_ONE),
        &format!("/api/v1/verification/status?code_hash={CODE_HASH_ONE}"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationStatusResponse>(response).await;
    assert_eq!(body.address, None);
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert!(body.verified);
    assert_eq!(body.onchain.master_address, "mock-master");
    assert_eq!(body.onchain.verification_record_address, "mock-record");
}

#[tokio::test]
async fn verification_status_resolves_code_hash_from_address() {
    let response = get(
        app_state(&[(ADDRESS_ONE, CODE_HASH_ONE)], CODE_HASH_ONE),
        &format!("/api/v1/verification/status?address={ADDRESS_ONE}"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationStatusResponse>(response).await;
    assert_eq!(body.address.as_deref(), Some(ADDRESS_ONE));
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert!(body.verified);
}

#[tokio::test]
async fn verification_status_reports_unverified_contract() {
    let response = get(
        unverified_registry_app_state(&[], CODE_HASH_ONE),
        &format!("/api/v1/verification/status?code_hash={CODE_HASH_ONE}"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationStatusResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert!(!body.verified);
}

#[tokio::test]
async fn verification_status_rejects_missing_target() {
    let response = get(app_state(&[], CODE_HASH_ONE), "/api/v1/verification/status").await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "address or code_hash").await;
}

#[tokio::test]
async fn verification_source_returns_verified_bundle_files() {
    let state = app_state(&[], CODE_HASH_ONE);
    let verify_response = post_verify(
        state.clone(),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;
    assert_eq!(verify_response.status(), StatusCode::OK);

    let verified = response_json::<VerifyResponse>(verify_response).await;
    let source_bundle_hash = verified
        .source_bundle_hash
        .as_deref()
        .expect("verify response should include source bundle hash");
    let response = get(
        state,
        &format!("/api/v1/verification/source?code_hash={CODE_HASH_ONE}"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationSourceResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert!(body.verified);
    assert_eq!(body.bundles.len(), 1);
    assert_eq!(body.bundles[0].source_bundle_hash, source_bundle_hash);
    assert_eq!(body.bundles[0].language, "tolk");
    assert_eq!(body.bundles[0].compiler_version, "1.4.1");
    assert_eq!(body.bundles[0].entrypoint, "main.tolk");
    assert_eq!(body.bundles[0].sources.len(), 1);
    assert_eq!(body.bundles[0].sources[0].path, "main.tolk");
    assert!(body.bundles[0].sources[0].is_entrypoint);
    assert_eq!(body.bundles[0].files.len(), 1);
    assert_eq!(body.bundles[0].files[0].path, "main.tolk");
    assert_eq!(
        body.bundles[0].files[0].content_text.as_deref(),
        Some("fun main() {}")
    );
}

#[tokio::test]
async fn verify_resolves_code_hash_from_address_with_mock_blockchain() {
    let response = post_verify(
        app_state(&[(ADDRESS_ONE, CODE_HASH_ONE)], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.address.as_deref(), Some(ADDRESS_ONE));
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert_eq!(body.compiled_code_hash, CODE_HASH_ONE);
    assert_eq!(body.verification_result, "match");
    assert!(body.source_bundle_hash.is_some());
    assert_eq!(
        body.onchain_registration
            .as_ref()
            .map(|registration| registration.status.as_str()),
        Some("confirmed")
    );
    assert_eq!(
        body.source_storage
            .as_ref()
            .map(|storage| storage.provider.as_str()),
        Some("mock")
    );
    assert_eq!(body.language, "tolk");
    assert_eq!(body.compile_params, json!({"compiler_version": "1.4.1"}));
    assert_eq!(body.files.len(), 1);
    assert_eq!(body.files[0].path, "main.tolk");
    assert!(body.files[0].is_entrypoint);
}

#[tokio::test]
async fn verify_accepts_valid_multipart_request_with_multiple_files() {
    let response = post_verify(
        app_state(&[(ADDRESS_TWO, CODE_HASH_TWO)], CODE_HASH_TWO),
        vec![
            text_part("address", ADDRESS_TWO),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_TWO_FILES),
            file_part(
                "files",
                "main.tolk",
                "text/plain",
                "import \"imports/lib.tolk\";",
            ),
            file_part("files", "imports/lib.tolk", "text/plain", "fun helper() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.address.as_deref(), Some(ADDRESS_TWO));
    assert_eq!(body.code_hash, CODE_HASH_TWO);
    assert_eq!(body.compiled_code_hash, CODE_HASH_TWO);
    assert_eq!(body.verification_result, "match");
    assert_eq!(body.files.len(), 2);
    assert_eq!(body.files[0].path, "main.tolk");
    assert_eq!(body.files[1].path, "imports/lib.tolk");
}

#[tokio::test]
async fn verify_passes_uploaded_file_contents_to_compiler() {
    let (state, recorded_requests) = recording_app_state(&[], CODE_HASH_ONE);
    let response = post_verify(
        state,
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_TWO_FILES),
            file_part(
                "files",
                "main.tolk",
                "text/plain",
                "import \"imports/lib.tolk\";",
            ),
            file_part("files", "imports/lib.tolk", "text/plain", "fun helper() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let (language, compiler_version, entrypoint, sources) = {
        let recorded_requests = recorded_requests
            .lock()
            .expect("recorded compiler requests mutex should not be poisoned");
        assert_eq!(recorded_requests.len(), 1);

        let request = &recorded_requests[0];
        let snapshot = (
            request.language.clone(),
            request.compiler_version.clone(),
            request.entrypoint.clone(),
            request
                .sources
                .iter()
                .map(|source| (source.path.clone(), source.content.clone()))
                .collect::<Vec<_>>(),
        );
        drop(recorded_requests);
        snapshot
    };

    assert_eq!(language, "tolk");
    assert_eq!(compiler_version, "1.4.1");
    assert_eq!(entrypoint, "main.tolk");
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].0, "main.tolk");
    assert_eq!(sources[0].1, "import \"imports/lib.tolk\";");
    assert_eq!(sources[1].0, "imports/lib.tolk");
    assert_eq!(sources[1].1, "fun helper() {}");
}

#[tokio::test]
async fn verify_passes_import_mappings_to_compiler() {
    let (state, recorded_requests) = recording_app_state(&[], CODE_HASH_ONE);
    let response = post_verify(
        state,
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK_WITH_IMPORT_MAPPINGS),
            text_part("sources", SOURCES_ALIASED_FILES),
            file_part(
                "files",
                "main.tolk",
                "text/plain",
                "import \"@contracts/lib\";",
            ),
            file_part(
                "files",
                "contracts/lib.tolk",
                "text/plain",
                "fun helper() {}",
            ),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let contracts_mapping = {
        let recorded_requests = recorded_requests
            .lock()
            .expect("recorded compiler requests mutex should not be poisoned");
        assert_eq!(recorded_requests.len(), 1);

        recorded_requests[0]
            .import_mappings
            .get("@contracts")
            .cloned()
    };
    assert_eq!(contracts_mapping.as_deref(), Some("contracts"));
}

#[tokio::test]
async fn verify_accepts_code_hash_without_address() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.address, None);
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert_eq!(body.compiled_code_hash, CODE_HASH_ONE);
    assert_eq!(body.verification_result, "match");
}

#[tokio::test]
async fn verify_accepts_address_and_code_hash_together() {
    let response = post_verify(
        app_state(&[(ADDRESS_ONE, CODE_HASH_ONE)], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.address.as_deref(), Some(ADDRESS_ONE));
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert_eq!(body.compiled_code_hash, CODE_HASH_ONE);
    assert_eq!(body.verification_result, "match");
}

#[tokio::test]
async fn verify_normalizes_base64_code_hash_input() {
    let response = post_verify(
        app_state(&[(ADDRESS_ONE, CODE_HASH_ONE)], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("code_hash", CODE_HASH_ONE_BASE64),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.address.as_deref(), Some(ADDRESS_ONE));
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert_eq!(body.compiled_code_hash, CODE_HASH_ONE);
    assert_eq!(body.verification_result, "match");
}

#[tokio::test]
async fn verify_returns_mismatch_when_compiled_hash_differs_from_target() {
    let response = post_verify(
        app_state(&[], CODE_HASH_TWO),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.code_hash, CODE_HASH_ONE);
    assert_eq!(body.compiled_code_hash, CODE_HASH_TWO);
    assert_eq!(body.verification_result, "mismatch");
    assert_eq!(body.source_bundle_hash, None);
    assert!(body.source_storage.is_none());
    assert!(body.onchain_registration.is_none());
}

#[tokio::test]
async fn verify_stores_source_bundle_on_hash_match() {
    let (state, recorded_requests) = recording_source_storage_app_state(&[], CODE_HASH_ONE);
    let response = post_verify(
        state,
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    let source_bundle_hash = body
        .source_bundle_hash
        .as_deref()
        .expect("matched verification should return source bundle hash");
    assert_eq!(
        body.source_storage
            .as_ref()
            .map(|storage| storage.commit.as_str()),
        Some("mock-commit")
    );
    assert_eq!(
        body.source_storage
            .as_ref()
            .map(|storage| storage.bundle_path.as_str()),
        Some("sources/mock-code-hash/mock-source-bundle-hash")
    );

    let recorded_snapshot = {
        let recorded_requests = recorded_requests
            .lock()
            .expect("recorded source storage requests mutex should not be poisoned");
        let snapshot = recorded_requests.clone();
        drop(recorded_requests);
        snapshot
    };
    assert_eq!(recorded_snapshot.len(), 1);
    assert_eq!(recorded_snapshot[0].code_hash, CODE_HASH_ONE);
    assert_eq!(recorded_snapshot[0].source_bundle_hash, source_bundle_hash);
    assert_eq!(recorded_snapshot[0].files.len(), 1);
    assert_eq!(recorded_snapshot[0].files[0].0, "main.tolk");
    assert_eq!(recorded_snapshot[0].files[0].1, b"fun main() {}");
}

#[tokio::test]
async fn verify_registers_source_bundle_on_hash_match() {
    let (state, recorded_requests) = recording_registry_app_state(&[], CODE_HASH_ONE);
    let response = post_verify(
        state,
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    let source_bundle_hash = body
        .source_bundle_hash
        .as_deref()
        .expect("matched verification should return source bundle hash");
    assert_eq!(source_bundle_hash.len(), 64);
    assert!(
        source_bundle_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    );
    assert_eq!(
        body.onchain_registration
            .as_ref()
            .map(|registration| registration.verification_record_address.as_str()),
        Some("mock-record")
    );
    assert_eq!(
        body.onchain_registration
            .as_ref()
            .map(|registration| registration.master_address.as_str()),
        Some("mock-master")
    );

    let recorded_snapshot = {
        let recorded_requests = recorded_requests
            .lock()
            .expect("recorded registry requests mutex should not be poisoned");
        let snapshot = recorded_requests
            .iter()
            .map(|request| {
                (
                    request.code_hash.clone(),
                    request.source_bundle_hash.clone(),
                )
            })
            .collect::<Vec<_>>();
        drop(recorded_requests);
        snapshot
    };
    assert_eq!(recorded_snapshot.len(), 1);
    assert_eq!(recorded_snapshot[0].0, CODE_HASH_ONE);
    assert_eq!(recorded_snapshot[0].1, source_bundle_hash);
}

#[tokio::test]
async fn verify_does_not_register_source_bundle_on_hash_mismatch() {
    let (state, recorded_requests) = recording_registry_app_state(&[], CODE_HASH_TWO);
    let response = post_verify(
        state,
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.verification_result, "mismatch");
    assert!(body.source_storage.is_none());
    assert!(body.onchain_registration.is_none());

    let recorded_requests_is_empty = {
        let recorded_requests = recorded_requests
            .lock()
            .expect("recorded registry requests mutex should not be poisoned");
        let is_empty = recorded_requests.is_empty();
        drop(recorded_requests);
        is_empty
    };
    assert!(recorded_requests_is_empty);
}

#[tokio::test]
async fn verify_returns_bad_gateway_when_source_storage_fails() {
    let (state, recorded_registry_requests) =
        failing_source_storage_recording_registry_app_state(&[], CODE_HASH_ONE);

    let response = post_verify(
        state,
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_error_contains(response, "source storage failed").await;

    let registry_requests_is_empty = {
        let recorded_requests = recorded_registry_requests
            .lock()
            .expect("recorded registry requests mutex should not be poisoned");
        let is_empty = recorded_requests.is_empty();
        drop(recorded_requests);
        is_empty
    };
    assert!(registry_requests_is_empty);
}

#[tokio::test]
async fn verify_returns_bad_gateway_when_registry_registration_fails() {
    let response = post_verify(
        failing_registry_app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_error_contains(response, "registry sender failed").await;
}

#[tokio::test]
async fn verify_returns_bad_request_when_compilation_fails() {
    let response = post_verify(
        failing_compiler_app_state(&[], "Tolk syntax error at main.tolk:1:5"),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun broken("),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json::<Value>(response).await;
    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("Tolk syntax error at main.tolk:1:5"),
        "expected error to contain compiler details, got {body}"
    );
}

#[tokio::test]
async fn verify_rejects_address_and_code_hash_mismatch() {
    let response = post_verify(
        app_state(&[(ADDRESS_ONE, CODE_HASH_TWO)], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "mismatch").await;
}

#[tokio::test]
async fn verify_rejects_address_without_onchain_code_hash() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "not found").await;
}

#[tokio::test]
async fn verify_rejects_missing_verification_target() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "address or code_hash").await;
}

#[tokio::test]
async fn verify_rejects_missing_language() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "language").await;
}

#[tokio::test]
async fn verify_rejects_missing_files() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "files").await;
}

#[tokio::test]
async fn verify_rejects_invalid_compile_params_json() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("address", ADDRESS_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", "{not json"),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "compile_params").await;
}

#[tokio::test]
async fn verify_rejects_missing_sources() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "sources").await;
}

#[tokio::test]
async fn verify_rejects_missing_entrypoint_source() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", r#"[{"path":"main.tolk","is_entrypoint":false}]"#),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "entrypoint").await;
}

#[tokio::test]
async fn verify_rejects_multiple_entrypoint_sources() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part(
                "sources",
                r#"[
                  {"path":"main.tolk","is_entrypoint":true},
                  {"path":"other.tolk","is_entrypoint":true}
                ]"#,
            ),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
            file_part("files", "other.tolk", "text/plain", "fun other() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "multiple entrypoint").await;
}

#[tokio::test]
async fn verify_rejects_uploaded_file_without_source_metadata() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
            file_part("files", "extra.tolk", "text/plain", "fun extra() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "no source metadata").await;
}

#[tokio::test]
async fn verify_rejects_source_metadata_without_uploaded_file() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_TWO_FILES),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "no uploaded file").await;
}

#[tokio::test]
async fn verify_rejects_invalid_source_path() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part(
                "sources",
                r#"[{"path":"../main.tolk","is_entrypoint":true}]"#,
            ),
            file_part("files", "../main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "invalid component").await;
}

#[tokio::test]
async fn verify_rejects_backslash_source_path() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part(
                "sources",
                r#"[{"path":"imports\\lib.tolk","is_entrypoint":true}]"#,
            ),
            file_part("files", "imports\\lib.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "separators").await;
}

#[tokio::test]
async fn verify_rejects_unsupported_language() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "func"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "unsupported language").await;
}

#[tokio::test]
async fn verify_rejects_unsupported_tolk_version() {
    let response = post_verify(
        app_state(&[], CODE_HASH_ONE),
        vec![
            text_part("code_hash", CODE_HASH_ONE),
            text_part("language", "tolk"),
            text_part("compile_params", r#"{"compiler_version":"1.4.0"}"#),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error_contains(response, "unsupported Tolk compiler_version").await;
}

async fn assert_error_contains(response: axum::response::Response, expected: &str) {
    let body = response_json::<Value>(response).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains(expected),
        "expected error to contain {expected}, got {body}"
    );
}

#[derive(Debug, Deserialize)]
struct VerifyResponse {
    address: Option<String>,
    code_hash: String,
    compiled_code_hash: String,
    verification_result: String,
    source_bundle_hash: Option<String>,
    source_storage: Option<SourceStorage>,
    onchain_registration: Option<OnchainRegistration>,
    language: String,
    compile_params: Value,
    files: Vec<FileSummary>,
}

#[derive(Debug, Deserialize)]
struct SourceStorage {
    provider: String,
    commit: String,
    bundle_path: String,
}

#[derive(Debug, Deserialize)]
struct OnchainRegistration {
    status: String,
    master_address: String,
    verification_record_address: String,
}

#[derive(Debug, Deserialize)]
struct FileSummary {
    path: String,
    is_entrypoint: bool,
}

#[derive(Debug, Deserialize)]
struct VerificationStatusResponse {
    address: Option<String>,
    code_hash: String,
    verified: bool,
    onchain: VerificationOnchain,
}

#[derive(Debug, Deserialize)]
struct VerificationSourceResponse {
    code_hash: String,
    verified: bool,
    bundles: Vec<VerifiedSourceBundle>,
}

#[derive(Debug, Deserialize)]
struct VerificationOnchain {
    master_address: String,
    verification_record_address: String,
}

#[derive(Debug, Deserialize)]
struct VerifiedSourceBundle {
    source_bundle_hash: String,
    language: String,
    compiler_version: String,
    entrypoint: String,
    sources: Vec<VerifiedSourceSummary>,
    files: Vec<VerifiedSourceFile>,
}

#[derive(Debug, Deserialize)]
struct VerifiedSourceSummary {
    path: String,
    is_entrypoint: bool,
}

#[derive(Debug, Deserialize)]
struct VerifiedSourceFile {
    path: String,
    content_text: Option<String>,
}
