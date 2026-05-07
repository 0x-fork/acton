use axum::{
    Router, middleware,
    routing::{get, post},
};
use faucet_backend::middlewares::require_acton_user_agent;

use crate::AppState;

mod challenge;
mod claim;
mod health;
mod robots;

pub(crate) use claim::CreateClaim;

pub(crate) fn router() -> Router<AppState> {
    let airdrop_routes = Router::new()
        .route("/challenge", get(challenge::get_challenge))
        .route("/claim", post(claim::create_claim))
        .route_layer(middleware::from_fn(require_acton_user_agent));

    Router::new()
        .route("/", get(health::root))
        .route("/robots.txt", get(robots::robots_txt))
        .route("/ready", get(health::ok))
        .route("/health", get(health::ok))
        .route("/metrics", get(health::ok))
        .route("/version", get(health::version))
        .merge(airdrop_routes)
}
