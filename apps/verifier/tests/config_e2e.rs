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
    assert_eq!(config.compiler_node_bin(), "node");
    assert_eq!(
        config.compiler_worker_path().to_string_lossy(),
        "compiler-worker/compile-tolk.mjs"
    );
    assert_eq!(config.compiler_timeout(), Duration::from_secs(5));
}
