# Agent Worktree Doctor

[简体中文](README.zh-CN.md)

Agent Worktree Doctor is a local, offline, read-only CLI for diagnosing Git
worktree topology and metadata. The initial release focuses on broken `.git`
forward pointers, administrative-directory back pointers, `commondir`, Git's
registered `prunable` and `locked` states, worktree configuration hazards, and
optional shallow submodule and tracked-status checks.

The project is under active development. See the
[product specification](docs/PRODUCT_SPEC.md) for the v0.1 contract; check the
current release notes before relying on a particular finding in automation.

## Install and use

Build from this checkout:

```bash
cargo install --path .
agent_worktree_doctor audit /path/to/worktree
agent_worktree_doctor audit repo_a repo_b --format json
agent_worktree_doctor audit repo --include-submodules --check-tracked-status
```

The intended command shape is:

```text
agent_worktree_doctor audit <PATH>... [--include-submodules]
  [--check-tracked-status] [--format text|json|sarif]
```

At least one path is required. Exit status `0` means the audit completed with
no findings, `1` means it completed with findings, and `2` means invocation,
I/O, timeout, or capacity limits made the audit incomplete. A clean report is
not proof that the filesystem will remain writable or that every Git operation
will succeed.

## Safety and privacy

- Read-only: no repair, `prune`, `remove`, `lock`, `unlock`, or active write probe.
- Offline: no network requests, telemetry, or background service.
- Explicit scope: at most 64 input paths; submodule inspection is opt-in and
  limited to one level of registered submodules with present work directories.
- Redacted by default: reports omit absolute paths, branch names, commit IDs,
  lock reasons, configuration values, and Git stderr.
- Bounded: metadata files, Git output, commands, the cooperative total-audit
  deadline, registered worktrees, and findings all have documented limits.

JSON uses `schema_version: 1`, `kind: "agent_worktree_audit"`, `complete`,
`summary`, and `findings`. Treat unknown finding codes and fields as
forward-compatible additions.

## Documentation

- [Product specification](docs/PRODUCT_SPEC.md)
- [Security policy](SECURITY.md)
- [Contribution guide](CONTRIBUTING.md)
- [Support](SUPPORT.md)

If this tool saves you time, you can [support Tinkora on Ko-fi](https://ko-fi.com/tinkora).

## License

MIT
