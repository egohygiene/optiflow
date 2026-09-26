---
title: External-drive read-only pilot
description: Run bounded read-only trials with local state, private reports, and explicit restart and rollback limits.
---

# External-drive read-only pilot

`v0.1.1` is a release candidate until [#89](https://github.com/egohygiene/optiflow/issues/89)
records signed publication and independent verification. The latest published
binary remains [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0).
Both have read authority only: scans, reports, and review plans do not delete,
replace, move, quarantine, optimize, or reclaim space. Later source features
are not retroactively available in the old binary.

## First external-drive trial

Install the exact host archive using the [verification and installation
instructions](getting-started.md). Start with a disposable copy of a small
collection before increasing its size. Put **all state and command output on
the local drive, outside every scan root**. This includes the SQLite database,
report captures, plans, and shell redirections. Do not scan your entire home
directory while storing trial output underneath it.

```bash
pilot_directory="${HOME}/optiflow-pilot"
source_directory="/Volumes/DisposableMedia"
mkdir -p "${pilot_directory}"

optiflow --no-config --state-directory "${pilot_directory}/state" \
  --json scan --no-probe "${source_directory}" \
  > "${pilot_directory}/scan.json"
scan_status=$?
printf "Scan exit code: %s\n" "${scan_status}"
```

On Linux, select the actual mounted directory instead of `/Volumes/...`.
Verify the expected volume is mounted at that path. The example assumes an
interactive shell; scripts using `set -e` must explicitly handle nonzero scan
outcomes before reading the JSON. Keep the JSON even when the exit is nonzero.

`--no-config` avoids user/project configuration. Clear any `OPTIFLOW_*`
environment overrides for a baseline trial. `--no-probe` avoids optional
`ffprobe` work; add probing only after the baseline succeeds. Hidden entries,
symbolic links, filesystem crossings, and OptiFlow's state are excluded by
default. Complete coverage means complete within the selected policy; excluded
files have not been inventoried. See [configuration](configuration.md) before
changing those defaults or loading explicitly trusted providers.

| Result | Meaning and next action |
| --- | --- |
| Exit `0`, `success` | Inspect the coverage and report summary within the selected policy. |
| Exit `3`, `partial_success` | Some requested scope was unavailable or unreadable; inspect coverage and diagnostics before relying on the inventory. |
| Exit `2`, `invalid_input` | For example, the only requested root was missing; reconnect/correct the root and start a fresh scan. |
| Exit `130` / `143`, `interrupted` | SIGINT / SIGTERM stopped the scan; completed artifacts must not be inferred. |

Other typed failures are documented in the [CLI contract](cli-contract.md).
Machine mode writes one `optiflow.command-result.v1` JSON envelope to stdout,
including diagnostics; it does not mix progress messages into that stream.
These contracts are unchanged by the pilot.

Read the run identifier from `result.run.run_id` in a successful scan. Keep
using the same state directory:

```bash
run_id="<run-id-from-scan>"
optiflow --no-config --state-directory "${pilot_directory}/state" \
  --json report "${run_id}" > "${pilot_directory}/report.json"
optiflow --no-config --state-directory "${pilot_directory}/state" \
  --json plan exact-duplicates --run "${run_id}" \
  --output "${pilot_directory}/review-plan.json" \
  > "${pilot_directory}/plan-result.json"
```

The plan declares `mutates_files: false`. A proposed keep path is a review
default. Logical duplicate bytes and allocation evidence are not a guarantee
of physical space that a later transaction would reclaim.

## Privacy and local writes

Source files are opened read-only. OptiFlow writes its local state and requested
report/plan artifacts, and the shell writes redirected output. Report and state
contents can disclose filenames, directory structure, native path bytes,
identities, hashes, media metadata, and provider paths. Base64 native-path
representations are reversible, not anonymization. State backups carry the same
exposure. Keep them private; share only a deliberately redacted excerpt when
reporting an issue. The pilot runner generates its own synthetic files and
retains aggregate measurements rather than a personal media inventory.

## Cancellation, restart, and reconnect

Press Ctrl-C once and wait for the typed interruption result. An interrupted
run is not promoted to a completed report/plan set. Rerun the same scan command
to start a **new run with a new identifier**. Stable, previously persisted
observations can reuse the cache; there is no exact queue/cursor resume command
and no promise to continue at the last pathname. An immediate interruption can
leave no useful cache to reuse.

An uncatchable kill or power loss is different: a running database row can
remain. Recovery can reconcile an already committed, marker-sealed artifact
set, but cannot turn uncommitted work into a complete scan. Start a new scan and
retain incomplete evidence for diagnosis. See the [state model](state-model.md)
and [artifact-set protocol](artifact-set-protocol.md).

If the drive disappears, stop relying on the current inventory. A root missing
at startup is invalid input; loss during observation can produce partial or
unavailable evidence. Reconnect the intended volume, verify its mount/root,
then rerun. Path reuse alone is not proof that the same file or volume returned;
fresh native identity/stability checks decide whether observations can be reused.

The deterministic [filesystem corpus](filesystem-corpus.md) supplies exact
reproduction recipes: `fs88-volume`, `fs88-disconnect`, `fs88-permission`,
`fs88-replacement`, `fs88-resize`, `fs88-metadata-race`, `fs88-state`, and
`fs88-run-recovery`. Permission denial and mid-observation changes use explicit
fault hooks, including when tests run as root. The pilot's rename/reconnect
trial simulates disappearance; it is not a physical USB removal or power-loss
test across every filesystem/controller.

## Upgrade and rollback

1. Stop every OptiFlow process using the state directory. Retain the verified
   `v0.1.0` executable and record the exact state path.
2. Copy the **entire closed state directory**, including the database and all
   artifact subdirectories, to a separate local backup outside scan roots.
   Copying only SQLite is insufficient. Keep the original directory location.
3. Install the independently verified new executable. Use it to read an old
   report, then run a new scan against the original state location.
4. To roll back, stop OptiFlow again, move the upgraded state aside, and restore
   the full backup **at its original absolute path**. Artifact references may
   contain absolute paths. Run the retained old executable and read the old report.

Do not run the old binary directly against upgraded state. This is an offline
backup/restore procedure, not a reverse migration or an in-place state downgrade.
Retain both state copies until review is complete; no source media is changed.
The automated trial verifies this sequence against the signed `v0.1.0` bundle
on each native host before it can qualify that host's release archive.

## Release qualification

The existing manual release workflow now builds and executes on native Linux
x86-64, Intel macOS, and Apple silicon macOS. Cross-compilation without native
execution cannot satisfy a target. Python 3.12, Git, Cosign 3.0.6, the built
binary, and the downloaded `binary-v0.1.0.tar.gz` are required to run the
repository-owned qualification harness; normal binary users do not need Python.

From a clean, committed checkout with a locked release build:

```bash
python3 scripts/qualify-read-only-pilot.py \
  --binary "target/release/optiflow" \
  --target "x86_64-unknown-linux-gnu" \
  --release-version "v0.1.1" \
  --source-revision "$(git rev-parse HEAD)" \
  --previous-bundle "/local/downloads/binary-v0.1.0.tar.gz" \
  --output-directory "target/pilot-v0.1.1"
```

Choose the native target and corresponding binary path on macOS. Use a fresh
output directory for each attempt. The harness clean-installs the exact archive,
verifies the old release's checksums, provenance, and Sigstore identity before
executing it, and creates disposable sources outside all state directories.

| Trial | Required evidence |
| --- | --- |
| Large tree | 4,096 deterministic files; cold and warm scans, with 4,096 warm cache hits. |
| Large files | Two independently materialized 256 MiB files; one exact duplicate group and two warm cache hits. |
| Read-only commands | Scan, report, review plan; before/after source digests and metadata agree. |
| Partial / disconnect | Typed partial coverage, absent root refusal without artifacts, successful reconnect. |
| Interruption / restart | SIGINT after a durable running row; interrupted status, no completed artifacts, new run on restart. |
| Upgrade / rollback | New binary reads old evidence and scans; restored complete backup is readable by the verified old binary. |
| Resources / cleanup | Per-command elapsed seconds, cold/warm state bytes, aggregate state bytes, peak child RSS, and successful disposable-workspace cleanup. |

The profile uses roughly 512 MiB of materialized source data plus local state
and retained archives. Each ordinary command has a 180-second timeout. Peak
child RSS is the operating system's maximum across child processes, including
Cosign and CLI commands, not a per-scan peak or total concurrent memory.
Timings and artifact growth are observations from that host; they are not a
universal throughput, memory, or external-drive SLO. Source snapshots compare
content and metadata (excluding access times and the deliberately renamed root's
metadata); this is bounded preservation evidence, not a kernel write audit.

The resulting receipt binds the source revision, archive digest, executable
digest, profile, outcomes, and measurements. Bundle preparation and independent
verification require all three matching receipts. The signed subject manifest
covers `pilot-qualification.json` as well as the archives, SBOM, and provenance.
Unit-test receipts are explicitly synthetic; they cannot stand in for native
workflow execution. #89 stays open until native release qualification, signed
publication, and independent download verification complete. #90 follows with
non-mutating execution and approval contracts.
