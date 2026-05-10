use anyhow::Context;
use std::str::FromStr;

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
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct PowConfig {
    pub enabled: bool,
    pub difficulty: u32,
    pub challenge_ttl_seconds: u64,
    pub max_challenges: u64,
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
                port: parse_env_number("PORT", 3001),
            },
            toncenter: ToncenterConfig {
                api_key: std::env::var("TONCENTER_API_KEY").ok(),
                url: std::env::var("TONCENTER_URL")
                    .unwrap_or_else(|_| "https://testnet.toncenter.com".to_string()),
                timeout_seconds: parse_env_number("TONCENTER_TIMEOUT_SECONDS", 10),
                connect_timeout_seconds: parse_env_number("TONCENTER_CONNECT_TIMEOUT_SECONDS", 5),
                max_retries: parse_env_number("TONCENTER_MAX_RETRIES", 3),
                retry_base_delay_ms: parse_env_number("TONCENTER_RETRY_BASE_DELAY_MS", 500),
            },
            worker: WorkerConfig {
                max_retries: parse_env_number("WORKER_MAX_RETRIES", 2),
                retry_base_delay_ms: parse_env_number("WORKER_RETRY_BASE_DELAY_MS", 1_000),
            },
            faucet: FaucetConfig {
                mnemonic: std::env::var("FAUCET_MNEMONIC")
                    .context("FAUCET_MNEMONIC must be set")?,
                amount: parse_env_number("FAUCET_AMOUNT", 1_000_000), // 0.5 TON default
                message: std::env::var("FAUCET_MESSAGE")
                    .unwrap_or_else(|_| "Testnet faucet".to_string()),
            },
            pow: PowConfig {
                enabled: parse_env_bool("POW_ENABLED", true),
                difficulty: parse_env_number("POW_DIFFICULTY", 21),
                challenge_ttl_seconds: parse_env_number("POW_CHALLENGE_TTL_SECONDS", 300),
                max_challenges: parse_env_number("POW_MAX_CHALLENGES", 10_000),
            },
        };

        Ok(config)
    }
}

fn parse_env_number<T>(name: &str, default: T) -> T
where
    T: FromStr,
{
    std::env::var(name)
        .ok()
        .and_then(|value| parse_number(&value))
        .unwrap_or(default)
}

fn parse_number<T>(value: &str) -> Option<T>
where
    T: FromStr,
{
    value.replace('_', "").parse().ok()
}

fn parse_env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .and_then(|value| parse_bool(&value))
        .unwrap_or(default)
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim_end().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" => Some(true),
        "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_bool, parse_number};

    #[test]
    fn parses_numbers_with_underscores() {
        assert_eq!(parse_number::<u64>("500_000_000"), Some(500_000_000));
        assert_eq!(parse_number::<u32>("1_000"), Some(1_000));
        assert_eq!(parse_number::<u16>("3001"), Some(3001));
    }

    #[test]
    fn rejects_invalid_numbers() {
        assert_eq!(parse_number::<u64>("500 TON"), None);
    }

    #[test]
    fn parses_bool_values() {
        for value in ["true", "TRUE", "yes", "on", "on "] {
            assert_eq!(parse_bool(value), Some(true));
        }

        for value in ["false", "FALSE", "no", "off", "off "] {
            assert_eq!(parse_bool(value), Some(false));
        }
    }

    #[test]
    fn rejects_invalid_bool_values() {
        assert_eq!(parse_bool(""), None);
        assert_eq!(parse_bool("1"), None);
        assert_eq!(parse_bool("0"), None);
        assert_eq!(parse_bool(" on"), None);
        assert_eq!(parse_bool(" off"), None);
        assert_eq!(parse_bool("maybe"), None);
    }
}
