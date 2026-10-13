//! Boxed ADNL query, answer, ping, and TCP authentication messages.

use tl_proto::{TlRead, TlWrite};

use tycho_crypto::tl::PublicKeyOwned;

use super::common::*;

#[derive(TlRead, TlWrite, Debug, Clone, PartialEq)]
#[tl(boxed)]
/// Boxed envelopes exchanged on an established ADNL connection.
pub enum Message {
    /// adnl.message.query query_id:int256 query:bytes = adnl.Message;
    #[tl(id = 0xb48bf97a)]
    Query {
        /// Correlation token echoed unchanged in the ADNL answer.
        query_id: Int256,
        /// Boxed query bytes; application decoding is separate from ADNL framing.
        query: Vec<u8>,
    },

    /// adnl.message.answer query_id:int256 answer:bytes = adnl.Message;
    #[tl(id = 0x0fac8416)]
    Answer {
        /// Correlation token echoed unchanged in the ADNL answer.
        query_id: Int256,
        /// Answer bytes for the matching query; unknown constructors remain opaque.
        answer: Vec<u8>,
    },

    /// tcp.ping random_id:long = tcp.Pong;
    #[tl(id = 0x4d082b9a)]
    Ping {
        /// Ping correlation token echoed in the pong.
        random_id: u64,
    },

    /// tcp.pong random_id:long = tcp.Pong;
    #[tl(id = 0xdc69fb03)]
    Pong {
        /// Ping correlation token echoed in the pong.
        random_id: u64,
    },

    /// tcp.authentificate nonce:bytes = tcp.Message;
    #[tl(id = 0x445bab12)]
    Authenticate {
        /// Challenge bytes for the TCP authentication exchange.
        nonce: Vec<u8>,
    },

    /// tcp.authentificationNonce nonce:bytes = tcp.Message;
    #[tl(id = 0xe35d4ab6)]
    AuthenticationNonce {
        /// Challenge bytes for the TCP authentication exchange.
        nonce: Vec<u8>,
    },

    /// tcp.authentificationComplete key:PublicKey signature:bytes = tcp.Message;
    #[tl(id = 0xf7ad9ea6)]
    AuthenticationComplete {
        /// Claimed identity for TCP authentication; the receiver must verify the signature.
        key: PublicKeyOwned,
        /// Ed25519 signature bytes; decoding alone does not authenticate the signer.
        signature: Vec<u8>,
    },
}
