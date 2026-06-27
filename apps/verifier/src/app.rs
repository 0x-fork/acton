use axum::{Router, routing::get};

use crate::{
    config::Config,
    handlers,
    state::{AppState, StateError},
};

/// Builds a router with default configuration.
///
/// # Errors
///
/// Returns an error when application state cannot be initialized.
pub fn router() -> Result<Router, StateError> {
    Ok(router_with_state(
        AppState::from_config(&Config::default())?,
    ))
}

pub fn router_with_state(state: AppState) -> Router {
    Router::<AppState>::new()
        .route("/healthz", get(handlers::health::handler))
        .nest("/api/v1", handlers::api::v1::router())
        .fallback(handlers::frontend::handler)
        .with_state(state)
}
