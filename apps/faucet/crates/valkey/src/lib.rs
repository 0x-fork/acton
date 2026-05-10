use anyhow::Context;
use faucet_config::ValkeyConfig;

const TOTAL_SENT_NANOTONS_KEY: &str = "faucet:stats:sent-nanotons";

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

    pub async fn add_sent_amount(&self, amount: u64) -> anyhow::Result<i64> {
        let amount = i64::try_from(amount).context("Sent amount does not fit in Valkey integer")?;
        let mut connection = self.connection.clone();
        redis::cmd("INCRBY")
            .arg(TOTAL_SENT_NANOTONS_KEY)
            .arg(amount)
            .query_async(&mut connection)
            .await
            .context("Failed to increment total sent amount")
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
