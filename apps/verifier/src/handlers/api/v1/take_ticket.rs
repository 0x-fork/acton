use axum::{Json, extract::State, http::HeaderMap, response::IntoResponse};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    client_compatibility::is_legacy_acton_client, config::TonNetwork, error::ApiError,
    payment::PaymentError, registry::VerifiedBundleRequest, state::AppState,
};

use super::validation;

#[utoipa::path(
    post,
    path = "/api/v1/take_ticket",
    operation_id = "take_ticket",
    request_body = TakeTicketRequest,
    responses(
        (status = 200, description = "Verification status or payment quote", body = TakeTicketResponse),
        (status = 400, description = "Invalid code hash, missing compiler metadata, missing or invalid User-Agent, or unsupported Blueprint version", body = crate::error::ErrorResponse),
        (status = 403, description = "Compiler disabled by server configuration", body = crate::error::ErrorResponse),
        (status = 502, description = "Verification registry failure", body = crate::error::ErrorResponse),
        (status = 503, description = "Verifier is read-only or payment history recovery is in progress", body = crate::error::ErrorResponse)
    ),
    params(
        ("User-Agent" = String, Header, description = "Required non-empty client identifier. Only Acton at or below 1.2.0 may omit compiler metadata and is exempt from compiler restrictions. All other clients must provide compiler and compiler_version, including for already verified code hashes. Blueprint versions below 0.47.1 are rejected")
    ),
    tag = "verification"
)]
pub async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TakeTicketRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let code_hash = validation::code_hash(&request.code_hash)?;
    if request.compiler.is_some() != request.compiler_version.is_some() {
        return Err(ApiError::bad_request(
            "compiler and compiler_version must be provided together".to_owned(),
        ));
    }
    let compiler_metadata = if is_legacy_acton_client(&headers) {
        None
    } else {
        let (Some(compiler), Some(version)) = (
            request.compiler.as_deref(),
            request.compiler_version.as_deref(),
        ) else {
            return Err(ApiError::bad_request(
                "compiler and compiler_version are required".to_owned(),
            ));
        };
        Some((compiler, version))
    };

    if let Some(bundle) = state
        .verification_registry()
        .verified_bundle(VerifiedBundleRequest {
            code_hash: code_hash.clone(),
        })
        .await?
        .bundle
    {
        return Ok(Json(TakeTicketResponse::AlreadyVerified {
            code_hash,
            source_bundle_hash: bundle.manifest.source_bundle_hash,
            storage_revision: bundle.storage_revision,
        }));
    }

    if state.read_only() {
        return Err(ApiError::read_only());
    }

    if let Some((compiler, version)) = compiler_metadata {
        state.ensure_compiler_allowed(compiler, version)?;
    }

    if !state.payment_verifier().is_ready() {
        return Err(PaymentError::RecoveryInProgress.into());
    }

    let quote = state.payment_verifier().quote(&code_hash);
    Ok(Json(TakeTicketResponse::PaymentRequired {
        code_hash,
        network: quote.network,
        payment_address: quote.payment_address,
        amount_nano: quote.amount_nano,
        comment: quote.comment,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub(super) struct TakeTicketRequest {
    #[schema(example = "a873d8c2d163f7fa10bbe38769706f0554505e8ea2dcea3f115288db8becf2ab")]
    code_hash: String,
    /// Compiler name, provided together with `compiler_version`.
    /// Required, including for already verified code hashes, except for Acton at or below 1.2.0
    /// (identified by User-Agent).
    /// These Acton clients are also exempt from the server's compiler deny list.
    #[schema(example = "tolk")]
    compiler: Option<String>,
    /// Exact compiler version. Has the same compatibility exceptions as `compiler`.
    #[schema(example = "1.4.2")]
    compiler_version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum TakeTicketResponse {
    AlreadyVerified {
        code_hash: String,
        source_bundle_hash: String,
        storage_revision: String,
    },
    PaymentRequired {
        code_hash: String,
        network: TonNetwork,
        payment_address: String,
        amount_nano: String,
        comment: String,
    },
}
