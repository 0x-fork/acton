//! ADNL/TCP encrypted transport. Enable `adnl` independently of the LiteAPI client.
//!
//! Transport code originates from MIT-licensed tonutils-rs and adnl-rs.
//! Connections authenticate possession of the configured server key, not blockchain data.

pub mod crypto;
/// Identity hashes, session parameters, and transport errors.
pub mod helper_types;
/// Stateful frame and handshake codecs.
pub mod primitives;
/// Connection-owning stream wrappers and handshake builders.
pub mod wrappers;

pub use helper_types::{AdnlAddress, AdnlAesParams, AdnlConnectionInfo, AdnlError};
pub use primitives::codec::AdnlCodec;
pub use primitives::handshake::AdnlHandshake;
pub use wrappers::builder::AdnlBuilder;
pub use wrappers::peer::AdnlPeer;

#[cfg(test)]
mod tests;
