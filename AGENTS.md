# TinySearch repository guidance

This Rust 2024 workspace has two crates. `crates/tinysearch-bus` owns the
transport-free wire contract, names, payloads, and pure catalog selection.
`crates/tinysearch` owns provider behavior, routing, the TinyBus adapter, and
the installable cdylib. Keep provider HTTP behavior out of the bus crate.

Do work on a feature branch in a worktree. Keep `vendor/tinybus` pinned as a
submodule; change its source in its own repository. Do not commit credentials.
Do not log provider queries, credentials, or result content. Provider credentials
arrive through sensitive module initialization and reinitialization config.

Every public item needs rustdoc. Keep types and behavior in focused modules,
with tests in neighboring `test.rs` files. Cover wire representations and real
in-memory TinyBus calls. No placeholders, ignored tests, or lint exemptions.

Run from repository root:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
```

PRs go to the canonical upstream repository and should be ready for review.
