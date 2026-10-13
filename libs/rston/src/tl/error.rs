//! Wire parsing errors and TL serialization traits.

use thiserror::Error;

/// A wire-format or input error; it does not represent blockchain proof verification.
#[derive(Debug, Error)]
pub enum TlError {
    /// TL decoder diagnostic preserved for the caller.
    #[error("{0}")]
    ParseError(String),
    /// Input does not satisfy the requested wire representation.
    #[error("Invalid data")]
    InvalidData,
    /// Input ended before a complete value could be decoded.
    #[error("Unexpected end of data")]
    UnexpectedEof,
    /// Underlying transport or input/output failure; discard an invalid encrypted stream.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

// Re-export from tl_proto for convenience
pub use tl_proto::{TlPacket, TlRead, TlResult, TlWrite};
