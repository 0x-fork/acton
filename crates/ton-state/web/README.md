# Storage page decoder

`storage-decoder.ts` uses the shared Tolk ABI storage decoder and JSON formatter
from `packages/transaction-ui`. Large integers remain decimal strings.

From the repository root, with workspace Bun dependencies installed, run:

```sh
just ton-state-web
cargo build -p ton-state
```

Commit `src/storage-decoder.js` together with decoder source changes. The Rust
binary embeds this generated browser bundle and serves it at `/storage-decoder.js`.
The deployed page does not load decoder code from a CDN.
