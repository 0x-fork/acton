# Synchronize TON state and query it over HTTP

`ton-state` starts from a validator database snapshot, downloads successor blocks
through P2P, and stores masterchain and shard states. Its HTTP API reads the last
fully applied checkpoint.
Each HTTP request keeps that checkpoint for its entire read. Synchronization can
commit newer blocks without making the request wait for state application.

## Run with Localton

Use an extracted database from a stopped validator or a consistent backup.
The global config and snapshot must belong to the same network.
Keep the snapshot unchanged and store updates in a separate directory.

Start the Localton network that produced your snapshot:

```sh
cargo run --release --manifest-path apps/localton/Cargo.toml -- \
  bootstrap --state-dir /path/to/localton-network
```

In another terminal, run the service from the repository root:

```sh
cargo run --release -p ton-state -- \
  /path/to/snapshot /path/to/localton-network/global.config.json .ton-state
```

The default HTTP endpoint is `http://127.0.0.1:8080`.
Open [`/docs`](http://127.0.0.1:8080/docs) to browse all supported methods and send
requests, including live SSE subscriptions. The page uses Scalar and loads its
pinned JavaScript bundle from jsDelivr. API requests go directly to this service.
The generated OpenAPI document is at [`/openapi.json`](http://127.0.0.1:8080/openapi.json).

The service advertises `127.0.0.1:19005` for P2P UDP traffic.
Use `--http` or `--address` to change these endpoints.
The advertised UDP address must be reachable from peers.

Stop the service with `Ctrl-C`. Run the same command to resume from its saved
checkpoint. `.ton-state/states` stores applied updates; `.ton-state/blocks` stores
downloaded blocks. The original snapshot remains necessary after restart.
`.ton-state/history` stores the block lookup index used for transaction history.
On startup, the service indexes cached blocks that are missing from this index.

## Query the applied state

Read the masterchain checkpoint:

```sh
curl -s http://127.0.0.1:8080/api/v2/getMasterchainInfo | jq
```

Read an account. This example uses the standard elector address:

```sh
curl -sG http://127.0.0.1:8080/api/v2/getAddressInformation \
  --data-urlencode 'address=-1:3333333333333333333333333333333333333333333333333333333333333333' \
  | jq
```

Read its balance in nanograms. One GRAM equals 1,000,000,000 nanograms:

```sh
curl -sG http://127.0.0.1:8080/api/v2/getAddressBalance \
  --data-urlencode 'address=-1:3333333333333333333333333333333333333333333333333333333333333333' \
  | jq
```

Account methods accept raw and user-friendly addresses. An absent account has
zero balance and `uninitialized` state. Account information includes code and
data as base64 BoCs, the last transaction, and the masterchain checkpoint.
`sync_utime` contains the account's shard-state time, as in TONLib.
`suspended` is currently always `false`; account suspension is not evaluated.

These GET routes use the [TON Center API v2](https://toncenter.com/api/v2/)
response format. Success responses contain `ok: true` and `result`.
Errors contain `ok: false`, `error`, and `code`.
An optional `seqno` must equal the current applied checkpoint. Other heights
return HTTP 409. These account routes do not support historical queries, POST,
or JSON-RPC.

The reported checkpoint can lag behind the network head. Network errors cause
download retries. An invalid state update or a storage error stops the service.

## Read account transactions

Read the elector's ten latest retained transactions:

```sh
curl -sG http://127.0.0.1:8080/api/v2/getTransactions \
  --data-urlencode 'address=-1:3333333333333333333333333333333333333333333333333333333333333333' \
  --data-urlencode 'limit=10' | jq
```

The response uses the TON Center v2 transaction format, including the full
transaction BoC, messages, and fees. Message bodies use `msg.dataRaw`; text
comments are not decoded.

Transactions are returned newest first. `limit` accepts 1–100 and defaults to 10.
For pagination, pass both `lt` and `hash` from a returned `transaction_id`.
The cursor is inclusive, so that transaction appears first on the next page.
Hashes accept hex or base64; use `--data-urlencode` for base64 query values.
Nonzero `to_lt`, `archival=true`, and unknown parameters return HTTP 400.

History comes from downloaded block files. A state snapshot alone does not
contain transaction history, and this method does not fetch missing blocks.
An account with no transactions returns an empty list. If the starting
transaction is unavailable, the method returns HTTP 404. A gap after some
results produces a shorter page. An explicit cursor beyond the applied shard
checkpoint returns HTTP 409.

## Send a signed message

Submit a signed inbound external message saved as `message.boc`:

```sh
base64 < message.boc | tr -d '\n' | jq -Rs '{boc: .}' | \
  curl -s http://127.0.0.1:8080/api/v2/sendBoc \
    -H 'Content-Type: application/json' --data-binary @-
```

A successful response is `{"ok":true,"result":{"@type":"ok"},"@extra":""}`.
It means the service queued a P2P broadcast, not that a validator accepted the
message or included it in a block. Check the resulting transaction separately,
for example through an SSE subscription opened before submission.

The service checks the message envelope and accepts BoCs up to 65,535 bytes
with a standard masterchain or basechain destination. It does not emulate the
message or validate its signature, expiry, balance, or wallet sequence number.
Receiving peers enforce network admission rules. Larger messages use FEC
broadcasts. Peers can discard repeated messages.

Invalid input returns an HTTP error in the v2 response format. HTTP 429 means
the submission queue is full; HTTP 503 means P2P submission failed and the
request can be retried. This route accepts POST JSON only.

## Send a message and wait for its transaction

Use `sendBocAndWaitTransaction` to submit a signed message and keep the HTTP
request open until its transaction appears in a fully applied block batch:

```sh
base64 < message.boc | tr -d '\n' | jq -Rs '{boc: ., timeout_ms: 30000}' | \
  curl -s http://127.0.0.1:8080/api/v2/sendBocAndWaitTransaction \
    -H 'Content-Type: application/json' --data-binary @- | jq
```

The response has `ok: true` and a `result` containing:

- `transaction`: the same v2 object as `getTransactions`, including its full BoC
- `block_id`: the containing block's workchain, shard, sequence number, and hashes
- `mc_block_seqno`: the masterchain checkpoint that committed the complete batch
- `normalized_message_hash`: the base64 TEP-467 hash used to match the external message

The observation starts before broadcast. The service returns the transaction that
consumes the external message, including an aborted transaction. For a wallet
message, this is the wallet's transaction. Subsequent transfers and other child
transactions may execute later and are not awaited.

`timeout_ms` defaults to 30,000 and accepts 1,000–120,000. It covers decoding,
submission, observation, and response encoding after the request body is read.
HTTP 504 with `error: "transaction_wait_timeout"` means the transaction was not
returned within that budget. It does not prove rejection; execution may occur
later, or the node may be behind the network. Errors include
`normalized_message_hash` once the message has been decoded. HTTP 503 reports a
submission failure or interrupted observation, also without ruling out execution.
Disconnecting the client does not withdraw a message already sent to peers.

Delivery is live only: a transaction committed before observation starts is not
replayed. Retrying an already included message may time out. Concurrent requests
for the same message can receive the same transaction; the method provides no
additional replay protection. Each request uses the existing message admission
checks. Up to 64 transaction waits are accepted; exceeding this limit returns
HTTP 429 before broadcast. Waiting releases its submission slot after broadcast,
so pending waits do not exhaust the separate `sendBoc` submission budget.

## Send a message and wait for its complete trace

Use `sendBocAndWaitTrace` to wait for the root transaction and every internal
message it produces, including messages from child transactions and bounces:

```sh
base64 < message.boc | tr -d '\n' | jq -Rs '{boc: ., timeout_ms: 120000}' | \
  curl -s http://127.0.0.1:8080/api/v2/sendBocAndWaitTrace \
    -H 'Content-Type: application/json' --data-binary @- | jq
```

The response contains only the trace identifier:

```json
{"ok":true,"result":{"trace_hash":"<base64 root transaction hash>"},"@extra":""}
```

`trace_hash` is the root transaction's cell hash, matching TON Center's `trace_id`.
The trace completes when every emitted internal message has a receiving
transaction in a committed block. External outgoing messages are terminal.
Aborted transactions can produce bounces, which are awaited too. Completion
does not mean that every contract executed successfully.

`timeout_ms` defaults to 120,000 and accepts 1,000–600,000 (up to ten minutes).
The live-only behavior and admission limits are shared with
`sendBocAndWaitTransaction`. The deadline covers the entire trace and is not
extended when more messages appear;
HTTP 504 returns `error: "trace_wait_timeout"`. A timeout does not stop execution.
HTTP 503 with `trace_pending_messages_limit_exceeded` means the trace exceeded
16,384 pending internal messages. Both wait routes share the 64 observation slots.

## Subscribe to finalized transactions and account states

Open `/account` in a browser, enter an address, and press Enter to view its live
state. The page reconnects and reads a fresh snapshot after a disconnection.
Select **Without code/data** to subscribe with `include_code_data: false` and hide
both fields from the displayed state. The choice is preserved in the page URL.

Open a live SSE subscription on the same HTTP listener:

```sh
curl -N http://127.0.0.1:8080/api/streaming/v2/sse \
  -H 'Content-Type: application/json' \
  -d '{"types":["transactions"],"addresses":["-1:3333333333333333333333333333333333333333333333333333333333333333"],"min_finality":"finalized"}'
```

The first SSE data message is `{"status":"subscribed"}`. Subsequent messages
contain `type: "transaction"`, `finality: "finalized"`, and one `transaction`
object with TON Center v3 fields. The service sends `: keepalive` comments after
15 seconds without an event. Ignore comment lines in the client.

Subscriptions accept 1–100 raw or user-friendly addresses. An address matches
the transaction's account, not its message destinations. Repeated forms of the
same address produce one event. `types` accepts `"transactions"`,
`"account_states"`, or both, and defaults to `["transactions"]`.
`min_finality` defaults to `"finalized"`. Other event types, finality levels,
and subscription fields return HTTP 400.

Events come from the existing P2P block download. They are published only after
the masterchain block and all associated shard state updates commit. The
transaction includes its shard `block_ref` and the committing `mc_block_seqno`.
During catch-up, events follow the service's applied checkpoint and can be older
than the network head. Keepalive confirms the connection, not sync progress.

The transaction object includes messages, execution phases, fees in nanograms,
and account state hashes. Account balances, code, data, normalized message hashes,
and trace links are not resolved. Their optional fields remain null or absent;
`child_transactions` is empty and does not imply that no child transactions exist.
The envelope is specific to this service: each event contains one transaction,
without TON Center's trace grouping. See the reference
[SSE subscription](https://docs.ton.org/api/streaming/sse) and
[notification schemas](https://docs.ton.org/api/streaming/reference).

To receive account state updates, select `account_states`:

```sh
curl -N http://127.0.0.1:8080/api/streaming/v2/sse \
  -H 'Content-Type: application/json' \
  -d '{"types":["account_states"],"addresses":["-1:3333333333333333333333333333333333333333333333333333333333333333"]}'
```

Each update contains `type: "account_state"`, `finality: "finalized"`, the raw
`address`, and `account_state` with the same fields as the `result` of
`getAddressInformation`: balance in nanograms, extra currencies, code/data BoCs,
last transaction ID, block ID, sync time, frozen hash, state, and suspended flag.
The suspended flag is currently always false, as in the HTTP response.

The first event for each account on a connection includes both `code` and `data`.
Later events include each of those fields only when its cell changes. An absent
field means unchanged; an empty string clears the field. All other fields are
sent in full on every event, including `block_id` and `last_transaction_id`.
Reconnecting starts a fresh baseline and sends both fields again on the next event.

Set `include_code_data: false` to exclude both BoCs from every account state event,
including the first event and any changes or clearing of those fields:

```json
{
  "types": ["account_states"],
  "addresses": ["-1:3333333333333333333333333333333333333333333333333333333333333333"],
  "include_code_data": false
}
```

All other fields remain complete. Omitted, `null`, or `true` keeps the default
code/data behavior. The flag does not affect transaction events.

An account with transactions in the committed batch produces one state event,
even if it has several transactions or appears in several shard blocks. The event
contains its state after the entire batch; deleted accounts use the same
zero-balance uninitialized representation as the HTTP API. Unchanged accounts do
not produce events just because the checkpoint or shard timestamp advances.
For `types: ["transactions", "account_states"]`, transaction events precede the
account state events for that batch.

The stream sends no initial account snapshot. To establish a starting state,
open the subscription first, then call `getAddressInformation` and reconcile
queued events using `account_state.block_id.seqno`, the masterchain checkpoint.
Maintain the stream's code/data baseline even for queued events older than that
HTTP snapshot, and display the reconstructed state only once its checkpoint
reaches the displayed snapshot. This prevents older code/data from overwriting
newer HTTP values while preserving changes needed by subsequent stream events.

Delivery is live only. Events published before subscription, during disconnection,
or across process restarts are not replayed. `Last-Event-ID` returns HTTP 400.
Clients must treat a disconnect as a possible gap and reconcile separately if
they need a complete history.

At most 64 connections are accepted. Each has a queue limited to 32 events and
2 MiB of serialized data. Queue overflow sends
`{"type":"error","error":"slow_consumer"}` and ends that connection.
A state-read or encoding failure, or an event larger than 1 MiB, ends current subscriptions
with `{"type":"error","error":"stream_failed"}`. State synchronization
continues. Reconnect creates a new live subscription and does not recover the gap.

## Verification and storage limits

The service checks block hashes, predecessor links, Merkle updates, and complete
shard frontiers. It trusts the snapshot and does not verify validator signatures
or re-execute transactions. New workchains require zerostates and are not supported.

Stored cells and downloaded blocks accumulate on disk. The service has no
garbage collection or authentication. The default HTTP listener accepts only
local connections.
