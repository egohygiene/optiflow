# Filesystem and removable-volume corpus

Checkpoint [#88](https://github.com/egohygiene/optiflow/issues/88) supplies
repeatable, synthetic evidence for the read-only external-drive pilot (#89)
and the future exact-duplicate transaction. It grants no mutation authority.
The broader [#65](https://github.com/egohygiene/optiflow/issues/65) stays open
for media, provider, perceptual, and candidate-output families.

## Run and reproduce

```bash
python3 scripts/filesystem-corpus.py --check
python3 -m unittest tests/test_filesystem_corpus.py
python3 scripts/filesystem-corpus.py --check --tier pr
python3 scripts/filesystem-corpus.py --check --tier scheduled
```

The same PR tier runs through `scripts/run-adversarial-tests.sh corpus` and
the Linux/macOS adversarial workflow. Weekly scheduled runs and the explicit
`corpus_tier=scheduled` workflow input run the heavier tier on Linux only;
they do not also launch the ordinary matrix or fuzz jobs.

[`cases.json`](../tests/fixtures/filesystem/cases.json) is the reviewed recipe
and expected-outcome catalog. Every entry has a stable ID, fixed seed (zero;
these recipes do not use randomness), recipe, proof selector, source, MIT
license, and cleanup contract. No downloaded or personal media is required.
Temporary native paths, inode/device numbers, timestamps, and run UUIDs are
intentionally not golden values.

[`index.json`](../tests/fixtures/filesystem/index.json) binds each recipe,
expected outcome, literal payload, and proof source with SHA-256, plus the
generator and `Cargo.lock`. The recipe digest describes a logical fixture,
not a portable serialized filesystem image. Sparse zero regions and integer
payloads are specified by recipe instead of checking large blobs into Git.
Optiflow continues to use BLAKE3 for content evidence. After reviewing an
intentional recipe, assertion, generator, or dependency change, regenerate:

```bash
python3 scripts/filesystem-corpus.py --write-index
git diff -- tests/fixtures/filesystem/
```

The runner always checks drift before execution. It builds locked test
binaries, resolves each exact selector, rejects zero/ignored tests, repeats
proofs in fresh temporary roots, and records per-fixture receipts under a
new `target/adversarial-evidence/filesystem/<tier>-*/` directory. Typed Rust
assertions determine success; receipts bind those assertions to their source
and expected-outcome digests. They are not a second source of runtime facts.
The new integration cases directly compare their normalized results with
the catalog. Reused unit/CLI cases retain their existing typed assertions.

The initial [local validation record](validation/optiflow-88-local.json) binds
the reviewed index to Linux PR/stress results and states toolchain/platform
limitations. It is checkpoint evidence, not a claim about future CI runs.

## Coverage and contracts

| Fixture ID | Evidence and expected boundary | Owning contract |
| --- | --- | --- |
| `fs88-identities` | 3 paths, 2 independent objects, 1 alias; 30 logical duplicate bytes, physical savings unknown; plan is read-only | [Safety](safety-model.md), [report schema](../schemas/report.schema.json) |
| `fs88-paths` | Repeated/overlapping roots coalesce; Unicode, non-UTF-8, newline and 32-deep paths round-trip; symlinks and broken links are excluded | [Observation](observation-protocol.md), [JSON](json-contract.md) |
| `fs88-allocation` | Sparse recipe reports actual `st_blocks × 512`; injected allocation gap and clone uncertainty remain unknown | [Filesystem identity](../src/filesystem/identity.rs), report schema |
| `fs88-volume` | 0444 file/0555 root scanned with separate state; unavailable root produces no artifacts; reconnect succeeds | [State](state-model.md), [CLI](cli-contract.md) |
| `fs88-permission` | Injected `PermissionDenied` before open; two attempts, unreadable/unavailable, no content or identity evidence | Observation protocol |
| `fs88-disconnect` | Rename synthetic volume after open; discard evidence, then freshly observe reconnected path | Observation protocol |
| `fs88-replacement` | Same-sized path replacement after open cannot publish current hash | Observation protocol |
| `fs88-resize` | Truncation and growth reject the entire observation | Observation protocol |
| `fs88-metadata-race` | Explicit mode change after evidence rejects current status | Observation protocol |
| `fs88-boundary` | Distinct device IDs respect opt-in crossing; unknown device keeps the existing documented policy | [Discovery](../src/discovery.rs) |
| `fs88-state` | Same-path replacement misses cache; old plans remain review-only; tampered report/plan sets are incomplete and cannot feed a new plan | State model, [artifact sets](artifact-set-protocol.md) |
| `fs88-cache-signature` | Provider/ctime mismatch and legacy null identity rows miss cache | State model, migrations 0002–0005 |
| `fs88-capacity` | ENOSPC at four staged bytes exposes no committed set; cleanup is idempotent; restored capacity permits publication | Artifact-set protocol |
| `fs88-artifact-state` | Missing member is incomplete; unknown marker schema is incompatible | Artifact-set protocol |
| `fs88-run-recovery` | Committed set recovers a stale running SQLite row | State model |
| `fs88-missing-run` | Malformed run and missing UUID have distinct typed outcomes | CLI contract |
| `fs88-stress` | 512 distinct four-byte files in 16 buckets; complete inventory, no duplicate groups | Report schema; scheduled tier |

The catalog reuses observation, discovery, cache, artifact, and recovery tests
from the [adversarial matrix](adversarial-testing.md). Existing property,
fuzz, smoke, schema, migration, and [performance](performance.md) evidence
remain complementary. This corpus does not replace their coverage.

## Resource and cleanup contract

| Budget | PR | Scheduled |
| --- | --- | --- |
| Fixtures × fresh repetitions | 16 × 2 | 17 × 10 |
| Whole execution wall time | 120 seconds | 600 seconds |
| One proof wall time | 20 seconds | 30 seconds |
| Memory | 2 GiB | 2 GiB |
| One writable file | 8 MiB | 16 MiB |
| Temporary workspace, logical bytes | 64 MiB | 128 MiB |
| Retained logs and final receipts | 8 MiB | 16 MiB |

Linux enforces a per-process address-space limit; macOS watches
the entire test process group's resident memory. File-size limits are inherited
by child processes. The watchdog checks aggregate temporary/evidence bytes,
resident memory, and wall time between 20 ms polling intervals, plus process
inspection time. Sampled ceilings can transiently overshoot; their measured
peaks are labeled as sampled (resident peak is null on Linux, where the hard
address-space ceiling applies instead). A breach kills the process group and fails the
run. The final receipt must also fit the evidence budget. Core dumps are
disabled. Seven small Python tests exercise these refusal paths.

Compilation has a separate 600-second timeout, runs before execution budgets,
and uses the ordinary Cargo cache. CI also has outer job deadlines. Build
artifacts are not fixture evidence. Failure logs are retained within the
evidence ceiling; each proof must clean its temporary workspace even on a
successful run, and the runner removes the isolated workspace on failure.

Source preservation checks compare bytes, native paths, links, modes,
identities, and modification/change timestamps around integration scans.
Access time is excluded because host read policy may update it. Harness faults
are explicit test-operator changes: snapshots after those changes distinguish
them from scanner writes. No source-media mutation command is added.

## Evidence limits

Directory rename emulates path disappearance and reconnection; it is not a
physical USB detach, kernel unmount, or power-loss durability proof. Mode bits
exercise readable sources with no write bits, not a kernel read-only mount.
Injected permission errors run even under root. Capacity loss is injected at
the artifact writer, not measured as free space on a real volume. Boundary
changes use deterministic device IDs, not privileged mount operations.

Sparse allocation varies by host, so tests compare observed allocation to
native metadata without requiring holes. Clone-capable filesystems are modeled
as unknown extent sharing; no APFS clone/reflink detection or physical savings
claim is introduced. Read-only reports and plans are historical evidence, not
current execution authorization. Real-drive trials belong to #89; mutation,
recovery and reclamation proofs belong to #90–#93/#96. Media/provider/candidate
families remain under #65, #94 and later checkpoints.
