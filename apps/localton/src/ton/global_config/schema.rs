//! OpenAPI descriptions for Localton's global-config HTTP responses.
//! Protocol parsing and serialization belong to `rston::global_config`.

use utoipa::ToSchema;

use super::super::tools::types::{DhtNodeDescriptor, Ed25519PublicKey, TonBlockHash};

#[derive(ToSchema)]
pub(super) struct GlobalConfig {
    #[serde(rename = "@type")]
    constructor: GlobalConfigConstructor,
    dht: DhtConfig,
    liteservers: Vec<LiteserverConfig>,
    validator: ValidatorConfig,
}

#[derive(ToSchema)]
enum GlobalConfigConstructor {
    #[serde(rename = "config.global")]
    Global,
}

#[derive(ToSchema)]
struct DhtConfig {
    #[serde(rename = "@type")]
    constructor: DhtConfigConstructor,
    k: i32,
    a: i32,
    network_id: Option<i32>,
    static_nodes: DhtNodes,
}

#[derive(ToSchema)]
enum DhtConfigConstructor {
    #[serde(rename = "dht.config.global")]
    Global,
}

#[derive(ToSchema)]
struct DhtNodes {
    #[serde(rename = "@type")]
    constructor: DhtNodesConstructor,
    nodes: Vec<DhtNodeDescriptor>,
}

#[derive(ToSchema)]
enum DhtNodesConstructor {
    #[serde(rename = "dht.nodes")]
    Nodes,
}

#[derive(ToSchema)]
struct LiteserverConfig {
    id: Ed25519PublicKey,
    ip: i32,
    port: u16,
}

#[derive(ToSchema)]
struct ValidatorConfig {
    #[serde(rename = "@type")]
    constructor: ValidatorConfigConstructor,
    zero_state: BlockIdExt,
    init_block: Option<BlockIdExt>,
    hardforks: Vec<BlockIdExt>,
}

#[derive(ToSchema)]
enum ValidatorConfigConstructor {
    #[serde(rename = "validator.config.global")]
    Global,
}

#[derive(ToSchema)]
struct BlockIdExt {
    #[serde(rename = "@type")]
    constructor: BlockIdExtConstructor,
    workchain: i32,
    shard: i64,
    seqno: u32,
    root_hash: TonBlockHash,
    file_hash: TonBlockHash,
}

#[derive(ToSchema)]
enum BlockIdExtConstructor {
    #[serde(rename = "ton.blockIdExt")]
    BlockIdExt,
}
