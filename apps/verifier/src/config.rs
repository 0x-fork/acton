use std::{
    env, fmt, fs, io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Deserialize;
use thiserror::Error;

const DEFAULT_CONFIG_PATH: &str = "config.toml";
const CONFIG_PATH_ENV: &str = "VERIFIER_CONFIG";
const MAINNET_TONCENTER_BASE_URL: &str = "https://toncenter.com";
const TESTNET_TONCENTER_BASE_URL: &str = "https://testnet.toncenter.com";
const LOCALNET_TONCENTER_BASE_URL: &str = "http://127.0.0.1:5411";
const DEFAULT_COMPILER_NODE_BIN: &str = "node";
const DEFAULT_COMPILER_WORKER_PATH: &str = "compiler-worker/compile.mjs";
const DEFAULT_COMPILER_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_REGISTRY_REGISTER_VALUE_NANO: u64 = 500_000_000;
const DEFAULT_REGISTRY_CONFIRMATION_ATTEMPTS: usize = 20;
const DEFAULT_REGISTRY_CONFIRMATION_DELAY_MS: u64 = 1_000;
const DEFAULT_WALLET_KIND: &str = "v5r1";
const DEFAULT_WALLET_WORKCHAIN: i32 = 0;
const DEFAULT_SOURCE_REPOSITORY_REMOTE: &str = "origin";
const DEFAULT_SOURCE_REPOSITORY_AUTHOR_NAME: &str = "ton-verifier";
const DEFAULT_SOURCE_REPOSITORY_AUTHOR_EMAIL: &str = "ton-verifier@example.invalid";

#[derive(Clone, Debug)]
pub struct Config {
    bind_addr: SocketAddr,
    network: TonNetwork,
    toncenter_base_url: Option<String>,
    toncenter_api_key: Option<String>,
    registry_master_address: Option<String>,
    registry_register_value_nano: u64,
    registry_confirmation_attempts: usize,
    registry_confirmation_delay: Duration,
    wallet_kind: String,
    wallet_workchain: i32,
    wallet_mnemonic_env: Option<String>,
    wallet_mnemonic_file: Option<PathBuf>,
    wallet_mnemonic: Option<String>,
    source_repository_path: Option<PathBuf>,
    source_repository_remote: String,
    source_repository_branch: Option<String>,
    source_repository_author_name: String,
    source_repository_author_email: String,
    compiler_node_bin: String,
    compiler_worker_path: PathBuf,
    compiler_timeout: Duration,
}

impl Config {
    /// Loads configuration from the `VERIFIER_CONFIG` path or `config.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file cannot be read or parsed as TOML.
    pub fn load() -> Result<Self, ConfigError> {
        let path = env::var_os(CONFIG_PATH_ENV)
            .map_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH), PathBuf::from);

        Self::load_from_path(path)
    }

    /// Loads configuration from a specific TOML file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file cannot be read or parsed as TOML.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let raw_config = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let file =
            toml::from_str::<ConfigFile>(&raw_config).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            })?;

        Ok(file.into_config())
    }

    #[must_use]
    pub const fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    #[must_use]
    pub const fn network(&self) -> TonNetwork {
        self.network
    }

    #[must_use]
    pub fn toncenter_base_url(&self) -> &str {
        self.toncenter_base_url
            .as_deref()
            .unwrap_or_else(|| self.network.default_toncenter_base_url())
    }

    #[must_use]
    pub fn toncenter_api_key(&self) -> Option<&str> {
        self.toncenter_api_key.as_deref()
    }

    #[must_use]
    pub fn registry_master_address(&self) -> Option<&str> {
        self.registry_master_address.as_deref()
    }

    #[must_use]
    pub const fn registry_register_value_nano(&self) -> u64 {
        self.registry_register_value_nano
    }

    #[must_use]
    pub const fn registry_confirmation_attempts(&self) -> usize {
        self.registry_confirmation_attempts
    }

    #[must_use]
    pub const fn registry_confirmation_delay(&self) -> Duration {
        self.registry_confirmation_delay
    }

    #[must_use]
    pub fn wallet_kind(&self) -> &str {
        &self.wallet_kind
    }

    #[must_use]
    pub const fn wallet_workchain(&self) -> i32 {
        self.wallet_workchain
    }

    #[must_use]
    pub fn wallet_mnemonic_env(&self) -> Option<&str> {
        self.wallet_mnemonic_env.as_deref()
    }

    #[must_use]
    pub fn wallet_mnemonic_file(&self) -> Option<&Path> {
        self.wallet_mnemonic_file.as_deref()
    }

    #[must_use]
    pub fn wallet_mnemonic(&self) -> Option<&str> {
        self.wallet_mnemonic.as_deref()
    }

    #[must_use]
    pub fn source_repository_path(&self) -> Option<&Path> {
        self.source_repository_path.as_deref()
    }

    #[must_use]
    pub fn source_repository_remote(&self) -> &str {
        &self.source_repository_remote
    }

    #[must_use]
    pub fn source_repository_branch(&self) -> Option<&str> {
        self.source_repository_branch.as_deref()
    }

    #[must_use]
    pub fn source_repository_author_name(&self) -> &str {
        &self.source_repository_author_name
    }

    #[must_use]
    pub fn source_repository_author_email(&self) -> &str {
        &self.source_repository_author_email
    }

    #[must_use]
    pub fn compiler_node_bin(&self) -> &str {
        &self.compiler_node_bin
    }

    #[must_use]
    pub fn compiler_worker_path(&self) -> &Path {
        &self.compiler_worker_path
    }

    #[must_use]
    pub const fn compiler_timeout(&self) -> Duration {
        self.compiler_timeout
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: default_bind_addr(),
            network: TonNetwork::Mainnet,
            toncenter_base_url: None,
            toncenter_api_key: None,
            registry_master_address: None,
            registry_register_value_nano: DEFAULT_REGISTRY_REGISTER_VALUE_NANO,
            registry_confirmation_attempts: DEFAULT_REGISTRY_CONFIRMATION_ATTEMPTS,
            registry_confirmation_delay: Duration::from_millis(
                DEFAULT_REGISTRY_CONFIRMATION_DELAY_MS,
            ),
            wallet_kind: DEFAULT_WALLET_KIND.to_owned(),
            wallet_workchain: DEFAULT_WALLET_WORKCHAIN,
            wallet_mnemonic_env: None,
            wallet_mnemonic_file: None,
            wallet_mnemonic: None,
            source_repository_path: None,
            source_repository_remote: DEFAULT_SOURCE_REPOSITORY_REMOTE.to_owned(),
            source_repository_branch: None,
            source_repository_author_name: DEFAULT_SOURCE_REPOSITORY_AUTHOR_NAME.to_owned(),
            source_repository_author_email: DEFAULT_SOURCE_REPOSITORY_AUTHOR_EMAIL.to_owned(),
            compiler_node_bin: DEFAULT_COMPILER_NODE_BIN.to_owned(),
            compiler_worker_path: PathBuf::from(DEFAULT_COMPILER_WORKER_PATH),
            compiler_timeout: Duration::from_millis(DEFAULT_COMPILER_TIMEOUT_MS),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TonNetwork {
    Mainnet,
    Testnet,
    Localnet,
}

impl TonNetwork {
    #[must_use]
    pub const fn uses_testnet_address_format(self) -> bool {
        matches!(self, Self::Testnet | Self::Localnet)
    }

    const fn default_toncenter_base_url(self) -> &'static str {
        match self {
            Self::Mainnet => MAINNET_TONCENTER_BASE_URL,
            Self::Testnet => TESTNET_TONCENTER_BASE_URL,
            Self::Localnet => LOCALNET_TONCENTER_BASE_URL,
        }
    }
}

impl fmt::Display for TonNetwork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mainnet => formatter.write_str("mainnet"),
            Self::Testnet => formatter.write_str("testnet"),
            Self::Localnet => formatter.write_str("localnet"),
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("failed to parse config {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    server: ServerConfig,
    #[serde(default)]
    network: NetworkConfig,
    #[serde(default)]
    toncenter: ToncenterConfig,
    #[serde(default)]
    registry: RegistryConfig,
    #[serde(default)]
    wallet: WalletConfig,
    #[serde(default)]
    source_repository: SourceRepositoryConfig,
    #[serde(default)]
    compiler: CompilerConfig,
}

impl ConfigFile {
    fn into_config(self) -> Config {
        Config {
            bind_addr: self.server.bind_addr.unwrap_or_else(default_bind_addr),
            network: self.network.name.unwrap_or(TonNetwork::Mainnet),
            toncenter_base_url: self.toncenter.base_url,
            toncenter_api_key: self.toncenter.api_key,
            registry_master_address: self.registry.master_address,
            registry_register_value_nano: self
                .registry
                .register_value_nano
                .unwrap_or(DEFAULT_REGISTRY_REGISTER_VALUE_NANO),
            registry_confirmation_attempts: self
                .registry
                .confirmation_attempts
                .unwrap_or(DEFAULT_REGISTRY_CONFIRMATION_ATTEMPTS),
            registry_confirmation_delay: Duration::from_millis(
                self.registry
                    .confirmation_delay_ms
                    .unwrap_or(DEFAULT_REGISTRY_CONFIRMATION_DELAY_MS),
            ),
            wallet_kind: self
                .wallet
                .kind
                .unwrap_or_else(|| DEFAULT_WALLET_KIND.to_owned()),
            wallet_workchain: self.wallet.workchain.unwrap_or(DEFAULT_WALLET_WORKCHAIN),
            wallet_mnemonic_env: self.wallet.mnemonic_env,
            wallet_mnemonic_file: self.wallet.mnemonic_file,
            wallet_mnemonic: self.wallet.mnemonic,
            source_repository_path: self.source_repository.path,
            source_repository_remote: self
                .source_repository
                .remote
                .unwrap_or_else(|| DEFAULT_SOURCE_REPOSITORY_REMOTE.to_owned()),
            source_repository_branch: self.source_repository.branch,
            source_repository_author_name: self
                .source_repository
                .author_name
                .unwrap_or_else(|| DEFAULT_SOURCE_REPOSITORY_AUTHOR_NAME.to_owned()),
            source_repository_author_email: self
                .source_repository
                .author_email
                .unwrap_or_else(|| DEFAULT_SOURCE_REPOSITORY_AUTHOR_EMAIL.to_owned()),
            compiler_node_bin: self
                .compiler
                .node_bin
                .unwrap_or_else(|| DEFAULT_COMPILER_NODE_BIN.to_owned()),
            compiler_worker_path: self
                .compiler
                .worker_path
                .unwrap_or_else(|| PathBuf::from(DEFAULT_COMPILER_WORKER_PATH)),
            compiler_timeout: Duration::from_millis(
                self.compiler
                    .timeout_ms
                    .unwrap_or(DEFAULT_COMPILER_TIMEOUT_MS),
            ),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct ServerConfig {
    bind_addr: Option<SocketAddr>,
}

#[derive(Debug, Default, Deserialize)]
struct NetworkConfig {
    name: Option<TonNetwork>,
}

#[derive(Debug, Default, Deserialize)]
struct ToncenterConfig {
    base_url: Option<String>,
    api_key: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RegistryConfig {
    master_address: Option<String>,
    register_value_nano: Option<u64>,
    confirmation_attempts: Option<usize>,
    confirmation_delay_ms: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct WalletConfig {
    kind: Option<String>,
    workchain: Option<i32>,
    mnemonic_env: Option<String>,
    mnemonic_file: Option<PathBuf>,
    mnemonic: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SourceRepositoryConfig {
    path: Option<PathBuf>,
    remote: Option<String>,
    branch: Option<String>,
    author_name: Option<String>,
    author_email: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CompilerConfig {
    node_bin: Option<String>,
    worker_path: Option<PathBuf>,
    timeout_ms: Option<u64>,
}

const fn default_bind_addr() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3000))
}
