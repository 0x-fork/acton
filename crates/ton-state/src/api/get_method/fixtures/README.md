# Get-method compatibility fixtures

`wallet-v5.json` contains public mainnet state and TON Center v2 results captured
on 2026-09-27. The tests execute its code and data through the HTTP handler and
native TVM. They compare `seqno` and `get_extensions`, including gas usage.
Synthetic block metadata keeps the tests independent of the live network.

`contracts.fif` generates the small contracts in `contracts.json`. Regenerate
the base64 code lines with Fift and its standard library:

```sh
fift -I /path/to/ton/crypto/fift/lib -s contracts.fif
```

Keep the emitted lines in the JSON file's existing key order. These contracts
exercise the execution context, stack conversion, gas exhaustion, exceptions,
alternative return, slices, and data writes that must never reach the state store.
