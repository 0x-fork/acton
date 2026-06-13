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
    compilers::{CompileRequest, CompileSource},
    error::ApiError,
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

    let compile_input = prepare_compile_input(language, compile_params.clone(), sources, files)?;
    let resolved_target = state.verification_service().resolve_target(target).await?;
    let compiled = state
        .compiler_service()
        .compile(CompileRequest {
            language: compile_input.language.clone(),
            compiler_version: compile_input.compiler_version,
            entrypoint: compile_input.entrypoint,
            sources: compile_input.compile_sources,
        })
        .await?;
    let compiled_code_hash = compiled.code_hash.to_ascii_lowercase();
    let verification_result =
        VerificationResult::from_hashes(&resolved_target.code_hash, &compiled_code_hash);

    print_verify_request(
        &resolved_target,
        &compile_input.language,
        &compile_params,
        &compile_input.sources,
        &compiled_code_hash,
        verification_result,
    );

    Ok(Json(VerifyResponse {
        address: resolved_target.address,
        code_hash: resolved_target.code_hash,
        compiled_code_hash,
        verification_result,
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
    compile_params: Value,
    sources: Option<Vec<SourceMetadata>>,
    files: Vec<ReceivedFile>,
) -> Result<CompileInput, ApiError> {
    if language != TOLK_LANGUAGE {
        return Err(ApiError::bad_request(format!(
            "unsupported language: {language}"
        )));
    }

    let compile_params = serde_json::from_value::<TolkCompileParams>(compile_params)
        .map_err(|err| ApiError::bad_request(format!("invalid Tolk compile_params: {err}")))?;
    if compile_params.compiler_version != SUPPORTED_TOLK_VERSION {
        return Err(ApiError::bad_request(format!(
            "unsupported Tolk compiler_version: {}",
            compile_params.compiler_version
        )));
    }
    let sources = sources
        .ok_or_else(|| ApiError::bad_request("missing required field: sources".to_owned()))?;
    let entrypoint = validate_sources(&sources)?;
    let files = match_files_to_sources(&sources, files)?;
    let compile_sources = build_compile_sources(&sources, files)?;

    Ok(CompileInput {
        language,
        compiler_version: compile_params.compiler_version,
        entrypoint,
        compile_sources,
        sources,
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
    verification_result: VerificationResult,
) {
    println!("verification request");
    println!("address: {}", target.address.as_deref().unwrap_or("<none>"));
    println!("code_hash: {}", target.code_hash);
    println!("compiled_code_hash: {compiled_code_hash}");
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
    entrypoint: String,
    compile_sources: Vec<CompileSource>,
    sources: Vec<SourceMetadata>,
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
}

#[derive(Debug, Serialize, Deserialize)]
struct VerifyResponse {
    address: Option<String>,
    code_hash: String,
    compiled_code_hash: String,
    verification_result: VerificationResult,
    language: String,
    compile_params: Value,
    files: Vec<FileSummary>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VerificationResult {
    Match,
    Mismatch,
}

impl VerificationResult {
    const fn from_hashes(target: &str, compiled: &str) -> Self {
        if target.eq_ignore_ascii_case(compiled) {
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
