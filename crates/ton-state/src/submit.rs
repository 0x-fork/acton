//! Admission and P2P submission of signed external messages.

#[cfg(test)]
mod tests;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use ton_indexer_core::normalized_external_message_hash;
use ton_p2p::{ExternalMessage, MessageSender};
use toncenter::v2::{TonlibResponse, requests::SendBocRequest, responses::ResultOk};
use tracing::{info, warn};
use tycho_types::{
    cell::HashBytes,
    models::{Message, StdAddr},
};

use crate::api::ApiError;
use crate::confirmation::Confirmations;

// Both HTTP methods share one outbound transport and its admission budget.
type Broadcast =
    Arc<dyn Fn(ExternalMessage) -> BoxFuture<'static, anyhow::Result<()>> + Send + Sync>;

#[derive(Clone)]
struct Submission {
    broadcast: Broadcast,
    capacity: Arc<Semaphore>,
    confirmations: Confirmations,
}

/// Submission shares the synchronization transport. Bounded admission keeps
/// message decoding and discovery from exhausting the HTTP worker pool.
pub(crate) fn router(sender: MessageSender, confirmations: Confirmations) -> Router {
    Submission {
        broadcast: Arc::new(move |message| {
            let sender = sender.clone();
            Box::pin(async move { sender.send(message).await })
        }),
        capacity: Arc::new(Semaphore::new(16)),
        confirmations,
    }
    .router()
}

impl Submission {
    fn router(self) -> Router {
        Router::new()
            .route("/api/v2/sendBoc", post(send_boc))
            .route(
                "/api/v2/sendBocAndWaitTransaction",
                post(send_boc_and_wait_transaction),
            )
            .layer(DefaultBodyLimit::max(96 * 1024))
            .with_state(self)
    }
}

/// Submit an external message
///
/// Broadcast a signed inbound external message through P2P. Accepts a base64 `BoC`
/// up to 65,535 decoded bytes for a standard masterchain or basechain destination
///
/// Only the envelope is checked; the message is not emulated. Success means
/// queued for broadcast, not accepted or included in a block
#[utoipa::path(
    post,
    path = "/api/v2/sendBoc",
    operation_id = "sendBoc",
    request_body = SendBocRequest,
    responses(
        (status = 200, description = "P2P broadcast queued", body = TonlibResponse<ResultOk>),
        (status = 400, description = "Invalid JSON, base64 or message", body = toncenter::v2::TonlibErrorResponse),
        (status = 413, description = "BoC exceeds 65,535 bytes or JSON exceeds 96 KiB", body = toncenter::v2::TonlibErrorResponse),
        (status = 415, description = "Expected application/json", body = toncenter::v2::TonlibErrorResponse),
        (status = 422, description = "Invalid request fields", body = toncenter::v2::TonlibErrorResponse),
        (status = 429, description = "Submission queue is full", body = toncenter::v2::TonlibErrorResponse),
        (status = 500, description = "Message decoding failed", body = toncenter::v2::TonlibErrorResponse),
        (status = 503, description = "P2P submission failed; retry later", body = toncenter::v2::TonlibErrorResponse),
    ),
)]
async fn send_boc(
    State(submission): State<Submission>,
    request: Result<Json<SendBocRequest>, JsonRejection>,
) -> Response {
    let started = Instant::now();
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => {
            return ApiError::new(error.status(), "expected JSON with a base64 boc field")
                .into_response();
        }
    };
    let Ok(permit) = submission.capacity.try_acquire_owned() else {
        return ApiError::new(StatusCode::TOO_MANY_REQUESTS, "too many pending messages")
            .into_response();
    };

    // Hold the permit inside the worker so cancellation cannot free its slot
    // while decoding is still running.
    let parsed = tokio::task::spawn_blocking(move || {
        let message = parse_message(request);
        (permit, message)
    })
    .await;
    let (_permit, message) = match parsed {
        Ok((permit, Ok(message))) => (permit, message),
        Ok((_, Err(error))) => return error.into_response(),
        Err(_) => {
            return ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "message decoding failed")
                .into_response();
        }
    };

    let hash = message.hash();
    if let Err(error) = (submission.broadcast)(message).await {
        warn!(
            operation = "http_message_submission",
            target = %hash,
            duration_ms = started.elapsed().as_millis(),
            outcome = "failed",
            error = %format!("{error:#}"),
            "could not submit external message through P2P",
        );

        return ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "P2P submission failed; retry later",
        )
        .into_response();
    }

    Json(TonlibResponse {
        ok: true,
        result: ResultOk {
            type_tag: Default::default(),
        },
        extra: String::new(),
        jsonrpc: None,
        id: None,
    })
    .into_response()
}

fn parse_message(request: SendBocRequest) -> Result<ExternalMessage, ApiError> {
    let boc = STANDARD
        .decode(request.boc)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "boc must contain valid base64"))?;
    if boc.len() > ExternalMessage::MAX_BYTES {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "boc exceeds 65535 bytes",
        ));
    }

    ExternalMessage::new(boc)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid inbound external message"))
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct SendBocAndWaitRequest {
    /// Signed inbound external message, encoded as a base64 `BoC`
    boc: String,
    /// Total processing budget after reading the request body; defaults to 30 seconds
    #[schema(minimum = 1000, maximum = 120_000, default = 30_000)]
    timeout_ms: Option<u64>,
}

#[derive(Serialize, utoipa::ToSchema)]
struct SendBocAndWaitResult {
    /// Original committed transaction in the same format as getTransactions
    transaction: toncenter::v2::responses::Transaction,
    /// Full coordinates of the block containing this transaction
    block_id: toncenter::v2::responses::TonBlockIdExt,
    /// Checkpoint whose complete batch made the transaction visible locally
    mc_block_seqno: u32,
    /// TEP-467 external-message lookup hash, in base64
    normalized_message_hash: String,
}

#[derive(Serialize, utoipa::ToSchema)]
struct WaitErrorResponse {
    ok: bool,
    error: &'static str,
    code: u16,
    /// Present once the message has been decoded; usable for subsequent lookup
    #[serde(skip_serializing_if = "Option::is_none")]
    normalized_message_hash: Option<String>,
}

struct WaitProgress {
    started: Instant,
    destination: Option<StdAddr>,
    hash: Option<HashBytes>,
    outcome: &'static str,
}

impl Drop for WaitProgress {
    fn drop(&mut self) {
        info!(
            operation = "send_boc_and_wait_transaction",
            target = ?self.destination,
            message_hash = ?self.hash,
            duration_ms = self.started.elapsed().as_millis(),
            outcome = self.outcome,
            "finished external message submission and observation",
        );
    }
}

/// Submit a message and wait for its transaction
///
/// Registers a live observation before P2P submission and returns the transaction
/// after its complete masterchain/shard batch commits. Aborted transactions are
/// returned too; child transactions are not awaited. Previously committed
/// transactions are not replayed, including after a retry or process restart
///
/// Timeout covers decoding, submission and observation after the body is read
/// A timeout or disconnect does not withdraw a message already sent to peers
#[utoipa::path(
    post,
    path = "/api/v2/sendBocAndWaitTransaction",
    operation_id = "sendBocAndWaitTransaction",
    request_body = SendBocAndWaitRequest,
    responses(
        (status = 200, description = "Transaction included in a committed block", body = TonlibResponse<SendBocAndWaitResult>),
        (status = 400, description = "Invalid message or timeout", body = WaitErrorResponse),
        (status = 413, description = "BoC exceeds 65,535 bytes or JSON exceeds 96 KiB", body = WaitErrorResponse),
        (status = 415, description = "Expected application/json", body = WaitErrorResponse),
        (status = 422, description = "Invalid request fields", body = WaitErrorResponse),
        (status = 429, description = "Submission or observation capacity exhausted", body = WaitErrorResponse),
        (status = 500, description = "Message decoding or transaction encoding failed", body = WaitErrorResponse),
        (status = 503, description = "Submission failed or transaction observation unavailable", body = WaitErrorResponse),
        (status = 504, description = "Transaction not observed before the deadline; execution may still occur", body = WaitErrorResponse),
    ),
)]
#[expect(
    clippy::significant_drop_tightening,
    reason = "The observation slot remains reserved until response encoding finishes"
)]
async fn send_boc_and_wait_transaction(
    State(submission): State<Submission>,
    request: Result<Json<SendBocAndWaitRequest>, JsonRejection>,
) -> Response {
    let mut progress = WaitProgress {
        started: Instant::now(),
        destination: None,
        hash: None,
        outcome: "cancelled",
    };
    let deadline_start = tokio::time::Instant::now();
    let outcome = async {
        let Json(request) = request.map_err(|error| {
            ApiError::new(
                error.status(),
                "expected JSON with boc and optional timeout_ms",
            )
        })?;
        let timeout_ms = request.timeout_ms.unwrap_or(30_000);
        if !(1_000..=120_000).contains(&timeout_ms) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "timeout_ms must be between 1000 and 120000",
            ));
        }
        let deadline = deadline_start + Duration::from_millis(timeout_ms);
        tokio::time::timeout_at(deadline, async {
            let permit = submission
                .capacity
                .clone()
                .try_acquire_owned()
                .map_err(|_| {
                    ApiError::new(StatusCode::TOO_MANY_REQUESTS, "too many pending messages")
                })?;

            // Decoding retains its admission slot even if the HTTP future is cancelled.
            let (permit, message, hash) = tokio::task::spawn_blocking(move || {
                let message = parse_message(SendBocRequest { boc: request.boc })?;
                let parsed = message.root().parse::<Message>().map_err(|_| {
                    ApiError::new(StatusCode::BAD_REQUEST, "invalid inbound external message")
                })?;
                let hash = normalized_external_message_hash(&parsed).map_err(|_| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "cannot normalize inbound external message",
                    )
                })?;
                Ok::<_, ApiError>((permit, message, hash))
            })
            .await
            .map_err(|_| {
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "message decoding failed")
            })??;

            let destination = message.destination().clone();
            progress.destination = Some(destination.clone());
            progress.hash = Some(hash);
            let started = progress.started;
            let mut observation = submission
                .confirmations
                .register(destination.clone(), hash)?;
            info!(
                operation = "send_boc_and_wait_transaction",
                target = %destination,
                message_hash = %hash,
                duration_ms = progress.started.elapsed().as_millis(),
                outcome = "registered",
                "registered transaction observation before broadcast",
            );

            let send = async move {
                let _permit = permit;
                (submission.broadcast)(message).await.map_err(|error| {
                    warn!(
                        operation = "send_boc_and_wait_transaction",
                        target = %destination,
                        message_hash = %hash,
                        duration_ms = started.elapsed().as_millis(),
                        error = %format!("{error:#}"),
                        outcome = "submission_failed",
                        "could not submit external message through P2P",
                    );
                    ApiError::new(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "P2P submission failed; execution may still occur",
                    )
                })
            };
            let observed = tokio::select! {
                biased;
                observed = &mut observation.receiver => observed,
                sent = send => {
                    match sent {
                        Ok(()) => (&mut observation.receiver).await,
                        // A peer may have received the message even if a later send
                        // failed. An already observed transaction takes precedence.
                        Err(error) => match observation.receiver.try_recv() {
                            Ok(transaction) => Ok(transaction),
                            Err(_) => return Err(error),
                        },
                    }
                }
            }
            .map_err(|_| {
                ApiError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "transaction_observation_unavailable",
                )
            })?;

            tokio::task::spawn_blocking(move || {
                // Keep the observation slot while serializing its retained transaction.
                let converted = (|| -> anyhow::Result<_> {
                    let tx = observed.transaction.load()?;
                    let address = StdAddr::new(i8::try_from(observed.block.workchain)?, tx.account);
                    Ok(SendBocAndWaitResult {
                        transaction: crate::api::transactions::convert(
                            &address,
                            &observed.transaction,
                            &tx,
                        )?,
                        block_id: crate::api::block_id(observed.block.try_into()?),
                        mc_block_seqno: observed.mc_seqno,
                        normalized_message_hash: STANDARD.encode(hash),
                    })
                })();
                let response = match converted {
                    Ok(result) => Ok(Json(TonlibResponse {
                        ok: true,
                        result,
                        extra: String::new(),
                        jsonrpc: None,
                        id: None,
                    })
                    .into_response()),
                    Err(error) => {
                        warn!(
                            operation = "send_boc_and_wait_transaction",
                            target = %observed.block,
                            transaction_hash = %observed.transaction.inner().repr_hash(),
                            duration_ms = started.elapsed().as_millis(),
                            error = %format!("{error:#}"),
                            outcome = "encoding_failed",
                            "could not encode committed transaction",
                        );
                        Err(ApiError::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "transaction encoding failed",
                        ))
                    }
                };
                drop(observation);
                response
            })
            .await
            .map_err(|_| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "transaction encoding failed",
                )
            })?
        })
        .await
        .map_err(|_| ApiError::new(StatusCode::GATEWAY_TIMEOUT, "transaction_wait_timeout"))?
    }
    .await;

    match outcome {
        Ok(response) => {
            progress.outcome = "confirmed";
            response
        }
        Err(error) => {
            progress.outcome = if error.status == StatusCode::GATEWAY_TIMEOUT {
                "timeout"
            } else {
                "failed"
            };
            (
                error.status,
                Json(WaitErrorResponse {
                    ok: false,
                    error: error.message,
                    code: error.status.as_u16(),
                    normalized_message_hash: progress.hash.map(|hash| STANDARD.encode(hash)),
                }),
            )
                .into_response()
        }
    }
}
