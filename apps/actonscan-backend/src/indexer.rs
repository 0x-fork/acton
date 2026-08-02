use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use ton_indexer_core::{Batch, IndexPipeline, RunOutcome, Sink};
use ton_indexer_liteserver::{CanonicalBlockSource, TonutilsLiteClient};

use crate::{SqliteStorage, config::IndexerConfig, stats::TpsStats};

const RECONNECT_DELAY: Duration = Duration::from_secs(2);

pub(crate) fn spawn(
    config: IndexerConfig,
    stats: TpsStats,
    storage: SqliteStorage,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(run(config, stats, storage))
}

async fn run(config: IndexerConfig, stats: TpsStats, storage: SqliteStorage) {
    loop {
        if let Err(error) = run_connection(&config, &stats, &storage).await {
            tracing::error!(%error, "Actonscan indexer disconnected");
            tokio::time::sleep(RECONNECT_DELAY).await;
        }
    }
}

async fn run_connection(
    config: &IndexerConfig,
    stats: &TpsStats,
    storage: &SqliteStorage,
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
        "connected Actonscan indexer to LiteServer"
    );

    let source = CanonicalBlockSource::new(client, start_seqno);
    let sink = TpsSink {
        stats: stats.clone(),
        storage: storage.clone(),
    };
    let mut pipeline = IndexPipeline::new(source, sink, storage.clone());
    loop {
        match pipeline.run_once().await? {
            RunOutcome::Idle => tokio::time::sleep(config.poll_interval).await,
            RunOutcome::Committed(checkpoint) => {
                tracing::debug!(seqno = checkpoint.seqno, "indexed Actonscan batch");
            }
        }
    }
}

struct TpsSink {
    stats: TpsStats,
    storage: SqliteStorage,
}

#[async_trait]
impl Sink for TpsSink {
    async fn commit(&mut self, batch: &Batch) -> ton_indexer_core::Result<()> {
        let sample = TpsStats::sample_from_batch(batch);
        self.storage
            .record_tps_sample(sample)
            .map_err(ton_indexer_core::Error::sink)?;
        self.stats.record_sample(sample).await;
        Ok(())
    }
}
