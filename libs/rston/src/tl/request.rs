//! LiteAPI query parameters. Optional flag fields control wire serialization.

use tl_proto::{TlRead, TlWrite};

use super::common::*;
use super::utils::*;

/// liteServer.query data:bytes = Object;
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
#[tl(
    boxed,
    id = "liteServer.query",
    scheme_inline = r##"liteServer.query data:bytes = Object;"##
)]
pub struct LiteQuery {
    #[tl(with = "struct_as_bytes")]
    /// Typed request encoded inside the outer TL bytes field.
    pub wrapped_request: WrappedRequest,
}

/// liteServer.query data:bytes = Object;
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
#[tl(
    boxed,
    id = "liteServer.query",
    scheme_inline = r##"liteServer.query data:bytes = Object;"##
)]
pub struct LiteQueryRaw {
    /// Serialized protocol payload. The enclosing constructor defines whether it contains a BoC or raw bytes.
    pub data: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// A typed query with an optional sequence wait prefix, embedded in `liteServer.query`.
pub struct WrappedRequest {
    #[tl(read_with = "lossy_read")]
    /// Optional server-side wait prefix applied before executing this request.
    pub wait_masterchain_seqno: Option<WaitMasterchainSeqno>,
    /// Request payload following the optional sequence wait; raw bytes retain their original constructor.
    pub request: Request,
}

#[derive(Debug, Clone, PartialEq)]
/// An opaque boxed query with an optional sequence wait prefix; unknown constructors are preserved.
pub struct RawWrappedRequest {
    /// Optional server-side wait prefix applied before executing this request.
    pub wait_masterchain_seqno: Option<WaitMasterchainSeqno>,
    /// Request payload following the optional sequence wait; raw bytes retain their original constructor.
    pub request: Vec<u8>,
}

impl RawWrappedRequest {
    /// Prefix the opaque query bytes with the optional sequence wait and encode the outer TL bytes wrapper.
    pub fn into_lite_query(self) -> LiteQueryRaw {
        let mut data = Vec::new();
        if let Some(wait_masterchain_seqno) = self.wait_masterchain_seqno {
            data.extend(tl_proto::serialize(wait_masterchain_seqno));
        }
        data.extend(self.request);
        LiteQueryRaw { data }
    }
}

/// liteServer.waitMasterchainSeqno seqno:int timeout_ms:int = Object;
#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
#[tl(
    boxed,
    id = "liteServer.waitMasterchainSeqno",
    scheme_inline = r##"liteServer.waitMasterchainSeqno seqno:int timeout_ms:int = Object;"##
)]
pub struct WaitMasterchainSeqno {
    /// Block sequence number or sequence-number selection flag, as defined by this constructor.
    pub seqno: u32,
    /// Server-side wait bound in milliseconds, separate from the client request deadline.
    pub timeout_ms: u32,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetMasterchainInfoExt` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetMasterchainInfoExt {
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: u32,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetBlock` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetBlock {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetState` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetState {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetBlockHeader` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetBlockHeader {
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
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `SendMessage` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct SendMessage {
    /// Serialized external message BoC to broadcast. Sending does not confirm inclusion in a block.
    pub body: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetAccountState` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetAccountState {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Account identifier; transaction cursors use the account hash without a workchain.
    pub account: AccountId,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `RunSmcMethod` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct RunSmcMethod {
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: u32,
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Account identifier; transaction cursors use the account hash without a workchain.
    pub account: AccountId,
    /// TVM method identifier supplied by the caller.
    pub method_id: u64,
    /// BoC-encoded TVM input stack for server-side method execution.
    pub params: Vec<u8>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetShardInfo` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetShardInfo {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Signed workchain identifier; -1 denotes the masterchain.
    pub workchain: i32,
    /// Shard identifier with its prefix marker, not an account address.
    pub shard: u64,
    /// Require the exact shard identifier instead of a containing shard.
    pub exact: bool,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetAllShardsInfo` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetAllShardsInfo {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetOneTransaction` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetOneTransaction {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Account identifier; transaction cursors use the account hash without a workchain.
    pub account: AccountId,
    /// Logical time used to identify or select a transaction; not wall-clock time.
    pub lt: u64,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetTransactions` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetTransactions {
    /// Requested or reported number of records, as defined by this constructor.
    pub count: u32,
    /// Account identifier; transaction cursors use the account hash without a workchain.
    pub account: AccountId,
    /// Logical time used to identify or select a transaction; not wall-clock time.
    pub lt: u64,
    /// Representation hash associated with this record; verify its binding to the requested object.
    pub hash: Int256,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `LookupBlock` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct LookupBlock {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockId,
    #[tl(flags_bit = "mode.0")]
    /// Block sequence number or sequence-number selection flag, as defined by this constructor.
    pub seqno: Option<()>,
    #[tl(flags_bit = "mode.1")]
    /// Logical time used to identify or select a transaction; not wall-clock time.
    pub lt: Option<u64>,
    #[tl(flags_bit = "mode.2")]
    /// Optional Unix-time selection value in seconds.
    pub utime: Option<u32>,
    #[tl(flags_bit = "mode.4")]
    /// Include the block state update.
    pub with_state_update: Option<()>,
    #[tl(flags_bit = "mode.5")]
    /// Include the block value flow.
    pub with_value_flow: Option<()>,
    #[tl(flags_bit = "mode.8")]
    /// Include the block extra data.
    pub with_extra: Option<()>,
    #[tl(flags_bit = "mode.9")]
    /// Include the shard hash tree.
    pub with_shard_hashes: Option<()>,
    #[tl(flags_bit = "mode.10")]
    /// Include previous-block signatures.
    pub with_prev_blk_signatures: Option<()>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `LookupBlockWithProof` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct LookupBlockWithProof {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockId,
    /// Masterchain block to which the returned shard data claims to bind.
    pub mc_block_id: BlockIdExt,
    #[tl(flags_bit = "mode.1")]
    /// Logical time used to identify or select a transaction; not wall-clock time.
    pub lt: Option<u64>,
    #[tl(flags_bit = "mode.2")]
    /// Optional Unix-time selection value in seconds.
    pub utime: Option<u32>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `ListBlockTransactions` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct ListBlockTransactions {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Requested or reported number of records, as defined by this constructor.
    pub count: u32,
    #[tl(flags_bit = "mode.7")]
    /// Exclusive transaction cursor for pagination.
    pub after: Option<TransactionId3>,
    #[tl(flags_bit = "mode.6")]
    /// Request descending transaction traversal.
    pub reverse_order: Option<()>,
    #[tl(flags_bit = "mode.5")]
    /// Request the proof data defined for this constructor.
    pub want_proof: Option<()>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetBlockProof` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetBlockProof {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Trusted starting block identity requested for the proof chain.
    pub known_block: BlockIdExt,
    #[tl(flags_bit = "mode.0")]
    /// Optional destination of the requested proof chain.
    pub target_block: Option<BlockIdExt>,
    #[tl(flags_bit = "mode.1")]
    /// Permit a weaker target as defined by the LiteAPI block-proof mode.
    pub allow_weak_target: Option<()>,
    #[tl(flags_bit = "mode.12")]
    /// Use the requested base block when building the returned lookup proof.
    pub base_block_from_request: Option<()>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetConfigAll` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetConfigAll {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
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
/// Parameters for the `GetConfigParams` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetConfigParams {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Requested configuration parameter indices; negative indices retain their protocol meaning.
    pub param_list: Vec<i32>,
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
/// Parameters for the `GetValidatorStats` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetValidatorStats {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Upper bound on returned library records.
    pub limit: u32,
    #[tl(flags_bit = "mode.0")]
    /// Exclusive validator-statistics pagination cursor.
    pub start_after: Option<Int256>,
    #[tl(flags_bit = "mode.2")]
    /// Optional Unix-time filter for validator statistics.
    pub modified_after: Option<u32>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetLibraries` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetLibraries {
    /// Representation hashes of requested library cells.
    pub library_list: Vec<Int256>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetLibrariesWithProof` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetLibrariesWithProof {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Representation hashes of requested library cells.
    pub library_list: Vec<Int256>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetShardBlockProof` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetShardBlockProof {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetOutMsgQueueSizes` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetOutMsgQueueSizes {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    #[tl(flags_bit = "mode.0")]
    /// Optional signed workchain filter; -1 denotes the masterchain.
    pub wc: Option<i32>,
    #[tl(flags_bit = "mode.0")]
    /// Shard identifier with its prefix marker, not an account address.
    pub shard: Option<u64>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetBlockOutMsgQueueSize` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetBlockOutMsgQueueSize {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    #[tl(flags_bit = "mode.0")]
    /// Request the proof data defined for this constructor.
    pub want_proof: Option<()>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetDispatchQueueInfo` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetDispatchQueueInfo {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    #[tl(flags_bit = "mode.0")]
    /// Request the proof data defined for this constructor.
    pub want_proof: Option<()>,
    #[tl(flags_bit = "mode.1")]
    /// Exclusive account cursor for dispatch-queue pagination.
    pub after_addr: Option<Int256>,
    /// Upper bound on accounts in the returned dispatch-queue page.
    pub max_accounts: u32,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `GetDispatchQueueMessages` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct GetDispatchQueueMessages {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: BlockIdExt,
    /// Account hash used to select a dispatch queue within the requested shard.
    pub addr: Int256,
    /// Exclusive logical-time cursor for dispatch-message pagination.
    pub after_lt: u64,
    /// Upper bound on messages in the returned dispatch-queue page.
    pub max_messages: u32,
    #[tl(flags_bit = "mode.0")]
    /// Request the proof data defined for this constructor.
    pub want_proof: Option<()>,
    #[tl(flags_bit = "mode.1")]
    /// Limit dispatch-message traversal to one account.
    pub one_account: Option<()>,
    #[tl(flags_bit = "mode.2")]
    /// Request serialized message cells in addition to dispatch metadata.
    pub message_boc: Option<()>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `NonfinalGetValidatorGroups` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct NonfinalGetValidatorGroups {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    #[tl(flags_bit = "mode.0")]
    /// Optional signed workchain filter; -1 denotes the masterchain.
    pub wc: Option<i32>,
    #[tl(flags_bit = "mode.0")]
    /// Shard identifier with its prefix marker, not an account address.
    pub shard: Option<u64>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `NonfinalGetCandidate` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct NonfinalGetCandidate {
    /// Protocol object identifier. A returned identifier must match the requested object before proof verification.
    pub id: NonfinalCandidateId,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
/// Parameters for the `NonfinalGetPendingShardBlocks` LiteAPI operation. Optional fields select wire flags; the server executes the query.
pub struct NonfinalGetPendingShardBlocks {
    #[tl(flags)]
    /// Protocol mode bits. For unit-typed flags, the serializer derives these from optional fields.
    pub mode: (),
    #[tl(flags_bit = "mode.0")]
    /// Optional signed workchain filter; -1 denotes the masterchain.
    pub wc: Option<i32>,
    #[tl(flags_bit = "mode.0")]
    /// Shard identifier with its prefix marker, not an account address.
    pub shard: Option<u64>,
}

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
#[tl(boxed)]
/// Known boxed LiteAPI operations. Parameters do not perform local validation or proof checking.
pub enum Request {
    /// liteServer.getMasterchainInfo = liteServer.MasterchainInfo;
    #[tl(id = 0x89b5e62e)]
    GetMasterchainInfo,

    /// liteServer.getMasterchainInfoExt mode:# = liteServer.MasterchainInfoExt;
    #[tl(id = 0x70a671df)]
    GetMasterchainInfoExt(GetMasterchainInfoExt),

    /// liteServer.getTime = liteServer.CurrentTime;
    #[tl(id = 0x16ad5a34)]
    GetTime,

    /// liteServer.getVersion = liteServer.Version;
    #[tl(id = 0x232b940b)]
    GetVersion,

    /// liteServer.getBlock id:tonNode.blockIdExt = liteServer.BlockData;
    #[tl(id = 0x6377cf0d)]
    GetBlock(GetBlock),

    /// liteServer.getState id:tonNode.blockIdExt = liteServer.BlockState;
    #[tl(id = 0xba6e2eb6)]
    GetState(GetState),

    /// liteServer.getBlockHeader id:tonNode.blockIdExt mode:# = liteServer.BlockHeader;
    #[tl(id = 0x21ec069e)]
    GetBlockHeader(GetBlockHeader),

    /// liteServer.sendMessage body:bytes = liteServer.SendMsgStatus;
    #[tl(id = 0x690ad482)]
    SendMessage(SendMessage),

    /// liteServer.getAccountState id:tonNode.blockIdExt account:liteServer.accountId = liteServer.AccountState;
    #[tl(id = 0x6b890e25)]
    GetAccountState(GetAccountState),

    /// liteServer.getAccountStatePrunned id:tonNode.blockIdExt account:liteServer.accountId = liteServer.AccountState;
    #[tl(id = 0x5a698507)]
    GetAccountStatePrunned(GetAccountState),

    /// liteServer.runSmcMethod mode:# id:tonNode.blockIdExt account:liteServer.accountId method_id:long params:bytes = liteServer.RunMethodResult;
    #[tl(id = 0x5cc65dd2)]
    RunSmcMethod(RunSmcMethod),

    /// liteServer.getShardInfo id:tonNode.blockIdExt workchain:int shard:long exact:Bool = liteServer.ShardInfo;
    #[tl(id = 0x46a2f425)]
    GetShardInfo(GetShardInfo),

    /// liteServer.getAllShardsInfo id:tonNode.blockIdExt = liteServer.AllShardsInfo;
    #[tl(id = 0x74d3fd6b)]
    GetAllShardsInfo(GetAllShardsInfo),

    /// liteServer.getOneTransaction id:tonNode.blockIdExt account:liteServer.accountId lt:long = liteServer.TransactionInfo;
    #[tl(id = 0xd40f24ea)]
    GetOneTransaction(GetOneTransaction),

    /// liteServer.getTransactions count:# account:liteServer.accountId lt:long hash:int256 = liteServer.TransactionList;
    #[tl(id = 0x1c40e7a1)]
    GetTransactions(GetTransactions),

    /// liteServer.lookupBlock mode:# id:tonNode.blockId lt:mode.1?long utime:mode.2?int = liteServer.BlockHeader;
    #[tl(id = 0xfac8f71e)]
    LookupBlock(LookupBlock),

    /// liteServer.lookupBlockWithProof mode:# id:tonNode.blockId mc_block_id:tonNode.blockIdExt lt:mode.1?long utime:mode.2?int = liteServer.LookupBlockResult;
    #[tl(id = 0x9c045ff8)]
    LookupBlockWithProof(LookupBlockWithProof),

    /// liteServer.listBlockTransactions id:tonNode.blockIdExt mode:# count:# after:mode.7?liteServer.transactionId3 reverse_order:mode.6?true want_proof:mode.5?true = liteServer.BlockTransactions;
    #[tl(id = 0xadfcc7da)]
    ListBlockTransactions(ListBlockTransactions),

    /// liteServer.listBlockTransactionsExt id:tonNode.blockIdExt mode:# count:# after:mode.7?liteServer.transactionId3 reverse_order:mode.6?true want_proof:mode.5?true = liteServer.BlockTransactionsExt;
    #[tl(id = 0x0079dd5c)]
    ListBlockTransactionsExt(ListBlockTransactions),

    /// liteServer.getBlockProof mode:# known_block:tonNode.blockIdExt target_block:mode.0?tonNode.blockIdExt = liteServer.PartialBlockProof;
    #[tl(id = 0x8aea9c44)]
    GetBlockProof(GetBlockProof),

    /// liteServer.getConfigAll mode:# id:tonNode.blockIdExt = liteServer.ConfigInfo;
    #[tl(id = 0x911b26b7)]
    GetConfigAll(GetConfigAll),

    /// liteServer.getConfigParams mode:# id:tonNode.blockIdExt param_list:(vector int) = liteServer.ConfigInfo;
    #[tl(id = 0x2a111c19)]
    GetConfigParams(GetConfigParams),

    /// liteServer.getValidatorStats#091a58bc mode:# id:tonNode.blockIdExt limit:int start_after:mode.0?int256 modified_after:mode.2?int = liteServer.ValidatorStats;
    #[tl(id = 0x091a58bc)]
    GetValidatorStats(GetValidatorStats),

    /// liteServer.getLibraries library_list:(vector int256) = liteServer.LibraryResult;
    #[tl(id = 0xd122b662)]
    GetLibraries(GetLibraries),

    /// liteServer.getLibrariesWithProof id:tonNode.blockIdExt mode:# library_list:(vector int256) = liteServer.LibraryResultWithProof;
    #[tl(id = 0xd97693bd)]
    GetLibrariesWithProof(GetLibrariesWithProof),

    /// liteServer.getShardBlockProof id:tonNode.blockIdExt = liteServer.ShardBlockProof;
    #[tl(id = 0x4ca60350)]
    GetShardBlockProof(GetShardBlockProof),

    /// liteServer.getOutMsgQueueSizes mode:# wc:mode.0?int shard:mode.0?long = liteServer.OutMsgQueueSizes;
    #[tl(id = 0x7bc19c36)]
    GetOutMsgQueueSizes(GetOutMsgQueueSizes),

    /// liteServer.getBlockOutMsgQueueSize mode:# id:tonNode.blockIdExt want_proof:mode.0?true = liteServer.BlockOutMsgQueueSize;
    #[tl(id = 0x8f6c7779)]
    GetBlockOutMsgQueueSize(GetBlockOutMsgQueueSize),

    /// liteServer.getDispatchQueueInfo mode:# id:tonNode.blockIdExt after_addr:mode.1?int256 max_accounts:int want_proof:mode.0?true = liteServer.DispatchQueueInfo;
    #[tl(id = 0x01e66bf3)]
    GetDispatchQueueInfo(GetDispatchQueueInfo),

    /// liteServer.getDispatchQueueMessages mode:# id:tonNode.blockIdExt addr:int256 after_lt:long max_messages:int want_proof:mode.0?true one_account:mode.1?true messages_boc:mode.2?true = liteServer.DispatchQueueMessages;
    #[tl(id = 0xbbfd6439)]
    GetDispatchQueueMessages(GetDispatchQueueMessages),

    /// liteServer.nonfinal.getValidatorGroups mode:# wc:mode.0?int shard:mode.0?long = liteServer.nonfinal.ValidatorGroups;
    #[tl(id = 0xa59915e3)]
    NonfinalGetValidatorGroups(NonfinalGetValidatorGroups),

    /// liteServer.nonfinal.getCandidate id:liteServer.nonfinal.candidateId = liteServer.nonfinal.Candidate;
    #[tl(id = 0x300794de)]
    NonfinalGetCandidate(NonfinalGetCandidate),

    /// liteServer.nonfinal.getPendingShardBlocks mode:# wc:mode.0?int shard:mode.0?long = liteServer.nonfinal.PendingShardBlocks;
    #[tl(id = 0x5a8ee82c)]
    NonfinalGetPendingShardBlocks(NonfinalGetPendingShardBlocks),
}
