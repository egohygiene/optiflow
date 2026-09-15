# Adversarial testing

OptiFlow's adversarial suite turns the read-only safety claims into a bounded,
repeatable test matrix. It complements the ordinary unit, integration, and
end-to-end suites; it does not grant permission to mutate source media.

## Threat and fault matrix

| Threat or fault | Required invariant | Automated evidence | Environment and bound |
| --- | --- | --- | --- |
| Rename, replacement, truncation, growth, or metadata race during observation | Evidence from incompatible file states is never published as current | `observation::tests` | Linux and macOS; deterministic hooks, two observation attempts maximum |
| Symbolic-link substitution or traversal | Link following remains opt-in and a path that becomes a link is rejected | `discovery::tests`, `observation::tests`, configuration CLI tests | Linux and macOS; temporary filesystem only |
| Unreadable inputs and directories | Permission failures become bounded, structured incomplete coverage instead of panics or silent omission | `discovery::tests`, CLI outcome tests | Linux and macOS; permission test is skipped when the runner can bypass mode bits |
| Filesystem or mount boundary | Traversal stays on the root device unless `cross_filesystems` is explicit | `discovery::tests::mount_boundary_requires_an_explicit_cross_filesystem_policy` | Pure device-identity decision test on every CI platform; no privileged mount required |
| SQLite migration or transaction failure | A failed migration cannot publish its schema version marker; stale cache identity is not reused | `state::tests` | In-memory and temporary SQLite; one injected failure per migration |
| Crash or disk exhaustion during artifact publication | Readers see committed, incomplete, or incompatible sets—never a partially committed set | `artifact_set::tests` | Deterministic byte/member failpoints; temporary filesystem only |
| Corrupt, truncated, oversized, or contradictory artifact metadata | Readers return a typed non-committed status and never trust unverified members | `artifact_set::tests`, `artifact-set-reader` fuzz target, contract tests | 15-second fuzz budget, 5-second input timeout, 64 KiB input maximum |
| Hostile path and configuration metadata, including control characters and non-UTF-8 bytes | Machine identity round-trips losslessly; human display cannot emit raw control characters | `properties` integration test, `config-document` fuzz target | 256 cases per property; 15-second fuzz budget and 64 KiB input maximum |
| Arbitrary report member and alias order | Review defaults, candidates, preconditions, summaries, and evidence are stable after volatile IDs/timestamps are removed | `properties::plan_decisions_ignore_report_member_and_alias_order` | 256 generated duplicate groups of two to eight members |
| External process hangs, cancellation, descendant-held pipes, non-zero exit, invalid JSON, or excessive output | The direct child is terminated and the caller receives a typed bounded error without waiting indefinitely for inherited pipe descriptors | `subprocess::tests` | Two-second test timeout, 1 KiB stdout/stderr limits, two-process concurrency limit |
| Disabled, unavailable, stale, partial, failed, timed-out, or successful-but-invalid PNG provider evidence | No review opportunity is emitted without current handle-bound and semantically valid evidence; source bytes remain unchanged | `media_profiles::tests`, `adapters::ffprobe::tests`, `media_profiles_cli` integration test, smoke test | Hermetic local scripts and one checked-in one-pixel PNG; installed `ffprobe` only in the existing Linux/macOS smoke matrix |
| Extension incompatibility, malformed evidence, unsafe plans, cancellation, partial coverage, precedence conflicts, or provider crash | Selection fails closed; rejected contributions never cross the host boundary; a fresh checkpoint-bound invocation remains possible | `extensions` integration test | Explicit manifests and locks; bounded process fixtures; no source-media mutation |

The matrix deliberately separates simulated boundaries from privileged system
tests. Device identity is injected into a pure policy decision rather than
mounting filesystems in CI. Permission behavior uses real mode bits where the
runner enforces them. This keeps failures reproducible without overstating what
an unprivileged runner proves.

## Local commands

Run the deterministic property and fault suites:

```console
./scripts/run-adversarial-tests.sh properties faults
```

With nightly Rust and `cargo-fuzz` 0.13.2 installed, run both bounded fuzz
targets. LeakSanitizer is disabled because it is incompatible with ptrace-based
runners; libFuzzer's remaining AddressSanitizer checks stay active.

```console
./scripts/run-adversarial-tests.sh fuzz
```

Every invocation writes one log per matrix slice plus `summary.tsv` under
`target/adversarial-evidence/`. The command continues through every requested
slice and exits unsuccessfully if any slice failed.

## CI evidence

The `Adversarial tests` workflow runs properties and deterministic fault cases
on Linux and macOS. A separate Linux job runs both fuzz targets under nightly
Rust. Each job has a 15-minute outer deadline, and fuzz inputs have their own
time and size bounds. Logs and any libFuzzer crash artifacts are uploaded for
30 days even when a step fails.

Generated cases are reproducible from the failure information printed by
proptest or the saved libFuzzer artifact. A fixed regression should be promoted
to a named deterministic test or a checked-in corpus input before the incident
is closed.
