//! LiteAPI queries and server services over ADNL/TCP.
//!
//! Enable `liteclient` to connect with [`LiteClient`] or distribute queries with
//! [`LiteClientPool`]. Network responses require independent proof verification.
//! The wire and transport layers originate from MIT-licensed tonutils-rs.

mod client;
pub mod config;
mod pool;

pub use crate::tl::utils::FromResponse;
pub use client::{LiteClient, LiteClientOptions};
pub use config::{ConfigGlobal, ConfigLiteServer, ConfigPublicKey};
pub use pool::LiteClientPool;
pub use types::LiteError;

/// Tower adapters for request envelopes and server errors.
pub mod layers;
/// Envelope decoding and multiplexing correlation.
pub mod peer;
/// Public LiteAPI listener with caller-provided services.
pub mod server;
/// Shared client and server errors and service traits.
pub mod types;
