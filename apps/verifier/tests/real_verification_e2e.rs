mod support;

use std::path::Path;

use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use support::{
    get, owned_file_part, owned_text_part, post_verify, real_compiler_app_state, response_json,
};

const TOLK_CODE_HASH: &str = "a873d8c2d163f7fa10bbe38769706f0554505e8ea2dcea3f115288db8becf2ab";
const SIMPLE_TOLK_CODE_HASH: &str =
    "63600fb71c1bfc85ed75dfbbd7b8e857ca98bc003fb2f758f07708fd1664edae";
const FUNC_CODE_HASH: &str = "6ef6e4084167bca1464f9d2ddc8448bbd66df303c4014af50aeb5a109fdfb8cc";
const TACT_CODE_HASH: &str = "f6b6d11538f0cb19c9f5b2812cb66d907b56c752c673d1bea205f07bce4c7f52";
const ALL_TOLK_VERSIONS: &[&str] = &[
    "0.6.0", "0.7.0", "0.8.0", "0.9.0", "0.10.0", "0.11.0", "0.12.0", "0.13.0", "0.99.0", "1.0.0",
    "1.1.0", "1.2.0", "1.3.0", "1.4.0", "1.4.1",
];

#[tokio::test]
async fn verify_tolk_with_real_compiler_and_stores_generated_abi() {
    let state = real_compiler_app_state(&[]);
    let response =
        verify_fixture(state.clone(), TOLK_CODE_HASH, fixture("valid-minimal.json")).await;

    assert_verified(response, "tolk", TOLK_CODE_HASH).await;

    let response = get(
        state,
        &format!("/api/v1/verification/source?code_hash={TOLK_CODE_HASH}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationSourceResponse>(response).await;
    assert!(body.verified);
    assert_eq!(body.bundles.len(), 1);
    assert_eq!(body.bundles[0].compiler.language, "tolk");
    assert_eq!(body.bundles[0].compiler.version, "1.4.1");
    assert_eq!(body.bundles[0].compiler.entrypoint, "main.tolk");
    let files = &body.bundles[0].files;
    let abi = files
        .iter()
        .find(|file| file.path == "output/main.abi.json")
        .expect("expected stored Tolk bundle to include generated ABI JSON");
    let abi =
        serde_json::from_str::<Value>(&abi.content).expect("generated Tolk ABI should be JSON");
    assert_eq!(abi["abi_schema_version"], "1.0");
    assert_eq!(abi["compiler_name"], "tolk");
    assert_eq!(abi["compiler_version"], "1.4.1");
}

#[tokio::test]
async fn verify_tolk_import_mappings_with_real_compiler() {
    let state = real_compiler_app_state(&[]);
    let response =
        verify_fixture(state, TOLK_CODE_HASH, fixture("valid-import-mapping.json")).await;

    assert_verified(response, "tolk", TOLK_CODE_HASH).await;
}

#[tokio::test]
async fn verify_all_tolk_npm_versions_with_real_compiler() {
    for compiler_version in ALL_TOLK_VERSIONS {
        let state = real_compiler_app_state(&[]);
        let response = verify_fixture(
            state,
            SIMPLE_TOLK_CODE_HASH,
            simple_tolk_fixture(compiler_version),
        )
        .await;

        assert_verified(response, "tolk", SIMPLE_TOLK_CODE_HASH).await;
    }
}

#[tokio::test]
async fn verify_func_with_real_compiler() {
    let state = real_compiler_app_state(&[]);
    let response = verify_fixture(state, FUNC_CODE_HASH, fixture("valid-func.json")).await;

    assert_verified(response, "func", FUNC_CODE_HASH).await;
}

#[tokio::test]
async fn verify_tact_with_real_compiler_and_stores_generated_sources() {
    let state = real_compiler_app_state(&[]);
    let response = verify_fixture(state.clone(), TACT_CODE_HASH, fixture("valid-tact.json")).await;
    assert_verified(response, "tact", TACT_CODE_HASH).await;

    let response = get(
        state,
        &format!("/api/v1/verification/source?code_hash={TACT_CODE_HASH}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationSourceResponse>(response).await;
    assert!(body.verified);
    assert_eq!(body.bundles.len(), 1);
    assert_eq!(body.bundles[0].compiler.language, "tact");
    assert_eq!(body.bundles[0].compiler.version, "1.6.13");
    assert_eq!(body.bundles[0].compiler.entrypoint, "contract.pkg");
    let files = &body.bundles[0].files;
    assert!(files.iter().any(|file| file.path == "contract.pkg"));
    assert!(
        files.iter().any(|file| has_extension(&file.path, "abi")),
        "expected stored Tact bundle to include generated ABI"
    );
    assert!(
        files.iter().any(|file| has_extension(&file.path, "tact")),
        "expected stored Tact bundle to include generated source"
    );
}

async fn verify_fixture(
    state: verifier::state::AppState,
    code_hash: &str,
    fixture: WorkerFixture,
) -> axum::response::Response {
    let WorkerFixture {
        language,
        compiler_version,
        import_mappings,
        entrypoint,
        sources,
    } = fixture;
    assert!(
        sources.iter().any(|source| source.path == entrypoint),
        "fixture entrypoint {entrypoint} should be present in sources"
    );

    let compiler_version = compiler_version.as_str();
    let compile_params = import_mappings.map_or_else(
        || json!({"compiler_version": compiler_version}),
        |import_mappings| {
            json!({"compiler_version": compiler_version, "import_mappings": import_mappings})
        },
    );
    let source_metadata = sources
        .iter()
        .map(|source| WorkerSourceMetadata::from_source(source, &entrypoint))
        .collect::<Vec<_>>();

    let mut parts = vec![
        owned_text_part("code_hash", code_hash.to_owned()),
        owned_text_part("language", language),
        owned_text_part("compile_params", compile_params.to_string()),
        owned_text_part(
            "sources",
            serde_json::to_string(&source_metadata).expect("source metadata should serialize"),
        ),
    ];
    for source in sources {
        parts.push(owned_file_part(
            "files",
            source.path,
            "text/plain",
            source.content,
        ));
    }

    post_verify(state, parts).await
}

async fn assert_verified(response: axum::response::Response, _language: &str, code_hash: &str) {
    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.code_hash, code_hash);
    assert_eq!(body.compiled_code_hash, code_hash);
    assert_eq!(body.verification_result, "match");
    assert!(body.source_bundle_hash.is_some());
    assert!(body.storage_revision.is_some());
}

fn fixture(name: &str) -> WorkerFixture {
    let path = Path::new("compiler-worker").join("fixtures").join(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|err| panic!("fixture is not valid JSON {}: {err}", path.display()))
}

fn simple_tolk_fixture(compiler_version: &str) -> WorkerFixture {
    WorkerFixture {
        language: "tolk".to_owned(),
        compiler_version: compiler_version.to_owned(),
        import_mappings: None,
        entrypoint: "main.tolk".to_owned(),
        sources: vec![WorkerSource {
            path: "main.tolk".to_owned(),
            content: "fun main(): int {\n    return 0;\n}\n".to_owned(),
            is_entrypoint: true,
            include_in_command: None,
            is_stdlib: None,
            has_include_directives: None,
        }],
    }
}

fn has_extension(path: &str, extension: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|actual| actual.eq_ignore_ascii_case(extension))
}

#[derive(Debug, Deserialize)]
struct WorkerFixture {
    language: String,
    compiler_version: String,
    #[serde(default)]
    import_mappings: Option<Value>,
    entrypoint: String,
    sources: Vec<WorkerSource>,
}

#[derive(Debug, Deserialize)]
struct WorkerSource {
    path: String,
    content: String,
    #[serde(default)]
    is_entrypoint: bool,
    #[serde(default)]
    include_in_command: Option<bool>,
    #[serde(default)]
    is_stdlib: Option<bool>,
    #[serde(default)]
    has_include_directives: Option<bool>,
}

#[derive(serde::Serialize)]
struct WorkerSourceMetadata<'a> {
    path: &'a str,
    is_entrypoint: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_in_command: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_stdlib: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    has_include_directives: Option<bool>,
}

impl<'a> WorkerSourceMetadata<'a> {
    fn from_source(source: &'a WorkerSource, entrypoint: &str) -> Self {
        Self {
            path: &source.path,
            is_entrypoint: source.is_entrypoint || source.path == entrypoint,
            include_in_command: source.include_in_command,
            is_stdlib: source.is_stdlib,
            has_include_directives: source.has_include_directives,
        }
    }
}

#[derive(Debug, Deserialize)]
struct VerifyResponse {
    code_hash: String,
    compiled_code_hash: String,
    verification_result: String,
    source_bundle_hash: Option<String>,
    storage_revision: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VerificationSourceResponse {
    verified: bool,
    bundles: Vec<VerifiedSourceBundle>,
}

#[derive(Debug, Deserialize)]
struct VerifiedSourceBundle {
    compiler: VerifiedCompiler,
    files: Vec<VerifiedSourceFile>,
}

#[derive(Debug, Deserialize)]
struct VerifiedCompiler {
    language: String,
    version: String,
    entrypoint: String,
}

#[derive(Debug, Deserialize)]
struct VerifiedSourceFile {
    path: String,
    content: String,
}
