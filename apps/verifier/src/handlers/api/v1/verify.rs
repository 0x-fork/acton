use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};

use axum::{
    Json,
    body::Bytes,
    extract::{
        Multipart as MultipartExtractor, State,
        multipart::{Field, Multipart},
    },
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    blockchain::normalize_code_hash,
    compilers::{CompileRequest, CompileSource},
    error::ApiError,
    registry::{RegisterBundleRequest, RegistrationReceipt},
    source_bundle::{
        SourceBundleFile, SourceBundleInput, SourceBundleSource, compute_source_bundle_hash,
    },
    source_storage::{
        SourceStorageFile, SourceStorageProvider, SourceStorageReceipt, SourceStorageSource,
        StoreSourceBundleRequest,
    },
    state::AppState,
    verification::{ResolvedVerificationTarget, VerificationTarget},
};

const TOLK_LANGUAGE: &str = "tolk";
const SUPPORTED_TOLK_VERSION: &str = "1.4.1";

pub async fn handler(
    State(state): State<AppState>,
    multipart: MultipartExtractor,
) -> Result<impl IntoResponse, ApiError> {
    handle_multipart(&state, multipart).await
}

async fn handle_multipart(
    state: &AppState,
    mut multipart: Multipart,
) -> Result<Json<VerifyResponse>, ApiError> {
    let mut address = None;
    let mut code_hash = None;
    let mut language = None;
    let mut compile_params = json!({});
    let mut sources = None;
    let mut files = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| ApiError::bad_request(err.to_string()))?
    {
        match field.name() {
            Some("address") => {
                address = Some(
                    field
                        .text()
                        .await
                        .map_err(|err| ApiError::bad_request(err.to_string()))?,
                );
            }
            Some("code_hash") => {
                code_hash = Some(
                    field
                        .text()
                        .await
                        .map_err(|err| ApiError::bad_request(err.to_string()))?,
                );
            }
            Some("language") => {
                language = Some(
                    field
                        .text()
                        .await
                        .map_err(|err| ApiError::bad_request(err.to_string()))?,
                );
            }
            Some("compile_params") => {
                let raw_params = field
                    .text()
                    .await
                    .map_err(|err| ApiError::bad_request(err.to_string()))?;
                compile_params = serde_json::from_str(&raw_params).map_err(|err| {
                    ApiError::bad_request(format!("invalid compile_params JSON: {err}"))
                })?;
            }
            Some("sources") => {
                let raw_sources = field
                    .text()
                    .await
                    .map_err(|err| ApiError::bad_request(err.to_string()))?;
                sources = Some(
                    serde_json::from_str::<Vec<SourceMetadata>>(&raw_sources).map_err(|err| {
                        ApiError::bad_request(format!("invalid sources JSON: {err}"))
                    })?,
                );
            }
            Some("files") => {
                files.push(read_file_part(field).await?);
            }
            _ => {}
        }
    }

    let target = VerificationTarget {
        address: non_empty_text(address),
        code_hash: non_empty_text(code_hash),
    };

    let language = language
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("missing required field: language".to_owned()))?;

    if files.is_empty() {
        return Err(ApiError::bad_request(
            "missing required field: files".to_owned(),
        ));
    }

    let compile_input = prepare_compile_input(language, &compile_params, sources, files)?;
    let resolved_target = state.verification_service().resolve_target(target).await?;
    let compiled = state
        .compiler_service()
        .compile(CompileRequest {
            language: compile_input.language.clone(),
            compiler_version: compile_input.compiler_version.clone(),
            entrypoint: compile_input.entrypoint.clone(),
            import_mappings: compile_input.import_mappings.clone(),
            sources: compile_input.compile_sources,
        })
        .await?;
    let compiled_code_hash = normalize_code_hash(&compiled.code_hash);
    let verification_result =
        VerificationResult::from_hashes(&resolved_target.code_hash, &compiled_code_hash);
    let (source_bundle_hash, source_storage, onchain_registration) = match verification_result {
        VerificationResult::Match => {
            let source_bundle_hash = compile_input.source_bundle_hash.clone();
            let source_storage = state
                .source_storage()
                .store_bundle(StoreSourceBundleRequest {
                    address: resolved_target.address.clone(),
                    code_hash: resolved_target.code_hash.clone(),
                    source_bundle_hash: source_bundle_hash.clone(),
                    language: compile_input.language.clone(),
                    compiler_version: compile_input.compiler_version.clone(),
                    entrypoint: compile_input.entrypoint.clone(),
                    compile_params: compile_params.clone(),
                    sources: compile_input
                        .sources
                        .iter()
                        .map(|source| SourceStorageSource {
                            path: source.path.clone(),
                            is_entrypoint: source.is_entrypoint,
                        })
                        .collect(),
                    files: compile_input.storage_files.clone(),
                })
                .await?;
            let registration = state
                .registry_client()
                .register_bundle(RegisterBundleRequest {
                    code_hash: resolved_target.code_hash.clone(),
                    source_bundle_hash: source_bundle_hash.clone(),
                })
                .await?;

            (
                Some(source_bundle_hash),
                Some(source_storage.into()),
                Some(registration.into()),
            )
        }
        VerificationResult::Mismatch => (None, None, None),
    };

    print_verify_request(
        &resolved_target,
        &compile_input.language,
        &compile_params,
        &compile_input.sources,
        &compiled_code_hash,
        source_bundle_hash.as_deref(),
        verification_result,
    );

    Ok(Json(VerifyResponse {
        address: resolved_target.address,
        code_hash: resolved_target.code_hash,
        compiled_code_hash,
        verification_result,
        source_bundle_hash,
        source_storage,
        onchain_registration,
        language: compile_input.language,
        compile_params,
        files: compile_input
            .sources
            .into_iter()
            .map(FileSummary::from_source_metadata)
            .collect(),
    }))
}

fn non_empty_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

async fn read_file_part(field: Field<'_>) -> Result<ReceivedFile, ApiError> {
    let file_name = field.file_name().map(ToOwned::to_owned);
    let content = field
        .bytes()
        .await
        .map_err(|err| ApiError::bad_request(err.to_string()))?;

    Ok(ReceivedFile { file_name, content })
}

fn prepare_compile_input(
    language: String,
    compile_params: &Value,
    sources: Option<Vec<SourceMetadata>>,
    files: Vec<ReceivedFile>,
) -> Result<CompileInput, ApiError> {
    if language != TOLK_LANGUAGE {
        return Err(ApiError::bad_request(format!(
            "unsupported language: {language}"
        )));
    }

    let tolk_compile_params =
        serde_json::from_value::<TolkCompileParams>(compile_params.clone())
            .map_err(|err| ApiError::bad_request(format!("invalid Tolk compile_params: {err}")))?;
    if tolk_compile_params.compiler_version != SUPPORTED_TOLK_VERSION {
        return Err(ApiError::bad_request(format!(
            "unsupported Tolk compiler_version: {}",
            tolk_compile_params.compiler_version
        )));
    }
    validate_import_mappings(&tolk_compile_params.import_mappings)?;
    let sources = sources
        .ok_or_else(|| ApiError::bad_request("missing required field: sources".to_owned()))?;
    let entrypoint = validate_sources(&sources)?;
    let files = match_files_to_sources(&sources, files)?;
    let source_bundle_hash = compute_source_bundle_hash(SourceBundleInput {
        language: &language,
        compiler_version: &tolk_compile_params.compiler_version,
        entrypoint: &entrypoint,
        compile_params,
        sources: sources
            .iter()
            .map(|source| SourceBundleSource {
                path: &source.path,
                is_entrypoint: source.is_entrypoint,
            })
            .collect(),
        files: files
            .iter()
            .map(|(path, file)| SourceBundleFile {
                path,
                bytes: file.content.as_ref(),
            })
            .collect(),
    })?;
    let storage_files = files
        .iter()
        .map(|(path, file)| SourceStorageFile {
            path: path.clone(),
            content: file.content.to_vec(),
        })
        .collect();
    let compile_sources = build_compile_sources(&sources, files)?;

    Ok(CompileInput {
        language,
        compiler_version: tolk_compile_params.compiler_version,
        import_mappings: tolk_compile_params.import_mappings,
        entrypoint,
        compile_sources,
        sources,
        source_bundle_hash,
        storage_files,
    })
}

fn validate_sources(sources: &[SourceMetadata]) -> Result<String, ApiError> {
    if sources.is_empty() {
        return Err(ApiError::bad_request(
            "sources must contain at least one source".to_owned(),
        ));
    }

    let mut seen_paths = BTreeSet::new();
    let mut entrypoint = None;

    for source in sources {
        validate_source_path(&source.path)?;
        if !seen_paths.insert(source.path.clone()) {
            return Err(ApiError::bad_request(format!(
                "duplicate source path: {}",
                source.path
            )));
        }
        if source.is_entrypoint {
            if entrypoint.is_some() {
                return Err(ApiError::bad_request(
                    "multiple entrypoint sources were provided".to_owned(),
                ));
            }
            entrypoint = Some(source.path.clone());
        }
    }

    entrypoint.ok_or_else(|| ApiError::bad_request("missing entrypoint source".to_owned()))
}

fn validate_source_path(path: &str) -> Result<(), ApiError> {
    if path.trim().is_empty() {
        return Err(ApiError::bad_request("source path is empty".to_owned()));
    }
    if path.contains('\\') {
        return Err(ApiError::bad_request(
            "source path must use '/' separators".to_owned(),
        ));
    }

    let path = Path::new(path);
    if path.is_absolute() {
        return Err(ApiError::bad_request(
            "source path must be relative".to_owned(),
        ));
    }

    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(ApiError::bad_request(
                    "source path contains an invalid component".to_owned(),
                ));
            }
        }
    }

    Ok(())
}

fn validate_import_mappings(import_mappings: &BTreeMap<String, String>) -> Result<(), ApiError> {
    for (prefix, target) in import_mappings {
        validate_import_mapping_component("import mapping prefix", prefix)?;
        validate_import_mapping_component("import mapping target", target)?;
    }

    Ok(())
}

fn validate_import_mapping_component(name: &str, value: &str) -> Result<(), ApiError> {
    if value.trim().is_empty() {
        return Err(ApiError::bad_request(format!("{name} is empty")));
    }
    if value.contains('\\') {
        return Err(ApiError::bad_request(format!(
            "{name} must use '/' separators"
        )));
    }

    let path = Path::new(value);
    if path.is_absolute() {
        return Err(ApiError::bad_request(format!("{name} must be relative")));
    }

    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(ApiError::bad_request(format!(
                    "{name} contains an invalid component"
                )));
            }
        }
    }

    Ok(())
}

fn match_files_to_sources(
    sources: &[SourceMetadata],
    files: Vec<ReceivedFile>,
) -> Result<BTreeMap<String, ReceivedFile>, ApiError> {
    let mut files_by_path = BTreeMap::new();
    for file in files {
        let file_name = file
            .file_name
            .clone()
            .ok_or_else(|| ApiError::bad_request("file part is missing filename".to_owned()))?;
        validate_source_path(&file_name)?;
        if files_by_path.insert(file_name.clone(), file).is_some() {
            return Err(ApiError::bad_request(format!(
                "duplicate uploaded file path: {file_name}"
            )));
        }
    }

    for source in sources {
        if !files_by_path.contains_key(&source.path) {
            return Err(ApiError::bad_request(format!(
                "source metadata has no uploaded file: {}",
                source.path
            )));
        }
    }

    for file_path in files_by_path.keys() {
        if !sources.iter().any(|source| source.path == *file_path) {
            return Err(ApiError::bad_request(format!(
                "uploaded file has no source metadata: {file_path}"
            )));
        }
    }

    Ok(files_by_path)
}

fn build_compile_sources(
    sources: &[SourceMetadata],
    mut files: BTreeMap<String, ReceivedFile>,
) -> Result<Vec<CompileSource>, ApiError> {
    let mut compile_sources = Vec::with_capacity(sources.len());
    for source in sources {
        let file = files.remove(&source.path).ok_or_else(|| {
            ApiError::bad_request(format!(
                "source metadata has no uploaded file: {}",
                source.path
            ))
        })?;
        let content = String::from_utf8(file.content.to_vec()).map_err(|err| {
            ApiError::bad_request(format!("source is not valid UTF-8: {}: {err}", source.path))
        })?;
        compile_sources.push(CompileSource {
            path: source.path.clone(),
            content,
        });
    }

    Ok(compile_sources)
}

fn print_verify_request(
    target: &ResolvedVerificationTarget,
    language: &str,
    compile_params: &Value,
    sources: &[SourceMetadata],
    compiled_code_hash: &str,
    source_bundle_hash: Option<&str>,
    verification_result: VerificationResult,
) {
    println!("verification request");
    println!("address: {}", target.address.as_deref().unwrap_or("<none>"));
    println!("code_hash: {}", target.code_hash);
    println!("compiled_code_hash: {compiled_code_hash}");
    println!(
        "source_bundle_hash: {}",
        source_bundle_hash.unwrap_or("<none>")
    );
    println!("verification_result: {verification_result}");
    println!("language: {language}");
    println!("compile_params: {compile_params}");

    for source in sources {
        println!(
            "source: path={} is_entrypoint={}",
            source.path, source.is_entrypoint
        );
    }
}

struct CompileInput {
    language: String,
    compiler_version: String,
    import_mappings: BTreeMap<String, String>,
    entrypoint: String,
    compile_sources: Vec<CompileSource>,
    sources: Vec<SourceMetadata>,
    source_bundle_hash: String,
    storage_files: Vec<SourceStorageFile>,
}

#[derive(Debug)]
struct ReceivedFile {
    file_name: Option<String>,
    content: Bytes,
}

#[derive(Clone, Debug, Deserialize)]
struct SourceMetadata {
    path: String,
    is_entrypoint: bool,
}

#[derive(Debug, Deserialize)]
struct TolkCompileParams {
    compiler_version: String,
    #[serde(default)]
    import_mappings: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct VerifyResponse {
    address: Option<String>,
    code_hash: String,
    compiled_code_hash: String,
    verification_result: VerificationResult,
    source_bundle_hash: Option<String>,
    source_storage: Option<SourceStorageResponse>,
    onchain_registration: Option<OnchainRegistration>,
    language: String,
    compile_params: Value,
    files: Vec<FileSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SourceStorageResponse {
    provider: SourceStorageProvider,
    commit: String,
    bundle_path: String,
}

impl From<SourceStorageReceipt> for SourceStorageResponse {
    fn from(receipt: SourceStorageReceipt) -> Self {
        Self {
            provider: receipt.provider,
            commit: receipt.commit,
            bundle_path: receipt.bundle_path,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct OnchainRegistration {
    status: OnchainRegistrationStatus,
    master_address: String,
    verification_record_address: String,
}

impl From<RegistrationReceipt> for OnchainRegistration {
    fn from(receipt: RegistrationReceipt) -> Self {
        Self {
            status: OnchainRegistrationStatus::Confirmed,
            master_address: receipt.master_address,
            verification_record_address: receipt.verification_record_address,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OnchainRegistrationStatus {
    Confirmed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VerificationResult {
    Match,
    Mismatch,
}

impl VerificationResult {
    fn from_hashes(target: &str, compiled: &str) -> Self {
        if target == compiled {
            Self::Match
        } else {
            Self::Mismatch
        }
    }
}

impl std::fmt::Display for VerificationResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Match => formatter.write_str("match"),
            Self::Mismatch => formatter.write_str("mismatch"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FileSummary {
    path: String,
    is_entrypoint: bool,
}

impl FileSummary {
    fn from_source_metadata(source: SourceMetadata) -> Self {
        Self {
            path: source.path,
            is_entrypoint: source.is_entrypoint,
        }
    }
}
