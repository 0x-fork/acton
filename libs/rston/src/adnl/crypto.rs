//! Ed25519 keys for ADNL identity and TCP authentication.
//! Key operations use the same crypto implementation as blockchain models.

pub use tycho_crypto::ed25519::{ExpandedSecretKey, PublicKey, SecretKey};

/// Boxed TL public keys used by ADNL.
pub mod tl {
    pub use tycho_crypto::tl::{PublicKey, PublicKeyOwned};
}

/// Ed25519 key pair
#[derive(Copy, Clone)]
pub struct KeyPair {
    /// Expanded signing key; keep it private to the connection owner.
    pub secret_key: ExpandedSecretKey,
    /// Identity advertised to remote peers.
    pub public_key: PublicKey,
}

impl KeyPair {
    /// Generates a new random KeyPair
    pub fn generate<R: super::helper_types::CryptoRandom>(rng: &mut R) -> Self {
        let mut secret_bytes = [0u8; 32];
        rng.fill_bytes(&mut secret_bytes);
        Self::from(&SecretKey::from_bytes(secret_bytes))
    }

    /// Signs a serialized TL representation of data
    #[inline(always)]
    pub fn sign_tl<T: tl_proto::TlWrite>(&self, data: T) -> [u8; 64] {
        self.secret_key.sign_tl(data, &self.public_key)
    }

    /// Signs raw bytes
    #[inline(always)]
    pub fn sign_raw(&self, data: &[u8]) -> [u8; 64] {
        self.secret_key.sign_raw(data, &self.public_key)
    }

    /// Computes shared secret using x25519
    #[inline(always)]
    pub fn compute_shared_secret(&self, other_public_key: &PublicKey) -> [u8; 32] {
        self.secret_key.compute_shared_secret(other_public_key)
    }
}

impl From<ExpandedSecretKey> for KeyPair {
    fn from(secret_key: ExpandedSecretKey) -> Self {
        let public_key = PublicKey::from(&secret_key);
        Self {
            secret_key,
            public_key,
        }
    }
}

impl From<&'_ SecretKey> for KeyPair {
    fn from(secret_key: &SecretKey) -> Self {
        let secret_key = secret_key.expand();
        let public_key = PublicKey::from(&secret_key);
        Self {
            secret_key,
            public_key,
        }
    }
}
