//! TON global network configuration shared by clients and node applications.
//!
//! Available with `models` and `serde`, without a networking feature. Parsing
//! validates encodings and checkpoint coordinates, not the trustworthiness of
//! hashes, node descriptors, or the source file. Applications own file access,
//! checkpoint selection, and cryptographic verification of DHT descriptors.

use std::fmt;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::cell::HashBytes;
use crate::models::{BlockId, ShardIdent};

/// DHT bootstrap nodes, liteservers, and checkpoints in a TON global configuration.
///
/// Missing sections allow endpoint-only and P2P-only configurations. Consumers
/// enforce the sections they need. Obtain the file from an independently trusted
/// source before using its server identities or block hashes as trust anchors.
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Fixed TON JSON constructor; `Default` supplies it for programmatic configs.
    #[serde(rename = "@type", default)]
    pub constructor: GlobalConfigConstructor,
    /// DHT parameters and entry points, independent of LiteAPI transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dht: Option<DhtConfig>,
    /// Endpoints in file order. An empty list parses but cannot serve queries.
    #[serde(default)]
    pub liteservers: Vec<ConfigLiteServer>,
    /// Network identity and checkpoints, absent in endpoint-only configurations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator: Option<ValidatorConfig>,
}

/// TON DHT bootstrap settings. Descriptor signatures are checked by the consumer.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DhtConfig {
    /// Fixed constructor distinguishing global DHT parameters from node descriptors.
    #[serde(rename = "@type", default)]
    pub constructor: DhtConfigConstructor,
    /// Number of closest nodes requested in DHT lookups.
    pub k: i32,
    /// DHT lookup parallelism supplied by the network configuration.
    pub a: i32,
    /// Optional DHT network discriminator; absence selects the default network.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_id: Option<i32>,
    /// Signed bootstrap descriptors, retained in their original order.
    pub static_nodes: ConfigDhtNodes,
}

/// Bootstrap descriptors used before a node has discovered DHT peers.
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConfigDhtNodes {
    /// Fixed constructor for an ordered bootstrap node list.
    #[serde(rename = "@type", default)]
    pub constructor: DhtNodesConstructor,
    /// An empty list is valid data; a joining node can require at least one entry.
    pub nodes: Vec<ConfigDhtNode>,
}

/// Signed DHT node advertisement. Parsing checks byte lengths, not its signature.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConfigDhtNode {
    /// Fixed constructor for the descriptor's signed TON object.
    #[serde(rename = "@type", default)]
    pub constructor: DhtNodeConstructor,
    /// Public identity against which the consumer checks the descriptor signature.
    pub id: ConfigPublicKey,
    /// Advertised ADNL UDP addresses and their validity metadata.
    pub addr_list: ConfigAdnlAddressList,
    /// Signed TL `int` bits; static descriptors commonly use -1.
    pub version: i32,
    /// Exactly 64 signature bytes, encoded as padded base64 in JSON.
    #[serde(with = "bytes_base64")]
    pub signature: [u8; 64],
}

impl fmt::Debug for ConfigDhtNode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigDhtNode")
            .field("version", &self.version)
            .field("addresses", &self.addr_list.addrs.len())
            .finish_non_exhaustive()
    }
}

/// Addresses committed by a signed DHT descriptor; their order is significant.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConfigAdnlAddressList {
    /// Fixed constructor for the address-list object committed by the signature.
    #[serde(rename = "@type", default)]
    pub constructor: AdnlAddressListConstructor,
    /// IPv4 UDP addresses. Unsupported address constructors are rejected.
    pub addrs: Vec<ConfigAdnlAddress>,
    /// Address-list version as a signed TL `int`.
    pub version: i32,
    /// ADNL reinitialization timestamp as supplied by the descriptor.
    pub reinit_date: i32,
    /// Descriptor priority, preserved for signature verification by the consumer.
    pub priority: i32,
    /// Expiry timestamp; zero denotes no expiry in static bootstrap descriptors.
    pub expire_at: i32,
}

/// Supported TON JSON address constructors for DHT bootstrap.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "@type")]
pub enum ConfigAdnlAddress {
    /// UDP IPv4 endpoint. IP values retain the signed TL `int` bit pattern.
    #[serde(rename = "adnl.address.udp")]
    Udp {
        /// IPv4 bits in network byte order.
        ip: i32,
        /// Nonzero UDP listening port.
        #[serde(deserialize_with = "read_port")]
        port: u16,
    },
}

impl ConfigAdnlAddress {
    /// Decode the signed IP field without modifying the descriptor's stored bits.
    pub const fn socket_addr(&self) -> SocketAddrV4 {
        match self {
            Self::Udp { ip, port } => SocketAddrV4::new(Ipv4Addr::from_bits(*ip as u32), *port),
        }
    }
}

/// Constructor required by TON global configuration JSON.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum GlobalConfigConstructor {
    /// Standard global network configuration document.
    #[default]
    #[serde(rename = "config.global")]
    Global,
}

/// Constructor for the global DHT section.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum DhtConfigConstructor {
    /// Global DHT lookup parameters and bootstrap nodes.
    #[default]
    #[serde(rename = "dht.config.global")]
    Global,
}

/// Constructor for the ordered DHT bootstrap list.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum DhtNodesConstructor {
    /// Standard TON DHT node list.
    #[default]
    #[serde(rename = "dht.nodes")]
    Nodes,
}

/// Constructor identifying a signed DHT advertisement.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum DhtNodeConstructor {
    /// Standard TON DHT node descriptor.
    #[default]
    #[serde(rename = "dht.node")]
    Node,
}

/// Constructor for addresses contained in a signed DHT advertisement.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum AdnlAddressListConstructor {
    /// Standard TON ADNL address list.
    #[default]
    #[serde(rename = "adnl.addressList")]
    AddressList,
}

/// Network identity and operator-supplied masterchain checkpoints.
///
/// Hashes use standard padded base64 in JSON. These identifiers are configuration
/// claims, not verified blocks. Hardfork entries are retained in file order and
/// do not automatically override the selected checkpoint or proof validation.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ValidatorConfigJson", into = "ValidatorConfigJson")]
pub struct ValidatorConfig {
    /// Masterchain state at sequence zero, identifying the network.
    pub zero_state: BlockId,
    /// Trusted starting block, when provided. Sequence zero must match `zero_state`.
    pub init_block: Option<BlockId>,
    /// Explicit fork checkpoints. No history or signature checks occur during parsing.
    pub hardforks: Vec<BlockId>,
}

#[derive(Serialize, Deserialize)]
struct ValidatorConfigJson {
    #[serde(rename = "@type", default)]
    constructor: ValidatorConfigConstructor,
    zero_state: ConfigBlockJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    init_block: Option<ConfigBlockJson>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hardforks: Vec<ConfigBlockJson>,
}

impl TryFrom<ValidatorConfigJson> for ValidatorConfig {
    type Error = &'static str;

    fn try_from(value: ValidatorConfigJson) -> Result<Self, Self::Error> {
        let zero_state = BlockId::try_from(value.zero_state)?;
        if zero_state.seqno != 0 {
            return Err("validator.zero_state must have seqno zero");
        }

        let init_block = value.init_block.map(BlockId::try_from).transpose()?;
        if init_block.is_some_and(|block| block.seqno == 0 && block != zero_state) {
            return Err("validator.init_block at seqno zero must match zero_state");
        }

        let hardforks = value
            .hardforks
            .into_iter()
            .map(BlockId::try_from)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            zero_state,
            init_block,
            hardforks,
        })
    }
}

impl From<ValidatorConfig> for ValidatorConfigJson {
    fn from(value: ValidatorConfig) -> Self {
        Self {
            constructor: ValidatorConfigConstructor::Global,
            zero_state: value.zero_state.into(),
            init_block: value.init_block.map(Into::into),
            hardforks: value.hardforks.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ConfigBlockJson {
    #[serde(rename = "@type", default)]
    constructor: BlockIdConstructor,
    workchain: i32,
    shard: i64,
    seqno: u32,
    #[serde(with = "hash_base64")]
    root_hash: HashBytes,
    #[serde(with = "hash_base64")]
    file_hash: HashBytes,
}

impl TryFrom<ConfigBlockJson> for BlockId {
    type Error = &'static str;

    fn try_from(value: ConfigBlockJson) -> Result<Self, Self::Error> {
        if value.workchain != -1 || value.shard != i64::MIN {
            return Err("validator checkpoints must belong to the masterchain");
        }
        Ok(Self {
            shard: ShardIdent::MASTERCHAIN,
            seqno: value.seqno,
            root_hash: value.root_hash,
            file_hash: value.file_hash,
        })
    }
}

impl From<BlockId> for ConfigBlockJson {
    fn from(value: BlockId) -> Self {
        Self {
            constructor: BlockIdConstructor::BlockIdExt,
            workchain: value.shard.workchain(),
            shard: value.shard.prefix() as i64,
            seqno: value.seqno,
            root_hash: value.root_hash,
            file_hash: value.file_hash,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
enum ValidatorConfigConstructor {
    #[default]
    #[serde(rename = "validator.config.global")]
    Global,
}

#[derive(Default, Serialize, Deserialize)]
enum BlockIdConstructor {
    #[default]
    #[serde(rename = "ton.blockIdExt")]
    BlockIdExt,
}

impl FromStr for GlobalConfig {
    type Err = serde_json::Error;

    fn from_str(json: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(json)
    }
}

/// An IPv4 endpoint and its ADNL identity, as encoded by the TON global config.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
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

/// Ed25519 identities advertised by liteservers and DHT bootstrap descriptors.
/// Parsing validates the point, not the descriptor signature or endpoint ownership.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
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
        return Err(serde::de::Error::custom("network port must be nonzero"));
    }
    Ok(port)
}

mod hash_base64 {
    use super::*;

    pub fn serialize<S: serde::Serializer>(
        hash: &HashBytes,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        bytes_base64::serialize(&hash.0, serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<HashBytes, D::Error> {
        bytes_base64::deserialize(deserializer).map(HashBytes)
    }
}

mod bytes_base64 {
    use super::*;

    pub fn serialize<S: serde::Serializer, const N: usize>(
        bytes: &[u8; N],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&crate::util::encode_base64(bytes))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>, const N: usize>(
        deserializer: D,
    ) -> Result<[u8; N], D::Error> {
        let encoded = String::deserialize(deserializer)?;
        let bytes = crate::util::decode_base64(encoded).map_err(serde::de::Error::custom)?;
        <[u8; N]>::try_from(bytes.as_slice())
            .map_err(|_| serde::de::Error::custom(format!("expected {N} base64-encoded bytes")))
    }
}

mod public_key_base64 {
    use super::*;

    pub fn serialize<S: serde::Serializer>(
        key: &[u8; 32],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        bytes_base64::serialize(key, serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[u8; 32], D::Error> {
        let key = bytes_base64::deserialize(deserializer)?;
        if tycho_crypto::ed25519::PublicKey::from_bytes(key).is_none() {
            return Err(serde::de::Error::custom("invalid Ed25519 public identity"));
        }
        Ok(key)
    }
}
