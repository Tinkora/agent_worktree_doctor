# Repository Guide for AI Agents

## Product boundary

This repository provides a local, offline, read-only diagnostic for explicitly
selected Git worktree paths. Preserve the v0.1 boundary in
`docs/PRODUCT_SPEC.md`: do not add repair, prune, remove, lock, unlock, active
write probes, implicit repository discovery, network access, telemetry, or a
background service.

Default reports must not expose absolute paths, branch names, commit IDs, lock
reasons, configuration values, or Git stderr. Treat filesystem paths and Git
output as untrusted input. Never turn a resource-limit failure into a clean
result.

## Conventions

- Use English Conventional Commits.
- Write code and code comments in English.
- Keep the default README in English with a Chinese entry point.
- Keep English and Chinese documentation behaviorally aligned.
- Use underscores for project-owned filenames and identifiers where external
  formats do not require another spelling.

## Required checks

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```
