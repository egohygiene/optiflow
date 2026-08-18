# CLI Outcome Contract

OptiFlow exposes one process-outcome contract to people, shell scripts, CI,
and orchestration tools. Domain findings and process outcomes are separate: a
complete scan of an empty directory, a complete scan with no duplicates, and a
complete plan with no actions are all successful results.

## Stable exit codes

| Code | Outcome | Meaning |
| ---: | --- | --- |
| `0` | `success` | The command completed its declared completeness contract. |
| `1` | `internal_failure` | An internal, persistence, invariant, serialization, validation, or artifact-commit failure prevented a trustworthy result. |
| `2` | `invalid_input` | The invocation or caller-controlled input was invalid. |
| `3` | `partial_success` | A valid result exists, but requested coverage or evidence is incomplete. |
| `4` | `capability_unavailable` | A capability required by the requested operation is unavailable. |
| `5` | `stale_state` | Stored or supplied state is missing, incompatible, or unsuitable for the requested conclusion. |
| `130` | `interrupted` | OptiFlow handled `SIGINT` and stopped without claiming completion. |
| `143` | `terminated` | OptiFlow handled `SIGTERM` and stopped without claiming completion. |

These numbers are identifiers, not a severity scale. Automation must compare
exact values and must not infer meaning from numeric ordering or diagnostic
message text.

## Complete and partial results

Complete success means the requested operation produced a trustworthy result
under its declared coverage contract. Warnings with `impact: "none"` do not
change that outcome. In particular, an unavailable optional `ffprobe` does not
make an otherwise complete file and duplicate inventory partial.

Partial success means OptiFlow produced a valid, inspectable result while
known exclusions reduced its coverage. Examples include one unavailable root
in a multi-root scan, a traversal error, or observations excluded because
current evidence could not be established. Each known limitation is a typed
diagnostic with `impact: "degrades_coverage"`.

If every scan input fails preflight, no meaningful inventory exists: the
outcome is `invalid_input` and OptiFlow creates neither scan state nor
artifacts. Once at least one valid input produces meaningful work, failures
limited to other roots produce a partial result instead.

`report` and `plan` propagate limitations from their source run. A report or
review-only plan derived from partial evidence returns `3`; the plan still
declares `mutates_files: false`. A missing or incompatible prerequisite that
cannot produce a valid result returns `5`. Malformed caller-supplied JSON is
invalid input (`2`), while a well-formed artifact with an unsupported schema is
incompatible state (`5`).

## Command-result envelope

`--output-format json` and its compatible `--json` alias write exactly one
`optiflow.command-result.v1` document to standard output whenever structured
rendering is possible:

```json
{
  "schema": "optiflow.command-result.v1",
  "command": "scan",
  "outcome": {
    "class": "partial_success",
    "exit_code": 3
  },
  "coverage": {
    "status": "partial"
  },
  "artifacts": [],
  "diagnostics": [],
  "result": {}
}
```

The checked-in contract is
[`schemas/command-result.schema.json`](../schemas/command-result.schema.json).
It uses a closed vocabulary for outcome classes, exit codes, diagnostic codes,
and native-path encodings. `outcome.exit_code` always equals the actual process
status. Human messages may improve between releases; `code`, `classification`,
`impact`, and typed context are the automation contract.

Command results are distinct from immutable run, report, and plan artifacts.
This change does not modify `optiflow.run.v1`–`v3`, `optiflow.report.v1`–`v3`,
or `optiflow.plan.v1`–`v3`. In JSON mode, an artifact-valued command result is
now carried in the envelope's `result` field and committed artifacts are listed
in `artifacts`.

## Stream ownership

- JSON mode owns stdout exclusively and writes one buffered JSON document plus
  a trailing newline. It emits no banner, progress text, color, or debug log.
- Human primary output uses stdout.
- Human diagnostics and future progress output use stderr.
- Diagnostic text and display paths escape terminal control characters.
- JSON native-path context uses the lossless tagged path representation.

The JSON renderer serializes the complete envelope before writing it, so a
serialization failure cannot leave a truncated JSON prefix on stdout.

## Signals and recoverability

On Linux and macOS, OptiFlow handles `SIGINT` and `SIGTERM` cooperatively. It
stops accepting new scan work, checks for cancellation between discovery
entries and observations, and checks every 1 MiB while hashing. An interrupted
database run is marked `interrupted`, never `completed`.

Already committed cache entries remain valid and readable. A validated run or
report artifact that was atomically committed before interruption may be
listed in the interruption envelope, but OptiFlow never presents the run as
complete. In-memory observations are not promoted to an immutable partial run
artifact because the current `v3` schemas do not model an interrupted run;
changing those immutable schemas is outside this contract.

OptiFlow v0.1.0 has no background worker pool. Graceful interruption is bounded
by its cooperative checkpoints, except that an operating-system metadata call,
SQLite call, or an already-running optional external probe must first return.
Repeated handled signals are idempotent; callers that require a hard deadline
should enforce one outside OptiFlow. Structured output
is either one complete JSON document or empty if the process cannot reach the
renderer safely.

## Shell and CI consumption

Capture the command status before another shell command overwrites it:

```bash
set +e
optiflow scan --output-format json "./media" > "optiflow-result.json"
optiflow_exit_code="$?"
set -e

case "${optiflow_exit_code}" in
  0)   printf "%s\n" "OptiFlow completed successfully." ;;
  1)   printf "%s\n" "OptiFlow encountered an internal failure." >&2 ;;
  2)   printf "%s\n" "OptiFlow rejected the invocation or input." >&2 ;;
  3)   printf "%s\n" "OptiFlow produced a partial result." >&2 ;;
  4)   printf "%s\n" "A required capability is unavailable." >&2 ;;
  5)   printf "%s\n" "Stored or supplied state is stale or incompatible." >&2 ;;
  130) printf "%s\n" "OptiFlow was interrupted." >&2 ;;
  143) printf "%s\n" "OptiFlow was terminated." >&2 ;;
  *)
    printf "%s\n" \
      "OptiFlow returned an undocumented status: ${optiflow_exit_code}" >&2
    ;;
esac
```

A review pipeline may intentionally accept both complete and partial results:

```bash
case "${optiflow_exit_code}" in
  0|3) process_optiflow_result "optiflow-result.json" ;;
  *)   exit "${optiflow_exit_code}" ;;
esac
```

Exit status is never mutation authorization. Any future apply operation must
independently revalidate every candidate against current metadata, a complete
hash, and byte-for-byte confirmation.
