//! ADNL identities, session parameters, and transport errors.

use crate::adnl::crypto::PublicKey;
use sha2::{Digest, Sha256};
use std::{array::TryFromSliceError, io::Error, time::Duration};
use thiserror::Error;

/// Randomness suitable for session keys and ephemeral identity generation.
pub trait CryptoRandom: rand8::RngCore + rand8::CryptoRng {}

impl<T> CryptoRandom for T where T: rand8::RngCore + rand8::CryptoRng {}

/// Wrapper struct to hold ADNL address, which is a hash of public key
#[derive(PartialEq, Clone)]
pub struct AdnlAddress([u8; 32]);

impl std::fmt::Debug for AdnlAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AdnlAddress")
            .field(&format!("{:02x?}", self.0))
            .finish()
    }
}

impl From<[u8; 32]> for AdnlAddress {
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl TryFrom<&[u8]> for AdnlAddress {
    type Error = TryFromSliceError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Ok(Self(value.try_into()?))
    }
}

impl From<&PublicKey> for AdnlAddress {
    fn from(value: &PublicKey) -> Self {
        let mut hasher = Sha256::new();
        hasher.update([0xc6, 0xb4, 0x13, 0x48]); // type id - always ed25519
        hasher.update(value.as_bytes());
        AdnlAddress(hasher.finalize().into())
    }
}

/// Session parameters for AES-CTR encryption of datagrams
#[derive(Clone)]
pub struct AdnlAesParams {
    rx_key: [u8; 32],
    tx_key: [u8; 32],
    rx_nonce: [u8; 16],
    tx_nonce: [u8; 16],
    padding: [u8; 64],
}

impl From<[u8; 160]> for AdnlAesParams {
    fn from(raw_buffer: [u8; 160]) -> Self {
        Self {
            rx_key: raw_buffer[..32].try_into().unwrap(),
            tx_key: raw_buffer[32..64].try_into().unwrap(),
            rx_nonce: raw_buffer[64..80].try_into().unwrap(),
            tx_nonce: raw_buffer[80..96].try_into().unwrap(),
            padding: raw_buffer[96..160].try_into().unwrap(),
        }
    }
}

impl AdnlAesParams {
    /// Client receive key; the server uses it to encrypt responses.
    pub fn rx_key(&self) -> &[u8; 32] {
        &self.rx_key
    }

    /// Client transmit key; the server uses it to decrypt requests.
    pub fn tx_key(&self) -> &[u8; 32] {
        &self.tx_key
    }

    /// Initial client receive counter; never reuse it with the same key on another connection.
    pub fn rx_nonce(&self) -> &[u8; 16] {
        &self.rx_nonce
    }

    /// Initial client transmit counter; never reuse it with the same key on another connection.
    pub fn tx_nonce(&self) -> &[u8; 16] {
        &self.tx_nonce
    }

    /// Serialize this structure into bytes to use in handshake packet
    pub fn to_bytes(&self) -> [u8; 160] {
        let mut result = [0u8; 160];
        result[..32].copy_from_slice(&self.rx_key);
        result[32..64].copy_from_slice(&self.tx_key);
        result[64..80].copy_from_slice(&self.rx_nonce);
        result[80..96].copy_from_slice(&self.tx_nonce);
        result[96..160].copy_from_slice(&self.padding);
        result
    }

    /// Generate random session parameters
    pub fn random<T: CryptoRandom>(csprng: &mut T) -> Self {
        let mut result = [0u8; 160];
        csprng.fill_bytes(&mut result);
        Self::from(result)
    }
}

impl Default for AdnlAesParams {
    fn default() -> Self {
        Self {
            rx_key: [0; 32],
            tx_key: [0; 32],
            rx_nonce: [0; 16],
            tx_nonce: [0; 16],
            padding: [0; 64],
        }
    }
}

impl AdnlAddress {
    #[inline]
    /// Copy the 256-bit ADNL identity hash in wire order.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    #[inline]
    /// Borrow the 256-bit ADNL identity hash in wire order.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Common error type
#[derive(Debug, Error)]
pub enum AdnlError {
    #[error("IO error")]
    /// Underlying transport or input/output failure; discard an invalid encrypted stream.
    IoError(#[from] Error),
    #[error("{operation} timed out after {timeout:?}")]
    /// Deadline expired. A request may already have reached the server.
    Timeout {
        /// Phase whose deadline expired.
        operation: &'static str,
        /// Configured deadline for the operation.
        timeout: Duration,
    },
    #[error("Integrity error")]
    /// Handshake or frame integrity failed; discard the connection.
    IntegrityError,
    #[error("Too short packet (64 bytes min)")]
    /// Frame body is smaller than the nonce and checksum overhead.
    TooShortPacket,
    #[error("Too long packet (16 MiB max)")]
    /// Frame body exceeds the 16 MiB limit.
    TooLongPacket,
    #[error("Receiver ADNL address mismatch")]
    /// The requested identity does not match an available server key.
    UnknownAddr(AdnlAddress),
    #[error("End of stream")]
    /// Peer closed the transport before the required frame arrived.
    EndOfStream,
    #[error("Invalid public key")]
    /// Identity bytes could not be decoded as an Ed25519 point.
    InvalidPublicKey,
}

/// Information about connected peers.
pub struct AdnlConnectionInfo {
    local_address: AdnlAddress,
    remote_address: AdnlAddress,
}

impl AdnlConnectionInfo {
    /// Record identities established by the handshake; application authorization is a separate responsibility.
    pub fn new(local_address: AdnlAddress, remote_address: AdnlAddress) -> Self {
        Self {
            local_address,
            remote_address,
        }
    }

    /// Identity owned by this endpoint after the handshake.
    pub fn local_address(&self) -> &AdnlAddress {
        &self.local_address
    }

    /// Peer identity established by the handshake, independent of blockchain trust.
    pub fn remote_address(&self) -> &AdnlAddress {
        &self.remote_address
    }
}
