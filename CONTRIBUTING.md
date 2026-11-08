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
