use axum::{Router, routing::post};

use crate::state::AppState;

mod verify;

pub fn router() -> Router<AppState> {
    Router::new().route("/verify", post(verify::handler))
}
