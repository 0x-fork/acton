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
const DEFAULT_COMPILER_NODE_BIN: &str = "node";
const DEFAULT_COMPILER_WORKER_PATH: &str = "compiler-worker/compile-tolk.mjs";
const DEFAULT_COMPILER_TIMEOUT_MS: u64 = 5_000;

#[derive(Clone, Debug)]
pub struct Config {
    bind_addr: SocketAddr,
    network: TonNetwork,
    toncenter_base_url: Option<String>,
    toncenter_api_key: Option<String>,
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
}

impl TonNetwork {
    const fn default_toncenter_base_url(self) -> &'static str {
        match self {
            Self::Mainnet => MAINNET_TONCENTER_BASE_URL,
            Self::Testnet => TESTNET_TONCENTER_BASE_URL,
        }
    }
}

impl fmt::Display for TonNetwork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mainnet => formatter.write_str("mainnet"),
            Self::Testnet => formatter.write_str("testnet"),
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
    compiler: CompilerConfig,
}

impl ConfigFile {
    fn into_config(self) -> Config {
        Config {
            bind_addr: self.server.bind_addr.unwrap_or_else(default_bind_addr),
            network: self.network.name.unwrap_or(TonNetwork::Mainnet),
            toncenter_base_url: self.toncenter.base_url,
            toncenter_api_key: self.toncenter.api_key,
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
struct CompilerConfig {
    node_bin: Option<String>,
    worker_path: Option<PathBuf>,
    timeout_ms: Option<u64>,
}

const fn default_bind_addr() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3000))
}
