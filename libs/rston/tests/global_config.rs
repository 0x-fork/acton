#![cfg(all(feature = "models", feature = "serde"))]

use rston::GlobalConfig;
use rston::global_config::ConfigAdnlAddress;
use rston::models::{BlockId, ShardIdent};

const MAINNET_CONFIG: &str = include_str!("fixtures/global_config/mainnet.json");

fn mainnet_json() -> serde_json::Value {
    serde_json::from_str(MAINNET_CONFIG).unwrap()
}

#[test]
fn full_mainnet_config_matches_go_block_identifiers() {
    let config: GlobalConfig = MAINNET_CONFIG.parse().unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/global_config/go-block-ids.json"
    ))
    .unwrap();
    let validator = config.validator.as_ref().unwrap();
    assert_eq!(
        validator.zero_state,
        expected["zero_state"]
            .as_str()
            .unwrap()
            .parse::<BlockId>()
            .unwrap()
    );
    assert_eq!(
        validator.init_block.unwrap(),
        expected["init_block"]
            .as_str()
            .unwrap()
            .parse::<BlockId>()
            .unwrap()
    );
    let hardforks = expected["hardforks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().parse::<BlockId>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(validator.hardforks, hardforks);

    let dht = config.dht.as_ref().unwrap();
    assert!(!dht.static_nodes.nodes.is_empty());
    let serialized = serde_json::to_value(&config).unwrap();
    assert_eq!(serialized["dht"], mainnet_json()["dht"]);
    assert_eq!(serialized["liteservers"], mainnet_json()["liteservers"]);
    assert_eq!(
        serde_json::from_value::<GlobalConfig>(serialized).unwrap(),
        config
    );
}

#[test]
fn preserves_dht_metadata_and_multiple_addresses() {
    let mut json = mainnet_json();
    json["dht"]["network_id"] = (-123).into();
    let node = &mut json["dht"]["static_nodes"]["nodes"][0];
    node["addr_list"]["addrs"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "@type": "adnl.address.udp",
            "ip": -1062731519,
            "port": 1234,
        }));
    node["addr_list"]["priority"] = 7.into();
    node["addr_list"]["expire_at"] = 123456.into();
    let config: GlobalConfig = serde_json::from_value(json.clone()).unwrap();
    let dht = config.dht.as_ref().unwrap();
    assert_eq!(dht.network_id, Some(-123));
    let address = dht.static_nodes.nodes[0].addr_list.addrs.last().unwrap();
    assert_eq!(address.socket_addr().to_string(), "192.168.1.1:1234");
    assert_eq!(serde_json::to_value(config).unwrap()["dht"], json["dht"]);
}

#[test]
fn accepts_endpoint_only_and_p2p_only_configs() {
    let endpoints: GlobalConfig = r#"{"liteservers":[]}"#.parse().unwrap();
    assert!(endpoints.dht.is_none());
    assert!(endpoints.validator.is_none());

    let mut json = mainnet_json();
    json.as_object_mut().unwrap().remove("liteservers");
    let config: GlobalConfig = serde_json::from_value(json).unwrap();
    assert!(config.liteservers.is_empty());
    assert!(config.validator.is_some());
    assert!(config.dht.is_some());
}

#[test]
fn rejects_bad_hashes_and_checkpoint_coordinates() {
    for bad_hash in ["not base64", "", "AQ=="] {
        let mut json = mainnet_json();
        json["validator"]["init_block"]["root_hash"] = bad_hash.into();
        assert!(serde_json::from_value::<GlobalConfig>(json).is_err());
    }

    for field in ["zero_state", "init_block"] {
        let mut json = mainnet_json();
        json["validator"][field]["workchain"] = 0.into();
        assert!(serde_json::from_value::<GlobalConfig>(json).is_err());
    }

    let mut json = mainnet_json();
    json["validator"]["zero_state"]["seqno"] = 1.into();
    assert!(serde_json::from_value::<GlobalConfig>(json).is_err());

    let mut json = mainnet_json();
    json["validator"]["init_block"]["seqno"] = 0.into();
    assert!(serde_json::from_value::<GlobalConfig>(json).is_err());
}

#[test]
fn accepts_an_init_block_equal_to_the_zero_state() {
    let mut json = mainnet_json();
    json["validator"]["init_block"] = json["validator"]["zero_state"].clone();
    let config: GlobalConfig = serde_json::from_value(json).unwrap();
    let validator = config.validator.unwrap();
    assert_eq!(validator.zero_state.shard, ShardIdent::MASTERCHAIN);
    assert_eq!(validator.init_block, Some(validator.zero_state));
}

#[test]
fn rejects_malformed_dht_signatures_and_unsupported_addresses() {
    let mut json = mainnet_json();
    json["dht"]["static_nodes"]["nodes"][0]["signature"] = "AQ==".into();
    assert!(serde_json::from_value::<GlobalConfig>(json).is_err());

    let address = serde_json::json!({"@type": "adnl.address.unknown", "ip": 0, "port": 1234});
    assert!(serde_json::from_value::<ConfigAdnlAddress>(address).is_err());
}

#[test]
fn rejects_unknown_ton_json_constructors() {
    let mut json = mainnet_json();
    json["@type"] = "config.unknown".into();
    assert!(serde_json::from_value::<GlobalConfig>(json).is_err());

    let mut json = mainnet_json();
    json["dht"]["@type"] = "dht.unknown".into();
    assert!(serde_json::from_value::<GlobalConfig>(json).is_err());

    let mut json = mainnet_json();
    json["validator"]["@type"] = "validator.unknown".into();
    assert!(serde_json::from_value::<GlobalConfig>(json).is_err());
}

#[cfg(feature = "liteclient")]
#[test]
fn selects_only_an_explicit_nonzero_init_block_as_the_trusted_checkpoint() {
    use rston::liteclient::VerifiedBlock;

    let config: GlobalConfig = MAINNET_CONFIG.parse().unwrap();
    let checkpoint = VerifiedBlock::from_trusted_config(&config).unwrap();
    assert_eq!(
        Some(*checkpoint.id()),
        config.validator.as_ref().unwrap().init_block
    );

    let mut config = config;
    config.validator.as_mut().unwrap().init_block = None;
    assert!(VerifiedBlock::from_trusted_config(&config).is_err());

    let validator = config.validator.as_mut().unwrap();
    validator.init_block = Some(validator.zero_state);
    assert!(VerifiedBlock::from_trusted_config(&config).is_err());

    config.validator = None;
    assert!(VerifiedBlock::from_trusted_config(&config).is_err());
}
