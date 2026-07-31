use actonscan_backend::{Config, TpsStats, app, spawn_indexer};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(config.logging_level()))
        .init();

    let stats = TpsStats::default();
    let _indexer = spawn_indexer(config.indexer().clone(), stats.clone());
    let listener = tokio::net::TcpListener::bind(config.bind_addr()).await?;
    tracing::info!(address = %config.bind_addr(), "starting Actonscan backend");
    axum::serve(listener, app(stats)).await?;
    Ok(())
}
