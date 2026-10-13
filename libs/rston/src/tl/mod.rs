//! Type Language (TL) structures for TON ADNL and LiteAPI traffic.
//!
//! This module is available with the `tl` feature and contains the handwritten
//! request, response, common identifier, and ADNL message types currently used
//! by the crate. Serialization is backed by `tl-proto`, while local tests check
//! implemented constructor ids against the checked-in LiteAPI schema snapshot.
//!
//! Constructors and flag bits follow the bundled LiteAPI schema. These are raw
//! wire types; decoding does not validate proofs or their claimed block identity.

/// ADNL wire envelopes and TCP authentication types.
pub mod adnl;
/// Shared wire identifiers and unverified signature and proof-link data.
pub mod common;
/// Wire parsing errors and TL serialization traits.
pub mod error;
/// LiteAPI requests with optional fields encoded by protocol flag bits.
pub mod request;
/// Unverified LiteAPI answers and server error payloads.
pub mod response;
/// TL wrapper codecs and response extraction adapters.
pub mod utils;

// Re-export commonly used types
pub use adnl::Message;
pub use common::{
    AccountId, BlockId, BlockIdExt, Int256, NonfinalCandidateId, NonfinalCandidateInfo,
    ZeroStateIdExt,
};
pub use error::TlError;
pub use request::{LiteQuery, LiteQueryRaw, RawWrappedRequest, Request, WrappedRequest};
pub use response::{Error, Response};

#[cfg(test)]
mod schema_check;

#[cfg(test)]
mod tests;
