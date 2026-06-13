use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    error::ApiError,
    registry::{SourceBundleStatusRequest, VerificationStatusReceipt, VerificationStatusRequest},
    source_storage::{StoredSourceBundle, StoredSourceFile},
    state::AppState,
    verification::VerificationTarget,
};

pub async fn status_handler(
    State(state): State<AppState>,
    Query(query): Query<VerificationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let resolved_target = state
        .verification_service()
        .resolve_target(query.into_target())
        .await?;
    let status = state
        .registry_client()
        .verification_status(VerificationStatusRequest {
            code_hash: resolved_target.code_hash.clone(),
        })
        .await?;

    Ok(Json(VerificationStatusResponse::new(
        resolved_target.address,
        resolved_target.code_hash,
        status,
    )))
}

pub async fn source_handler(
    State(state): State<AppState>,
    Query(query): Query<VerificationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let resolved_target = state
        .verification_service()
        .resolve_target(query.into_target())
        .await?;
    let status = state
        .registry_client()
        .verification_status(VerificationStatusRequest {
            code_hash: resolved_target.code_hash.clone(),
        })
        .await?;
    let mut bundles = Vec::new();

    if status.verified {
        for bundle in state
            .source_storage()
            .list_bundles(&resolved_target.code_hash)
            .await?
        {
            let bundle_status = state
                .registry_client()
                .source_bundle_status(SourceBundleStatusRequest {
                    code_hash: resolved_target.code_hash.clone(),
                    source_bundle_hash: bundle.manifest.source_bundle_hash.clone(),
                })
                .await?;
            if bundle_status.verified {
                bundles.push(SourceBundleResponse::from(bundle));
            }
        }
    }

    Ok(Json(VerificationSourceResponse {
        address: resolved_target.address,
        code_hash: resolved_target.code_hash,
        verified: status.verified,
        onchain: OnchainVerification::from(status),
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

#[derive(Debug, Serialize, Deserialize)]
struct VerificationStatusResponse {
    address: Option<String>,
    code_hash: String,
    verified: bool,
    onchain: OnchainVerification,
}

impl VerificationStatusResponse {
    fn new(address: Option<String>, code_hash: String, status: VerificationStatusReceipt) -> Self {
        Self {
            address,
            code_hash,
            verified: status.verified,
            onchain: OnchainVerification::from(status),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct VerificationSourceResponse {
    address: Option<String>,
    code_hash: String,
    verified: bool,
    onchain: OnchainVerification,
    bundles: Vec<SourceBundleResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OnchainVerification {
    master_address: String,
    verification_record_address: String,
}

impl From<VerificationStatusReceipt> for OnchainVerification {
    fn from(status: VerificationStatusReceipt) -> Self {
        Self {
            master_address: status.master_address,
            verification_record_address: status.verification_record_address,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SourceBundleResponse {
    source_bundle_hash: String,
    commit: Option<String>,
    bundle_path: String,
    language: String,
    compiler_version: String,
    entrypoint: String,
    compile_params: Value,
    sources: Vec<SourceFileSummary>,
    files: Vec<SourceFileResponse>,
}

impl From<StoredSourceBundle> for SourceBundleResponse {
    fn from(bundle: StoredSourceBundle) -> Self {
        let manifest = bundle.manifest;
        Self {
            source_bundle_hash: manifest.source_bundle_hash,
            commit: bundle.commit,
            bundle_path: manifest.bundle_path,
            language: manifest.language,
            compiler_version: manifest.compiler_version,
            entrypoint: manifest.entrypoint,
            compile_params: manifest.compile_params,
            sources: manifest
                .sources
                .into_iter()
                .map(|source| SourceFileSummary {
                    path: source.path,
                    is_entrypoint: source.is_entrypoint,
                })
                .collect(),
            files: bundle
                .files
                .into_iter()
                .map(SourceFileResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SourceFileSummary {
    path: String,
    is_entrypoint: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct SourceFileResponse {
    path: String,
    sha256: String,
    content_base64: String,
    content_text: Option<String>,
}

impl From<StoredSourceFile> for SourceFileResponse {
    fn from(file: StoredSourceFile) -> Self {
        Self {
            path: file.path,
            sha256: file.sha256,
            content_base64: file.content_base64,
            content_text: file.content_text,
        }
    }
}
