//! Queries over a multiplexed ADNL connection.

use std::time::{Duration, Instant};

use tokio::net::ToSocketAddrs;
use tower::{Layer, Service, ServiceExt};

use super::config::ConfigLiteServer;
use super::layers::WrapRawMessagesLayer;
use super::peer::LitePeer;
use super::types::LiteError;
use crate::adnl::AdnlPeer;
use crate::tl::request::{RawWrappedRequest, Request, WaitMasterchainSeqno};
use crate::tl::response::Response;
use crate::tl::utils::FromResponse;

/// Bounds for connection setup and each query. All methods require a Tokio runtime.
#[derive(Debug, Clone, Copy)]
pub struct LiteClientOptions {
    /// Deadline for TCP connection setup and the ADNL handshake together.
    pub connect_timeout: Duration,
    /// Deadline for service readiness and the response together.
    /// A timeout cannot establish whether a state-changing request was executed.
    pub request_timeout: Duration,
}

impl Default for LiteClientOptions {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(10),
        }
    }
}

/// LiteAPI queries through an authenticated ADNL/TCP connection.
///
/// The caller supplies a trusted server identity. Responses, including proofs,
/// remain unverified blockchain data. Requests are never retried automatically.
/// Dropping the client releases its connection. A failed transport requires a
/// new connection; a request timeout alone does not discard the connection.
pub struct LiteClient {
    inner: tower::util::BoxService<RawWrappedRequest, Vec<u8>, LiteError>,
    request_timeout: Duration,
    wait_seqno: Option<WaitMasterchainSeqno>,
    target: String,
}

impl LiteClient {
    /// Connect with the default ten-second setup and request deadlines.
    pub async fn connect<A: ToSocketAddrs>(
        address: A,
        public_key: impl AsRef<[u8]>,
    ) -> Result<Self, LiteError> {
        Self::connect_with_options(address, public_key, LiteClientOptions::default()).await
    }

    /// Connect with explicit deadlines. The setup deadline covers both phases.
    pub async fn connect_with_options<A: ToSocketAddrs>(
        address: A,
        public_key: impl AsRef<[u8]>,
        options: LiteClientOptions,
    ) -> Result<Self, LiteError> {
        let target = hex::encode(public_key.as_ref());
        let started = Instant::now();
        log::debug!("operation=lite_connect node={target} outcome=started");

        let result = tokio::time::timeout(
            options.connect_timeout,
            AdnlPeer::connect(public_key, address),
        )
        .await
        .map_err(|_| LiteError::Timeout {
            operation: "lite_connect",
            timeout: options.connect_timeout,
        })
        .and_then(|result| result.map_err(LiteError::from));

        log::debug!(
            "operation=lite_connect node={target} duration_ms={} outcome={}",
            started.elapsed().as_millis(),
            if result.is_ok() { "success" } else { "failure" }
        );

        let peer = LitePeer::new(result?);
        let multiplex =
            tokio_tower::multiplex::Client::<_, Box<dyn std::error::Error + Send + Sync>, _>::new(
                peer,
            );

        Ok(Self {
            inner: WrapRawMessagesLayer.layer(multiplex).boxed(),
            request_timeout: options.request_timeout,
            wait_seqno: None,
            target,
        })
    }

    /// Connect to an endpoint from a previously parsed global network config.
    pub async fn connect_liteserver(
        server: &ConfigLiteServer,
        options: LiteClientOptions,
    ) -> Result<Self, LiteError> {
        Self::connect_with_options(server.socket_addr(), server.public_key(), options).await
    }

    /// Set the deadline for subsequent queries, including service readiness.
    pub fn set_request_timeout(&mut self, timeout: Duration) {
        self.request_timeout = timeout;
    }

    /// Prefix the next query with a server-side masterchain sequence wait.
    /// The prefix is consumed once, including when that query fails or times out.
    /// The request deadline still applies to the whole operation.
    pub fn wait_masterchain_seqno(&mut self, seqno: u32, timeout: Duration) {
        self.wait_seqno = Some(WaitMasterchainSeqno {
            seqno,
            timeout_ms: timeout.as_millis().min(u32::MAX as u128) as u32,
        });
    }

    /// Send boxed TL request bytes and return the answer bytes unchanged.
    /// Unknown constructors and `liteServer.error` answers remain available to
    /// custom decoders. Cancellation and timeouts never trigger a retry.
    pub async fn query_raw(&mut self, request: impl AsRef<[u8]>) -> Result<Vec<u8>, LiteError> {
        let request = RawWrappedRequest {
            wait_masterchain_seqno: self.wait_seqno.take(),
            request: request.as_ref().to_vec(),
        };
        let timeout = self.request_timeout;
        let started = Instant::now();
        log::debug!("operation=lite_query node={} outcome=started", self.target);

        let result = tokio::time::timeout(timeout, async {
            self.inner.ready().await?.call(request).await
        })
        .await
        .map_err(|_| LiteError::Timeout {
            operation: "lite_query",
            timeout,
        })
        .and_then(|result| result);

        log::debug!(
            "operation=lite_query node={} duration_ms={} outcome={}",
            self.target,
            started.elapsed().as_millis(),
            if result.is_ok() { "success" } else { "failure" }
        );
        result
    }

    /// Decode a known response variant. Server error codes are preserved.
    /// A different response constructor or trailing bytes cause an error.
    /// This checks the wire format, not the validity of blockchain proofs.
    pub async fn query_typed<T: FromResponse>(&mut self, request: Request) -> Result<T, LiteError> {
        let bytes = self.query_raw(tl_proto::serialize(request)).await?;
        let mut remaining = bytes.as_slice();
        let response =
            <Response as tl_proto::TlRead>::read_from(&mut remaining).map_err(|error| {
                LiteError::TlError(crate::tl::TlError::ParseError(error.to_string()))
            })?;

        if !remaining.is_empty() {
            return Err(LiteError::TlError(crate::tl::TlError::InvalidData));
        }

        match response {
            Response::Error(error) => Err(LiteError::ServerError(error)),
            response => T::from_response(response),
        }
    }
}
