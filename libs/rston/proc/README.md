# rston-proc

`rston-proc` provides the `Load` and `Store` derive macros for
[rston](https://crates.io/crates/rston). They convert Rust types to and from TON cells.

Use the macros through `rston::prelude`. The `rston` dependency includes
`rston-proc`; applications do not need to add it separately.

```toml
[dependencies]
rston = "0.4.0"
```

```rust
use rston::prelude::*;

#[derive(Load, Store)]
struct Counter {
    value: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cell = CellBuilder::build_from(Counter { value: 42 })?;
    let counter: Counter = cell.parse()?;
    println!("{}", counter.value);
    Ok(())
}
```

The macro crate and `rston` use the same release version.
Both require Rust 1.88 or later.

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).
