//! Typed TON global configuration used by bootstrap, join, and HTTP discovery.
//!
//! The shared protocol model lives in `rston::global_config`. Localton owns file
//! persistence, node-join requirements, address policy, and fork operations.

use std::{
    fs,
    net::Ipv4Addr,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use rston::{
    cell::HashBytes,
    global_config::{
        ConfigDhtNodes, ConfigLiteServer, ConfigPublicKey, DhtConfig,
        GlobalConfig as NetworkGlobalConfig, ValidatorConfig,
    },
    models::{BlockId, ShardIdent},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::storage::write_json_atomic;

use super::tools::types::{
    DhtNodeDescriptor, TonBlockHash, TonPublicKey, ZeroStateId, is_public_ipv4,
};

#[allow(dead_code)]
mod schema;

/// An on-disk global config that was parsed and accepted for joining a TON node.
///
/// Official binaries consume a filename, not a Rust value. This type keeps that
/// filename while proving once, before any child starts, that the file matches
/// the complete typed schema and contains at least one DHT entry point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GlobalConfigFile {
    path: PathBuf,
}

impl GlobalConfigFile {
    /// Opens the final network config used by persistent TON processes.
    pub(crate) fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        GlobalConfig::load(&path)?.validate_for_node_join()?;
        Ok(Self { path })
    }

    /// Returns the already validated file expected by official TON binaries.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Complete `global.config.json` document consumed by TON nodes and clients.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NetworkGlobalConfig", into = "NetworkGlobalConfig")]
pub(crate) struct GlobalConfig(NetworkGlobalConfig);

impl GlobalConfig {
    /// Reads one typed global config while preserving the source path in diagnostics.
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read global config {}", path.display()))?;
        Self::from_json_bytes(&bytes)
            .with_context(|| format!("invalid global config {}", path.display()))
    }

    /// Atomically persists a typed config so readers never observe a partial JSON file.
    pub(crate) fn save_atomic(&self, path: &Path) -> Result<()> {
        write_json_atomic(path, self)
            .with_context(|| format!("failed to save global config {}", path.display()))
    }

    /// Returns authenticated liteserver endpoints in their configured priority order.
    pub(crate) fn liteserver_endpoints(
        &self,
    ) -> impl ExactSizeIterator<Item = (Ipv4Addr, u16, TonPublicKey)> + '_ {
        self.0.liteservers.iter().map(|liteserver| {
            (
                Ipv4Addr::from(liteserver.ip as u32),
                liteserver.port,
                TonPublicKey::from_bytes(*liteserver.public_key()),
            )
        })
    }

    /// Builds the single-liteserver config that defines a fresh Localton network.
    ///
    /// The zerostate is also the initial block until validator-engine produces the
    /// first masterchain block. DHT nodes can be empty only for the preliminary file
    /// used to initialize the DHT database during genesis bootstrap.
    pub(crate) fn local(
        zero_state: ZeroStateId,
        dht_nodes: Vec<DhtNodeDescriptor>,
        liteserver_ip: Ipv4Addr,
        liteserver_port: u16,
        liteserver_public_key: TonPublicKey,
    ) -> Self {
        let zero_state = BlockId {
            shard: ShardIdent::MASTERCHAIN,
            seqno: 0,
            root_hash: HashBytes(*zero_state.root_hash().as_bytes()),
            file_hash: HashBytes(*zero_state.file_hash().as_bytes()),
        };
        Self(NetworkGlobalConfig {
            dht: Some(DhtConfig {
                constructor: Default::default(),
                k: 3,
                a: 3,
                network_id: None,
                static_nodes: ConfigDhtNodes {
                    nodes: dht_nodes
                        .into_iter()
                        .map(DhtNodeDescriptor::into_global_config_node)
                        .collect(),
                    ..Default::default()
                },
            }),
            liteservers: vec![liteserver(
                liteserver_ip,
                liteserver_port,
                liteserver_public_key,
            )],
            validator: Some(ValidatorConfig {
                zero_state,
                init_block: Some(zero_state),
                hardforks: Vec::new(),
            }),
            ..Default::default()
        })
    }

    /// Parses and fully validates a downloaded global config.
    pub(crate) fn from_json_bytes(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).context("global config does not match the TON JSON schema")
    }

    /// Checks the network entry points required by the complete join workflow.
    ///
    /// Validator-engine discovers peers through DHT, while Localton compares its
    /// node with an authenticated upstream liteserver before publishing readiness.
    pub(crate) fn validate_for_node_join(&self) -> Result<()> {
        ensure!(
            !self.0.dht.as_ref().unwrap().static_nodes.nodes.is_empty(),
            "global config has no DHT entry points"
        );
        ensure!(
            !self.0.liteservers.is_empty(),
            "global config has no liteserver endpoints"
        );
        Ok(())
    }

    /// Rejects a host address that public TON overlay peers cannot reach.
    ///
    /// DHT queries may succeed from behind NAT even when full-node replies are sent
    /// to an advertised loopback or private address. Failing before validator-engine
    /// starts avoids an indefinite `process.initial_sync` proof-download loop.
    pub(crate) fn validate_advertise_ip(&self, advertise_ip: Ipv4Addr) -> Result<()> {
        let public_network = self
            .0
            .dht
            .as_ref()
            .unwrap()
            .static_nodes
            .nodes
            .iter()
            .flat_map(|node| &node.addr_list.addrs)
            .any(|address| is_public_ipv4(*address.socket_addr().ip()));

        if public_network {
            ensure!(
                is_public_ipv4(advertise_ip),
                "joining a public TON network requires a publicly reachable \
                 --advertise-ip, but {advertise_ip} is not public; forward the node's \
                 ADNL UDP port to a static public IPv4 address"
            );
        }

        Ok(())
    }

    /// Registers one masterchain block as the network's newest hardfork.
    ///
    /// On its next start the node truncates to the block before this one and
    /// applies the hardfork from `db/static`, so the entry must name a block that
    /// sits exactly one above the node's current top block.
    pub(crate) fn push_hardfork(
        &mut self,
        seqno: u32,
        root_hash: TonBlockHash,
        file_hash: TonBlockHash,
    ) {
        self.0.validator.as_mut().unwrap().hardforks.push(BlockId {
            shard: ShardIdent::MASTERCHAIN,
            seqno,
            root_hash: HashBytes(*root_hash.as_bytes()),
            file_hash: HashBytes(*file_hash.as_bytes()),
        });
    }

    /// Returns the masterchain blocks already registered as hardforks.
    pub(crate) fn hardfork_seqnos(&self) -> Vec<u32> {
        self.0
            .validator
            .as_ref()
            .unwrap()
            .hardforks
            .iter()
            .map(|block| block.seqno)
            .collect()
    }

    /// Advances fresh-node bootstrap only after the verified hardfork states are
    /// published in the native archive. Existing nodes retain their databases;
    /// new nodes must start at or after the last declared hardfork.
    pub(crate) fn use_hardfork_for_bootstrap(&mut self, seqno: u32) -> Result<()> {
        let validator = self.0.validator.as_mut().unwrap();
        let fork = validator
            .hardforks
            .last()
            .filter(|fork| fork.seqno == seqno)
            .context("Bootstrap block must be the latest registered hardfork")?;
        validator.init_block = Some(*fork);
        Ok(())
    }

    /// Routes this host's chain operations through its own liteserver identity.
    /// DHT and validator sections retain the downloaded network identity.
    pub(crate) fn with_local_liteserver(mut self, port: u16, public_key: TonPublicKey) -> Self {
        self.0.liteservers = vec![liteserver(Ipv4Addr::LOCALHOST, port, public_key)];
        self
    }

    /// Borrows the complete shared model for transports without dropping network identity.
    pub(crate) fn network_config(&self) -> &NetworkGlobalConfig {
        &self.0
    }
}

impl TryFrom<NetworkGlobalConfig> for GlobalConfig {
    type Error = &'static str;

    fn try_from(config: NetworkGlobalConfig) -> std::result::Result<Self, Self::Error> {
        if config.dht.is_none() {
            return Err("node global config has no DHT section");
        }
        if config.validator.is_none() {
            return Err("node global config has no validator section");
        }
        Ok(Self(config))
    }
}

impl From<GlobalConfig> for NetworkGlobalConfig {
    fn from(config: GlobalConfig) -> Self {
        config.0
    }
}

impl utoipa::PartialSchema for GlobalConfig {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        <schema::GlobalConfig as utoipa::PartialSchema>::schema()
    }
}

impl ToSchema for GlobalConfig {
    fn schemas(
        schemas: &mut Vec<(
            String,
            utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>,
        )>,
    ) {
        schema::GlobalConfig::schemas(schemas);
    }
}

fn liteserver(ip: Ipv4Addr, port: u16, public_key: TonPublicKey) -> ConfigLiteServer {
    ConfigLiteServer {
        ip: i32::from_be_bytes(ip.octets()),
        port,
        id: ConfigPublicKey::Ed25519 {
            key: *public_key.as_bytes(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_config_round_trips_through_the_typed_schema() {
        let config = GlobalConfig::local(
            ZeroStateId::new(
                TonBlockHash::from_bytes([1; 32]),
                TonBlockHash::from_bytes([2; 32]),
            ),
            Vec::new(),
            Ipv4Addr::new(192, 168, 27, 4),
            18_004,
            TonPublicKey::from_bytes([3; 32]),
        );
        let json = serde_json::to_vec(&config).unwrap();
        let decoded = GlobalConfig::from_json_bytes(&json).unwrap();

        assert_eq!(decoded, config);
        decoded
            .validate_advertise_ip(Ipv4Addr::new(192, 168, 27, 4))
            .unwrap();
    }

    #[test]
    fn parses_the_repository_mainnet_fixture() {
        let config = GlobalConfig::from_json_bytes(include_bytes!(
            "../../../../crates/ton-indexer-liteserver/fixtures/mainnet-global.config.json"
        ))
        .unwrap();

        config.validate_for_node_join().unwrap();
        assert!(config.validate_advertise_ip(Ipv4Addr::LOCALHOST).is_err());
        config
            .validate_advertise_ip(Ipv4Addr::new(203, 12, 34, 56))
            .unwrap();
    }

    #[test]
    fn join_requires_an_upstream_liteserver() {
        let mut config = GlobalConfig::from_json_bytes(include_bytes!(
            "../../../../crates/ton-indexer-liteserver/fixtures/mainnet-global.config.json"
        ))
        .unwrap();
        config.0.liteservers.clear();

        assert_eq!(
            config.validate_for_node_join().unwrap_err().to_string(),
            "global config has no liteserver endpoints"
        );
    }

    #[test]
    fn node_configs_require_dht_and_validator_sections() {
        assert!(GlobalConfig::from_json_bytes(br#"{"liteservers":[]}"#).is_err());
    }

    #[test]
    fn http_schema_describes_the_complete_shared_model() {
        let schema =
            serde_json::to_value(<GlobalConfig as utoipa::PartialSchema>::schema()).unwrap();
        let fields = schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        expect_test::expect![[r#"["@type","dht","liteservers","validator"]"#]]
            .assert_eq(&serde_json::to_string(&fields).unwrap());

        let mut components = Vec::new();
        <GlobalConfig as ToSchema>::schemas(&mut components);
        let (_, dht) = components
            .iter()
            .find(|(name, _)| name == "DhtConfig")
            .unwrap();
        let dht = serde_json::to_value(dht).unwrap();
        assert!(dht["properties"].get("network_id").is_some());
    }
}
