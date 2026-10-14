use std::path::PathBuf;

use thiserror::Error;
use verifier::{config::ConfigError, source_storage::SourceStorageError};

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("source_repository.path must be set to a non-empty path in config {config_path}")]
    MissingSourceRepository { config_path: PathBuf },
    #[error("registry removal requires server.read_only = true in config {config_path}")]
    ReadOnlyRequired { config_path: PathBuf },
    #[error("removal cancelled: backend shutdown was not confirmed")]
    BackendNotConfirmed,
    #[error("failed to read backend shutdown confirmation: {0}")]
    ConfirmationIo(std::io::Error),
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
    #[error(
        "registry removal failed: {source}; planned tracked files and index entries were restored"
    )]
    RemovalFailed { source: Box<SourceStorageError> },
    #[error(
        "registry removal failed: {operation}; restoring planned tracked files also failed: {recovery}; inspect git status before retrying"
    )]
    RecoveryFailed {
        operation: Box<SourceStorageError>,
        recovery: Box<SourceStorageError>,
    },
    #[error(
        "registry removal failed: {operation}; checking HEAD also failed: {check}; recovery was skipped to preserve possible commits; inspect git status and git log before retrying"
    )]
    HeadCheckFailed {
        operation: Box<SourceStorageError>,
        check: Box<SourceStorageError>,
    },
    #[error(
        "registry removal failed: {source}; HEAD changed to {head}; local commit preserved, push was not attempted; inspect git status and the commit before publishing"
    )]
    LocalCommitPreserved {
        source: Box<SourceStorageError>,
        head: String,
    },
    #[error(
        "removal stopped at code hash {code_hash}: {source}; earlier local commits are preserved at {head}; remaining bundles were not processed and push was not attempted"
    )]
    PartialRemoval {
        code_hash: String,
        head: String,
        source: Box<Self>,
    },
    #[error(
        "push to {remote}/{branch} failed: {source}; local commits are preserved at {head}; inspect the remote before retrying publication"
    )]
    PushFailed {
        remote: String,
        branch: String,
        head: String,
        source: Box<SourceStorageError>,
    },
}
