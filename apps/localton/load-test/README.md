# Recursive Localton workload

`RecursiveDeployer` creates a binary tree of copies of itself. A node processes its first `Expand` message, marks itself as expanded, and deploys two children. Each child receives half of the node balance left after a fixed `0.03 GRAM` execution reserve. `treeId` keeps separate workload runs in disjoint address spaces.

A node becomes a leaf when its balance is at most `0.09 GRAM`. Balance halving and the fixed threshold make the recursion finite. The persisted `expanded` flag prevents duplicate message delivery from expanding the same node twice.

This contract is intentionally unsafe for public networks: the root external message is unsigned. Use it only with an isolated Localton network.

## Build and test

```bash
acton build
acton check
acton test
```

## Create the root external message

The only helper script builds the root `StateInit`, prints its deterministic address, and writes an external message to `root-message.boc`:

```bash
cargo xtask prepare-recursive-load 1
```

The external message cannot carry funds. Fund the printed root address before sending the BoC. This example transfers `10 GRAM` from the Localton genesis wallet:

```bash
curl --request POST http://127.0.0.1:18001/acton_fundAccount \
  --header 'content-type: application/json' \
  --data '{"address":"<ROOT_ADDRESS>","amount":10000000000}'
```

Send the generated external message through the Localton liteserver:

```bash
target/debug/localton lite send load-test/root-message.boc --state-dir .localton
```

The root accepts the external message and creates the first two internal deployment messages. Every activated child continues the same process on-chain without further scripts or external messages.

To build a fresh root, fund it, and send the external message in one command, pass the amount in GRAM and a previously unused positive tree ID:

```bash
cargo xtask run-recursive-load 5 1
cargo xtask run-recursive-load 10 2
```

Run these commands from `apps/localton`. The xtask uses the Localton faucet,
waits for liteserver confirmation of the exact root balance, submits the BoC,
and waits until the deterministic root account becomes active.
