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
- Publish schema-versioned run, report, and plan artifacts as coherent,
  marker-sealed sets.
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
├── effective-policy.json
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
optiflow config validate
optiflow config show
optiflow config explain <setting>
```

Global options:

```text
--state-directory <DIRECTORY>  Override persistent local state
--config <FILE>                Select one explicit configuration file
--no-config                    Disable configuration-file loading
--output-format <FORMAT>       Select human or JSON command-result output
--json                         Compatible alias for --output-format json
```

Scan policy options:

```text
--follow-symlinks      Follow symbolic links
--no-follow-symlinks   Explicitly keep link following disabled
--include-hidden       Include hidden files and directories
--exclude-hidden       Explicitly exclude hidden paths
--cross-filesystems    Cross filesystem boundaries
--stay-on-filesystem   Explicitly preserve filesystem boundaries
--probe                Explicitly enable optional ffprobe inspection
--no-probe             Skip optional ffprobe metadata extraction
```

The defaults are intentionally conservative for external drives and large
archives.

Configuration follows compiled defaults < user file < nearest project
`optiflow.toml` < recognized environment < explicit CLI. See
[Configuration and effective policy](docs/configuration.md) for the strict
`optiflow.config.v1` schema, exact locations and variables, supported settings,
path rules, provenance, fingerprints, and locked invariants.

## JSON pipeline contract

Every JSON invocation emits one `optiflow.command-result.v1` envelope. The
envelope carries the typed outcome, exact process exit code, coverage,
diagnostics, committed artifact references, and domain result. Its schema is
`schemas/command-result.schema.json`.

Machine results and committed domain artifacts have independent identifiers:

| Artifact | Identifier | Schema |
| --- | --- | --- |
| Command result | `optiflow.command-result.v1` | `schemas/command-result.schema.json` |
| Configuration | `optiflow.config.v1` | `schemas/config-v1.schema.json` |
| Effective policy | `optiflow.effective-policy.v1` | `schemas/effective-policy-v1.schema.json` |
| Artifact set | `optiflow.artifact-set.v1` | `schemas/artifact-set-v1.schema.json` |
| Run | `optiflow.run.v5` | `schemas/run.schema.json` |
| Report | `optiflow.report.v5` | `schemas/report.schema.json` |
| Plan | `optiflow.plan.v5` | `schemas/plan.schema.json` |

Use `--output-format json` (or `--json`) for subprocess integration. JSON owns
stdout; human primary output uses stdout and human diagnostics use stderr.
Complete success is `0`, while a valid result with reduced coverage is `3`.
See [CLI outcome contract](docs/cli-contract.md) for the complete stable exit
matrix, stream rules, signal behavior, and shell examples.

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
The [artifact-set commit protocol](docs/artifact-set-protocol.md) defines
coherent scan/plan publication, marker validation, and crash recovery.

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
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
task contracts
./scripts/smoke-test.sh
```

The end-to-end test uses synthetic files in a temporary directory, includes
spaces and Unicode in paths, and verifies that both duplicate inputs remain
unchanged after scanning and planning.

### Documentation

The product documentation is built with Zensical from the checked-in Markdown
under `docs/`. Its Python environment is pinned independently from the Rust
crate with `uv.lock`.

```bash
task docs:serve
task docs:build
task site:serve
```

The strict documentation build validates internal links and anchors. The site
build generates an architecture portal from the canonical root corpus, then
composes it with the LaunchKit-derived landing source, Zensical output, and
canonical JSON Schemas into a collision-checked `dist/` artifact. See [Site
publication architecture](docs/site-publication.md) for the boundary between
the landing page, architecture, documentation, generated API reference,
schemas, repository intelligence, and release guidance.

## Project documents

- [Complete architecture document inventory](META.md)
- [Architecture portal source and generation contract](web/architecture/README.md)
- [Purpose](PURPOSE.md), [vision](VISION.md), and [principles](PRINCIPLES.md)
- [Architecture](ARCHITECTURE.md)
- [MVP specification](docs/mvp-spec.md)
- [Safety model](docs/safety-model.md)
- [Artifact-set commit protocol](docs/artifact-set-protocol.md)
- [State model](docs/state-model.md)
- [JSON contract](docs/json-contract.md)
- [CLI outcome contract](docs/cli-contract.md)
- [Configuration and effective policy](docs/configuration.md)
- [Development model](docs/development-model.md)
- [Site publication architecture](docs/site-publication.md)
- [Cloud-native placement](docs/cloud-native-placement.md)
- [Roadmap](ROADMAP.md)

## License

MIT
