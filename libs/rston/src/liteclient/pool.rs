//! Round-robin queries across configured liteservers.

use super::{ConfigGlobal, ConfigLiteServer, LiteClient, LiteClientOptions, LiteError};
use crate::tl::{Request, utils::FromResponse};

/// A pool of lazily connected liteservers selected in config order.
///
/// Each query rotates to the next endpoint. Connection failures skip an endpoint
/// before sending any request. Once sent, a query is never retried, including
/// state-changing operations and ambiguous timeouts. Subsequent queries can use
/// another server. No server response is treated as a verified blockchain proof.
pub struct LiteClientPool {
    servers: Vec<(ConfigLiteServer, Option<LiteClient>)>,
    next: usize,
    options: LiteClientOptions,
}

impl LiteClientPool {
    /// Create a pool without network access. An empty config returns an error.
    pub fn new(config: ConfigGlobal, options: LiteClientOptions) -> Result<Self, LiteError> {
        if config.liteservers.is_empty() {
            return Err(LiteError::NoServers);
        }

        Ok(Self {
            servers: config
                .liteservers
                .into_iter()
                .map(|server| (server, None))
                .collect(),
            next: 0,
            options,
        })
    }

    async fn next_client(&mut self) -> Result<&mut LiteClient, LiteError> {
        let mut last_error = LiteError::NoServers;

        for _ in 0..self.servers.len() {
            let index = self.next;
            self.next = (index + 1) % self.servers.len();

            if self.servers[index].1.is_none() {
                match LiteClient::connect_liteserver(&self.servers[index].0, self.options).await {
                    Ok(client) => self.servers[index].1 = Some(client),
                    Err(error) => {
                        log::debug!(
                            "operation=lite_pool_connect node={} outcome=failure error={error}",
                            self.servers[index].0.socket_addr()
                        );
                        last_error = error;
                        continue;
                    }
                }
            }

            return Ok(self.servers[index].1.as_mut().unwrap());
        }

        Err(last_error)
    }

    /// Send the bytes once through the next reachable endpoint.
    /// Connection setup can take up to one setup deadline per configured server.
    pub async fn query_raw(&mut self, request: impl AsRef<[u8]>) -> Result<Vec<u8>, LiteError> {
        self.next_client().await?.query_raw(request).await
    }

    /// Send a known request once and decode its response without proof validation.
    pub async fn query_typed<T: FromResponse>(&mut self, request: Request) -> Result<T, LiteError> {
        self.next_client().await?.query_typed(request).await
    }

    /// Drop all active connections. The next query reconnects lazily.
    /// Call this after transport failures or when network access resumes.
    pub fn disconnect(&mut self) {
        for (_, client) in &mut self.servers {
            *client = None;
        }
    }
}
