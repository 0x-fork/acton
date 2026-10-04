# Faucet bot

Minimal Rust project scaffold for a TON faucet bot using teloxide.

```text
faucet-bot/
├── .gitignore
├── Cargo.lock
├── Cargo.toml
├── README.md
├── deny.toml
├── justfile
├── rust-toolchain.toml
├── rustfmt.toml
├── src/
│   └── main.rs
└── tests/
    └── .gitkeep
```

Create a Telegram bot with @BotFather, then run from this directory:

```sh
export TELOXIDE_TOKEN="<bot token>"
just run
```

The bot receives commands through long polling:

- `/start` replies with `Hello, world!`.
- `/help` lists the available commands.

Press Ctrl+C to stop the bot.

Use `just check` to run formatting, dependency, Clippy, cargo-deny, and test checks.
