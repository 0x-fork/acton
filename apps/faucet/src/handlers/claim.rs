use apalis::prelude::TaskSink;
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use tonlib_core::TonAddress;

use crate::AppState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct CreateClaim {
    pub(crate) address: String,
    pub(crate) challenge: String,
    pub(crate) nonce: u64,
}

//noinspection RsLiveness
#[axum::debug_handler]
pub(super) async fn create_claim(
    State(mut state): State<AppState>,
    Json(payload): Json<CreateClaim>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    if TonAddress::from_str(&payload.address).is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid TON address" })),
        ));
    }

    if state.pow_cache.get(&payload.challenge).is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid or expired challenge" })),
        ));
    }

    if !verify_pow(
        &payload.challenge,
        payload.nonce,
        state.config.pow.difficulty,
    ) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid PoW solution" })),
        ));
    }

    // Atomically consume challenge to prevent reuse in concurrent requests.
    if state.pow_cache.remove(&payload.challenge).is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid or expired challenge" })),
        ));
    }

    state.storage.push(payload).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Failed to queue claim" })),
        )
    })?;

    Ok((
        StatusCode::OK,
        Json(json!({ "message": "Your claim has been queued. It will be processed soon." })),
    ))
}

fn verify_pow(challenge: &str, nonce: u64, difficulty: u32) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(challenge.as_bytes());
    hasher.update(nonce.to_be_bytes());
    let result = hasher.finalize();

    let mut zero_bits = 0;
    for &byte in result.iter() {
        let leading_zeros = byte.leading_zeros();
        zero_bits += leading_zeros;
        if leading_zeros < 8 {
            break;
        }
    }
    zero_bits >= difficulty
}
