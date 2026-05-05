use axum::{Json, extract::State};
use rand::RngCore;
use serde_json::{Value, json};

use crate::AppState;

pub(super) async fn get_challenge(State(state): State<AppState>) -> Json<Value> {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let challenge = hex::encode(bytes);

    state.pow_cache.insert(challenge.clone(), ());

    Json(json!({
        "challenge": challenge,
        "difficulty": state.config.pow.difficulty
    }))
}
