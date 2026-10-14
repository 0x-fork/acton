//! LiteAPI queries and server services over ADNL/TCP.
//!
//! Enable `liteclient` to connect with [`LiteClient`] or distribute queries with
//! [`LiteClientPool`]. High-level data methods verify against [`VerifiedBlock`];
//! block lookup checks consistency without authenticating chain membership.
//! Raw and typed queries return unverified server responses. Get-method results
//! authenticate source state, while their output remains server computation.
//! Verification accepts at most 8 MiB per proof or data BoC payload.
//! The wire and transport layers originate from MIT-licensed tonutils-rs.
//! Proof verification follows MIT-licensed tonutils-go.

mod api;
mod client;
mod pool;
mod proof;

pub use crate::tl::utils::FromResponse;
pub use api::{
    AccountInfo, BlockHeader, BlockLookup, BlockProofOptions, GetMethodResult, MethodId,
    VerifiedBlock,
};
pub use client::{LiteClient, LiteClientOptions};
pub use pool::LiteClientPool;
pub use proof::ProofError;
pub use types::LiteError;

/// Tower adapters for request envelopes and server errors.
pub mod layers;
/// Envelope decoding and multiplexing correlation.
pub mod peer;
/// Public LiteAPI listener with caller-provided services.
pub mod server;
/// Shared client and server errors and service traits.
pub mod types;
