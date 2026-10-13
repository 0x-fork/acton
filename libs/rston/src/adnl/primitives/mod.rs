//! AES framing and handshake primitives for ADNL/TCP.

use aes::Aes256;
use ctr::Ctr128BE;

/// Encrypted frames with bounded lengths and SHA-256 integrity checks.
pub mod codec;
/// Encrypted session-key exchange for a configured Ed25519 identity.
pub mod handshake;

/// AES-256 in CTR mode with the protocol-defined big-endian counter.
pub type AdnlAes = Ctr128BE<Aes256>;
