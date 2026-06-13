use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use thiserror::Error;

use crate::config::Config;

const TONCENTER_API_KEY_HEADER: &str = "X-API-Key";

#[async_trait]
pub trait BlockchainClient: Send + Sync + 'static {
    async fn get_code_hash(&self, address: &str) -> Result<Option<String>, BlockchainError>;
}

#[derive(Clone)]
pub struct ToncenterClient {
    http: Client,
    base_url: String,
    api_key: Option<String>,
}

impl ToncenterClient {
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self::new(
            config.toncenter_base_url().to_owned(),
            config.toncenter_api_key().map(ToOwned::to_owned),
        )
    }

    #[must_use]
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            http: Client::new(),
            base_url,
            api_key,
        }
    }

    fn account_states_url(&self) -> String {
        format!(
            "{}/api/v3/accountStates",
            self.base_url.trim_end_matches('/')
        )
    }
}

#[async_trait]
impl BlockchainClient for ToncenterClient {
    async fn get_code_hash(&self, address: &str) -> Result<Option<String>, BlockchainError> {
        let mut request = self
            .http
            .get(self.account_states_url())
            .query(&[("address", address), ("include_boc", "false")]);

        if let Some(api_key) = &self.api_key {
            request = request.header(TONCENTER_API_KEY_HEADER, api_key);
        }

        let response = request.send().await.map_err(BlockchainError::Transport)?;
        let status = response.status();
        let body = response.text().await.map_err(BlockchainError::Transport)?;

        if !status.is_success() {
            return Err(BlockchainError::api(status, body));
        }

        let account_states =
            serde_json::from_str::<AccountStatesResponse>(&body).map_err(BlockchainError::Json)?;

        Ok(account_states
            .accounts
            .into_iter()
            .find_map(|account| non_empty_text(account.code_hash)))
    }
}

fn non_empty_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

#[derive(Debug, Error)]
pub enum BlockchainError {
    #[error("toncenter transport error: {0}")]
    Transport(reqwest::Error),
    #[error("toncenter API error: status={status}, body={body}")]
    Api { status: StatusCode, body: String },
    #[error("toncenter malformed response: {0}")]
    Json(serde_json::Error),
}

impl BlockchainError {
    const fn api(status: StatusCode, body: String) -> Self {
        Self::Api { status, body }
    }
}

#[derive(Debug, Deserialize)]
struct AccountStatesResponse {
    accounts: Vec<AccountState>,
}

#[derive(Debug, Deserialize)]
struct AccountState {
    code_hash: Option<String>,
}
