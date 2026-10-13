//! TL bytes-wrapper codecs and typed response extraction.

use tl_proto::{TlRead, TlResult};

#[cfg(feature = "liteclient")]
use crate::liteclient::types::LiteError;

#[cfg(feature = "liteclient")]
use super::response::*;

/// Read an optional boxed prefix without consuming the following request on failure.
pub fn lossy_read<'tl, T: TlRead<'tl>>(packet: &mut &'tl [u8]) -> TlResult<Option<T>> {
    let mut slice = *packet;
    let result = T::read_from(&mut slice);
    if let Ok(x) = result {
        *packet = slice;
        Ok(Some(x))
    } else {
        Ok(None)
    }
}

/// Encode a TL value within a length-prefixed bytes field.
pub mod struct_as_bytes {
    use tl_proto::{TlPacket, TlRead, TlResult, TlWrite};

    /// Return the encoded TL bytes-field size, including its length prefix and padding.
    pub fn size_hint<T: TlWrite>(v: &T) -> usize {
        tl_proto::serialize(v).max_size_hint()
    }

    /// Serialize a TL value inside a length-prefixed TL bytes field.
    pub fn write<P: TlPacket, T: TlWrite>(v: &T, packet: &mut P) {
        tl_proto::serialize(v).write_to(packet)
    }

    /// Decode exactly one value from a TL bytes field, advance the outer cursor, and reject trailing inner bytes.
    pub fn read<'tl, T: TlRead<'tl>>(packet: &mut &'tl [u8]) -> TlResult<T> {
        let mut bytes = <&'tl [u8]>::read_from(packet)?;
        let value = T::read_from(&mut bytes)?;
        if !bytes.is_empty() {
            return Err(tl_proto::TlError::InvalidData);
        }
        Ok(value)
    }
}

#[cfg(feature = "liteclient")]
/// Extract a typed payload from a known boxed response without validating blockchain proofs.
pub trait FromResponse: Sized {
    /// Extract the expected constructor or return `LiteError::UnexpectedMessage`.
    fn from_response(response: Response) -> Result<Self, LiteError>;
}

#[cfg(feature = "liteclient")]
macro_rules! impl_from_response {
    ($($variant:ident),* $(,)?) => {
        $(
            impl FromResponse for $variant {
                fn from_response(response: Response) -> Result<Self, LiteError> {
                    match response {
                        Response::$variant(value) => Ok(value),
                        _ => Err(LiteError::UnexpectedMessage),
                    }
                }
            }
        )*
    };
}

#[cfg(feature = "liteclient")]
impl_from_response!(
    MasterchainInfo,
    MasterchainInfoExt,
    CurrentTime,
    Version,
    BlockData,
    BlockState,
    BlockHeader,
    SendMsgStatus,
    AccountState,
    RunMethodResult,
    ShardInfo,
    AllShardsInfo,
    TransactionInfo,
    TransactionList,
    TransactionId,
    BlockTransactions,
    BlockTransactionsExt,
    PartialBlockProof,
    ConfigInfo,
    ValidatorStats,
    LibraryResult,
    LibraryResultWithProof,
    ShardBlockProof,
    LookupBlockResult,
    OutMsgQueueSizes,
    BlockOutMsgQueueSize,
    DispatchQueueInfo,
    DispatchQueueMessages,
    NonfinalValidatorGroups,
    NonfinalCandidate,
    NonfinalPendingShardBlocks,
    Error,
);
