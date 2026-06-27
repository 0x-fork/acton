use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use thiserror::Error;

use crate::{
    bundle_validation::{StoredBundleValidationError, validate_stored_bundle},
    source_storage::{
        CompilerMetadata, SourceBundleManifest, SourceStorage, SourceStorageError,
        StoredSourceBundle, StoredSourceFile,
    },
};

const INDEX_SCHEMA_VERSION: i64 = 5;
const UNKNOWN_REVISION: &str = "unknown";

#[async_trait]
pub trait VerificationIndex: Send + Sync + 'static {
    async fn ensure_current(
        &self,
        source_storage: &dyn SourceStorage,
    ) -> Result<(), VerificationIndexError>;

    async fn upsert_bundle(
        &self,
        bundle: &StoredSourceBundle,
        indexed_revision: Option<&str>,
    ) -> Result<(), VerificationIndexError>;

    async fn status(
        &self,
        code_hash: &str,
    ) -> Result<IndexedVerificationStatus, VerificationIndexError>;

    async fn bundles(
        &self,
        code_hash: &str,
    ) -> Result<Vec<StoredSourceBundle>, VerificationIndexError>;
}

pub type SharedVerificationIndex = Arc<dyn VerificationIndex>;

#[derive(Clone, Debug)]
pub struct IndexedVerificationStatus {
    pub verified: bool,
    pub bundle_count: usize,
}

pub struct SqliteVerificationIndex {
    connection: Mutex<Connection>,
}

impl SqliteVerificationIndex {
    /// Opens a `SQLite` verification index at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error when the parent directory cannot be created, the
    /// database cannot be opened, or schema initialization fails.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, VerificationIndexError> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| VerificationIndexError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        Self::from_connection(Connection::open(path)?)
    }

    /// Opens an in-memory `SQLite` verification index.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or initialized.
    pub fn in_memory() -> Result<Self, VerificationIndexError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, VerificationIndexError> {
        initialize_schema(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, VerificationIndexError> {
        self.connection
            .lock()
            .map_err(|_| VerificationIndexError::Lock)
    }

    fn indexed_revision(&self) -> Result<Option<String>, VerificationIndexError> {
        let connection = self.connection()?;
        connection
            .query_row(
                "select indexed_revision from registry_index_state where id = 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(VerificationIndexError::Sqlite)
    }

    fn replace_all(
        &self,
        indexed_revision: &str,
        bundles: &[StoredSourceBundle],
    ) -> Result<(), VerificationIndexError> {
        let indexed_at = now_unix_seconds()?;
        {
            let mut connection = self.connection()?;
            let transaction = connection.transaction()?;

            transaction.execute("delete from bundle_files", [])?;
            transaction.execute("delete from verified_bundles", [])?;
            transaction.execute("delete from registry_index_state", [])?;

            for bundle in bundles {
                insert_bundle(&transaction, bundle, indexed_at)?;
            }
            set_index_state(&transaction, indexed_revision, indexed_at)?;

            transaction.commit()?;
            drop(connection);
        }
        Ok(())
    }
}

#[async_trait]
impl VerificationIndex for SqliteVerificationIndex {
    async fn ensure_current(
        &self,
        source_storage: &dyn SourceStorage,
    ) -> Result<(), VerificationIndexError> {
        let indexed_revision = revision_or_unknown(source_storage.current_revision().await?);
        if self.indexed_revision()? == Some(indexed_revision.clone()) {
            return Ok(());
        }

        let mut bundles = Vec::new();
        for code_hash in source_storage.list_code_hashes().await? {
            for bundle in source_storage.list_bundles(&code_hash).await? {
                validate_stored_bundle(&bundle, &code_hash)?;
                bundles.push(bundle);
            }
        }

        self.replace_all(&indexed_revision, &bundles)
    }

    async fn upsert_bundle(
        &self,
        bundle: &StoredSourceBundle,
        indexed_revision: Option<&str>,
    ) -> Result<(), VerificationIndexError> {
        validate_stored_bundle(bundle, &bundle.manifest.code_hash)?;

        let indexed_at = now_unix_seconds()?;
        {
            let mut connection = self.connection()?;
            let transaction = connection.transaction()?;

            delete_bundle_rows(
                &transaction,
                &bundle.manifest.code_hash,
                &bundle.manifest.source_bundle_hash,
            )?;
            insert_bundle(&transaction, bundle, indexed_at)?;
            if let Some(indexed_revision) = indexed_revision {
                set_index_state(&transaction, indexed_revision, indexed_at)?;
            }

            transaction.commit()?;
            drop(connection);
        }
        Ok(())
    }

    async fn status(
        &self,
        code_hash: &str,
    ) -> Result<IndexedVerificationStatus, VerificationIndexError> {
        let bundle_count = {
            let connection = self.connection()?;
            connection.query_row(
                "select count(*) from verified_bundles where code_hash = ?1",
                params![code_hash],
                |row| row.get::<_, i64>(0),
            )?
        };

        Ok(IndexedVerificationStatus {
            verified: bundle_count > 0,
            bundle_count: usize::try_from(bundle_count).map_err(|_| {
                VerificationIndexError::InvalidInteger {
                    field: "bundle_count",
                    value: bundle_count,
                }
            })?,
        })
    }

    async fn bundles(
        &self,
        code_hash: &str,
    ) -> Result<Vec<StoredSourceBundle>, VerificationIndexError> {
        {
            let connection = self.connection()?;
            let mut statement = connection.prepare(
                r"
                select
                  source_bundle_hash,
                  verified_at,
                  compiler_json,
                  storage_revision
                from verified_bundles
                where code_hash = ?1
                order by source_bundle_hash
                ",
            )?;
            let rows = statement.query_map(params![code_hash], |row| {
                let source_bundle_hash: String = row.get(0)?;
                Ok(IndexedBundleRow {
                    source_bundle_hash,
                    verified_at: row.get(1)?,
                    compiler_json: row.get(2)?,
                    storage_revision: row.get(3)?,
                })
            })?;

            let mut bundles = Vec::new();
            for row in rows {
                let row = row?;
                let bundle = bundle_from_row(&connection, code_hash, row)?;
                validate_stored_bundle(&bundle, code_hash)?;
                bundles.push(bundle);
            }

            drop(statement);
            drop(connection);
            Ok(bundles)
        }
    }
}

#[derive(Debug, Error)]
pub enum VerificationIndexError {
    #[error("failed to create registry index directory {path}: {source}", path = path.display())]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("registry index mutex is poisoned")]
    Lock,
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Time(#[from] SystemTimeError),
    #[error("timestamp is too large for SQLite integer: {0}")]
    TimestampOutOfRange(u64),
    #[error("registry index integer field {field} has invalid value: {value}")]
    InvalidInteger { field: &'static str, value: i64 },
    #[error(transparent)]
    SourceStorage(#[from] SourceStorageError),
    #[error(transparent)]
    BundleValidation(#[from] StoredBundleValidationError),
}

struct IndexedBundleRow {
    source_bundle_hash: String,
    verified_at: i64,
    compiler_json: String,
    storage_revision: String,
}

fn initialize_schema(connection: &Connection) -> Result<(), VerificationIndexError> {
    let user_version =
        connection.query_row("pragma user_version", [], |row| row.get::<_, i64>(0))?;
    if user_version != INDEX_SCHEMA_VERSION {
        connection.execute_batch(
            r"
            drop table if exists bundle_sources;
            drop table if exists bundle_files;
            drop table if exists verified_code_hashes;
            drop table if exists verified_bundles;
            drop table if exists registry_index_state;
            ",
        )?;
    }

    connection.execute_batch(
        r"
        pragma foreign_keys = on;

        create table if not exists registry_index_state (
          id integer primary key check (id = 1),
          indexed_revision text not null,
          indexed_at integer not null
        );

        create table if not exists verified_bundles (
          code_hash text not null,
          source_bundle_hash text not null,
          verified_at integer not null,
          compiler_json text not null,
          storage_revision text not null,
          indexed_at integer not null,
          primary key (code_hash, source_bundle_hash)
        );

        create table if not exists bundle_files (
          code_hash text not null,
          source_bundle_hash text not null,
          path text not null,
          content_hash text not null,
          content text not null,
          include_in_command integer,
          is_stdlib integer,
          has_include_directives integer,
          primary key (code_hash, source_bundle_hash, path)
        );

        create index if not exists verified_bundles_by_code_hash
          on verified_bundles (code_hash);

        create index if not exists bundle_files_by_bundle
          on bundle_files (code_hash, source_bundle_hash);
        ",
    )?;
    connection.pragma_update(None, "user_version", INDEX_SCHEMA_VERSION)?;

    Ok(())
}

fn insert_bundle(
    transaction: &Transaction<'_>,
    bundle: &StoredSourceBundle,
    indexed_at: i64,
) -> Result<(), VerificationIndexError> {
    let manifest = &bundle.manifest;
    transaction.execute(
        r"
        insert into verified_bundles (
          code_hash,
          source_bundle_hash,
          verified_at,
          compiler_json,
          storage_revision,
          indexed_at
        ) values (?1, ?2, ?3, ?4, ?5, ?6)
        on conflict (code_hash, source_bundle_hash) do update set
          verified_at = excluded.verified_at,
          compiler_json = excluded.compiler_json,
          storage_revision = excluded.storage_revision,
          indexed_at = excluded.indexed_at
        ",
        params![
            &manifest.code_hash,
            &manifest.source_bundle_hash,
            i64::try_from(manifest.verified_at).map_err(|_| {
                VerificationIndexError::TimestampOutOfRange(manifest.verified_at)
            })?,
            serde_json::to_string(&manifest.compiler)?,
            &bundle.storage_revision,
            indexed_at,
        ],
    )?;

    for file in &bundle.files {
        transaction.execute(
            r"
            insert into bundle_files (
              code_hash,
              source_bundle_hash,
              path,
              content_hash,
              content,
              include_in_command,
              is_stdlib,
              has_include_directives
            ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                &manifest.code_hash,
                &manifest.source_bundle_hash,
                &file.path,
                &file.content_hash,
                &file.content,
                option_bool_to_i64(file.include_in_command),
                option_bool_to_i64(file.is_stdlib),
                option_bool_to_i64(file.has_include_directives),
            ],
        )?;
    }

    Ok(())
}

fn delete_bundle_rows(
    transaction: &Transaction<'_>,
    code_hash: &str,
    source_bundle_hash: &str,
) -> Result<(), VerificationIndexError> {
    transaction.execute(
        "delete from bundle_files where code_hash = ?1 and source_bundle_hash = ?2",
        params![code_hash, source_bundle_hash],
    )?;
    transaction.execute(
        "delete from verified_bundles where code_hash = ?1 and source_bundle_hash = ?2",
        params![code_hash, source_bundle_hash],
    )?;
    Ok(())
}

fn set_index_state(
    transaction: &Transaction<'_>,
    indexed_revision: &str,
    indexed_at: i64,
) -> Result<(), VerificationIndexError> {
    transaction.execute(
        r"
        insert into registry_index_state (
          id,
          indexed_revision,
          indexed_at
        ) values (1, ?1, ?2)
        on conflict (id) do update set
          indexed_revision = excluded.indexed_revision,
          indexed_at = excluded.indexed_at
        ",
        params![indexed_revision, indexed_at],
    )?;
    Ok(())
}

fn bundle_from_row(
    connection: &Connection,
    code_hash: &str,
    row: IndexedBundleRow,
) -> Result<StoredSourceBundle, VerificationIndexError> {
    let files = bundle_files(connection, code_hash, &row.source_bundle_hash)?;
    let compiler = serde_json::from_str::<CompilerMetadata>(&row.compiler_json)?;

    Ok(StoredSourceBundle {
        storage_revision: row.storage_revision,
        manifest: SourceBundleManifest {
            code_hash: code_hash.to_owned(),
            source_bundle_hash: row.source_bundle_hash,
            verified_at: u64::try_from(row.verified_at).map_err(|_| {
                VerificationIndexError::InvalidInteger {
                    field: "verified_at",
                    value: row.verified_at,
                }
            })?,
            compiler,
        },
        files,
    })
}

fn bundle_files(
    connection: &Connection,
    code_hash: &str,
    source_bundle_hash: &str,
) -> Result<Vec<StoredSourceFile>, VerificationIndexError> {
    let mut statement = connection.prepare(
        r"
        select
          path,
          content_hash,
          content,
          include_in_command,
          is_stdlib,
          has_include_directives
        from bundle_files
        where code_hash = ?1 and source_bundle_hash = ?2
        order by path
        ",
    )?;
    let rows = statement.query_map(params![code_hash, source_bundle_hash], |row| {
        Ok(StoredSourceFile {
            path: row.get(0)?,
            content_hash: row.get(1)?,
            content: row.get(2)?,
            include_in_command: option_i64_to_bool(row.get(3)?),
            is_stdlib: option_i64_to_bool(row.get(4)?),
            has_include_directives: option_i64_to_bool(row.get(5)?),
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(VerificationIndexError::Sqlite)
}

fn now_unix_seconds() -> Result<i64, VerificationIndexError> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    i64::try_from(seconds).map_err(|_| VerificationIndexError::TimestampOutOfRange(seconds))
}

fn revision_or_unknown(revision: Option<String>) -> String {
    revision.unwrap_or_else(|| UNKNOWN_REVISION.to_owned())
}

const fn option_bool_to_i64(value: Option<bool>) -> Option<i64> {
    match value {
        Some(value) => Some(if value { 1 } else { 0 }),
        None => None,
    }
}

const fn option_i64_to_bool(value: Option<i64>) -> Option<bool> {
    match value {
        Some(value) => Some(value != 0),
        None => None,
    }
}
