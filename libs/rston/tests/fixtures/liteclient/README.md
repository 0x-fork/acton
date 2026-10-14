# LiteAPI proof fixtures

These fixtures are serialized TL responses. Tests run offline and require no liteserver.
`manifest.json` records block identifiers, addresses, and results checked with
[tonutils-go at 25fa771](https://github.com/xssnick/tonutils-go/tree/25fa771be6b195c41a1929486921eb14e5ee4794).

The mainnet responses were captured on 2026-10-14:

| File | Scenario |
| --- | --- |
| `account.tl` | Elector account at the masterchain head |
| `shard-account.tl` | Present basechain account with a masterchain shard proof |
| `absent-account.tl` | Proven account absence in the basechain |
| `config.tl` | Configuration parameters 1, 4, 28, and 34 |
| `config-all.tl` | Complete configuration |
| `transaction.tl` | Elector transaction with inclusion proof |
| `run-method.tl` | Elector `active_election_id` result and source-state proof |
| `forward.tl` | Forward chain from the preceding key block, with Simplex signatures |
| `backward.tl` | Backward chain from the head to that key block |

Go's high-level methods verified each saved response. The manifest includes Go's
decoded balances, transaction hash, configuration hashes, method output, and selectors.
The manifest's checkpoint and head are fixture inputs; they are not a recommended trust source for applications.

`ordinary.tl` and `ordinary-boundary.tl` are synthetic forward proofs, verified by Go.
They use deterministic test keys with seed bytes 1, 2, and 3, valid block hashes,
validator configuration, and Ed25519 signatures. These keys have no network use.
The first set has weights 30, 40, and 30, with the first two signatures present.
The boundary set has equal weights and all signatures present. Removing one
signature leaves exactly two thirds, which both implementations reject.
Go's old `SignatureSet` and `SignatureSetOrdinary` encode the first proof identically.

`arguments.boc` contains positional get-method arguments 11 and 22.
It was encoded with Go's `RunGetMethodByID` argument convention: push arguments
in reverse order before calling `Stack.ToCell`. Rust's bottom-to-top `Tuple`
encodes the same cell tree from `[11, 22]`.
