use axum::{
    Router,
    routing::{get, post},
};

use crate::state::AppState;

mod verification;
mod verify;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/verify", post(verify::handler))
        .route("/verification/status", get(verification::status_handler))
        .route("/verification/source", get(verification::source_handler))
}
