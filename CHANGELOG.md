# Changelog

All notable changes are documented here.

## [0.1.0-alpha.3] - 2026-08-31

- Publish the first release created after repository-level immutable releases
  were enabled.

## [0.1.0-alpha.2] - 2026-08-31

- Resolve draft releases by exact tag and release ID with bounded retries.
- Re-download the exact remote asset inventory and verify its published checksums.
- Normalize CycloneDX references to remove runner-local paths and reject invalid
  graphs.

## [0.1.0-alpha.1] - 2026-08-31

- Establish the read-only `audit` command and redacted text and JSON reporting.
- Diagnose an initial set of Git worktree pointer and metadata failures.
- Define bounded execution, stable finding identifiers, and incomplete-audit semantics.
- Add bilingual product and community documentation.
