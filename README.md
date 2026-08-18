# optiflow

> Observe first. Prove relationships. Plan safely.

`optiflow` is a local-first Rust CLI for inventorying messy media collections,
proving byte-identical duplicate groups, calculating reclaimable storage, and
producing immutable review plans.

Version `0.1.0` is deliberately read-only. It has no apply, delete, replace,
move, quarantine, or optimization command.

## Current capabilities

- Scan one or more files and directories without modifying them.
- Exclude hidden trees, symbolic links, filesystem crossings, and optiflow's
  own state directory by default.
- Classify files by inspected content rather than filename extension.
- Collect optional container, stream, codec, dimensions, duration, sample-rate,
  channel, and bitrate metadata through `ffprobe` JSON.
- Persist observations and reusable analysis in a local SQLite database.
- Narrow exact-duplicate candidates by byte length before complete hashing.
- Calculate complete streaming BLAKE3-256 hashes only for size candidates.
- Produce exact-duplicate groups with transparent evidence.
- Calculate potential reclaimable storage without deleting anything.
- Write schema-versioned run, report, and plan artifacts atomically.
- Generate deterministic review plans with apply-time safety preconditions.
- Emit human-readable or stable JSON output for `flow` and other pipelines.

## Requirements

- Rust stable with edition 2024 support
- macOS or Linux
- Optional: `ffprobe` for media stream inventory
- Optional: `jq` for the repository smoke test

On macOS:

```bash
brew install ffmpeg jq rust
```

## Quick start

```bash
cargo build --release

./target/release/optiflow doctor

./target/release/optiflow scan "/path/to/Media"
```

The scan prints its run identifier and writes immutable artifacts beneath the
local state directory:

```text
runs/<run-id>/
├── run.json
└── report.json
```

Generate a separate, review-only exact-duplicate plan:

```bash
./target/release/optiflow plan exact-duplicates \
  --run "<run-id>"
```

The generated `plan-exact-duplicates.json` explicitly declares
`"mutates_files": false`. Its lexicographically first `keep_path` is only a
stable review default; it is not presented as the objectively best copy.

## Commands

```text
optiflow doctor
optiflow scan <inputs...>
optiflow report <run-or-report-path>
optiflow plan exact-duplicates --run <run-or-report-path>
optiflow cache status
```

Global options:

```text
--state-directory <DIRECTORY>  Override persistent local state
--json                         Emit the primary command result as JSON
```

Scan policy options:

```text
--follow-symlinks      Follow symbolic links
--include-hidden       Include hidden files and directories
--cross-filesystems    Cross filesystem boundaries
--no-probe             Skip optional ffprobe metadata extraction
```

The defaults are intentionally conservative for external drives and large
archives.

## JSON pipeline contract

Every JSON document includes an explicit schema identifier:

| Artifact | Identifier | Schema |
| --- | --- | --- |
| Run | `optiflow.run.v3` | `schemas/run.schema.json` |
| Report | `optiflow.report.v3` | `schemas/report.schema.json` |
| Plan | `optiflow.plan.v3` | `schemas/plan.schema.json` |

Use `--json` for subprocess integration. Human status text is otherwise written
to standard output, while command failures use a non-zero exit status.

## State locations

- macOS: `~/Library/Application Support/optiflow`
- Linux: `$XDG_STATE_HOME/optiflow` or `~/.local/state/optiflow`
- Override: `--state-directory` or `OPTIFLOW_STATE_DIRECTORY`

The primary database stays on a local filesystem. optiflow uses SQLite's
rollback journal rather than assuming WAL-safe behavior on removable or network
volumes.

## Safety boundary

An exact group in `v0.1.0` means equal byte length plus an equal complete BLAKE3
content hash. Before a future destructive operation, every plan precondition
requires optiflow to:

1. Re-read filesystem metadata.
2. Reject files changed since planning.
3. Recalculate the complete BLAKE3 hash.
4. Perform byte-for-byte confirmation.
5. Refuse mutation if any precondition fails.

See [Safety Model](docs/safety-model.md) for the complete invariant set.

## Ecosystem integration

`optiflow` is independently installable and useful as a standalone CLI. In the
Ego Hygiene suite, [`flow`](https://github.com/egohygiene/flow) is the unified
orchestration facade: it invokes `optiflow` through the CLI and consumes the
versioned JSON artifacts. Sibling tools do not embed optiflow's source or take
an unversioned Rust dependency on its default branch.

## Development

```bash
task validate
```

Equivalent commands:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
task contracts
./scripts/smoke-test.sh
```

The end-to-end test uses synthetic files in a temporary directory, includes
spaces and Unicode in paths, and verifies that both duplicate inputs remain
unchanged after scanning and planning.

## Project documents

- [Architecture](ARCHITECTURE.md)
- [MVP specification](docs/mvp-spec.md)
- [Safety model](docs/safety-model.md)
- [State model](docs/state-model.md)
- [JSON contract](docs/json-contract.md)
- [Development model](docs/development-model.md)
- [Roadmap](ROADMAP.md)

## License

MIT
