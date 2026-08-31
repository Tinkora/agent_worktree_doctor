# Agent Worktree Doctor v0.1 Product Specification

[简体中文](PRODUCT_SPEC.zh-CN.md)

## Status and purpose

This document defines the narrow v0.1 contract. It describes the intended
surface, including findings that may be delivered incrementally during the
alpha series; release notes and executable behavior remain authoritative for
what a particular build implements.

The tool answers one question for explicitly supplied paths: does the observable
Git worktree topology and metadata contain a known inconsistency or risk? It is
a diagnostic, not a repair utility, permission oracle, or general repository
health checker.

## Public evidence

- Git documents the forward `.git` gitfile, administrative `gitdir` backlink,
  `commondir`, and stable `worktree list --porcelain -z` contract in
  [git-worktree](https://git-scm.com/docs/git-worktree) and
  [gitrepository-layout](https://git-scm.com/docs/gitrepository-layout).
- [Codex issue #19786](https://github.com/openai/codex/issues/19786) records a
  linked worktree whose private gitdir became read-only inside a sandbox.
- [Claude Code issue #60131](https://github.com/anthropics/claude-code/issues/60131)
  records a missing per-worktree submodule gitdir that caused checkout failure.
- [Claude Code issue #76144](https://github.com/anthropics/claude-code/issues/76144)
  records a malformed backlink that made a live worktree appear prunable.

These incidents support local topology diagnosis. They do not justify active
write probes or automatic repair.

## Command contract

```text
agent_worktree_doctor audit <PATH>... [--include-submodules]
  [--check-tracked-status] [--format text|json|sarif]
```

- One to 64 paths are required.
- `text` is the human-oriented default. `json` is stable structured output.
  `sarif` is intended for code-scanning consumers.
- `--include-submodules` inspects only one level of registered submodules whose
  work directories are present. It does not recursively discover repositories.
- `--check-tracked-status` enables the potentially more expensive tracked-change
  check. It remains read-only and does not report filenames.

Exit status:

| Code | Meaning |
| --- | --- |
| `0` | Audit completed and produced no findings. |
| `1` | Audit completed and produced one or more findings. |
| `2` | Invalid invocation or an I/O, timeout, or capacity boundary made the audit incomplete. |

Consumers must inspect `complete`; they must not interpret an incomplete audit
as healthy.

## Findings

| Code | Symbol | Meaning |
| --- | --- | --- |
| AWD001 | `INPUT_PATH_MISSING` | An explicitly supplied path does not exist. |
| AWD002 | `NOT_A_GIT_WORKTREE` | The path is not recognized as a Git worktree. |
| AWD003 | `GIT_PROBE_FAILED` | A bounded, read-only Git query failed. |
| AWD004 | `MALFORMED_GITFILE` | A `.git` file cannot be parsed as the expected pointer. |
| AWD005 | `FORWARD_GITDIR_MISSING` | The `.git` forward pointer targets a missing location. |
| AWD006 | `COMMONDIR_INVALID` | `commondir` is absent, malformed, or resolves to an invalid location. |
| AWD007 | `BACKWARD_GITDIR_MISSING` | The administrative worktree directory lacks its `gitdir` back pointer. |
| AWD008 | `BIDIRECTIONAL_LINK_MISMATCH` | Forward and backward worktree links do not agree. |
| AWD009 | `WORKTREE_PRUNABLE` | Git reports a registered worktree as prunable. |
| AWD010 | `LOCKED_WORKTREE_UNAVAILABLE` | A locked registered worktree is unavailable. |
| AWD011 | `EXPLICIT_CORE_WORKTREE` | An effective explicit `core.worktree` is outside the v0.1 audit boundary. |
| AWD012 | `SUBMODULE_GITDIR_MISSING` | An in-scope registered submodule points to missing Git metadata. |
| AWD013 | `SUBMODULE_SCOPE_MISMATCH` | An in-scope submodule's work directory and registered metadata disagree. |
| AWD014 | `TRACKED_CHANGES_PRESENT` | Opt-in status checking found tracked changes; filenames are omitted. |
| AWD015 | `AUDIT_INCOMPLETE` | A resource or observation boundary prevented a complete result. |

The table defines identifiers and intended categories, not a promise that every
possible corruption shape is detectable. Git behavior, platform path semantics,
and inaccessible metadata can narrow what is observable.

## Output and privacy contract

JSON documents use:

```json
{
  "schema_version": 1,
  "kind": "agent_worktree_audit",
  "complete": true,
  "summary": {},
  "findings": []
}
```

Fields inside `summary` and finding objects may grow compatibly. Consumers
should key on the numeric code and tolerate unknown fields and codes.

Default output must not expose absolute paths, branch names, commit IDs, lock
reasons, configuration values, or captured Git stderr. Findings identify inputs
using bounded, non-sensitive labels suitable for correlation within one report.
Users should still sanitize reports before publishing them because filesystem
behavior and user-provided labels may reveal context.

## Read-only trust boundary

Allowed observations are filesystem metadata reads and bounded Git queries that
do not mutate repository state. The product never runs repair, `git worktree
prune`, `git worktree remove`, `git worktree lock`, `git worktree unlock`, or an
active write probe. It does not edit configuration, create locks, traverse the
machine looking for repositories, contact the network, or run as a service.

The `git` executable resolved from `PATH` is part of the caller's trust boundary.
Every `PATH` entry must be absolute; the audit fails closed before spawning Git
when an empty or relative entry could select an executable from the working
directory. Users remain responsible for supplying a trusted absolute `PATH`.

An effective explicit `core.worktree` value is outside the v0.1 audit boundary.
The audit reports `AWD011`, marks the result incomplete, and stops processing
that input before deriving or reading files from Git's reported worktree root.

A read-only audit cannot prove future writability, ACL behavior, mount health,
sandbox permissions, or safety of a subsequent mutating Git command.

## Resource limits

| Resource | v0.1 limit |
| --- | --- |
| Input paths | 64 |
| Registered worktrees | 10,000 |
| One metadata file | 64 KiB |
| Captured Git stdout | 16 MiB |
| Captured Git stderr | 16 MiB |
| One Git probe | 5 seconds |
| Cooperative total-audit deadline | 60 seconds |
| Findings | 10,000 |

Crossing a limit produces an incomplete audit and exit status `2`; the tool
must not silently claim a clean result.

The 60-second deadline is cooperative: it is enforced while bounded Git child
processes run and between observations. Metadata is accepted only when it is a
regular file and within the size limit before it is opened, so a FIFO is
rejected instead of read. The tool cannot impose a strict wall-clock bound on
operating-system filesystem calls, such as access to an unavailable network
mount.

## Non-goals

- Repairing or deleting worktrees or metadata.
- Proving that a path is writable by attempting a write.
- Recovering deleted files or acting as a replacement for backup tools.
- Process supervision or MCP server cleanup.
- Full recursive submodule validation, object integrity (`git fsck`), remote
  availability, authentication, hooks, build health, or application correctness.
