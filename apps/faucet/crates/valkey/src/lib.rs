use anyhow::Context;
use faucet_config::ValkeyConfig;

const SENT_AMOUNT_WINDOW_KEY: &str = "faucet:antifraud:sent-amount-window";
const SENT_AMOUNT_WINDOW_SEQ_KEY: &str = "faucet:antifraud:sent-amount-window:seq";
const SENT_AMOUNT_WINDOW_SCRIPT: &str = include_str!("../scripts/reserve_sliding_window.lua");

const TOTAL_SENT_NANOTONS_KEY: &str = "faucet:stats:sent-nanotons";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SentAmountWindowReservation {
    pub id: String,
    pub total: u64,
    pub max: u64,
    pub window_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SentAmountWindowDecision {
    Reserved(SentAmountWindowReservation),
    Limited {
        current: u64,
        attempted: u64,
        max: u64,
        window_seconds: u64,
        retry_after_ms: u64,
    },
}

#[derive(Clone)]
pub struct ValkeyStore {
    connection: redis::aio::MultiplexedConnection,
}

impl ValkeyStore {
    pub async fn new(config: &ValkeyConfig) -> anyhow::Result<Self> {
        let client = redis::Client::open(config.uri.as_str()).context("Invalid Valkey URI")?;
        let connection = client
            .get_multiplexed_async_connection()
            .await
            .context("Failed to connect to Valkey")?;

        Ok(Self { connection })
    }

    pub async fn add_sent_amount(&self, amount: u64) -> anyhow::Result<u64> {
        let mut connection = self.connection.clone();
        redis::cmd("INCRBY")
            .arg(TOTAL_SENT_NANOTONS_KEY)
            .arg(amount)
            .query_async(&mut connection)
            .await
            .context("Failed to increment total sent amount")
    }

    pub async fn reserve_sent_amount_window(
        &self,
        amount: u64,
        max_amount: u64,
        window_seconds: u64,
    ) -> anyhow::Result<SentAmountWindowDecision> {
        anyhow::ensure!(window_seconds > 0, "Sent amount window must be positive");

        let ttl_seconds = window_seconds.saturating_mul(2).max(1);

        let mut connection = self.connection.clone();
        let response: (u64, u64, String, u64) = redis::Script::new(SENT_AMOUNT_WINDOW_SCRIPT)
            .key(SENT_AMOUNT_WINDOW_KEY)
            .key(SENT_AMOUNT_WINDOW_SEQ_KEY)
            .arg(amount)
            .arg(max_amount)
            .arg(window_seconds)
            .arg(ttl_seconds)
            .invoke_async(&mut connection)
            .await
            .context("Failed to reserve sent amount window")?;

        let decision = response.0;
        let current_or_total = response.1;
        let retry_after_ms = response.3;

        match decision {
            1 => Ok(SentAmountWindowDecision::Reserved(
                SentAmountWindowReservation {
                    id: response.2,
                    total: current_or_total,
                    max: max_amount,
                    window_seconds,
                },
            )),
            0 => Ok(SentAmountWindowDecision::Limited {
                current: current_or_total,
                attempted: amount,
                max: max_amount,
                window_seconds,
                retry_after_ms,
            }),
            value => anyhow::bail!("Unexpected sent amount window decision: {value}"),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn accepts_plain_and_tls_valkey_uris() {
        redis::Client::open("redis://127.0.0.1:6379/0").unwrap();
        redis::Client::open("rediss://user:password@hostname").unwrap();
    }
}
