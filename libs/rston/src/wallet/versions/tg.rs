//! TG Wallet revision 00: signature-prefixed transfers and a stable address anchor.
//!
//! Deployment uses a trampoline whose code hash is
//! `9149ae51c1e4689710cebf7830297b16acfbadb363a920a537893e7ffeeca768`.
//! Execution requires the wallet implementation in blockchain configuration
//! parameter `-123`; the trampoline alone cannot execute transfers.
//!
//! A request sends one message inline or up to 255 messages in a Tolk array.
//! The caller supplies the current sequence number, expiration, and send modes.
//! These types model deployment data and external transfers. They do not perform
//! key rotation, inspect account state, or verify signatures.
//!
//! Reference: [Wallet Engine's revision-00 wire types](https://github.com/i582/wallet-engine/blob/master/vendor/ton/src/ton_wallet/wallet_tlb/wallet.rs)
//! and [anchor/signing key handling](https://github.com/i582/wallet-engine/blob/master/src/wallet/crypto.rs).

use crate::cell::{Cell, CellBuilder, CellContext, CellSlice, HashBytes, Load, Store};
use crate::error::Error;
use crate::wallet::WalletMessage;

const SEND_ONE_EXTERNAL: u32 = 0x6389_6e75;
const SEND_BULK_EXTERNAL: u32 = 0x7389_6e75;

/// Initial TG Wallet data, before the trampoline initializes the account.
///
/// The wire order is revision `uint8`, sequence number `uint32`, wallet ID
/// `uint32`, and anchor public key `bits256`. After rotation, the original
/// anchor must still be used for address derivation and deployment state.
#[derive(Debug, Clone, PartialEq, Load, Store)]
pub struct TgWalletData {
    /// Implementation revision selected by the trampoline. This module uses zero.
    pub revision: u8,
    /// Sequence number for replay protection; deployment starts at zero.
    pub seqno: u32,
    /// Network and subwallet identifier, preserving all 32 wire bits.
    pub wallet_id: i32,
    /// Ed25519 anchor public key that fixes the account address.
    pub public_key: HashBytes,
}

impl TgWalletData {
    /// Creates revision-00 deployment data with sequence number zero.
    pub fn new(wallet_id: i32, public_key: HashBytes) -> Self {
        Self {
            revision: 0,
            seqno: 0,
            wallet_id,
            public_key,
        }
    }

    /// Serializes the initial data without querying or changing account state.
    pub fn to_cell(&self) -> Result<Cell, Error> {
        CellBuilder::build_from(self)
    }
}

/// An unsigned TG Wallet external transfer request.
///
/// The body contains an operation tag, wallet ID, expiration, and sequence
/// number as `uint32`, followed by one message or a Tolk message array.
/// Signing prepends a 512-bit Ed25519 signature over the body representation hash.
/// Send modes are preserved; the caller is responsible for contract requirements.
#[derive(Debug, Clone, PartialEq)]
pub struct TgWalletExtMsgBody {
    /// Must match the ID stored in the target wallet.
    pub wallet_id: i32,
    /// Expiration as a Unix timestamp in seconds.
    pub valid_until: u32,
    /// Must match the current on-chain sequence number.
    pub msg_seqno: u32,
    /// Between one and 255 complete outgoing messages, in execution order.
    pub msgs: Vec<WalletMessage>,
}

impl TgWalletExtMsgBody {
    /// Builds an unsigned body, rejecting empty or oversized message lists.
    pub fn to_cell(&self) -> Result<Cell, Error> {
        CellBuilder::build_from(self)
    }

    /// Reads a signature-prefixed request without verifying its signature.
    ///
    /// Consumes the entire slice. Unsupported tags, malformed arrays, trailing
    /// data, and truncated signatures return a cell decoding error.
    pub fn read_signed(parser: &mut CellSlice<'_>) -> Result<(Self, Vec<u8>), Error> {
        let mut signature = vec![0; 64];
        parser.load_raw(&mut signature, 512)?;
        Ok((Self::load_from(parser)?, signature))
    }
}

impl Store for TgWalletExtMsgBody {
    fn store_into(&self, dst: &mut CellBuilder, context: &dyn CellContext) -> Result<(), Error> {
        if self.msgs.is_empty() {
            return Err(Error::EmptyWalletMessages);
        }
        if self.msgs.len() > 255 {
            return Err(Error::TooManyMessages {
                actual: self.msgs.len(),
                max: 255,
            });
        }
        let single = self.msgs.len() == 1;
        dst.store_u32(if single {
            SEND_ONE_EXTERNAL
        } else {
            SEND_BULK_EXTERNAL
        })?;
        self.wallet_id.store_into(dst, context)?;
        dst.store_u32(self.valid_until)?;
        dst.store_u32(self.msg_seqno)?;

        if single {
            return self.msgs[0].store_into(dst, context);
        }

        dst.store_u8(self.msgs.len() as u8)?;
        let mut next: Option<Cell> = None;
        // Tolk reserves one reference for the next chunk, including at the tail.
        for messages in self.msgs.rchunks(3) {
            let mut chunk = CellBuilder::new();
            next.store_into(&mut chunk, context)?;
            for message in messages {
                message.store_into(&mut chunk, context)?;
            }
            next = Some(chunk.build_ext(context)?);
        }
        next.store_into(dst, context)
    }
}

impl<'a> Load<'a> for TgWalletExtMsgBody {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        let opcode = parser.load_u32()?;
        if !matches!(opcode, SEND_ONE_EXTERNAL | SEND_BULK_EXTERNAL) {
            return Err(Error::InvalidTag);
        }
        let wallet_id = Load::load_from(parser)?;
        let valid_until = parser.load_u32()?;
        let msg_seqno = parser.load_u32()?;
        let msgs = if opcode == SEND_ONE_EXTERNAL {
            vec![WalletMessage::load_from(parser)?]
        } else {
            let count = usize::from(parser.load_u8()?);
            let mut next = Option::<Cell>::load_from(parser)?;
            let mut messages = Vec::with_capacity(count);
            while let Some(chunk) = next {
                let mut slice = chunk.as_slice()?;
                next = Option::<Cell>::load_from(&mut slice)?;
                let chunk_len = slice.size_refs();
                if chunk_len == 0 || chunk_len > 3 || messages.len() + chunk_len as usize > count {
                    return Err(Error::InvalidData);
                }
                for _ in 0..chunk_len {
                    messages.push(WalletMessage::load_from(&mut slice)?);
                }
                if !slice.is_empty() {
                    return Err(Error::InvalidData);
                }
            }
            if messages.is_empty() || messages.len() != count {
                return Err(Error::InvalidData);
            }
            messages
        };
        if !parser.is_empty() {
            return Err(Error::InvalidData);
        }
        Ok(Self {
            wallet_id,
            valid_until,
            msg_seqno,
            msgs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::{SendMsgFlags, WalletVersion, get_code};

    #[test]
    fn matches_wallet_engine_tolk_vectors() -> anyhow::Result<()> {
        // Independent hashes from wallet-engine/tests/wallet_contract_parity.rs,
        // produced by serializing the original rev00 Tolk types with toCell().
        assert_eq!(
            get_code(WalletVersion::TgWallet)?.repr_hash().to_string(),
            "9149ae51c1e4689710cebf7830297b16acfbadb363a920a537893e7ffeeca768",
        );
        let storage = TgWalletData {
            revision: 0,
            seqno: 0x0102_0304,
            wallet_id: 0x7fff_7ffd,
            public_key: HashBytes([0x11; 32]),
        };
        assert_eq!(
            storage.to_cell()?.repr_hash().to_string(),
            "879617849c5eb8056d68eeb4fa84d090bfc11123e069740b854f3678f8678d64",
        );
        for (values, expected_hash) in [
            (
                vec![0xa3],
                "b97c66d7258200b0afbe52c5cbef29085a21710ee5836ea2569ab59535f4e48d",
            ),
            (
                (0xa0..=0xa7).collect(),
                "e054c096b5d08ce2e644d7854de8d3aa19bac7e878da8e4c62801501f8ff7c8c",
            ),
            (
                (0..=254).collect(),
                "131f1b9ad39ae9fe40b2bfe64618b563aa11458c5c0382c26971a2c006bb7819",
            ),
        ] {
            let single = values.len() == 1;
            let msgs = values
                .into_iter()
                .enumerate()
                .map(|(index, value): (usize, u8)| {
                    Ok(WalletMessage {
                        mode: SendMsgFlags::from_bits_retain(if single { 3 } else { index as u8 }),
                        msg: CellBuilder::build_from(value)?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let request = TgWalletExtMsgBody {
                wallet_id: 0x7fff_7ffd,
                valid_until: 0x7100_0000,
                msg_seqno: 0x0102_0304,
                msgs,
            };
            let body = request.to_cell()?;
            assert_eq!(body.repr_hash().to_string(), expected_hash);
            let mut signed = CellBuilder::new();
            signed.store_raw(&[0; 64], 512)?;
            signed.store_slice(body.as_slice()?)?;
            let signed = signed.build()?;
            let (parsed, signature) = TgWalletExtMsgBody::read_signed(&mut signed.as_slice()?)?;
            assert_eq!(parsed, request);
            assert_eq!(signature, vec![0; 64]);
        }
        Ok(())
    }
}
