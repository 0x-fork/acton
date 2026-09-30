# Tolk compiler corpus

The `fixtures/` directory is a vendored copy of `tolk-tester/tests` from
`ton-blockchain/ton` commit `83fe78b06c9e66e1069e5f58bb2c2e78018dd13c`.

For Tolk 1.5, the local fixtures use `bitsN` instead of the removed `bytesN` types.
When restoring the corpus, replace `bytesN` with `bits(N * 8)` in type references and
expected types. Use `bits1023` for the former `bytes128` boundary case, as in upstream
commit `9334027d63528da1a78b2efbe898bc586d4e6cec`.

The smart-cast and cell-builder fixtures also include the Tolk 1.5 changes for
ternary expressions: their result type includes both branches, even for a constant
condition. The older warning fixture expects the nullable result for the same reason.

The alias fixtures also use the Tolk 1.5 receiver rules from upstream. When restoring
the corpus, apply the corresponding changes in `dicts-demo`, `generics-2`,
`indexed-access`, `inline-tests`, `lazy-algo-tests`, `methods-tests`, `mutate-methods`,
`overloads-tests`, `self-keyword`, `smart-cast-tests`, and `type-aliases-tests`.
Alias methods require a compatible declared receiver; overload expectations select
the nearest alias. Keep unrelated additions to the upstream corpus separate.

The fixtures are intentionally not synchronized automatically. Update them as a reviewed change,
then run the `tolk_compiler_corpus` test and account for every new semantic difference explicitly.

The upstream fixtures are distributed under the GNU Lesser General Public License v2.1. See
`LICENSE.LGPL` in this directory.
