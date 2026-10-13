# Contributing to rston

Run these commands from `libs/rston` in an Acton checkout.
The library has its own Cargo workspace, which includes `rston-proc` and the fuzz targets.

## Tests

Run the tests and documentation examples with all features:

```sh
cargo test --workspace --all-features
```

Check the library without default features:

```sh
cargo check --lib --no-default-features
```

## Benchmarks

Run the BoC or dictionary benchmarks:

```sh
cargo bench --bench boc
cargo bench --bench dict
```

The [benches](benches/) directory contains additional targets for dictionary updates, cell slices, and cell usage tracking.
Each target has an entry in [Cargo.toml](Cargo.toml).
The `callgrind_*` targets require Valgrind with Callgrind.

## Formatting and Clippy

The workspace uses stable rustfmt with the shared [Acton configuration](../../rustfmt.toml).

Format the workspace:

```sh
cargo fmt --all
```

Run the formatting and Clippy checks:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Miri

Install Miri for nightly Rust:

```sh
rustup toolchain install nightly --profile minimal --component miri
```

Run the library tests under Miri:

```sh
cargo +nightly miri test --lib
```

## Fuzzing

Install cargo-fuzz:

```sh
cargo install cargo-fuzz --locked
```

Run a BoC decoder target:

```sh
cargo +nightly fuzz run boc_decode
```

Run the BoC encode/decode target:

```sh
cargo +nightly fuzz run boc_decode_encode
```

The [fuzz/fuzz_targets](fuzz/fuzz_targets/) directory contains the other targets.
Use the target name from [fuzz/Cargo.toml](fuzz/Cargo.toml) with `cargo +nightly fuzz run`.

## API documentation

Build the API documentation:

```sh
cargo doc --no-deps --all-features --open
```


## Publishing a release

`rston` and `rston-proc` share one release version. Update both manifests,
the exact `rston-proc` dependency, the README example, and the changelog.
Commit the release changes before publishing.

Check Rust 1.88 with minimal and all features:

```sh
cargo +1.88.0 check -p rston --lib --no-default-features --locked
cargo +1.88.0 check -p rston --lib --all-features --locked
```

Use Cargo 1.90 or later to verify both packages without uploading:

```sh
cargo publish -p rston-proc -p rston --dry-run --locked
```

Cargo verifies the packaged sources and resolves the macro dependency from
its local staging registry. Inspect both archives under `target/package`.
They include the README and both license files. The library archive also
includes wallet resources and test fixtures.

Publication is manual. The **Publish rston** GitHub Actions workflow accepts
`version` and `dry_run` inputs. The version must match both manifests.
Leave `dry_run` enabled to repeat verification without uploading.
To publish, run it from `master` with `dry_run` disabled.
Store a publishing token in the repository secret `CARGO_REGISTRY_TOKEN`.

Before the first publication, sign in to crates.io, verify your email,
and create a publishing token. The token must permit creation of both crates.
For later releases, the token owner must own both crates.

To publish the committed release from your computer:

```sh
cargo login
cargo publish -p rston-proc -p rston --locked
```

Cargo publishes the macro crate before the library. See
[Cargo publishing](https://doc.rust-lang.org/cargo/reference/publishing.html)
for account and ownership requirements.
