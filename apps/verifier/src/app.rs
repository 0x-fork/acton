use axum::{Router, routing::get};

use crate::{config::Config, handlers, state::AppState};

pub fn router() -> Router {
    router_with_state(AppState::from_config(&Config::default()))
}

pub fn router_with_state(state: AppState) -> Router {
    Router::<AppState>::new()
        .route("/healthz", get(handlers::health::handler))
        .nest("/api/v1", handlers::api::v1::router())
        .with_state(state)
}
