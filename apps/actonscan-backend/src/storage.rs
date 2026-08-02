use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;
use ton_indexer_core::{BlockId, CheckpointStore, Error as IndexerError};

use crate::stats::{SAMPLE_RETENTION_SECONDS, TpsSample, TpsStats};

const SCHEMA_VERSION: i64 = 1;

/// `SQLite` storage for the indexer checkpoint and rolling TPS samples.
#[derive(Clone)]
pub struct SqliteStorage {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteStorage {
    /// Opens the database and creates its schema.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory or database cannot be initialized.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| StorageError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        initialize_schema(&connection)?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// Loads the persisted TPS samples into a new in-memory accumulator.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be read.
    pub fn load_tps_stats(&self) -> Result<TpsStats, StorageError> {
        Ok(TpsStats::from_samples(self.load_tps_samples()?))
    }

    pub(crate) fn record_tps_sample(&self, sample: TpsSample) -> Result<(), StorageError> {
        let cutoff = sample.timestamp.saturating_sub(SAMPLE_RETENTION_SECONDS);

        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "insert into tps_samples (masterchain_seqno, block_time, transactions)
             values (?1, ?2, ?3)
             on conflict (masterchain_seqno) do update set
               block_time = excluded.block_time,
               transactions = excluded.transactions",
            params![
                sample.masterchain_seqno,
                sample.timestamp,
                sample.transactions
            ],
        )?;
        transaction.execute(
            "delete from tps_samples where block_time < ?1",
            params![cutoff],
        )?;
        transaction.commit()?;
        drop(connection);
        Ok(())
    }

    fn load_tps_samples(&self) -> Result<Vec<TpsSample>, StorageError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "select masterchain_seqno, block_time, transactions
             from tps_samples
             order by masterchain_seqno asc",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(TpsSample {
                masterchain_seqno: row.get(0)?,
                timestamp: row.get(1)?,
                transactions: row.get(2)?,
            })
        })?;

        let mut samples = Vec::new();
        for row in rows {
            samples.push(row?);
        }
        drop(statement);
        drop(connection);
        Ok(samples)
    }

    fn load_checkpoint(&self) -> Result<Option<BlockId>, StorageError> {
        let connection = self.connection()?;
        let value = connection
            .query_row(
                "select value from indexer_checkpoint where id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        drop(connection);

        value
            .map(|value| serde_json::from_str(&value).map_err(StorageError::Json))
            .transpose()
    }

    fn save_checkpoint(&self, checkpoint: &BlockId) -> Result<(), StorageError> {
        let value = serde_json::to_string(checkpoint)?;
        let connection = self.connection()?;
        connection.execute(
            "insert into indexer_checkpoint (id, value) values (1, ?1)
             on conflict (id) do update set
               value = excluded.value",
            params![value],
        )?;
        drop(connection);
        Ok(())
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        self.connection.lock().map_err(|_| StorageError::Lock)
    }
}

#[async_trait]
impl CheckpointStore for SqliteStorage {
    async fn load(&self) -> ton_indexer_core::Result<Option<BlockId>> {
        self.load_checkpoint().map_err(IndexerError::checkpoint)
    }

    async fn save(&self, checkpoint: &BlockId) -> ton_indexer_core::Result<()> {
        self.save_checkpoint(checkpoint)
            .map_err(IndexerError::checkpoint)
    }
}

fn initialize_schema(connection: &Connection) -> Result<(), StorageError> {
    let version = connection.query_row("pragma user_version", [], |row| row.get::<_, i64>(0))?;
    match version {
        0 => connection.execute_batch(
            "create table if not exists indexer_checkpoint (
               id integer primary key check (id = 1),
               value text not null
             );
             create table if not exists tps_samples (
               masterchain_seqno integer primary key,
               block_time integer not null,
               transactions integer not null
             );
             pragma user_version = 1;",
        )?,
        SCHEMA_VERSION => {}
        version => return Err(StorageError::UnsupportedSchemaVersion(version)),
    }
    Ok(())
}

/// Errors produced by the Actonscan `SQLite` storage.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("failed to create Actonscan database directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Actonscan database mutex is poisoned")]
    Lock,
    #[error("unsupported Actonscan database schema version {0}")]
    UnsupportedSchemaVersion(i64),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use ton_indexer_core::Hash256;

    #[tokio::test]
    async fn database_reopen_restores_checkpoint_and_tps_samples() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("actonscan.sqlite3");
        let checkpoint = BlockId {
            workchain: BlockId::MASTERCHAIN_WORKCHAIN,
            shard: BlockId::FULL_SHARD,
            seqno: 42,
            root_hash: Hash256::new([1; 32]),
            file_hash: Hash256::new([2; 32]),
        };
        let sample = TpsSample {
            masterchain_seqno: 42,
            timestamp: 1_000,
            transactions: 120,
        };

        let storage = SqliteStorage::open(&path).unwrap();
        storage.record_tps_sample(sample).unwrap();
        storage.record_tps_sample(sample).unwrap();
        storage.save(&checkpoint).await.unwrap();
        drop(storage);

        let storage = SqliteStorage::open(path).unwrap();
        assert_eq!(storage.load().await.unwrap(), Some(checkpoint));
        assert_eq!(storage.load_tps_samples().unwrap(), vec![sample]);
    }

    #[test]
    fn database_prunes_expired_tps_samples() {
        let directory = tempfile::tempdir().unwrap();
        let storage = SqliteStorage::open(directory.path().join("actonscan.sqlite3")).unwrap();
        let old = TpsSample {
            masterchain_seqno: 1,
            timestamp: 1_000,
            transactions: 10,
        };
        let current = TpsSample {
            masterchain_seqno: 2,
            timestamp: 1_000 + SAMPLE_RETENTION_SECONDS + 1,
            transactions: 20,
        };

        storage.record_tps_sample(old).unwrap();
        storage.record_tps_sample(current).unwrap();

        assert_eq!(storage.load_tps_samples().unwrap(), vec![current]);
    }
}
