# Synthetic explanation CLI walkthrough

Relates to #133. This checkpoint exercises the existing process SDK and pinned
explanation owners with a deterministic fake provider. It does not execute a
model or qualify explanation usefulness. The issue remains open for its remaining
model, configuration and observability acceptance work.

## Repeat it locally

Use Rust 1.89 or newer, Python 3.12 or newer and Linux procfs. Procfs is needed
only to observe the specific fixture child before sending the cancellation
control; the script never reads process environments. No FFmpeg, model server,
credentials, network endpoint or private media is needed.

```sh
cargo build --locked --bin optiflow --example fake_explanation --jobs 2
task explain:fake:observers
task explain:fake:proof \
  OPTIFLOW="$PWD/target/debug/optiflow" \
  FIXTURE="$PWD/target/debug/examples/fake_explanation" \
  OUTPUT="/absolute/existing-parent/new-private-proof"
```

The output directory must not exist. The driver creates it with mode 0700 and
retains command stdout/stderr, immutable source records, provider pins and outputs.
Keep that directory private: ordinary scan reports contain synthetic file paths.
`receipt.json` is a separate allowlisted harness observation with hashes,
correlation IDs, elapsed times, byte counts and explicit limits on its claims.
It contains no command argv, raw source paths, prompts, environment values or
media bytes. It is not production telemetry or signed release evidence.

`task explain:fake:build` builds only the fixture. The fixture's `setup` command
authors a manifest, operator lock and profile using the public SDK's typed
fingerprint functions and current schema bytes. It never installs or starts a
model. The explicitly trusted `serve` process reads bounded stdin, checks that
its inherited environment is empty and emits a schema-bound SDK result. Its
source does not open media, sockets or output files. The SDK is a trusted process
boundary, not an OS sandbox; this proof does not attest network isolation of an
arbitrary replacement provider.

## What the proof checks

Four newly generated files total 209 logical bytes: two identical 70-byte PNGs,
an instruction-like filename with a synthetic content canary and another unique
binary file. `scan --no-probe` observes four files, two media files, two unsupported
files, one duplicate group containing two files, and 70 potential logical bytes.
`plan exact-duplicates` records one review action. No apply command runs.

The actual CLI then performs:

1. `explain prepare` with independently computed BLAKE3 pins for the exact current
   report v6 and plan v5 bytes. Only count facts and record identities enter the
   packet; source filenames, contents and paths are absent.
2. `explain invoke` with one explicitly pinned local process, manifest, lock,
   configuration, profile, template and output schema. There is one SDK invocation
   on this successful path; no retry, cache reuse, fallback or discovery is
   requested. The fake process accepts only a normalized packet, checks an empty
   environment and returns fixed advisory notes with exact fact citations.
3. `explain accept` and `explain inspect`, checking exact packet/candidate identity,
   host-projected numeric values, citation membership and unchanged note text.
   The inspection must reproduce the same sidecar. Provider/model/runtime/profile,
   template, output-schema, manifest, configuration and authorization identities
   remain bound to the result.

The host receives synthetic secret/proxy canaries; the fake child observes **no
environment variables**, including PATH/HOME/provider keys. A filename saying to
publish secrets remains ordinary source data and never enters the normalized
packet. This establishes the exercised subprocess handoff, not semantic resistance
of a real language model.

Negative controls require native and structured outcomes to agree. A valid-looking
JSON refusal paired with a panic exit is rejected by the observer. Disabled
configuration must return its exact disabled result with no candidate/SDK outputs;
the fixture's disabled mode would fail if executed. Unknown citations, changed
returned identity, malformed SDK JSON, unavailable output, oversized stdout,
timeout, remote profile, changed model with a stale profile pin and unknown secret
profile fields refuse with exit 2 and the expected input diagnostic. Existing
destination, changed report bytes with old pins and changed sidecar text also
refuse. These cases publish no newly accepted advisory. All baseline media bytes,
modes and modification times, report/plan/packet bytes and already accepted SDK,
candidate and sidecar bytes must remain unchanged.

The cancellation control observes the actual fixture executable as the CLI's
child before sending SIGINT. It requires exit **130**, structured `interrupted`,
and absent candidate/SDK outputs. The baseline CLI incorrectly mapped this signal
to input failure 2. The narrow repair routes explanation errors with a recorded
signal through the same `interruption_execution` owner used by other commands;
ordinary input failures and all SDK/advisory validators remain unchanged.

## Actual observation and remaining limits

The original `2026-11-14-fake-walkthrough-receipt.json` and build observation
preserve their byte-for-byte bindings to checkpoint `795c96f`, including that
driver's source hash. A later observer review found that its input-refusal check
accepted a matching message substring within any diagnostic. This was less precise
than the guide's exact-refusal claim, although the retained actual outputs already
contained the expected single diagnostic and null result.

The correction requires each of the twelve case-specific **complete** messages,
exactly one `execution_plan_invalid/error/input/blocks_command` diagnostic,
`artifacts: []`, `result: null`, and the closed expected envelope with matching
native exit 2 and structured `invalid_input`. Timeout and oversized-output messages
bind the exact selected fixture path rather than a hardcoded machine path. The
SIGINT case similarly requires its exact single
`operation_interrupted/error/interruption/blocks_command` diagnostic and complete
SIGINT message with exit 130. Extra diagnostics, substituted reasons, unexpected
result data, extra fields, duplicate JSON members and nonempty error stderr refuse.

`2026-11-14-strict-observer-reobservation.json` records a targeted read of the
existing proof07 captures using this stricter observer. All twelve refusals and
the interruption passed. This **did not execute the CLI, fixture or whole
walkthrough again**. Native exits are taken from the pinned original receipt;
capture hashes are newly bound at reobservation, while the original receipt bound
their byte counts. The original eight successes and whole-run filesystem checks
remain the original observation. Thirteen focused Python tests passed, including
156 mutations across the twelve refusal cases, plus panic/competing-stderr,
duplicate-JSON, citation, value, identity and authority controls.

To reobserve private retained captures without running a product command:

```sh
python3 -B scripts/reobserve-fake-explanation.py \
  --captures /absolute/private/retained-proof-directory \
  --fixture /absolute/preserved/fake_explanation \
  --receipt-sha256 INDEPENDENT_ORIGINAL_RECEIPT_SHA256 \
  --output /absolute/existing-parent/new-reobservation.json
```

The dated receipt and build observation beside this guide record the actual
executable/source hashes and final command outcomes. Earlier local failed runs
were retained rather than overwritten: proof01 caught the observer's `plan`
command-name assumption; proof02 found unavailable procfs `children`; proof03
demonstrated the real SIGINT mapping defect; proof04 corrected the observer's
duplicate-file versus candidate count; proof05 corrected the expected remote
profile diagnostic wording. None of those are passing qualification receipts.

Exact citations only establish membership and host-projected recorded counts.
The sidecar continues to say suggestion meaning is unverified, current source
eligibility and physical reclamation are unknown, and apply authority is false.
Its `provider_execution_observed` field remains false by its existing admission
contract; the separate `invoke` result records the actual SDK process execution.
Neither record claims model execution.

`diagnostics doctor` is exercised, and reports no runtime logging observation,
subscriber initialization or compiled exporter. Queue/drop counts remain null.
The harness's timings/correlation and fake diagnostic are **not** an existing
production AI observability port. Token use and provider cost are null, rather
than guessed zero. Missing-model SDK output currently surfaces through the generic
advisory-boundary input refusal; it does not provide a richer model-service
availability taxonomy. This proof uses explicit SDK locks with `--no-config`;
it does not qualify every suite config-overlay precedence case.

Actual model/profile/resource suitability, semantic usefulness, optional real
service networking and dedicated bounded runtime telemetry remain separate work.
No paid/cloud fallback, model acquisition, automatic lyrics, human approval,
signing, publication, native-volume trial, release matrix or hosted CI is claimed.
