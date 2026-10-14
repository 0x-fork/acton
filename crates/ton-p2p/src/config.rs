//! Reads the zerostate, starting block, and DHT entry points from a global config.

use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use everscale_network::{crypto, overlay, proto};
use rston::{
    global_config::{ConfigAdnlAddress, ConfigPublicKey, DhtConfig, ValidatorConfig},
    models::{BlockId, ShardIdent},
};
use serde::Deserialize;

/// Network identity and DHT entry points from an operator-supplied global config.
///
/// Parsing validates block coordinates and descriptor fields. The transport
/// verifies DHT signatures when it adds these entry points.
#[derive(Deserialize)]
pub struct NetworkConfig {
    dht: DhtConfig,
    validator: ValidatorConfig,
}

impl NetworkConfig {
    /// Loads a global config. P2P-only starts do not require a `liteservers` field.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read global config {}", path.display()))?;

        Self::from_json(&bytes).with_context(|| format!("invalid global config {}", path.display()))
    }

    /// Parses the P2P subset while accepting unrelated standard config fields.
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let config: Self = serde_json::from_slice(bytes)?;
        ensure!(
            !config.dht.static_nodes.nodes.is_empty(),
            "global config has no DHT entry points"
        );

        Ok(config)
    }

    /// Derives the public masterchain overlay from the network's zerostate.
    #[must_use]
    pub fn masterchain_overlay(&self) -> overlay::IdShort {
        overlay::IdFull::for_workchain_overlay(-1, &self.validator.zero_state.file_hash.0)
            .compute_short_id()
    }

    /// Identifies the network in persistent storage, preventing cross-network resumes.
    #[must_use]
    pub const fn zero_state(&self) -> BlockId {
        self.validator.zero_state
    }

    /// Returns the configured starting block, falling back to the zerostate.
    /// Its hashes are trusted; this client does not validate the history before it.
    #[must_use]
    pub fn initial_block(&self) -> BlockId {
        self.validator
            .init_block
            .unwrap_or(self.validator.zero_state)
    }

    /// Selects an operator-trusted starting block for a new download directory.
    /// Existing storage keeps its persisted anchor. The caller must establish
    /// the network identity; this only checks masterchain coordinates.
    pub fn set_initial_block(&mut self, block: BlockId) -> Result<()> {
        ensure!(
            block.shard == ShardIdent::MASTERCHAIN,
            "starting block must belong to the masterchain"
        );
        ensure!(
            block.seqno != 0 || block == self.zero_state(),
            "starting block at seqno zero must match the network zerostate"
        );

        self.validator.init_block = Some(block);
        Ok(())
    }

    pub(crate) fn dht_nodes(&self) -> impl Iterator<Item = Result<proto::dht::NodeOwned>> + '_ {
        self.dht.static_nodes.nodes.iter().map(|node| {
            // The transport represents one IPv4 address. Changing a descriptor's
            // address list before checking its signature would invalidate it.
            ensure!(
                node.addr_list.addrs.len() == 1,
                "DHT bootstrap descriptors must contain exactly one UDP IPv4 address"
            );
            ensure!(
                node.addr_list.priority == 0,
                "nonzero DHT address priority is not supported"
            );
            let ConfigAdnlAddress::Udp { ip, port } = node.addr_list.addrs[0];
            let ConfigPublicKey::Ed25519 { key } = &node.id;

            Ok(proto::dht::NodeOwned {
                id: crypto::tl::PublicKeyOwned::Ed25519 { key: *key },
                addr_list: proto::adnl::AddressList {
                    address: Some(proto::adnl::Address {
                        ip: ip as u32,
                        port: u32::from(port),
                    }),
                    version: node.addr_list.version as u32,
                    reinit_date: node.addr_list.reinit_date as u32,
                    expire_at: node.addr_list.expire_at as u32,
                },
                version: node.version as u32,
                signature: node.signature.to_vec().into(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use everscale_network::adnl::NodeIdFull;
    use tl_proto_network::BoxedConstructor;

    const MAINNET_CONFIG: &[u8] =
        include_bytes!("../../ton-indexer-liteserver/fixtures/mainnet-global.config.json");

    #[test]
    fn shared_descriptors_preserve_valid_mainnet_dht_signatures() {
        let config = NetworkConfig::from_json(MAINNET_CONFIG).unwrap();
        let mut verified = 0;
        for node in config.dht_nodes() {
            let mut node = node.unwrap();
            let identity = NodeIdFull::try_from(node.id.as_equivalent_ref()).unwrap();
            let signature = std::mem::take(&mut node.signature);
            identity.verify(node.as_boxed(), &signature).unwrap();
            verified += 1;
        }
        assert!(verified > 0);
    }

    #[test]
    fn p2p_bootstrap_keeps_its_zero_state_fallback() {
        let mut json: serde_json::Value = serde_json::from_slice(MAINNET_CONFIG).unwrap();
        json["validator"]
            .as_object_mut()
            .unwrap()
            .remove("init_block");
        let mut config = NetworkConfig::from_json(&serde_json::to_vec(&json).unwrap()).unwrap();
        assert_eq!(config.initial_block(), config.zero_state());

        let mut other_network = config.zero_state();
        other_network.root_hash.0[0] ^= 1;
        assert!(config.set_initial_block(other_network).is_err());
        assert_eq!(config.initial_block(), config.zero_state());
    }
}
