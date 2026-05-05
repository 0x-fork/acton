use axum::{
    Router,
    routing::{get, post},
};

use crate::AppState;

mod challenge;
mod claim;
mod health;
mod robots;

pub(crate) use claim::CreateClaim;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(health::root))
        .route("/robots.txt", get(robots::robots_txt))
        .route("/ready", get(health::ok))
        .route("/health", get(health::ok))
        .route("/metrics", get(health::ok))
        .route("/version", get(health::version))
        .route("/challenge", get(challenge::get_challenge))
        .route("/claim", post(claim::create_claim))
}
