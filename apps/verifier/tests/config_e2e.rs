use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;

use verifier::config::Config;

#[test]
fn repository_config_toml_loads() {
    let config = Config::load_from_path("config.toml").expect("repository config should load");

    assert_eq!(
        config.bind_addr(),
        "127.0.0.1:3000"
            .parse::<SocketAddr>()
            .expect("test bind address should be valid")
    );
    assert_eq!(config.network().to_string(), "mainnet");
    assert_eq!(config.toncenter_base_url(), "https://toncenter.com");
    assert_eq!(config.toncenter_api_key(), None);
    assert_eq!(config.registry_master_address(), None);
    assert_eq!(config.registry_register_value_nano(), 500_000_000);
    assert_eq!(config.registry_confirmation_attempts(), 20);
    assert_eq!(config.registry_confirmation_delay(), Duration::from_secs(1));
    assert_eq!(config.wallet_kind(), "v5r1");
    assert_eq!(config.wallet_workchain(), 0);
    assert_eq!(config.wallet_mnemonic_env(), None);
    assert_eq!(config.wallet_mnemonic_file(), None);
    assert_eq!(config.wallet_mnemonic(), None);
    assert_eq!(config.source_repository_path(), None);
    assert_eq!(config.source_repository_remote(), "origin");
    assert_eq!(config.source_repository_branch(), None);
    assert_eq!(config.source_repository_author_name(), "ton-verifier");
    assert_eq!(
        config.source_repository_author_email(),
        "ton-verifier@example.invalid"
    );
    assert_eq!(config.compiler_node_bin(), "node");
    assert_eq!(
        config.compiler_worker_path().to_string_lossy(),
        "compiler-worker/compile.mjs"
    );
    assert_eq!(config.compiler_timeout(), Duration::from_secs(5));
}

#[test]
fn localnet_network_uses_localnet_endpoint_by_default() {
    let mut config_file =
        tempfile::NamedTempFile::new().expect("temporary config file should be created");
    writeln!(
        config_file,
        r#"
[network]
name = "localnet"
"#
    )
    .expect("temporary config should be writable");
    config_file
        .flush()
        .expect("temporary config should be flushed");

    let config = Config::load_from_path(config_file.path()).expect("localnet config should load");

    assert_eq!(config.network().to_string(), "localnet");
    assert_eq!(config.toncenter_base_url(), "http://127.0.0.1:5411");
}

#[test]
fn registry_and_wallet_config_load_from_toml() {
    let mut config_file =
        tempfile::NamedTempFile::new().expect("temporary config file should be created");
    writeln!(
        config_file,
        r#"
[network]
name = "testnet"

[toncenter]
base_url = "http://127.0.0.1:5412"
api_key = "test-key"

[registry]
master_address = "EQD0000000000000000000000000000000000000000000000"
register_value_nano = 700000000
confirmation_attempts = 3
confirmation_delay_ms = 25

[wallet]
kind = "v5r1"
workchain = 0
mnemonic_env = "VERIFIER_TEST_MNEMONIC"

[source_repository]
path = "/tmp/verifier-sources"
remote = "github"
branch = "verified-sources"
author_name = "Verifier Bot"
author_email = "verifier@example.com"
"#
    )
    .expect("temporary config should be writable");
    config_file
        .flush()
        .expect("temporary config should be flushed");

    let config = Config::load_from_path(config_file.path()).expect("testnet config should load");

    assert_eq!(config.network().to_string(), "testnet");
    assert_eq!(config.toncenter_base_url(), "http://127.0.0.1:5412");
    assert_eq!(config.toncenter_api_key(), Some("test-key"));
    assert_eq!(
        config.registry_master_address(),
        Some("EQD0000000000000000000000000000000000000000000000")
    );
    assert_eq!(config.registry_register_value_nano(), 700_000_000);
    assert_eq!(config.registry_confirmation_attempts(), 3);
    assert_eq!(
        config.registry_confirmation_delay(),
        Duration::from_millis(25)
    );
    assert_eq!(config.wallet_kind(), "v5r1");
    assert_eq!(config.wallet_workchain(), 0);
    assert_eq!(config.wallet_mnemonic_env(), Some("VERIFIER_TEST_MNEMONIC"));
    assert_eq!(
        config
            .source_repository_path()
            .map(|path| path.to_string_lossy()),
        Some("/tmp/verifier-sources".into())
    );
    assert_eq!(config.source_repository_remote(), "github");
    assert_eq!(config.source_repository_branch(), Some("verified-sources"));
    assert_eq!(config.source_repository_author_name(), "Verifier Bot");
    assert_eq!(
        config.source_repository_author_email(),
        "verifier@example.com"
    );
}
