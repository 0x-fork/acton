use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use ton_indexer_core::{Batch, IndexPipeline, MemoryCheckpointStore, RunOutcome, Sink};
use ton_indexer_liteserver::{CanonicalBlockSource, TonutilsLiteClient};

use crate::{config::IndexerConfig, stats::TpsStats};

const RECONNECT_DELAY: Duration = Duration::from_secs(2);

pub(crate) fn spawn(config: IndexerConfig, stats: TpsStats) -> tokio::task::JoinHandle<()> {
    tokio::spawn(run(config, stats))
}

async fn run(config: IndexerConfig, stats: TpsStats) {
    let checkpoints = MemoryCheckpointStore::default();
    loop {
        if let Err(error) = run_connection(&config, &stats, &checkpoints).await {
            tracing::error!(%error, "TPS indexer disconnected");
            tokio::time::sleep(RECONNECT_DELAY).await;
        }
    }
}

async fn run_connection(
    config: &IndexerConfig,
    stats: &TpsStats,
    checkpoints: &MemoryCheckpointStore,
) -> Result<()> {
    let mut client = TonutilsLiteClient::connect_path_with_parallelism(
        &config.global_config_path,
        config.parallelism,
    )
    .await?;
    let tip = client.latest().await?;
    stats.set_startup_tip(tip.seqno).await;
    let start_seqno = tip
        .seqno
        .saturating_sub(config.backfill_batches.saturating_sub(1));
    tracing::info!(
        tip_seqno = tip.seqno,
        start_seqno,
        parallelism = client.exact_block_parallelism(),
        "connected TPS indexer to LiteServer"
    );

    let source = CanonicalBlockSource::new(client, start_seqno);
    let sink = TpsSink(stats.clone());
    let mut pipeline = IndexPipeline::new(source, sink, checkpoints.clone());
    loop {
        match pipeline.run_once().await? {
            RunOutcome::Idle => tokio::time::sleep(config.poll_interval).await,
            RunOutcome::Committed(checkpoint) => {
                tracing::debug!(seqno = checkpoint.seqno, "indexed TPS batch");
            }
        }
    }
}

struct TpsSink(TpsStats);

#[async_trait]
impl Sink for TpsSink {
    async fn commit(&mut self, batch: &Batch) -> ton_indexer_core::Result<()> {
        self.0.record_batch(batch).await;
        Ok(())
    }
}
