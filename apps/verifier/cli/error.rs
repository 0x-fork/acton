use std::path::PathBuf;

use thiserror::Error;
use verifier::{config::ConfigError, source_storage::SourceStorageError};

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("source_repository.path must be set to a non-empty path in config {config_path}")]
    MissingSourceRepository { config_path: PathBuf },
    #[error("invalid code hash {value:?}: expected a 256-bit code hash in hexadecimal or base64")]
    InvalidCodeHash { value: String },
    #[error(transparent)]
    SourceStorage(#[from] SourceStorageError),
    #[error("source_repository.path must point to the checkout root: {path}")]
    NotCheckoutRoot { path: PathBuf },
    #[error("source repository has detached HEAD; check out a named branch before removal")]
    DetachedHead,
    #[error("source repository branch mismatch: expected {expected}, current {current}")]
    BranchMismatch { expected: String, current: String },
    #[error("remote {remote:?} returned an invalid HEAD for refs/heads/{branch}")]
    InvalidRemoteHead { remote: String, branch: String },
    #[error(
        "source repository HEAD does not match {remote}/{branch}: local HEAD={local_head}, remote HEAD={remote_head}; inspect and synchronize the checkout before retrying"
    )]
    HeadMismatch {
        remote: String,
        branch: String,
        local_head: String,
        remote_head: String,
    },
    #[error("registry removal failed: {source}; planned files and index entries were restored")]
    RemovalFailed { source: Box<SourceStorageError> },
    #[error(
        "registry removal failed: {operation}; restoring planned files also failed: {recovery}; inspect git status before retrying"
    )]
    RecoveryFailed {
        operation: Box<SourceStorageError>,
        recovery: Box<SourceStorageError>,
    },
}
