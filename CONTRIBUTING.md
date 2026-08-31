# Contributing

[简体中文](CONTRIBUTING.zh-CN.md)

Open an issue before widening the product boundary or changing output contracts.
Keep changes focused, add outcome-based tests, and use English Conventional
Commits. Code and code comments are English in this repository.

Before submitting a pull request, run:

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Do not add repairs, active write probes, implicit repository discovery, network
access, telemetry, or sensitive path and Git metadata to default reports. Keep
English and Chinese documentation aligned when behavior changes.
