# Cell hash fixtures

These cells come from a native LocalTON bridge/indexer scenario. Expected hashes
were independently computed with tonutils-go v1.17.2. Both root hashes also
match Fift, which uses the TON node's C++ cell implementation (build commit
`aab23908803cf3cb574bcf2338e1f284e98a286b`).

- `nested-merkle-block.boc`: workchain 0, full shard, block 294. Nested Merkle
  proofs in its state update expose a sparse level mask in a descendant cell.
- `sparse-level-mask.boc`: the subtree at reference path `200000010320011` in that
  block. Its ordinary root has level mask `010`; level 2 must preserve that mask
  in the hash descriptor, without introducing a level-1 bit.

The C++ implementation uses `LevelMask::apply(level)` when constructing the hash
descriptor in `crypto/vm/cells/DataCell.cpp`. `apply` preserves the original bits:
`mask & ((1u << level) - 1)`. This was also checked against TON commit
`9334027d63528da1a78b2efbe898bc586d4e6cec`.

To reproduce the C++ check, run these Fift statements from this directory:

```fift
"sparse-level-mask.boc" file>B B>boc hashB Bx. cr
"nested-merkle-block.boc" file>B B>boc hashB Bx. cr
```

The results are `4E099EF87B52E04FEE9FEBC47329511D0A9084DB2BBE3441B2EE81F4F8077642`
and `EBD9BCF0A55DACC9E73AB586CCA32E2F2B8A82B20644187A1405020BF8FE1266`.
