//! Errors and the service contract shared by LiteAPI clients and servers.

use crate::adnl::helper_types::AdnlError;
use crate::tl::TlError;
use std::time::Duration;
use thiserror::Error;
use tower::Service;

use crate::tl::{request::WrappedRequest, response::Response};

#[derive(Debug, Error)]
/// Transport, protocol, proof, or remote execution failures.
/// Timeouts do not imply a request was not executed.
pub enum LiteError {
    /// Blockchain data or its proof did not match the requested authenticated object.
    #[error("proof verification failed: {0}")]
    Proof(#[source] super::ProofError),
    /// The server reported a failing get-method exit code; execution is not authenticated.
    #[error("remote get method exited with code {exit_code}")]
    ContractExecution {
        /// Unverified TVM exit code reported by the server.
        exit_code: i32,
    },
    /// The server did not acknowledge the submitted message with status one.
    #[error("liteserver rejected message with status {status}")]
    MessageRejected {
        /// Original server-reported status.
        status: u32,
    },
    /// No endpoints were configured. No request was sent.
    #[error("network config has no liteservers")]
    NoServers,
    #[error("Liteserver error {0}")]
    /// The server returned an error answer with its original code and message.
    ServerError(crate::tl::response::Error),
    #[error("{operation} timed out after {timeout:?}")]
    /// Deadline expired. A request may already have reached the server.
    Timeout {
        /// Phase whose deadline expired.
        operation: &'static str,
        /// Configured deadline for the operation.
        timeout: Duration,
    },
    #[error("TL parsing error: {0}")]
    /// The response or envelope could not be decoded as the expected TL object.
    TlError(TlError),
    #[error("Unexpected TL message")]
    /// The received constructor is not valid for this operation.
    UnexpectedMessage,
    #[error("ADNL error: {0}")]
    /// The encrypted transport failed; open a new connection before sending again.
    AdnlError(#[from] AdnlError),
    #[error("Unknown error: {0}")]
    /// Error raised by the multiplexed service, with its underlying diagnostic context.
    UnknownError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl From<super::ProofError> for LiteError {
    fn from(error: super::ProofError) -> Self {
        match error {
            super::ProofError::ContractExecution(exit_code) => {
                Self::ContractExecution { exit_code }
            }
            error => Self::Proof(error),
        }
    }
}

/// A typed LiteAPI service whose futures can run on spawned connection tasks.
pub trait LiteService: Service<WrappedRequest, Response = Response, Error = LiteError>
where
    Self::Future: Send + 'static,
{
}

impl<T> LiteService for T
where
    T: Service<WrappedRequest, Response = Response, Error = LiteError>,
    T::Future: Send + 'static,
{
}
