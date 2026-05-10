use axum::{Json, extract::State};
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub(super) struct ChallengeResponse {
    version: u32,
    challenge: String,
    difficulty: u32,
}

pub(super) async fn get_challenge(State(state): State<AppState>) -> Json<ChallengeResponse> {
    let challenge = state.pow.create();
    let version = state.pow.version();

    state.pow_challenges.insert(challenge.clone(), version);

    Json(ChallengeResponse {
        version,
        challenge,
        difficulty: state.pow.difficulty(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::ChallengeResponse;

    #[test]
    fn serializes_current_challenge_version() {
        let response = ChallengeResponse {
            version: 1,
            challenge: "challenge".to_string(),
            difficulty: 21,
        };

        assert_eq!(
            serde_json::to_value(response).unwrap(),
            json!({
                "version": 1,
                "challenge": "challenge",
                "difficulty": 21,
            })
        );
    }
}
