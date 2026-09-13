//! Asynchronous access to TON Center using the shared API response models.

#[cfg(test)]
mod tests;

use crate::Network;
use anyhow::Context;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::env;
use std::ffi::OsStr;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use ton_api::toncenter::{v2, v3};
use toncenter_keys::api_key as toncenter_api_key;
use tycho_types::boc::Boc;
use tycho_types::prelude::Cell;

const USE_PROXY_ENV: &str = "ACTON_USE_PROXY";
const TONCENTER_MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(1200);
static TONCENTER_REQUEST_GATE: LazyLock<Mutex<Option<Instant>>> =
    LazyLock::new(|| Mutex::new(None));

const fn user_agent() -> &'static str {
    concat!("acton/", env!("CARGO_PKG_VERSION"))
}

fn http_client_builder() -> reqwest::ClientBuilder {
    let builder = Client::builder().use_rustls_tls().user_agent(user_agent());
    if proxy_enabled() {
        builder
    } else {
        builder.no_proxy()
    }
}

fn proxy_enabled() -> bool {
    proxy_enabled_from_value(env::var_os(USE_PROXY_ENV).as_deref())
}

fn proxy_enabled_from_value(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| {
        let value = value.to_string_lossy();
        let value = value.trim();
        value == "1" || value == "true"
    })
}

/// Client for `TON Center` V2/V3 API.
///
/// Used for fetching transaction metadata, block information, and library cells.
pub(crate) struct TonCenterClient {
    client: Client,
    api_key: Option<String>,
    base_url: String,
}

impl TonCenterClient {
    /// Creates a new `TON Center` client for the specified network.
    pub(crate) fn new(network: Network) -> anyhow::Result<Self> {
        let base_url = match network {
            Network::Mainnet => "https://toncenter.com/api/v3".to_string(),
            Network::Testnet => "https://testnet.toncenter.com/api/v3".to_string(),
            Network::Localnet | Network::Custom(_) => {
                anyhow::bail!("Network {network} is not yet supported in retrace")
            }
        };
        Ok(Self {
            client: http_client_builder().build()?,
            api_key: toncenter_api_key(&network),
            base_url,
        })
    }

    /// Applies a simple global rate limit for unauthenticated `TON Center` requests.
    ///
    /// `TON Center` has stricter limits without an API key, so we serialize
    /// requests and keep at least 1 second between request starts.
    async fn maybe_wait_for_rate_limit(&self) {
        if self.api_key.is_some() {
            return;
        }

        let mut last_request = TONCENTER_REQUEST_GATE.lock().await;
        if let Some(last) = *last_request {
            let elapsed = last.elapsed();
            if elapsed < TONCENTER_MIN_REQUEST_INTERVAL {
                let wait_for = TONCENTER_MIN_REQUEST_INTERVAL - elapsed;
                log::debug!("throttle for {wait_for:?}");
                tokio::time::sleep(wait_for).await;
            }
        }
        *last_request = Some(Instant::now());
    }

    /// Sends an authenticated request and decodes a shared TON Center response.
    /// Error envelopes are checked before success deserialization so API errors
    /// retain their endpoint context without including the response payload.
    async fn get<T: DeserializeOwned>(
        &self,
        version: &str,
        method: &str,
        query: &[(&str, String)],
    ) -> anyhow::Result<T> {
        let url = format!("{}/{method}", self.base_url.replace("/api/v3", version));
        let mut request = self.client.get(url).query(query);
        if let Some(key) = &self.api_key {
            request = request.header("X-API-Key", key);
        }

        self.maybe_wait_for_rate_limit().await;
        let response = request
            .send()
            .await
            .with_context(|| format!("TON Center {method} request failed"))?;
        let status = response.status();
        let value: serde_json::Value = response
            .json()
            .await
            .with_context(|| format!("Failed to decode TON Center {method} response ({status})"))?;

        if let Some(error) = value.get("error") {
            anyhow::bail!("TON Center {method} error ({status}): {error}");
        }
        if !status.is_success()
            || value.get("ok").and_then(serde_json::Value::as_bool) == Some(false)
        {
            anyhow::bail!("TON Center {method} request failed ({status})");
        }

        serde_json::from_value(value)
            .with_context(|| format!("Failed to decode TON Center {method} response"))
    }

    /// Unwraps the V2 envelope; result schemas are owned by `ton-api`.
    async fn get_v2<T: DeserializeOwned>(
        &self,
        method: &str,
        query: &[(&str, String)],
    ) -> anyhow::Result<T> {
        let response: v2::TonlibResponse<T> = self.get("/api/v2", method, query).await?;
        Ok(response.result)
    }

    /// Fetches indexed transaction metadata with V3 filters.
    pub(crate) async fn get_transactions(
        &self,
        query: &[(&str, String)],
    ) -> anyhow::Result<v3::TransactionsResponse> {
        self.get("/api/v3", "transactions", query).await
    }

    /// Loads the shard header containing a transaction, including its random seed.
    pub(crate) async fn get_blocks(
        &self,
        block: &v3::BlockId,
    ) -> anyhow::Result<v3::BlocksResponse> {
        self.get(
            "/api/v3",
            "blocks",
            &[
                ("workchain", block.workchain.to_string()),
                ("shard", block.shard.clone()),
                ("seqno", block.seqno.to_string()),
            ],
        )
        .await
    }

    /// Loads raw archival transactions in newest-to-oldest order for replay.
    pub(crate) async fn get_account_transactions(
        &self,
        address: &str,
        lt: u64,
        hash: &str,
        to_lt: u64,
        limit: u32,
    ) -> anyhow::Result<Vec<v2::Transaction>> {
        self.get_v2(
            "getTransactions",
            &[
                ("address", address.to_owned()),
                ("lt", lt.to_string()),
                ("hash", hash.to_owned()),
                ("to_lt", to_lt.to_string()),
                ("limit", limit.to_string()),
                ("archival", "true".to_owned()),
            ],
        )
        .await
    }

    /// Fetches the global library cell needed to resolve an exotic code cell.
    pub(crate) async fn get_libraries(&self, hash: &str) -> anyhow::Result<String> {
        let libraries: v2::LibraryResult = self
            .get_v2("getLibraries", &[("libraries", hash.to_owned())])
            .await?;
        libraries
            .result
            .into_iter()
            .next()
            .map(|library| library.data)
            .with_context(|| format!("TON Center library {hash} not found"))
    }

    /// Fetches the configuration in effect at the replayed masterchain block.
    pub(crate) async fn get_config_all(&self, seqno: u32) -> anyhow::Result<Cell> {
        let config: v2::ConfigInfo = self
            .get_v2("getConfigAll", &[("seqno", seqno.to_string())])
            .await?;
        Boc::decode_base64(config.config.bytes)
            .context("Failed to decode blockchain config BOC data")
    }

    /// Loads a serialized `ShardAccount`, preserving its previous transaction reference.
    pub(crate) async fn get_shard_account_cell(
        &self,
        seqno: u32,
        address: &str,
    ) -> anyhow::Result<Cell> {
        let cell: v2::TvmCell = self
            .get_v2(
                "getShardAccountCell",
                &[
                    ("address", address.to_owned()),
                    ("seqno", seqno.to_string()),
                ],
            )
            .await?;
        Boc::decode_base64(cell.bytes).context("Failed to decode shard account cell BOC data")
    }
}
