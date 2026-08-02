//! Actonscan backend HTTP API and network indexer.

pub mod config;
mod indexer;
pub mod stats;
mod storage;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, Method, StatusCode, header::CACHE_CONTROL},
    response::IntoResponse,
    routing::get,
};
use tower_http::{compression::CompressionLayer, cors::CorsLayer};
use utoipa::OpenApi;

use crate::{
    config::IndexerConfig,
    stats::{TpsSnapshot, TpsStatus, TpsWindow},
};

pub use config::Config;
pub use stats::TpsStats;
pub use storage::SqliteStorage;

#[derive(OpenApi)]
#[openapi(paths(tps), components(schemas(TpsSnapshot, TpsStatus, TpsWindow)))]
struct ApiDoc;

/// Builds the public Actonscan backend router.
pub fn app(stats: TpsStats) -> Router {
    let api = Router::new().route("/stats/tps", get(tps));
    Router::new()
        .route("/healthz", get(health))
        .route("/openapi.json", get(openapi))
        .nest("/api/v1", api)
        .with_state(stats)
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods([Method::GET, Method::OPTIONS])
                .allow_headers(tower_http::cors::Any),
        )
        .layer(CompressionLayer::new())
}

/// Starts the LiteServer-backed statistics indexer in the current Tokio runtime.
#[must_use]
pub fn spawn_indexer(
    config: IndexerConfig,
    stats: TpsStats,
    storage: SqliteStorage,
) -> tokio::task::JoinHandle<()> {
    indexer::spawn(config, stats, storage)
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn openapi() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[utoipa::path(
    get,
    path = "/api/v1/stats/tps",
    responses((status = 200, description = "Rolling network TPS", body = TpsSnapshot))
)]
async fn tps(State(stats): State<TpsStats>) -> impl IntoResponse {
    (
        [(
            CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=1, stale-while-revalidate=4"),
        )],
        Json(stats.snapshot().await),
    )
}
