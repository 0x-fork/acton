//! Live matching of submitted messages against fully committed block batches.

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use axum::http::StatusCode;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};
use ton_indexer_core::{Batch, BlockId, normalized_external_message_hash};
use tycho_types::cell::{HashBytes, Lazy};
use tycho_types::models::{Message, MsgInfo, StdAddr, Transaction};

use crate::api::ApiError;

const MAX_WAITERS: usize = 64;

/// Bounded, live-only observations of committed transactions. Registrations must
/// precede broadcast; previously published batches are never replayed.
#[derive(Clone)]
pub(crate) struct Confirmations {
    waiters: Arc<Mutex<Vec<Waiter>>>,
    capacity: Arc<Semaphore>,
}

struct Waiter {
    destination: StdAddr,
    hash: HashBytes,
    sender: oneshot::Sender<ConfirmedTransaction>,
}

/// Retains the original transaction cell until the HTTP response is encoded.
/// Block coordinates refer to the batch already committed by the synchronizer.
#[derive(Clone)]
pub(crate) struct ConfirmedTransaction {
    pub(crate) block: BlockId,
    pub(crate) mc_seqno: u32,
    pub(crate) transaction: Lazy<Transaction>,
}

/// Dropping an observation unregisters it immediately, including on cancellation.
/// The capacity permit lives until its buffered result is consumed or dropped.
pub(crate) struct Observation {
    pub(crate) receiver: oneshot::Receiver<ConfirmedTransaction>,
    confirmations: Confirmations,
    _slot: OwnedSemaphorePermit,
}

impl Default for Confirmations {
    fn default() -> Self {
        Self {
            waiters: Arc::default(),
            capacity: Arc::new(Semaphore::new(MAX_WAITERS)),
        }
    }
}

impl Confirmations {
    /// Reserves a live observation before submission. A full or closed registry
    /// rejects the request before any message is sent to the network.
    pub(crate) fn register(
        &self,
        destination: StdAddr,
        hash: HashBytes,
    ) -> Result<Observation, ApiError> {
        let mut waiters = self.waiters.lock().expect("confirmation lock poisoned");
        if self.capacity.is_closed() {
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "shutting_down",
            ));
        }
        let slot = self.capacity.clone().try_acquire_owned().map_err(|_| {
            ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "too many pending transaction waits",
            )
        })?;
        let (sender, receiver) = oneshot::channel();
        waiters.push(Waiter {
            destination,
            hash,
            sender,
        });

        Ok(Observation {
            receiver,
            confirmations: self.clone(),
            _slot: slot,
        })
    }

    /// Observes a batch only after its complete state and history have committed.
    /// All matching requests receive the first matching transaction, including
    /// aborted transactions. Call [`Self::fail`] on error to report the gap.
    pub(crate) fn publish(&self, batch: &Batch) -> Result<()> {
        if self
            .waiters
            .lock()
            .expect("confirmation lock poisoned")
            .is_empty()
        {
            return Ok(());
        }

        for block in batch.blocks() {
            for lazy in block.transactions() {
                let tx = lazy.load().with_context(|| {
                    format!(
                        "cannot decode transaction {} in block {}",
                        lazy.inner().repr_hash(),
                        block.id()
                    )
                })?;
                let interested = self
                    .waiters
                    .lock()
                    .expect("confirmation lock poisoned")
                    .iter()
                    .any(|waiter| {
                        i32::from(waiter.destination.workchain) == block.id().workchain
                            && waiter.destination.address == tx.account
                    });
                if !interested {
                    continue;
                }
                let Some(cell) = &tx.in_msg else {
                    continue;
                };
                let message = cell.parse::<Message>().with_context(|| {
                    format!(
                        "cannot decode incoming message in transaction {} in block {}",
                        lazy.inner().repr_hash(),
                        block.id()
                    )
                })?;
                let MsgInfo::ExtIn(info) = &message.info else {
                    continue;
                };
                let hash = normalized_external_message_hash(&message).with_context(|| {
                    format!(
                        "cannot normalize incoming message in transaction {} in block {}",
                        lazy.inner().repr_hash(),
                        block.id()
                    )
                })?;
                let mut waiters = self.waiters.lock().expect("confirmation lock poisoned");
                let mut index = 0;
                while index < waiters.len() {
                    let waiter = &waiters[index];
                    if waiter.hash == hash
                        && info.dst == waiter.destination.clone().into()
                        && i32::from(waiter.destination.workchain) == block.id().workchain
                        && waiter.destination.address == tx.account
                    {
                        let waiter = waiters.swap_remove(index);
                        let _ = waiter.sender.send(ConfirmedTransaction {
                            block: block.id(),
                            mc_seqno: batch.checkpoint().seqno,
                            transaction: lazy.clone(),
                        });
                    } else {
                        index += 1;
                    }
                }
                drop(waiters);
            }
        }
        Ok(())
    }

    /// Fails current observations after a publication gap. Later requests can
    /// register again; they cannot recover the missed batch.
    pub(crate) fn fail(&self) {
        self.waiters
            .lock()
            .expect("confirmation lock poisoned")
            .clear();
    }

    /// Rejects new observations and wakes pending requests during shutdown.
    pub(crate) fn close(&self) {
        self.capacity.close();
        self.fail();
    }
}

impl Drop for Observation {
    fn drop(&mut self) {
        self.receiver.close();
        self.confirmations
            .waiters
            .lock()
            .expect("confirmation lock poisoned")
            .retain(|waiter| !waiter.sender.is_closed());
    }
}
