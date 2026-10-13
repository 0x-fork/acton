# Simplex signature fixture

`mc_simplex_signatures.boc` contains the `block_signatures_simplex#12` signature
set for mainnet masterchain block 98841714. It is a signature cell, not a complete
block proof.

The public data was retrieved from TON Center on 2026-10-14:

- `https://toncenter.com/api/v2/getMasterchainBlockSignatures?seqno=98841714`
- `https://toncenter.com/api/v2/getBlockHeader?workchain=-1&shard=-9223372036854775808&seqno=98841714`
- `https://toncenter.com/api/v2/getConfigParam?config_id=34&seqno=98841714`
- `https://toncenter.com/api/v2/getConfigParam?config_id=28&seqno=98841714`

The cell was independently encoded with tonutils-go v1.17.2 cell builders,
following TON's `crypto/block/block.tlb` schema. Dictionary keys follow the
signature order returned by TON Center. The declared weight is the sum of the
signers' weights from the masterchain validator subset. All 68 signatures were
verified with `ton.CheckBlockSignaturesSimplex` before encoding.

The original boxed TL candidate is 160 bytes. Its snake contains 127 bytes in
the first cell and 33 bytes in the second cell, without a comment prefix.
The Rust tests retain both cells and round-trip the signature dictionary.
