# Global configuration fixtures

`mainnet.json` is the complete public mainnet configuration used for the
LiteAPI captures on 2026-10-14. It includes DHT descriptors, liteservers, and
validator checkpoints. Tests run offline.

`go-block-ids.json` records identifiers decoded from this file by
[tonutils-go at 25fa771](https://github.com/xssnick/tonutils-go/tree/25fa771be6b195c41a1929486921eb14e5ee4794).
The tests compare those identifiers and preserve all DHT metadata and endpoints
through JSON serialization. Parsing does not verify descriptor signatures.

The saved checkpoints are test inputs, not a recommended trust source for applications.
