use crate::AppState;
use apalis::prelude::TaskSink;
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use ton::ton_core::types::TonAddress;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct CreateClaim {
    pub(crate) address: String,
    pub(crate) challenge: String,
    pub(crate) nonce: u64,
}

#[derive(Deserialize)]
pub(super) struct CreateClaimRequest {
    address: String,
    challenge: String,
    nonce: u64,
    version: u32,
}

#[derive(Serialize)]
pub(super) struct ClaimResponse {
    message: &'static str,
}

#[derive(Serialize)]
pub(super) struct ErrorResponse {
    error: &'static str,
}

type ClaimResult = Result<(StatusCode, Json<ClaimResponse>), (StatusCode, Json<ErrorResponse>)>;

//noinspection RsLiveness
#[axum::debug_handler]
pub(super) async fn create_claim(
    State(mut state): State<AppState>,
    Json(payload): Json<CreateClaimRequest>,
) -> ClaimResult {
    if TonAddress::from_str(&payload.address).is_err() {
        return Err(bad_request("Invalid TON address"));
    }

    if !state.pow.can_process_version(payload.version) {
        return Err(bad_request("Unsupported challenge version"));
    }

    let challenge_version = state
        .pow_challenges
        .get(&payload.challenge)
        .ok_or_else(|| bad_request("Invalid or expired challenge"))?;

    if challenge_version != payload.version {
        return Err(bad_request("Invalid challenge version"));
    }

    if !state.pow.verify(&payload.challenge, payload.nonce) {
        return Err(bad_request("Invalid PoW solution"));
    }

    if state.pow_challenges.remove(&payload.challenge).is_none() {
        return Err(bad_request("Invalid or expired challenge"));
    }

    state
        .storage
        .push(CreateClaim {
            address: payload.address,
            challenge: payload.challenge,
            nonce: payload.nonce,
        })
        .await
        .map_err(|_| response_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to queue claim"))?;

    Ok((
        StatusCode::OK,
        Json(ClaimResponse {
            message: "Your claim has been queued. It will be processed soon.",
        }),
    ))
}

fn bad_request(error: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    response_error(StatusCode::BAD_REQUEST, error)
}

fn response_error(status: StatusCode, error: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::CreateClaimRequest;

    #[test]
    fn deserializes_challenge_version() {
        let request: CreateClaimRequest = serde_json::from_value(json!({
            "address": "UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJKZ",
            "challenge": "challenge",
            "nonce": 42,
            "version": 1,
        }))
        .unwrap();

        assert_eq!(
            request.address,
            "UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJKZ"
        );
        assert_eq!(request.challenge, "challenge");
        assert_eq!(request.nonce, 42);
        assert_eq!(request.version, 1);
    }
}
