//! Errors and the service contract shared by LiteAPI clients and servers.

use crate::adnl::helper_types::AdnlError;
use crate::tl::TlError;
use std::time::Duration;
use thiserror::Error;
use tower::Service;

use crate::tl::{request::WrappedRequest, response::Response};

#[derive(Debug, Error)]
/// Transport, deadline, or protocol errors. Timeouts do not imply a request was not executed.
pub enum LiteError {
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
