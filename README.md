# optiflow

> Observe first. Prove relationships. Plan safely.

`optiflow` is a local-first Rust CLI for inventorying messy media collections,
proving byte-identical duplicate groups, calculating reclaimable storage, and
producing immutable review plans.

The `v0.1.x` product is deliberately read-only. It has no apply, delete, replace,
move, quarantine, or optimization command.

## Current capabilities

The capabilities below describe [merged source at `ddc4b01`](https://github.com/egohygiene/optiflow/tree/ddc4b010caf5ad51c3d3f4b04e6b967a87ff8eea),
checked on 2026-09-26. The latest published binary is still
[`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0);
later extensions, media profiles, PNG validation, and performance work are
available in source. This checkout prepares **`v0.1.1`, pending signed
publication**, with a [read-only external-drive pilot](docs/external-drive-pilot.md).
A version bump or merged PR alone does not make a release available.

- Scan one or more files and directories without modifying them.
- Exclude hidden trees, symbolic links, filesystem crossings, and optiflow's
  own state directory by default.
- Classify files by inspected content rather than filename extension.
- Collect optional container, stream, codec, dimensions, duration, sample-rate,
  channel, and bitrate metadata through `ffprobe` JSON.
- Derive deterministic read-only lossless-PNG review opportunities from
  current, provider-bound evidence without creating outputs or estimating
  savings.
- Persist observations and reusable analysis in a local SQLite database.
- Narrow exact-duplicate candidates by byte length before complete hashing.
- Calculate complete streaming BLAKE3-256 hashes only for size candidates.
- Produce exact-duplicate groups with transparent evidence.
- Calculate potential reclaimable storage without deleting anything.
- Publish schema-versioned run, report, and plan artifacts as coherent,
  marker-sealed sets.
- Generate deterministic review plans with apply-time safety preconditions.
- Emit human-readable or stable JSON output for `flow` and other pipelines.
- Load explicitly selected, operator-locked extension manifests for typed
  inspectors, analyzers, policy contributors, planners, validators,
  report/export providers, and read-only lifecycle observers.

### Media capability matrix

**OptiFlow does not yet produce optimized media or apply replacements.**
Inventory, optional probing, review evidence, and candidate validation are
separate capabilities. Exact deduplication here means evidence and review
planning, not deletion.

Legend: **✓** merged implementation; **~** conditional or limited subset;
**P** planned only; **—** no implementation or selected format-specific profile.
Each status links to its evidence or precise roadmap boundary.

| Files | Inventory / exact dedup | Optional probe | Review evidence | Candidate validation | Candidate production | Apply / replace | Maturity / evidence |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Any regular file (baseline) | [✓][inventory] | [—][probe] | [—][absent] | [—][absent] | [—][absent] | [P][transactions] | [Format-independent inventory][inventory] |
| PNG | [✓][inventory] | [~][probe] | [~][png-review] | [~][png-bytes] | [P][png-plan] | [P][transactions] | [Review + library validator][png-bytes] |
| JPEG | [✓][inventory] | [~][probe] | [—][absent] | [P][quality-plan] | [P][image-plan] | [P][transactions] | [Probe eligible; encoding planned][image-plan] |
| GIF | [✓][inventory] | [~][probe] | [—][absent] | [—][absent] | [—][format-plans] | [P][transactions] | [No GIF producer selected][format-plans] |
| SVG | [✓][inventory] | [—][probe] | [—][absent] | [—][absent] | [—][format-plans] | [P][transactions] | [Inventory only][probe] |
| WebP | [✓][inventory] | [~][probe] | [—][absent] | [P][quality-plan] | [P][format-plans] | [P][transactions] | [Delivery-format evaluation][format-plans] |
| AVIF / HEIF | [✓][inventory] | [~][probe] | [—][absent] | [P][quality-plan] | [P][format-plans] | [P][transactions] | [AVIF evaluation; HEIF inventory/probe][format-plans] |
| JPEG XL | [✓][inventory] | [~][probe] | [—][absent] | [P][quality-plan] | [P][format-plans] | [P][transactions] | [Delivery-format evaluation][format-plans] |
| TIFF, RAW, other images | [✓][inventory] | [~][probe] | [—][absent] | [—][absent] | [—][format-plans] | [P][transactions] | [Recognition varies; preserve RAW][probe] |
| Audio | [✓][inventory] | [~][probe] | [—][absent] | [P][av-validation] | [P][av-plan] | [P][transactions] | [Stream inspection; transforms planned][av-plan] |
| Video | [✓][inventory] | [~][probe] | [—][absent] | [P][av-validation] | [P][av-plan] | [P][transactions] | [Stream inspection; transforms planned][av-plan] |

The baseline row covers arbitrary readable regular files; media rows add the
listed conditional capabilities. Classification inspects bytes, not suffixes.
The locked `infer` recognizers only establish classification eligibility;
optional `ffprobe` coverage depends on the exact installed provider and the
input. SVG has no built-in image recognizer. RAW and container recognition
is partial. **P does not mean supported**: the modern-format entries are
evaluation candidates, and planned apply refers to the shared transaction
engine, not a promise of optimization for every row.

PNG review opportunities do not prove that a candidate can be made smaller.
The separate byte validator is library-only: static noninterlaced 8-bit
RGB/RGBA with a documented metadata subset, exact sample/chunk preservation,
and a strictly smaller encoded candidate. It does not run during scans or
create files. See the [capability evidence and optimizer strategy](docs/optimizer-strategy.md)
for source ownership, limits, OxiPNG sequencing, and the `image_optim` comparison.

[inventory]: docs/optimizer-strategy.md#inventory-and-exact-duplicates
[probe]: docs/optimizer-strategy.md#classification-and-optional-probing
[absent]: docs/optimizer-strategy.md#current-boundaries
[png-review]: docs/media-profiles.md#selection-and-evidence
[png-bytes]: docs/png-candidate-contract.md#actual-byte-validation
[transactions]: ROADMAP.md#v020--transactional-exact-duplicate-resolution
[png-plan]: ROADMAP.md#adapter-framework
[image-plan]: ROADMAP.md#encoders-and-formats
[quality-plan]: ROADMAP.md#quality-validation
[format-plans]: docs/optimizer-strategy.md#format-plans
[av-plan]: ROADMAP.md#ffmpeg-adapter
[av-validation]: ROADMAP.md#validation

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
optiflow extensions list --manifest <FILE> --lock <FILE>
optiflow extensions inspect <EXTENSION_ID> --manifest <FILE> --lock <FILE>
optiflow extensions doctor --manifest <FILE> --lock <FILE>
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
| Report | `optiflow.report.v6` | `schemas/report.schema.json` |
| Plan | `optiflow.plan.v5` | `schemas/plan.schema.json` |
| Media profile evidence | `optiflow.media-profile-evidence.v1` | `schemas/media-profile-evidence-v1.schema.json` |
| Extension manifest | `optiflow.extension-manifest.v1` | `schemas/extension-manifest-v1.schema.json` |
| Extension lock | `optiflow.extension-lock.v1` | `schemas/extension-lock-v1.schema.json` |
| Extension invocation | `optiflow.extension-invocation.v1` | `schemas/extension-invocation-v1.schema.json` |
| Extension result | `optiflow.extension-result.v1` | `schemas/extension-result-v1.schema.json` |

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
The [safe extension SDK](docs/plugin-sdk.md) defines the separate provider
declaration, operator trust, deterministic resolution, and core result-
acceptance boundaries. Extensions receive no source mutation, destructive,
signing, or publication authority.
The [artifact-set commit protocol](docs/artifact-set-protocol.md) defines
coherent scan/plan publication, marker validation, and crash recovery.
The [handle-bound observation protocol](docs/observation-protocol.md) documents
how OptiFlow rejects replacement and in-read races without mixing evidence.
The [media-profile evidence contract](docs/media-profiles.md) defines the first
lossless-PNG review profile, exact provider provenance, limitation semantics,
and why an opportunity is neither an output nor a savings guarantee.

## Ecosystem integration

`optiflow` is independently installable and useful as a standalone CLI. In the
Ego Hygiene suite, [`flow`](https://github.com/egohygiene/flow) is the unified
orchestration facade: it invokes `optiflow` through the CLI and consumes the
versioned JSON artifacts. Sibling tools do not embed optiflow's source or take
an unversioned Rust dependency on its default branch.

## Development

```bash
task validate
task performance:check
```

Equivalent commands:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
task contracts
task extensions:examples
./scripts/smoke-test.sh
python3 scripts/run-performance-baseline.py \
  --binary "target/release/optiflow" \
  --budgets "performance/budgets-v1.json" \
  --output "target/performance-baseline.json" \
  --enforce
```

The end-to-end test uses synthetic files in a temporary directory, includes
spaces and Unicode in paths, and verifies that both duplicate inputs remain
unchanged after scanning and planning.

The [performance baseline](docs/performance.md) uses a separate synthetic
fixture to enforce tolerant release-mode ceilings for cold discovery, complete
candidate hashing, warm-cache reuse, peak resident memory, and committed
artifact size. It retains three independent trials, uses median wall time and
worst-case resource/artifact evidence, and never reads user media or invokes
optional probes.

Dependency admission, security reporting, supported binary targets, signed
release verification, and immutable rollback are defined in the
[release and dependency policy](docs/release-policy.md) and [security
policy](SECURITY.md).

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
- [Handle-bound observation protocol](docs/observation-protocol.md)
- [State model](docs/state-model.md)
- [JSON contract](docs/json-contract.md)
- [Media-profile evidence](docs/media-profiles.md)
- [Media capabilities and optimizer strategy](docs/optimizer-strategy.md)
- [CLI outcome contract](docs/cli-contract.md)
- [Configuration and effective policy](docs/configuration.md)
- [Safe extension SDK](docs/plugin-sdk.md)
- [Generated extension capability reference](docs/extension-capabilities.md)
- [Development model](docs/development-model.md)
- [Performance budgets](docs/performance.md)
- [Release and dependency policy](docs/release-policy.md)
- [Operator-ready release path](docs/current-release-milestone.md)
- [Security policy](SECURITY.md)
- [Site publication architecture](docs/site-publication.md)
- [Cloud-native placement](docs/cloud-native-placement.md)
- [Roadmap](ROADMAP.md)

## License

MIT
