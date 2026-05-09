use axum::{Json, extract::State};
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub(super) struct ChallengeResponse {
    challenge: String,
    difficulty: u32,
}

pub(super) async fn get_challenge(State(state): State<AppState>) -> Json<ChallengeResponse> {
    let challenge = state.pow.create();

    state.pow_challenges.insert(challenge.clone(), ());

    Json(ChallengeResponse {
        challenge,
        difficulty: state.pow.difficulty(),
    })
}
