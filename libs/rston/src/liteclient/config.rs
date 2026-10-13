//! Liteserver endpoints from TON global configuration files.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::str::FromStr;

use base64::Engine;
use serde::{Deserialize, Serialize};

/// The liteserver section of a TON global configuration JSON document.
/// Other sections are ignored. Parsing does not establish trust in the network,
/// server identities, or zero state; obtain the file from a trusted source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigGlobal {
    /// Endpoints in file order. An empty list parses but cannot serve queries.
    pub liteservers: Vec<ConfigLiteServer>,
}

impl FromStr for ConfigGlobal {
    type Err = serde_json::Error;

    fn from_str(json: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(json)
    }
}

/// An IPv4 endpoint and its ADNL identity, as encoded by the TON global config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigLiteServer {
    /// IPv4 bits stored as a signed TL `int`, not an address string.
    pub ip: i32,
    /// TCP listening port. Port zero is rejected when deserializing JSON.
    #[serde(deserialize_with = "read_port")]
    pub port: u16,
    /// Public key used to authenticate the encrypted handshake.
    pub id: ConfigPublicKey,
}

impl ConfigLiteServer {
    /// Interpret signed IP bits in network byte order.
    pub fn socket_addr(&self) -> SocketAddrV4 {
        SocketAddrV4::new(Ipv4Addr::from(self.ip as u32), self.port)
    }

    /// Return the Ed25519 identity bytes supplied to the ADNL handshake.
    pub fn public_key(&self) -> &[u8; 32] {
        match &self.id {
            ConfigPublicKey::Ed25519 { key } => key,
        }
    }
}

/// Boxed JSON identity. Only Ed25519 identities can establish ADNL/TCP sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "@type")]
pub enum ConfigPublicKey {
    /// `pub.ed25519` with a standard, padded base64 key in JSON.
    #[serde(rename = "pub.ed25519")]
    Ed25519 {
        /// Exactly 32 bytes; malformed encoding and invalid points are rejected.
        #[serde(with = "public_key_base64")]
        key: [u8; 32],
    },
}

fn read_port<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u16, D::Error> {
    let port = u16::deserialize(deserializer)?;
    if port == 0 {
        return Err(serde::de::Error::custom("liteserver port must be nonzero"));
    }
    Ok(port)
}

mod public_key_base64 {
    use super::*;

    pub fn serialize<S: serde::Serializer>(
        key: &[u8; 32],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(key))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[u8; 32], D::Error> {
        let encoded = String::deserialize(deserializer)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(serde::de::Error::custom)?;
        let key = <[u8; 32]>::try_from(bytes.as_slice())
            .map_err(|_| serde::de::Error::custom("liteserver identity must contain 32 bytes"))?;
        if crate::adnl::crypto::PublicKey::from_bytes(key).is_none() {
            return Err(serde::de::Error::custom(
                "invalid Ed25519 liteserver identity",
            ));
        }
        Ok(key)
    }
}
