use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::{
    error::ApiError,
    registry::{VerificationStatusReceipt, VerificationStatusRequest, VerifiedBundlesRequest},
    source_storage::{CompilerMetadata, StoredSourceBundle, StoredSourceFile},
    state::AppState,
    verification::VerificationTarget,
};

#[utoipa::path(
    get,
    path = "/api/v1/verification/status",
    params(
        ("address" = Option<String>, Query, description = "TON address to resolve to the current code hash"),
        ("code_hash" = Option<String>, Query, description = "Code hash to check directly")
    ),
    responses(
        (status = 200, description = "Verification status for the resolved code hash", body = VerificationStatusResponse),
        (status = 400, description = "Invalid or missing verification target", body = crate::error::ErrorResponse),
        (status = 502, description = "Blockchain or registry lookup failure", body = crate::error::ErrorResponse)
    ),
    tag = "verification"
)]
pub async fn status_handler(
    State(state): State<AppState>,
    Query(query): Query<VerificationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let resolved_target = state
        .verification_service()
        .resolve_target(query.into_target())
        .await?;
    let status = state
        .verification_registry()
        .status(VerificationStatusRequest {
            code_hash: resolved_target.code_hash.clone(),
        })
        .await?;

    Ok(Json(VerificationStatusResponse::new(
        resolved_target.address,
        resolved_target.code_hash,
        &status,
    )))
}

#[utoipa::path(
    get,
    path = "/api/v1/verification/source",
    params(
        ("address" = Option<String>, Query, description = "TON address to resolve to the current code hash"),
        ("code_hash" = Option<String>, Query, description = "Code hash to load verified source bundles for")
    ),
    responses(
        (status = 200, description = "Verified source bundles for the resolved code hash", body = VerificationSourceResponse),
        (status = 400, description = "Invalid or missing verification target", body = crate::error::ErrorResponse),
        (status = 502, description = "Blockchain, registry, or source lookup failure", body = crate::error::ErrorResponse)
    ),
    tag = "verification"
)]
pub async fn source_handler(
    State(state): State<AppState>,
    Query(query): Query<VerificationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let resolved_target = state
        .verification_service()
        .resolve_target(query.into_target())
        .await?;
    let receipt = state
        .verification_registry()
        .verified_bundles(VerifiedBundlesRequest {
            code_hash: resolved_target.code_hash.clone(),
        })
        .await?;
    let verified = !receipt.bundles.is_empty();
    let bundles = receipt
        .bundles
        .into_iter()
        .map(SourceBundleResponse::from)
        .collect();

    Ok(Json(VerificationSourceResponse {
        code_hash: resolved_target.code_hash,
        verified,
        bundles,
    }))
}

#[derive(Debug, Deserialize)]
pub(super) struct VerificationQuery {
    address: Option<String>,
    code_hash: Option<String>,
}

impl VerificationQuery {
    fn into_target(self) -> VerificationTarget {
        VerificationTarget {
            address: non_empty_text(self.address),
            code_hash: non_empty_text(self.code_hash),
        }
    }
}

fn non_empty_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct VerificationStatusResponse {
    address: Option<String>,
    code_hash: String,
    verified: bool,
    bundle_count: usize,
}

impl VerificationStatusResponse {
    const fn new(
        address: Option<String>,
        code_hash: String,
        status: &VerificationStatusReceipt,
    ) -> Self {
        Self {
            address,
            code_hash,
            verified: status.verified,
            bundle_count: status.bundle_count,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct VerificationSourceResponse {
    code_hash: String,
    verified: bool,
    bundles: Vec<SourceBundleResponse>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct SourceBundleResponse {
    source_bundle_hash: String,
    verified_at: u64,
    storage_revision: String,
    compiler: CompilerResponse,
    files: Vec<SourceFileResponse>,
}

impl From<StoredSourceBundle> for SourceBundleResponse {
    fn from(bundle: StoredSourceBundle) -> Self {
        let manifest = bundle.manifest;
        Self {
            source_bundle_hash: manifest.source_bundle_hash,
            verified_at: manifest.verified_at,
            storage_revision: bundle.storage_revision,
            compiler: CompilerResponse::from(manifest.compiler),
            files: bundle
                .files
                .into_iter()
                .map(SourceFileResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct CompilerResponse {
    language: String,
    version: String,
    entrypoint: String,
    #[schema(value_type = Object)]
    params: Value,
}

impl From<CompilerMetadata> for CompilerResponse {
    fn from(compiler: CompilerMetadata) -> Self {
        Self {
            language: compiler.language,
            version: compiler.version,
            entrypoint: compiler.entrypoint,
            params: compiler.params,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct SourceFileResponse {
    path: String,
    content_hash: String,
    include_in_command: Option<bool>,
    is_stdlib: Option<bool>,
    has_include_directives: Option<bool>,
    content: String,
}

impl From<StoredSourceFile> for SourceFileResponse {
    fn from(file: StoredSourceFile) -> Self {
        Self {
            path: file.path,
            content_hash: file.content_hash,
            include_in_command: file.include_in_command,
            is_stdlib: file.is_stdlib,
            has_include_directives: file.has_include_directives,
            content: file.content,
        }
    }
}
