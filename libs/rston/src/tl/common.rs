//! Wire identifiers, signature sets, and proof links shared by LiteAPI constructors.

use core::fmt;
use std::{fmt::Display, str::FromStr};

use hex::FromHex;
use tl_proto::{TlRead, TlWrite};

/// `true = True;`
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
pub struct True;

/// `string ? = String;`
#[derive(TlRead, TlWrite, Clone, PartialEq)]
pub struct String(Vec<u8>);

impl fmt::Debug for String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", std::string::String::from_utf8_lossy(&self.0))
    }
}

impl From<&str> for String {
    fn from(s: &str) -> Self {
        Self(s.as_bytes().to_vec())
    }
}

impl String {
    /// Build a TL string from UTF-8 text. Received TL strings can also contain non-UTF-8 bytes.
    pub fn new(str: std::string::String) -> Self {
        Self(str.into_bytes())
    }
}

/// `int256 8*[ int ] = Int256;`
#[derive(TlRead, TlWrite, Clone, PartialEq, Eq, Default, Hash)]
pub struct Int256(#[doc = "All 256 bits in TL wire byte order."] pub [u8; 32]);

impl FromStr for Int256 {
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
    type Err = hex::FromHexError;
}

impl fmt::Debug for Int256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for Int256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Int256 {
    /// Encode all 256 bits as lowercase hex in wire byte order.
    pub fn to_hex(&self) -> std::string::String {
        hex::encode(self.0)
    }

    /// Decode exactly 32 bytes from hex without reversing byte order.
    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        Ok(Int256(<[u8; 32]>::from_hex(s)?))
    }

    /// Generate a random correlation value; callers must not use it as a signing key.
    pub fn random() -> Self {
        Self(rand8::random())
    }
}

/// `tonNode.blockId workchain:int shard:long seqno:int = tonNode.BlockId;`
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
pub struct BlockId {
    /// Signed workchain identifier; -1 denotes the masterchain.
    pub workchain: i32,
    /// Shard identifier with its prefix marker, not an account address.
    pub shard: i64,
    /// Block sequence number or sequence-number selection flag, as defined by this constructor.
    pub seqno: i32,
}

/// `tonNode.blockIdExt workchain:int shard:long seqno:int root_hash:int256 file_hash:int256 = tonNode.BlockIdExt;`
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockIdExt {
    /// Signed workchain identifier; -1 denotes the masterchain.
    pub workchain: i32,
    /// Shard identifier with its prefix marker, not an account address.
    pub shard: i64,
    /// Block sequence number or sequence-number selection flag, as defined by this constructor.
    pub seqno: i32,
    /// Representation hash of the root cell, distinct from the serialized file hash.
    pub root_hash: Int256,
    /// SHA-256 of the serialized block file, distinct from the cell representation hash.
    pub file_hash: Int256,
}

/// Convert a model identity into its LiteAPI wire representation. The signed
/// TL int carries all bits of the model's unsigned sequence number.
#[cfg(feature = "models")]
impl From<&crate::models::BlockId> for BlockIdExt {
    fn from(id: &crate::models::BlockId) -> Self {
        Self {
            workchain: id.shard.workchain(),
            shard: id.shard.prefix() as i64,
            seqno: id.seqno as i32,
            root_hash: Int256(id.root_hash.0),
            file_hash: Int256(id.file_hash.0),
        }
    }
}

#[cfg(feature = "models")]
impl From<crate::models::BlockId> for BlockIdExt {
    fn from(id: crate::models::BlockId) -> Self {
        Self::from(&id)
    }
}

/// Decode a LiteAPI identity, checking the shard's prefix-marker encoding.
/// Hashes retain wire byte order, and the sequence number retains all 32 bits.
/// This validates representation only; callers still authenticate chain membership
/// and block contents through proofs or an independently trusted checkpoint.
#[cfg(feature = "models")]
impl TryFrom<&BlockIdExt> for crate::models::BlockId {
    type Error = crate::error::ParseBlockIdError;

    fn try_from(id: &BlockIdExt) -> Result<Self, Self::Error> {
        let shard = crate::models::ShardIdent::new(id.workchain, id.shard as u64)
            .ok_or(crate::error::ParseBlockIdError::InvalidShardIdent)?;

        Ok(Self {
            shard,
            seqno: id.seqno as u32,
            root_hash: crate::cell::HashBytes(id.root_hash.0),
            file_hash: crate::cell::HashBytes(id.file_hash.0),
        })
    }
}

#[cfg(feature = "models")]
impl TryFrom<BlockIdExt> for crate::models::BlockId {
    type Error = crate::error::ParseBlockIdError;

    fn try_from(id: BlockIdExt) -> Result<Self, Self::Error> {
        Self::try_from(&id)
    }
}

impl fmt::Display for BlockIdExt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({},{:X},{}):{}:{}",
            self.workchain, self.shard, self.seqno, self.root_hash, self.file_hash
        )
    }
}

/// `liteServer.accountId workchain:int id:int256 = liteServer.AccountId;`
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
pub struct AccountId {
    /// Signed workchain identifier; -1 denotes the masterchain.
    pub workchain: i32,
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: Int256,
}

/// `liteServer.transactionId3 account:int256 lt:long = liteServer.TransactionId3;`
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
pub struct TransactionId3 {
    /// Account identifier; transaction cursors use the account hash without a workchain.
    pub account: Int256,
    /// Logical time used to identify or select a transaction; not wall-clock time.
    pub lt: u64,
}

/// `liteServer.signature node_id_short:int256 signature:bytes = liteServer.Signature;`
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
pub struct Signature {
    /// SHA-256 of the boxed Ed25519 public key identifying the signer.
    pub node_id_short: Int256,

    /// Ed25519 signature bytes; decoding alone does not authenticate the signer.
    pub signature: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
#[tl(
    boxed,
    scheme_inline = r##"liteServer.signatureSet.ordinary#f644a6e6 validator_set_hash:int catchain_seqno:int signatures:(vector liteServer.signature) = liteServer.SignatureSet;
        liteServer.signatureSet.simplex cc_seqno:int validator_set_hash:int signatures:(vector liteServer.signature) session_id:int256 slot:int candidate:bytes = liteServer.SignatureSet;"##
)]
/// Unverified ordinary or Simplex signatures carried by a LiteAPI block proof.
pub enum SignatureSet {
    /// liteServer.signatureSet.ordinary#f644a6e6 validator_set_hash:int catchain_seqno:int signatures:(vector liteServer.signature) = liteServer.SignatureSet;
    #[tl(id = 0xf644a6e6)]
    Ordinary {
        /// Subset hash supplied by the server; compare it with a trusted validator set.
        validator_set_hash: i32,
        /// Catchain sequence number supplied by the server; compare with trusted validator metadata.
        catchain_seqno: i32,
        /// Unverified signer data; check identities, uniqueness, weights, and the required quorum.
        signatures: Vec<Signature>,
    },
    /// liteServer.signatureSet.simplex cc_seqno:int validator_set_hash:int signatures:(vector liteServer.signature) session_id:int256 slot:int candidate:bytes = liteServer.SignatureSet;
    #[tl(id = "liteServer.signatureSet.simplex")]
    Simplex {
        /// Consensus sequence number supplied by the server; compare with trusted validator metadata.
        cc_seqno: i32,
        /// Subset hash supplied by the server; compare it with a trusted validator set.
        validator_set_hash: i32,
        /// Unverified signer data; check identities, uniqueness, weights, and the required quorum.
        signatures: Vec<Signature>,
        /// Simplex session identifier included in the finalization signing domain.
        session_id: Int256,
        /// Simplex slot included in the candidate identifier.
        slot: i32,

        /// Original TL candidate bytes; hash these exact bytes when verifying a Simplex vote.
        candidate: Vec<u8>,
    },
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
#[tl(
    boxed,
    scheme_inline = r##"liteServer.blockLinkBack to_key_block:Bool from:tonNode.blockIdExt to:tonNode.blockIdExt dest_proof:bytes proof:bytes state_proof:bytes = liteServer.BlockLink;
        liteServer.blockLinkForward to_key_block:Bool from:tonNode.blockIdExt to:tonNode.blockIdExt dest_proof:bytes config_proof:bytes signatures:liteServer.SignatureSet = liteServer.BlockLink;"##
)]
/// Unverified backward or forward links. Verify the chain from a trusted starting block.
pub enum BlockLink {
    /// liteServer.blockLinkBack to_key_block:Bool from:tonNode.blockIdExt to:tonNode.blockIdExt dest_proof:bytes proof:bytes state_proof:bytes = liteServer.BlockLink;
    #[tl(id = "liteServer.blockLinkBack")]
    BlockLinkBack {
        /// Whether the destination is claimed to be a key block; confirm it from verified data.
        to_key_block: bool,
        /// Starting block identity of this proof link.
        from: BlockIdExt,
        /// Destination block identity of this proof link.
        to: BlockIdExt,

        /// Unverified serialized proof of the destination block.
        dest_proof: Vec<u8>,

        /// Unverified serialized proof; validate against an independently trusted block or state.
        proof: Vec<u8>,

        /// Unverified serialized proof of the claimed state root.
        state_proof: Vec<u8>,
    },
    /// liteServer.blockLinkForward to_key_block:Bool from:tonNode.blockIdExt to:tonNode.blockIdExt dest_proof:bytes config_proof:bytes signatures:liteServer.SignatureSet = liteServer.BlockLink;
    #[tl(id = "liteServer.blockLinkForward")]
    BlockLinkForward {
        /// Whether the destination is claimed to be a key block; confirm it from verified data.
        to_key_block: bool,
        /// Starting block identity of this proof link.
        from: BlockIdExt,
        /// Destination block identity of this proof link.
        to: BlockIdExt,

        /// Unverified serialized proof of the destination block.
        dest_proof: Vec<u8>,

        /// Unverified serialized configuration proof used to authenticate validator or config data.
        config_proof: Vec<u8>,
        /// Unverified signer data; check identities, uniqueness, weights, and the required quorum.
        signatures: SignatureSet,
    },
}

/// `tonNode.zeroStateIdExt workchain:int root_hash:int256 file_hash:int256 = tonNode.ZeroStateIdExt;`
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
pub struct ZeroStateIdExt {
    /// Signed workchain identifier; -1 denotes the masterchain.
    pub workchain: i32,
    #[tl(size_hint = 32)]
    /// Representation hash of the root cell, distinct from the serialized file hash.
    pub root_hash: Int256,
    #[tl(size_hint = 32)]
    /// SHA-256 of the serialized block file, distinct from the cell representation hash.
    pub file_hash: Int256,
}

/// `liteServer.transactionId mode:# account:mode.0?int256 lt:mode.1?long hash:mode.2?int256 = liteServer.TransactionId;`
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
pub struct TransactionId {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    #[tl(flags_bit = "mode.0")]
    /// Account identifier; transaction cursors use the account hash without a workchain.
    pub account: Option<Int256>,
    #[tl(flags_bit = "mode.1")]
    /// Logical time used to identify or select a transaction; not wall-clock time.
    pub lt: Option<u64>,
    #[tl(flags_bit = "mode.2")]
    /// Representation hash associated with this record; verify its binding to the requested object.
    pub hash: Option<Int256>,
}

/// `liteServer.libraryEntry hash:int256 data:bytes = liteServer.LibraryEntry;`
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
pub struct LibraryEntry {
    /// Representation hash associated with this record; verify its binding to the requested object.
    pub hash: Int256,

    /// Serialized protocol payload. The enclosing constructor defines whether it contains a BoC or raw bytes.
    pub data: Vec<u8>,
}

/// `liteServer.nonfinal.candidateId block_id:tonNode.blockIdExt creator:int256 collated_data_hash:int256 = liteServer.nonfinal.CandidateId;`
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
pub struct NonfinalCandidateId {
    /// Full block identity associated with this protocol record.
    pub block_id: BlockIdExt,
    /// Creator identity advertised for a nonfinal candidate.
    pub creator: Int256,
    /// Hash identifying the collated data associated with this candidate.
    pub collated_data_hash: Int256,
}

/// `liteServer.nonfinal.candidateInfo id:liteServer.nonfinal.candidateId available:Bool approved_weight:long signed_weight:long total_weight:long = liteServer.nonfinal.CandidateInfo;`
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
pub struct NonfinalCandidateInfo {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: NonfinalCandidateId,
    /// Whether the server currently advertises the candidate data as available.
    pub available: bool,
    /// Server-reported approval weight; recompute from a trusted validator subset.
    pub approved_weight: u64,
    /// Server-reported aggregate signature weight; recompute from a trusted validator subset.
    pub signed_weight: u64,
    /// Server-reported total validator weight; compare with the trusted subset total.
    pub total_weight: u64,
}
