# Contributing

Bug reports and pull requests are welcome on
[GitHub](https://github.com/uiuifree/rust-google-crux-api).

Run the same checks as CI before opening a pull request:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --doc
```

The minimum supported Rust version is declared in `Cargo.toml` (`rust-version`) and
checked in CI.

## Live tests

`tests/live.rs` calls the real API and is ignored by default. Run it with an API key:

```bash
CRUX_API_KEY=... cargo test --test live -- --ignored
```

In CI it runs on pushes to `main`, weekly, and on manual runs when the repository secret
`CRUX_API_KEY` is set.
