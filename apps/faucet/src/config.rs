use anyhow::Context;

#[derive(Clone, Debug)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub toncenter: ToncenterConfig,
    pub worker: WorkerConfig,
    pub faucet: FaucetConfig,
    pub pow: PowConfig,
}

#[derive(Clone, Debug)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug)]
pub struct ToncenterConfig {
    pub api_key: Option<String>,
    pub url: String,
    pub timeout_seconds: u64,
    pub connect_timeout_seconds: u64,
    pub max_retries: u32,
    pub retry_base_delay_ms: u64,
}

#[derive(Clone, Debug)]
pub struct WorkerConfig {
    pub max_retries: u32,
    pub retry_base_delay_ms: u64,
}

#[derive(Clone, Debug)]
pub struct FaucetConfig {
    pub mnemonic: String,
    pub amount: u64,
}

#[derive(Clone, Debug)]
pub struct PowConfig {
    pub difficulty: u32,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let config = Config {
            database: DatabaseConfig {
                url: std::env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "sqlite:./db.sqlite".to_string()),
            },
            server: ServerConfig {
                host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: std::env::var("PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(3001),
            },
            toncenter: ToncenterConfig {
                api_key: std::env::var("TONCENTER_API_KEY").ok(),
                url: std::env::var("TONCENTER_URL")
                    .unwrap_or_else(|_| "https://testnet.toncenter.com".to_string()),
                timeout_seconds: std::env::var("TONCENTER_TIMEOUT_SECONDS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(10),
                connect_timeout_seconds: std::env::var("TONCENTER_CONNECT_TIMEOUT_SECONDS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(5),
                max_retries: std::env::var("TONCENTER_MAX_RETRIES")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(3),
                retry_base_delay_ms: std::env::var("TONCENTER_RETRY_BASE_DELAY_MS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(500),
            },
            worker: WorkerConfig {
                max_retries: std::env::var("WORKER_MAX_RETRIES")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(2),
                retry_base_delay_ms: std::env::var("WORKER_RETRY_BASE_DELAY_MS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1_000),
            },
            faucet: FaucetConfig {
                mnemonic: std::env::var("FAUCET_MNEMONIC")
                    .context("FAUCET_MNEMONIC must be set")?,
                amount: std::env::var("FAUCET_AMOUNT")
                    .ok()
                    .and_then(|a| a.parse().ok())
                    .unwrap_or(1_000_000), // 0.001 TON default
            },
            pow: PowConfig {
                difficulty: std::env::var("POW_DIFFICULTY")
                    .ok()
                    .and_then(|d| d.parse().ok())
                    .unwrap_or(21),
            },
        };

        Ok(config)
    }
}
