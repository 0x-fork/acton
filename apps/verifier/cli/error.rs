use std::path::PathBuf;

use thiserror::Error;
use verifier::config::ConfigError;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("source_repository.path must be set to a non-empty path in config {config_path}")]
    MissingSourceRepository { config_path: PathBuf },
    #[error("invalid code hash {value:?}: expected a 256-bit code hash in hexadecimal or base64")]
    InvalidCodeHash { value: String },
    #[error("registry removal is not implemented yet; use --dry-run to prepare the request only")]
    RemovalNotImplemented,
}
