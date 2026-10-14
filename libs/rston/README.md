# rston

`rston` is a Rust library for TON cells, BoC serialization, dictionaries, Merkle proofs, and blockchain models.
Use it to read blockchain data, construct messages, and serialize your own types into cells.

It is a TON-focused fork of [tycho-types](https://github.com/broxus/tycho-types), developed in the [Acton repository](https://github.com/ton-blockchain/acton).

The wallet and mnemonic implementations, contract code, and English word list come from [ton-rs](https://github.com/ston-fi/ton-rs/tree/8ee38e8a3c1c034dc78e3d2a6e71aae3d3848790), licensed under MIT or Apache-2.0.

## Installation

`rston` requires Rust 1.88 or later.

Add `rston` to your `Cargo.toml`:

```toml
[dependencies]
rston = "0.4.0"
```

## Basic usage

This example serializes a counter into a cell, encodes it as BoC bytes, and decodes those bytes back into the counter:

```rust
use rston::prelude::*;

#[derive(Load, Store)]
struct Counter {
    value: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cell = CellBuilder::build_from(Counter { value: 42 })?;
    let bytes = Boc::encode(&cell);
    let decoded = Boc::decode(bytes)?;
    let counter: Counter = decoded.parse()?;

    println!("{}", counter.value);
    Ok(())
}
```

Output:

```text
42
```

`Load` and `Store` derive macros implement conversion between Rust types and cells.
Both macros are available through `rston::prelude` without a separate dependency.

## Core types

| Type | Purpose |
| --- | --- |
| `Cell` | An immutable cell with up to 1023 bits and four references |
| `CellBuilder` | Constructs a cell from bits, values, and references |
| `CellSlice` | Reads values and references from an existing cell |
| `Boc` | Encodes and decodes cell trees as BoC bytes |
| `BocRepr` | Encodes and decodes Rust types that implement `Load` and `Store` |
| `Dict`, `AugDict`, `RawDict` | Store values in TON dictionaries |
| `MerkleProof`, `MerkleUpdate` | Represent proofs and updates for cell trees |

The `models` module includes addresses, messages, accounts, transactions, blocks, shard states, and blockchain configuration.
For custom cell formats, `CellBuilder` and `CellSlice` provide operations for individual fields.

The `mnemonic` module validates 24-word TON mnemonics and 12- or 24-word BIP39 mnemonics,
then derives Ed25519 key pairs. Select `MnemonicScheme::Bip39` explicitly for BIP39;
constructors without a scheme use TON. BIP39 phrases use SLIP-0010 at `m/44'/607'/0'`.
`MnemonicScheme::Rotation` supports TG Wallet phrases with an address anchor and a
current signing key. It accepts 12 words before rotation or two independent 12-word
phrases after rotation. Use `Wallet::new_with_mnemonic` to preserve the anchor address.
The module also exposes `KeyPair` and the English `WORDLIST_EN_SET`.

The `wallet` module derives wallet addresses and prepares signed external messages
using `rston` cells. It exposes `Wallet`, `WalletVersion`, wallet data types,
and contract code. Message signing supports V2, V3, V4, V5R1, and TG Wallet rev00.
TG Wallet requires its runtime implementation in network configuration parameter -123.
Pass outgoing messages as `Vec<WalletMessage>`. Each entry contains the complete
message cell in `msg` and a `SendMsgFlags` bitmask in `mode`.
Combine flags with `|`, such as `SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR`.

## TVM stack encoding

Use `models::vm` to encode get-method arguments and decode result stacks.
The `models` feature is enabled by default.

```toml
[dependencies]
rston = { version = "0.4.0", features = ["models"] }
```

```rust
use rston::boc::Boc;
use rston::cell::CellBuilder;
use rston::models::vm::{Tuple, TupleItem};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Tuple(vec![TupleItem::Int(42.into())]);
    let bytes = Boc::encode(CellBuilder::build_from(&arguments)?);
    let decoded = Boc::decode(bytes)?.parse::<Tuple>()?;

    assert_eq!(decoded, arguments);
    Ok(())
}
```

`Tuple` stores the bottom stack value first and the top value last.
Slice values contain the selected bits and references in a separate cell;
the original cell and slice offsets are not retained.
Supported continuations are normalized into `ContData`, which stores code,
captured stack, save list, and arity. Encoding emits a standard continuation
with codepage zero; decoding rejects unsupported continuation constructors.
Use this module to prepare or read stack data. It does not execute smart contracts.

## Global network configuration

`GlobalConfig` reads and writes the complete standard TON global configuration.
It includes DHT lookup parameters, optional `network_id`, signed bootstrap descriptors,
liteservers, and validator checkpoints. Node signatures and advertised addresses remain
unverified until the consumer checks them.

The module requires `models` and `serde`, which are enabled by default.
It works without `adnl` or `liteclient`.
Checkpoint fields use `BlockId`; their hashes are encoded as padded base64 in configuration JSON.
Missing sections allow endpoint-only or P2P-only files. Applications check the sections they need.

```rust
use rston::GlobalConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: GlobalConfig = std::fs::read_to_string("global.config.json")?.parse()?;
    if let Some(validator) = &config.validator {
        println!("Network zero state: {}", validator.zero_state);
    }
    println!("Liteservers: {}", config.liteservers.len());
    Ok(())
}
```

Parsing checks protocol constructors, byte lengths, ports, and masterchain checkpoint coordinates.
It does not establish trust in the source file or verify DHT signatures.

## LiteAPI queries

Enable `liteclient` and use Tokio to query the liteservers in a trusted global config:

```toml
[dependencies]
rston = { version = "0.4.0", features = ["liteclient"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use rston::GlobalConfig;
use rston::liteclient::{LiteClientOptions, LiteClientPool};
use rston::tl::{Request, response::MasterchainInfo};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: GlobalConfig = std::fs::read_to_string("global.config.json")?.parse()?;
    let mut client = LiteClientPool::new(config, LiteClientOptions::default())?;
    let info: MasterchainInfo = client.query_typed(Request::GetMasterchainInfo).await?;

    println!("Masterchain sequence: {}", info.last.seqno);
    Ok(())
}
```

The pool rotates between endpoints and skips connection failures before sending.
It does not retry a sent request. `LiteClient` provides one persistent connection.
Both support `query_raw()` for constructors outside the typed API.
The default setup and query deadlines are ten seconds each; change them with `LiteClientOptions`.
Raw and typed wire queries return unverified responses.
Use the following methods to authenticate data against a trusted block.

## Verified LiteAPI queries

Load a global configuration from an independently trusted source.
It contains liteservers, the network's `validator.zero_state`, an optional
`validator.init_block`, and any `validator.hardforks` checkpoints.
`VerifiedBlock::from_trusted_config` selects the nonzero init block as a trusted checkpoint.
This records your trust in the file; it does not verify that block cryptographically.
`get_masterchain_info` verifies a proof chain from your checkpoint to the server's reported head.
The caller decides whether that head is recent enough.

This example authenticates the head and reads the elector's account state:

```rust
use rston::GlobalConfig;
use rston::liteclient::{LiteClientOptions, LiteClientPool, VerifiedBlock};
use rston::models::StdAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: GlobalConfig = std::fs::read_to_string("global.config.json")?.parse()?;
    let checkpoint = VerifiedBlock::from_trusted_config(&config)?;
    let mut client = LiteClientPool::new(config, LiteClientOptions::default())?;
    let head = client.get_masterchain_info(&checkpoint).await?;
    let elector: StdAddr =
        "-1:3333333333333333333333333333333333333333333333333333333333333333".parse()?;
    let state = client.get_account(&head, &elector).await?;

    if let Some(account) = state.account {
        println!("Elector balance: {} nanograms", account.balance.tokens);
    }
    Ok(())
}
```

Endpoint-only configurations still work for raw and typed wire queries.
An absent init block or an init block at sequence zero cannot start verified queries.
The zero state identifies the network; verification from genesis is not supported.
Hardfork entries are retained without automatically selecting or trusting a fork.
Use `VerifiedBlock::from_trusted` if you maintain a separate trusted block checkpoint.

| Method | Guarantee |
| --- | --- |
| `get_account` | Account state, balance, transaction cursor, or absence matches the requested block; shard ancestry is checked |
| `get_config` | Requested masterchain parameters match the committed state; an empty selection requests the complete configuration |
| `get_transaction` | Transaction inclusion, address, and logical time match the request; pass its containing shard block |
| `get_block_proof` | Retrieves unverified links; call the response's `verify` method before using its destination |
| `verify_proof_chain` | Authenticates forward and backward links, validator subsets, Ordinary or Simplex signatures, and a weight strictly above two thirds |
| `run_get_method` | Authenticates source account state; returned stack values and exit codes remain server claims |
| `send_message` | Submits an external incoming message once and returns its hash after acknowledgement; acknowledgement does not confirm inclusion |

Successful get-method exit codes are zero and one. Other codes return `LiteError::ContractExecution`.
Arguments and results use `Tuple` in bottom-to-top order.
Recompute the method locally if your application needs authenticated execution results.

Offline response `verify` methods use the same checks without a network connection.
Missing required proofs and pruned required data return errors.
Verification accepts at most 8 MiB per proof or data BoC payload.
Proof chains accept at most 64 responses and 1024 links, with at most 128 links per response.
The request deadline covers the whole chain. Verification checks that deadline between synchronous proof batches.
The supplied checkpoint stays unchanged on failure or cancellation.

The ADNL and LiteAPI wire modules originate from [tonutils-rs](https://github.com/lapismyt/tonutils-rs), under the MIT license in `LICENSE-tonutils`.
The upstream transport also credits [adnl-rs](https://github.com/tonstack/adnl-rs) and [lite-client](https://github.com/tonstack/lite-client).
The high-level proof verification follows [tonutils-go](https://github.com/xssnick/tonutils-go), under the [MIT license](LICENSE-tonutils-go).

## Features

The default features are `base64`, `serde`, `models`, `sync`, and `wallet`.

| Feature | Adds |
| --- | --- |
| `base64` | Base64 helpers for BoC and byte representations |
| `serde` | Serde serialization and JSON global configuration with `models`. Enables `base64` |
| `models` | TON blockchain models |
| `tl` | LiteAPI wire types and TL encoding, without networking |
| `adnl` | ADNL/TCP handshake and encrypted frames. Enables `tl` |
| `liteclient` | Raw, typed, and verified queries, server services, deadlines, and pools. Enables `adnl`, `tl`, `models`, and `serde` |
| `wallet` | Wallet contracts and signed messages. Enables `models`, `base64`, and `sync` |
| `sync` | Cells that can be shared across threads |
| `bigint` | Conversion helpers for `num-bigint` integers |
| `rayon` | Parallel BoC encoding and Merkle operations. Enables `sync` |
| `rand8`, `rand9` | Random value generation through the corresponding `rand` version |
| `arbitrary` | Value generation for fuzz tests |

With `default-features = false`, cells use the single-threaded implementation.
Cell builders, slices, BoC bytes, and dictionaries remain available.

## API documentation

See the [API reference on docs.rs](https://docs.rs/rston) for modules, type definitions, and more examples.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for tests, benchmarks, formatting, Miri, and fuzzing commands.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.
