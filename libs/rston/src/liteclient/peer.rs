//! Multiplexing correlation for LiteAPI queries carried by an ADNL peer.

use std::task::Poll;

use crate::adnl::AdnlError;
use futures::{Sink, Stream};
use pin_project::pin_project;
use rand8::random;
use tokio_tower::multiplex::TagStore;
use tokio_util::bytes::Bytes;

use crate::{
    liteclient::types::LiteError,
    tl::{adnl::Message, common::Int256},
};

#[pin_project]
/// ADNL stream adapter that decodes envelopes and correlates replies by query ID.
pub struct LitePeer<T> {
    #[pin]
    inner: T,
}

impl<T> LitePeer<T> {
    /// Wrap an established ADNL stream; handshake and lifecycle management belong to the caller.
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T> Sink<Message> for LitePeer<T>
where
    T: Sink<Bytes, Error = AdnlError>,
{
    type Error = LiteError;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.project()
            .inner
            .poll_ready(cx)
            .map_err(LiteError::AdnlError)
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        let data = tl_proto::serialize(item).into();
        self.project()
            .inner
            .start_send(data)
            .map_err(LiteError::AdnlError)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.project()
            .inner
            .poll_flush(cx)
            .map_err(LiteError::AdnlError)
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.project()
            .inner
            .poll_close(cx)
            .map_err(LiteError::AdnlError)
    }
}

impl<T> Stream for LitePeer<T>
where
    T: Stream<Item = Result<Bytes, AdnlError>>,
{
    type Item = Result<Message, LiteError>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let inner = self.project().inner.poll_next(cx);
        match inner {
            Poll::Ready(Some(Ok(bytes))) => {
                let decoded = tl_proto::deserialize(&bytes);
                Poll::Ready(Some(decoded.map_err(|e| {
                    LiteError::TlError(crate::tl::TlError::ParseError(e.to_string()))
                })))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(LiteError::AdnlError(e)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(PartialEq, Eq)]
/// Correlation token used by the multiplexed transport.
pub enum LiteTag {
    /// Query correlation token, matched with an ADNL answer.
    Int256(Int256),
    /// Ping correlation token, matched with a pong.
    Long(u64),
    /// Correlation slot for the TCP authentication exchange.
    Auth,
}

impl<T> TagStore<Message, Message> for LitePeer<T> {
    type Tag = LiteTag;

    fn assign_tag(self: std::pin::Pin<&mut Self>, r: &mut Message) -> Self::Tag {
        match r {
            Message::Answer { query_id, .. } => {
                *query_id = Int256::random();
                LiteTag::Int256(query_id.clone())
            }
            Message::Query { query_id, .. } => {
                *query_id = Int256::random();
                LiteTag::Int256(query_id.clone())
            }
            Message::Ping { random_id } => {
                *random_id = random();
                LiteTag::Long(*random_id)
            }
            Message::Pong { random_id } => {
                *random_id = random();
                LiteTag::Long(*random_id)
            }
            Message::Authenticate { .. }
            | Message::AuthenticationNonce { .. }
            | Message::AuthenticationComplete { .. } => LiteTag::Auth,
        }
    }

    fn finish_tag(self: std::pin::Pin<&mut Self>, r: &Message) -> Self::Tag {
        match r {
            Message::Answer { query_id, .. } => LiteTag::Int256(query_id.clone()),
            Message::Query { query_id, .. } => LiteTag::Int256(query_id.clone()),
            Message::Ping { random_id } => LiteTag::Long(*random_id),
            Message::Pong { random_id } => LiteTag::Long(*random_id),
            Message::Authenticate { .. }
            | Message::AuthenticationNonce { .. }
            | Message::AuthenticationComplete { .. } => LiteTag::Auth,
        }
    }
}
