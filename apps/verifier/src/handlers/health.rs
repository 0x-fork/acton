use axum::{Json, response::IntoResponse};
use serde::Serialize;

pub async fn handler() -> impl IntoResponse {
    Json(HealthResponse { ok: true })
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
}
