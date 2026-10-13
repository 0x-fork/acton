//! Unverified LiteAPI response payloads. Callers validate proofs and object identities.

use tl_proto::{TlRead, TlWrite};

use super::common::*;

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `MasterchainInfo` LiteAPI answer. Values come from the server and require independent validation.
pub struct MasterchainInfo {
    /// Server-reported latest masterchain block; verify its proof before trusting its identity.
    pub last: BlockIdExt,
    /// Server-reported state root; authenticate it through a verified block proof.
    pub state_root_hash: Int256,
    /// Server-reported zero-state identity; compare with an independently trusted network anchor.
    pub init: ZeroStateIdExt,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `MasterchainInfoExt` LiteAPI answer. Values come from the server and require independent validation.
pub struct MasterchainInfoExt {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Server protocol version used to interpret supported capabilities.
    pub version: u32,
    /// Capability bitmask advertised by the server.
    pub capabilities: u64,
    /// Server-reported latest masterchain block; verify its proof before trusting its identity.
    pub last: BlockIdExt,
    /// Unix timestamp of the server-reported latest block.
    pub last_utime: u32,
    /// Server-reported Unix time in seconds; not a trusted clock.
    pub now: u32,
    /// Server-reported state root; authenticate it through a verified block proof.
    pub state_root_hash: Int256,
    /// Server-reported zero-state identity; compare with an independently trusted network anchor.
    pub init: ZeroStateIdExt,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `CurrentTime` LiteAPI answer. Values come from the server and require independent validation.
pub struct CurrentTime {
    /// Server-reported Unix time in seconds; not a trusted clock.
    pub now: u32,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `Version` LiteAPI answer. Values come from the server and require independent validation.
pub struct Version {
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: u32,
    /// Server protocol version used to interpret supported capabilities.
    pub version: u32,
    /// Capability bitmask advertised by the server.
    pub capabilities: u64,
    /// Server-reported Unix time in seconds; not a trusted clock.
    pub now: u32,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `BlockData` LiteAPI answer. Values come from the server and require independent validation.
pub struct BlockData {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,

    /// Serialized protocol payload. The enclosing constructor defines whether it contains a BoC or raw bytes.
    pub data: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `BlockState` LiteAPI answer. Values come from the server and require independent validation.
pub struct BlockState {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Representation hash of the root cell, distinct from the serialized file hash.
    pub root_hash: Int256,
    /// SHA-256 of the serialized block file, distinct from the cell representation hash.
    pub file_hash: Int256,

    /// Serialized protocol payload. The enclosing constructor defines whether it contains a BoC or raw bytes.
    pub data: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `BlockHeader` LiteAPI answer. Values come from the server and require independent validation.
pub struct BlockHeader {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    #[tl(flags_bit = "mode.0")]
    /// Include the block state update.
    pub with_state_update: Option<()>,
    #[tl(flags_bit = "mode.1")]
    /// Include the block value flow.
    pub with_value_flow: Option<()>,
    #[tl(flags_bit = "mode.4")]
    /// Include the block extra data.
    pub with_extra: Option<()>,
    #[tl(flags_bit = "mode.5")]
    /// Include the shard hash tree.
    pub with_shard_hashes: Option<()>,
    #[tl(flags_bit = "mode.6")]
    /// Include previous-block signatures.
    pub with_prev_blk_signatures: Option<()>,

    /// Unverified serialized header proof; validate against the expected block identity.
    pub header_proof: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `SendMsgStatus` LiteAPI answer. Values come from the server and require independent validation.
pub struct SendMsgStatus {
    /// Server-reported message acceptance status; does not prove on-chain execution.
    pub status: u32,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `AccountState` LiteAPI answer. Values come from the server and require independent validation.
pub struct AccountState {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Shard block whose ancestry the returned proof claims to establish.
    pub shardblk: BlockIdExt,

    /// Unverified serialized proof binding a shard descriptor to the masterchain.
    pub shard_proof: Vec<u8>,

    /// Unverified serialized proof; validate against an independently trusted block or state.
    pub proof: Vec<u8>,

    /// Unverified serialized account or block state; validate its proof and expected root hash.
    pub state: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `RunMethodResult` LiteAPI answer. Values come from the server and require independent validation.
pub struct RunMethodResult {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Shard block whose ancestry the returned proof claims to establish.
    pub shardblk: BlockIdExt,
    #[tl(flags_bit = "mode.0")]

    /// Unverified serialized proof binding a shard descriptor to the masterchain.
    pub shard_proof: Option<Vec<u8>>,
    #[tl(flags_bit = "mode.0")]

    /// Unverified serialized proof; validate against an independently trusted block or state.
    pub proof: Option<Vec<u8>>,
    #[tl(flags_bit = "mode.1")]

    /// Unverified serialized proof of the claimed state root.
    pub state_proof: Option<Vec<u8>>,
    #[tl(flags_bit = "mode.3")]

    /// Optional BoC-encoded initial TVM c7 environment returned by the server.
    pub init_c7: Option<Vec<u8>>,
    #[tl(flags_bit = "mode.4")]

    /// Optional additional library data returned for TVM execution.
    pub lib_extras: Option<Vec<u8>>,
    /// Result of server-side TVM execution; success does not authenticate the account state.
    pub exit_code: i32,
    #[tl(flags_bit = "mode.2")]

    /// Server-returned method output or library entries; authenticate the source state before trusting them.
    pub result: Option<Vec<u8>>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `ShardInfo` LiteAPI answer. Values come from the server and require independent validation.
pub struct ShardInfo {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Shard block whose ancestry the returned proof claims to establish.
    pub shardblk: BlockIdExt,

    /// Unverified serialized proof binding a shard descriptor to the masterchain.
    pub shard_proof: Vec<u8>,

    /// Serialized shard descriptor whose association with the masterchain requires proof verification.
    pub shard_descr: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `AllShardsInfo` LiteAPI answer. Values come from the server and require independent validation.
pub struct AllShardsInfo {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,

    /// Unverified serialized proof; validate against an independently trusted block or state.
    pub proof: Vec<u8>,

    /// Serialized protocol payload. The enclosing constructor defines whether it contains a BoC or raw bytes.
    pub data: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `TransactionInfo` LiteAPI answer. Values come from the server and require independent validation.
pub struct TransactionInfo {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,

    /// Unverified serialized proof; validate against an independently trusted block or state.
    pub proof: Vec<u8>,

    /// Unverified transaction BoC; check its hash and inclusion proof.
    pub transaction: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `TransactionList` LiteAPI answer. Values come from the server and require independent validation.
pub struct TransactionList {
    /// Returned block or transaction identifiers; completeness requires validating the associated proof.
    pub ids: Vec<BlockIdExt>,

    /// Unverified transaction-list BoC; validate inclusion and pagination boundaries.
    pub transactions: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `TransactionMetadata` LiteAPI answer. Values come from the server and require independent validation.
pub struct TransactionMetadata {
    #[tl(flags)]
    mode: (),
    depth: u32,
    initiator: AccountId,
    initiator_lt: u64,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `TransactionId` LiteAPI answer. Values come from the server and require independent validation.
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
    #[tl(flags_bit = "mode.8")]
    /// Transaction origin metadata carried with a dispatch message.
    pub metadata: Option<TransactionMetadata>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `BlockTransactions` LiteAPI answer. Values come from the server and require independent validation.
pub struct BlockTransactions {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Number of transactions requested for this page.
    pub req_count: u32,
    /// Whether further records remain according to the server.
    pub incomplete: bool,
    /// Returned block or transaction identifiers; completeness requires validating the associated proof.
    pub ids: Vec<TransactionId>,

    /// Unverified serialized proof; validate against an independently trusted block or state.
    pub proof: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `BlockTransactionsExt` LiteAPI answer. Values come from the server and require independent validation.
pub struct BlockTransactionsExt {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Number of transactions requested for this page.
    pub req_count: u32,
    /// Whether further records remain according to the server.
    pub incomplete: bool,

    /// Unverified transaction-list BoC; validate inclusion and pagination boundaries.
    pub transactions: Vec<u8>,

    /// Unverified serialized proof; validate against an independently trusted block or state.
    pub proof: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `PartialBlockProof` LiteAPI answer. Values come from the server and require independent validation.
pub struct PartialBlockProof {
    /// Whether the server reports that the proof reaches the requested destination.
    pub complete: bool,
    /// Starting block identity of this proof link.
    pub from: BlockIdExt,
    /// Destination block identity of this proof link.
    pub to: BlockIdExt,
    /// Unverified forward or backward links connecting the stated block identities.
    pub steps: Vec<BlockLink>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `ConfigInfo` LiteAPI answer. Values come from the server and require independent validation.
pub struct ConfigInfo {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,

    /// Unverified serialized proof of the claimed state root.
    pub state_proof: Vec<u8>,

    /// Unverified serialized configuration proof used to authenticate validator or config data.
    pub config_proof: Vec<u8>,
    #[tl(flags_bit = "mode.0")]
    /// Include the state root proof.
    pub with_state_root: Option<()>,
    #[tl(flags_bit = "mode.1")]
    /// Include state library data.
    pub with_libraries: Option<()>,
    #[tl(flags_bit = "mode.2")]
    /// Include the state extra root.
    pub with_state_extra_root: Option<()>,
    #[tl(flags_bit = "mode.3")]
    /// Include the shard hash tree.
    pub with_shard_hashes: Option<()>,
    #[tl(flags_bit = "mode.4")]
    /// Include validator configuration.
    pub with_validator_set: Option<()>,
    #[tl(flags_bit = "mode.5")]
    /// Include special contract configuration.
    pub with_special_smc: Option<()>,
    #[tl(flags_bit = "mode.6")]
    /// Include the accounts dictionary root.
    pub with_accounts_root: Option<()>,
    #[tl(flags_bit = "mode.7")]
    /// Include previous-block references.
    pub with_prev_blocks: Option<()>,
    #[tl(flags_bit = "mode.8")]
    /// Include workchain configuration.
    pub with_workchain_info: Option<()>,
    #[tl(flags_bit = "mode.9")]
    /// Include capability configuration.
    pub with_capabilities: Option<()>,
    #[tl(flags_bit = "mode.15")]
    /// Read configuration from the key block instead of the current state.
    pub extract_from_key_block: Option<()>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `ValidatorStats` LiteAPI answer. Values come from the server and require independent validation.
pub struct ValidatorStats {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Requested or reported number of records, as defined by this constructor.
    pub count: u32,
    /// Whether the server reports that the proof reaches the requested destination.
    pub complete: bool,

    /// Unverified serialized proof of the claimed state root.
    pub state_proof: Vec<u8>,

    /// Unverified serialized proof binding the returned data to its claimed state.
    pub data_proof: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `LibraryResult` LiteAPI answer. Values come from the server and require independent validation.
pub struct LibraryResult {
    /// Server-returned method output or library entries; authenticate the source state before trusting them.
    pub result: Vec<LibraryEntry>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `LibraryResultWithProof` LiteAPI answer. Values come from the server and require independent validation.
pub struct LibraryResultWithProof {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Server-returned method output or library entries; authenticate the source state before trusting them.
    pub result: Vec<LibraryEntry>,

    /// Unverified serialized proof of the claimed state root.
    pub state_proof: Vec<u8>,

    /// Unverified serialized proof binding the returned data to its claimed state.
    pub data_proof: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `ShardBlockLink` LiteAPI answer. Values come from the server and require independent validation.
pub struct ShardBlockLink {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,

    /// Unverified serialized proof; validate against an independently trusted block or state.
    pub proof: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `ShardBlockProof` LiteAPI answer. Values come from the server and require independent validation.
pub struct ShardBlockProof {
    /// Masterchain block to which the returned shard proof claims to bind.
    pub masterchain_id: BlockIdExt,
    /// Unverified shard-to-masterchain links; validate the entire chain before trusting the target.
    pub links: Vec<ShardBlockLink>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `LookupBlockResult` LiteAPI answer. Values come from the server and require independent validation.
pub struct LookupBlockResult {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Masterchain block to which the returned shard data claims to bind.
    pub mc_block_id: BlockIdExt,

    /// Unverified masterchain state proof relative to the client-specified anchor.
    pub client_mc_state_proof: Vec<u8>,

    /// Unverified serialized proof binding this shard object to a masterchain block.
    pub mc_block_proof: Vec<u8>,
    /// Unverified shard links returned for block lookup.
    pub shard_links: Vec<ShardBlockLink>,

    /// Unverified serialized block header; validate its root against the expected block identity.
    pub header: Vec<u8>,

    /// Unverified header data used to bind the preceding block.
    pub prev_header: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `OutMsgQueueSize` LiteAPI answer. Values come from the server and require independent validation.
pub struct OutMsgQueueSize {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Server-reported queue size; authenticate the corresponding state before relying on it.
    pub size: u32,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `OutMsgQueueSizes` LiteAPI answer. Values come from the server and require independent validation.
pub struct OutMsgQueueSizes {
    /// Per-shard outgoing-queue sizes reported by the server.
    pub shards: Vec<OutMsgQueueSize>,
    /// Advertised limit for external messages queued by the validator group.
    pub ext_msg_queue_size_limit: u32,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `BlockOutMsgQueueSize` LiteAPI answer. Values come from the server and require independent validation.
pub struct BlockOutMsgQueueSize {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Server-reported queue size; authenticate the corresponding state before relying on it.
    pub size: u64,
    #[tl(flags_bit = "mode.0")]

    /// Unverified serialized proof; validate against an independently trusted block or state.
    pub proof: Option<Vec<u8>>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `AccountDispatchQueueInfo` LiteAPI answer. Values come from the server and require independent validation.
pub struct AccountDispatchQueueInfo {
    /// Account hash used to select a dispatch queue within the requested shard.
    pub addr: Int256,
    /// Server-reported queue size; authenticate the corresponding state before relying on it.
    pub size: u64,
    /// Lower logical-time boundary of this dispatch queue.
    pub min_lt: u64,
    /// Upper logical-time boundary of this dispatch queue.
    pub max_lt: u64,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `DispatchQueueInfo` LiteAPI answer. Values come from the server and require independent validation.
pub struct DispatchQueueInfo {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Dispatch-queue metadata for the accounts in this page.
    pub account_dispatch_queues: Vec<AccountDispatchQueueInfo>,
    /// Whether the server reports that the proof reaches the requested destination.
    pub complete: bool,
    #[tl(flags_bit = "mode.0")]

    /// Unverified serialized proof; validate against an independently trusted block or state.
    pub proof: Option<Vec<u8>>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `DispatchQueueMessage` LiteAPI answer. Values come from the server and require independent validation.
pub struct DispatchQueueMessage {
    /// Account hash used to select a dispatch queue within the requested shard.
    pub addr: Int256,
    /// Logical time used to identify or select a transaction; not wall-clock time.
    pub lt: u64,
    /// Representation hash associated with this record; verify its binding to the requested object.
    pub hash: Int256,
    /// Transaction origin metadata carried with a dispatch message.
    pub metadata: TransactionMetadata,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `DispatchQueueMessages` LiteAPI answer. Values come from the server and require independent validation.
pub struct DispatchQueueMessages {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Dispatch records in this page; serialized message cells may be requested separately.
    pub messages: Vec<DispatchQueueMessage>,
    /// Whether the server reports that the proof reaches the requested destination.
    pub complete: bool,
    #[tl(flags_bit = "mode.0")]

    /// Unverified serialized proof; validate against an independently trusted block or state.
    pub proof: Option<Vec<u8>>,
    #[tl(flags_bit = "mode.2")]

    /// Optional BoC with the dispatch message cells requested by the client.
    pub messages_boc: Option<Vec<u8>>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `NonfinalCandidate` LiteAPI answer. Values come from the server and require independent validation.
pub struct NonfinalCandidate {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: NonfinalCandidateId,

    /// Serialized protocol payload. The enclosing constructor defines whether it contains a BoC or raw bytes.
    pub data: Vec<u8>,

    /// Unverified collated candidate data used during validation.
    pub collated_data: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `NonfinalValidatorGroupInfo` LiteAPI answer. Values come from the server and require independent validation.
pub struct NonfinalValidatorGroupInfo {
    /// Expected identity of the following block before it is finalized.
    pub next_block_id: BlockId,
    /// Consensus sequence number supplied by the server; compare with trusted validator metadata.
    pub cc_seqno: i32,
    /// Previous block identities used by a nonfinal candidate.
    pub prev: Vec<BlockIdExt>,
    /// Nonfinal candidates advertised for this shard; none establishes block finality by itself.
    pub candidates: Vec<NonfinalCandidateInfo>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `NonfinalValidatorGroups` LiteAPI answer. Values come from the server and require independent validation.
pub struct NonfinalValidatorGroups {
    /// Validator-group records for nonfinal block production.
    pub groups: Vec<NonfinalValidatorGroupInfo>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `NonfinalPendingShardBlocks` LiteAPI answer. Values come from the server and require independent validation.
pub struct NonfinalPendingShardBlocks {
    /// Nonfinal blocks reported as signed by the validator group.
    pub signed_blocks: Vec<BlockIdExt>,
    /// Nonfinal candidates advertised for this shard; none establishes block finality by itself.
    pub candidates: Vec<BlockIdExt>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Payload of the `Error` LiteAPI answer. Values come from the server and require independent validation.
pub struct Error {
    /// LiteAPI error code preserved for caller-specific handling.
    pub code: i32,

    /// Server-provided diagnostic text; TL strings may contain non-UTF-8 bytes.
    pub message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "code={}, message={}", self.code, self.message)
    }
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
#[tl(boxed)]
/// Known boxed LiteAPI answers, including server errors. Payloads are unverified until the caller validates them.
pub enum Response {
    /// liteServer.masterchainInfo last:tonNode.blockIdExt state_root_hash:int256 init:tonNode.zeroStateIdExt = liteServer.MasterchainInfo;
    #[tl(id = 0x85832881)]
    MasterchainInfo(MasterchainInfo),

    /// liteServer.masterchainInfoExt mode:# version:int capabilities:long last:tonNode.blockIdExt last_utime:int now:int state_root_hash:int256 init:tonNode.zeroStateIdExt = liteServer.MasterchainInfoExt;
    #[tl(id = 0xa8cce0f5)]
    MasterchainInfoExt(MasterchainInfoExt),

    /// liteServer.currentTime now:int = liteServer.CurrentTime;
    #[tl(id = 0xe953000d)]
    CurrentTime(CurrentTime),

    /// liteServer.version mode:# version:int capabilities:long now:int = liteServer.Version;
    #[tl(id = 0x5a0491e5)]
    Version(Version),

    /// liteServer.blockData id:tonNode.blockIdExt data:bytes = liteServer.BlockData;
    #[tl(id = 0xa574ed6c)]
    BlockData(BlockData),

    /// liteServer.blockState id:tonNode.blockIdExt root_hash:int256 file_hash:int256 data:bytes = liteServer.BlockState;
    #[tl(id = 0xabaddc0c)]
    BlockState(BlockState),

    /// liteServer.blockHeader id:tonNode.blockIdExt mode:# header_proof:bytes = liteServer.BlockHeader;
    #[tl(id = 0x752d8219)]
    BlockHeader(BlockHeader),

    /// liteServer.sendMsgStatus status:int = liteServer.SendMsgStatus;
    #[tl(id = 0x3950e597)]
    SendMsgStatus(SendMsgStatus),

    /// liteServer.accountState id:tonNode.blockIdExt shardblk:tonNode.blockIdExt shard_proof:bytes proof:bytes state:bytes = liteServer.AccountState;
    #[tl(id = 0x7079c751)]
    AccountState(AccountState),

    /// liteServer.runMethodResult mode:# id:tonNode.blockIdExt shardblk:tonNode.blockIdExt shard_proof:mode.0?bytes proof:mode.0?bytes state_proof:mode.1?bytes init_c7:mode.3?bytes lib_extras:mode.4?bytes exit_code:int result:mode.2?bytes = liteServer.RunMethodResult;
    #[tl(id = 0xa39a616b)]
    RunMethodResult(RunMethodResult),

    /// liteServer.shardInfo id:tonNode.blockIdExt shardblk:tonNode.blockIdExt shard_proof:bytes shard_descr:bytes = liteServer.ShardInfo;
    #[tl(id = 0x9fe6cd84)]
    ShardInfo(ShardInfo),

    /// liteServer.allShardsInfo id:tonNode.blockIdExt proof:bytes data:bytes = liteServer.AllShardsInfo;
    #[tl(id = 0x098fe72d)]
    AllShardsInfo(AllShardsInfo),

    /// liteServer.transactionInfo id:tonNode.blockIdExt proof:bytes transaction:bytes = liteServer.TransactionInfo;
    #[tl(id = 0x0edeed47)]
    TransactionInfo(TransactionInfo),

    /// liteServer.transactionList ids:(vector tonNode.blockIdExt) transactions:bytes = liteServer.TransactionList;
    #[tl(id = 0x6f26c60b)]
    TransactionList(TransactionList),

    /// liteServer.transactionId mode:# account:mode.0?int256 lt:mode.1?long hash:mode.2?int256 metadata:mode.8?liteServer.transactionMetadata = liteServer.TransactionId;
    #[tl(id = 0xab101c41)]
    TransactionId(TransactionId),

    /// liteServer.blockTransactions id:tonNode.blockIdExt req_count:# incomplete:Bool ids:(vector liteServer.transactionId) proof:bytes = liteServer.BlockTransactions;
    #[tl(id = 0xbd8cad2b)]
    BlockTransactions(BlockTransactions),

    /// liteServer.blockTransactionsExt id:tonNode.blockIdExt req_count:# incomplete:Bool transactions:bytes proof:bytes = liteServer.BlockTransactionsExt;
    #[tl(id = 0xfb8ffce4)]
    BlockTransactionsExt(BlockTransactionsExt),

    /// liteServer.partialBlockProof complete:Bool from:tonNode.blockIdExt to:tonNode.blockIdExt steps:(vector liteServer.BlockLink) = liteServer.PartialBlockProof;
    #[tl(id = 0x8ed0d2c1)]
    PartialBlockProof(PartialBlockProof),

    /// liteServer.configInfo mode:# id:tonNode.blockIdExt state_proof:bytes config_proof:bytes = liteServer.ConfigInfo;
    #[tl(id = 0xae7b272f)]
    ConfigInfo(ConfigInfo),

    /// liteServer.validatorStats mode:# id:tonNode.blockIdExt count:int complete:Bool state_proof:bytes data_proof:bytes = liteServer.ValidatorStats;
    #[tl(id = 0xb9f796d8)]
    ValidatorStats(ValidatorStats),

    /// liteServer.libraryResult result:(vector liteServer.libraryEntry) = liteServer.LibraryResult;
    #[tl(id = 0x117ab96b)]
    LibraryResult(LibraryResult),

    /// liteServer.libraryResult result:(vector liteServer.libraryEntry) = liteServer.LibraryResult;
    #[tl(id = 0x10a927bf)]
    LibraryResultWithProof(LibraryResultWithProof),

    /// liteServer.shardBlockProof masterchain_id:tonNode.blockIdExt links:(vector liteServer.shardBlockLink) = liteServer.ShardBlockProof;
    #[tl(id = 0x1d62a07a)]
    ShardBlockProof(ShardBlockProof),

    /// liteServer.lookupBlockResult id:tonNode.blockIdExt mode:# mc_block_id:tonNode.blockIdExt client_mc_state_proof:bytes mc_block_proof:bytes shard_links:(vector liteServer.shardBlockLink) header:bytes prev_header:bytes = liteServer.LookupBlockResult;
    #[tl(id = 0x99786be7)]
    LookupBlockResult(LookupBlockResult),

    /// liteServer.outMsgQueueSizes shards:(vector liteServer.outMsgQueueSize) ext_msg_queue_size_limit:int = liteServer.OutMsgQueueSizes;
    #[tl(id = 0xf8504a03)]
    OutMsgQueueSizes(OutMsgQueueSizes),

    /// liteServer.blockOutMsgQueueSize mode:# id:tonNode.blockIdExt size:long proof:mode.0?bytes = liteServer.BlockOutMsgQueueSize;
    #[tl(id = 0x8acdbe1b)]
    BlockOutMsgQueueSize(BlockOutMsgQueueSize),

    /// liteServer.dispatchQueueInfo mode:# id:tonNode.blockIdExt account_dispatch_queues:(vector liteServer.accountDispatchQueueInfo) complete:Bool proof:mode.0?bytes = liteServer.DispatchQueueInfo;
    #[tl(id = 0x5d1132d0)]
    DispatchQueueInfo(DispatchQueueInfo),

    /// liteServer.dispatchQueueMessages mode:# id:tonNode.blockIdExt messages:(vector liteServer.dispatchQueueMessage) complete:Bool proof:mode.0?bytes messages_boc:mode.2?bytes = liteServer.DispatchQueueMessages;
    #[tl(id = 0x4b407931)]
    DispatchQueueMessages(DispatchQueueMessages),

    /// liteServer.nonfinal.candidate id:liteServer.nonfinal.candidateId data:bytes collated_data:bytes = liteServer.nonfinal.Candidate;
    #[tl(id = 0x80c3468c)]
    NonfinalCandidate(NonfinalCandidate),

    /// liteServer.nonfinal.validatorGroups groups:(vector liteServer.nonfinal.validatorGroupInfo) = liteServer.nonfinal.ValidatorGroups;
    #[tl(id = 0x8d0b9dfe)]
    NonfinalValidatorGroups(NonfinalValidatorGroups),

    /// liteServer.nonfinal.pendingShardBlocks signed_blocks:(vector tonNode.blockIdExt) candidates:(vector tonNode.blockIdExt) = liteServer.nonfinal.PendingShardBlocks;
    #[tl(id = 0x1fd06e4d)]
    NonfinalPendingShardBlocks(NonfinalPendingShardBlocks),

    /// liteServer.error code:int message:string = liteServer.Error;
    #[tl(id = 0xbba9e148)]
    Error(Error),
}
