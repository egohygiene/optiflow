# MVP Specification

## Goal

Deliver useful storage intelligence without accepting destructive authority.

## Supported platforms

- macOS
- Linux

Windows behavior is not guaranteed in `v0.1.0`, although platform-neutral code
paths are preferred where they do not weaken the filesystem model.

## Functional requirements

- `doctor` reports state readiness and optional tool availability.
- `scan` accepts multiple files and directories.
- Discovery defaults to no symlink following, no hidden trees, and no filesystem
  boundary crossing.
- Content classification is independent of filename extension.
- Optional media probing uses structured `ffprobe` JSON.
- Repeated scans reuse unchanged path analysis from SQLite.
- Accepted evidence is derived through one opened handle and rejected when the
  path or handle changes during observation.
- Exact candidates are narrowed by byte length.
- Complete candidate identity uses BLAKE3-256.
- Reports include exact groups and reclaimable bytes.
- `plan exact-duplicates` creates a separate immutable review artifact.
- `report` accepts a run identifier, report file, or run artifact directory.
- Every primary result supports machine-readable JSON.

## Non-functional requirements

- No source mutation APIs in the binary.
- No shell invocation for external adapters.
- Paths containing spaces and Unicode are supported.
- JSON artifacts are committed atomically.
- SQLite uses a rollback journal and full synchronization.
- CI requires formatting, strict Clippy, tests, and a synthetic end-to-end run.
- No GPU is required.

## Explicit non-goals

- Selecting the objectively best duplicate
- Deleting or moving duplicates
- Perceptual media similarity
- Media optimization or format normalization
- Video subsection detection
- Interactive terminal review UI
- Cross-run content identity for non-candidate moved files

## Acceptance criteria

- Two byte-identical files produce one exact group.
- A unique same-length file with different bytes is not grouped.
- Reclaimable bytes equal one copy per duplicate group beyond the retained copy.
- Planning leaves all source files unchanged.
- A plan contains size, modification-time, and complete-hash preconditions for
  every group member.
- A second unchanged scan reports cache hits.
- Rename replacement, truncation, growth, and metadata races cannot produce
  current exact-duplicate evidence.
- Missing `ffprobe` does not prevent hashing or duplicate reporting.
- Inaccessible files are reported rather than silently treated as duplicates.
