mod support;

use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use support::{
    app_state, file_part, get, post_verify, recording_app_state, response_json, text_part,
};

const ADDRESS_ONE: &str = "EQD0000000000000000000000000000000000000000000000";
const ADDRESS_TWO: &str = "EQD1111111111111111111111111111111111111111111111";
const CODE_HASH_ONE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CODE_HASH_TWO: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const COMPILE_PARAMS_TOLK: &str = r#"{"compiler_version":"1.4.1"}"#;
const SOURCES_MAIN: &str = r#"[{"path":"main.tolk","is_entrypoint":true}]"#;
const SOURCES_TWO_FILES: &str = r#"[
  {"path":"main.tolk","is_entrypoint":true},
  {"path":"imports/lib.tolk","is_entrypoint":false}
]"#;

#[tokio::test]
async fn healthz_returns_ok() {
    let response = get(app_state(&[], CODE_HASH_ONE), "/healthz").await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<Value>(response).await;
    assert_eq!(body, json!({"ok": true}));
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
    language: String,
    compile_params: Value,
    files: Vec<FileSummary>,
}

#[derive(Debug, Deserialize)]
struct FileSummary {
    path: String,
    is_entrypoint: bool,
}
