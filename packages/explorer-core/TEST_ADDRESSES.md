# Explorer test addresses

Short registry of live-chain accounts that are useful for manual Explorer checks

These addresses are not fixtures and their balances or recent history can change. Keep each entry focused on behavior that is expected to remain useful and update the last-checked date after verifying it

## Mainnet

| Address | What to verify | Last checked |
| --- | --- | --- |
| [`EQDV9A9W0GpbnFhiI6hJkGUSNIqU7Nxx-rn5FVQsAc7ZkfZB`](https://actonscan.com/address/EQDV9A9W0GpbnFhiI6hJkGUSNIqU7Nxx-rn5FVQsAc7ZkfZB?network=mainnet) | Frozen account state, no Contract type row, Unfreezer link next to Tonscan | 2026-07-30 |
| [`EQCBMyAieemf3vF3umY0lCaQxLhwvbTFuL8eQxPYrpeZ8O4O`](https://actonscan.com/address/EQCBMyAieemf3vF3umY0lCaQxLhwvbTFuL8eQxPYrpeZ8O4O?network=mainnet) | Active account that is also suspended, suspended overview must not depend on account state | 2026-07-30 |
| [`Ef9mDsqzIg2i8fdw0Bb7UGafA3Gc1qX5IYjp6AOZwGlfvim2`](https://actonscan.com/address/Ef9mDsqzIg2i8fdw0Bb7UGafA3Gc1qX5IYjp6AOZwGlfvim2?network=mainnet) | Suspended account with a long resolved name, name ellipsis and spacing between QR and edit controls | 2026-07-30 |
| [`EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c`](https://actonscan.com/address/EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c?network=mainnet) | Zero Address, Uninit state, suspended overview, large mixed action history | 2026-07-30 |
| [`EQAlL9ItlyCN7VbZyDV3lLxoTcwPCl3zUT62xB9VAwJ_USDC`](https://actonscan.com/address/EQAlL9ItlyCN7VbZyDV3lLxoTcwPCl3zUT62xB9VAwJ_USDC?network=mainnet) | Mobile account header, address suffix preservation, QR, favorite, edit, and copy action layout | 2026-07-30 |
| [`EQDXgkYbrxDpRZD6PUZd0jwdjZmYYQd7l5YOE2UeXunLD8Wm`](https://actonscan.com/address/EQDXgkYbrxDpRZD6PUZd0jwdjZmYYQd7l5YOE2UeXunLD8Wm?network=mainnet) | Wallet with many Renew DNS actions, useful for checking that domains such as `bybit.ton` are rendered as NFT chips | 2026-07-31 |
| [`EQDoxOcDo0EkHBNVL6tFfH5K-BAWI9PSO44zlgWdOwgeqw7m`](https://actonscan.com/address/EQDoxOcDo0EkHBNVL6tFfH5K-BAWI9PSO44zlgWdOwgeqw7m?network=mainnet) | Wallet with repeated 250-action bulk sends, useful for checking the initial ten-action preview, remaining-action count, incremental expansion, and uninterrupted loading of later transactions | 2026-08-01 |
| [`EQDYzZmfsrGzhObKJUw4gzdeIxEai3jAFbiGKGwxvxHinaPP`](https://actonscan.com/address/EQDYzZmfsrGzhObKJUw4gzdeIxEai3jAFbiGKGwxvxHinaPP?network=mainnet) | Wallet with more than 1,000 NFTs and safety-filtered items in early batches, useful for checking uninterrupted incremental loading in the NFTs tab | 2026-07-31 |
| [`EQCeTFSYKmcPZIQ-0Hvi98bXvXygrxru56LzUllC-Jup2727`](https://actonscan.com/address/EQCeTFSYKmcPZIQ-0Hvi98bXvXygrxru56LzUllC-Jup2727?network=mainnet) | Multisig orders with expired entries, useful for checking the red Expired status and cross icon | 2026-07-31 |
| [`kQB1tqrLMLJZk0YnmU_1UKr-r90QBnUubm0So09b39LPk3rZ`](https://actonscan.com/address/kQB1tqrLMLJZk0YnmU_1UKr-r90QBnUubm0So09b39LPk3rZ?network=mainnet) | Multisig with 1,487 orders, useful for checking incremental loading while scrolling the Orders table | 2026-07-31 |

## Testnet

| Address | What to verify | Last checked |
| --- | --- | --- |
| [`kQAgO7g7m2763OuP-AaTVOZVhEjg5zYyCKDF660QzJp71KLB`](https://actonscan.com/address/kQAgO7g7m2763OuP-AaTVOZVhEjg5zYyCKDF660QzJp71KLB?network=testnet) | Alternating incoming and outgoing transfers around `0.01 GRAM`, useful for the small-transfer spam filter and for confirming outgoing transfers remain visible | 2026-07-30 |
| [`kQB6XGzpO7rglhK1tR9A4l2QQu6yaYE6ALUp1vAOHMaGAfGD`](https://actonscan.com/address/kQB6XGzpO7rglhK1tR9A4l2QQu6yaYE6ALUp1vAOHMaGAfGD?network=testnet) | Repeated one-nano outgoing self-transfers, useful for confirming the spam filter never hides outgoing actions | 2026-07-30 |

## Adding an entry

- Use a full user-friendly URL-safe address and include the network in the link
- Describe the observable behavior rather than the implementation that currently renders it
- Do not add an address until the scenario has been verified manually
- Update or remove entries when live-chain data no longer demonstrates the described behavior
