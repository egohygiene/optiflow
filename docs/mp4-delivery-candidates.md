# Explicit MP4 delivery candidates

Relates to #146/#135. The source profile is distinct from the historical
Matroska/FFV1 v1/v2 lanes. It admits up to 512 MiB of explicitly selected input,
up to 256 MiB of candidate bytes, progressive SDR AVC video with a constant
1–120 fps clock and one mono/stereo AAC-LC track at 44.1/48 kHz. Geometry is
bounded to 4096×2160, video samples to 36,000, audio packets to 50,000 and
selected media edits to fifteen minutes. Requested limits may be smaller.

The public `mp4_delivery::inspect` API hashes the entire no-follow, singly-linked
source and reads only bounded container/sample ranges. It independently maps
finite unfragmented MP4 sample sizes/chunks/timestamps, supported leading/media
edits, square pixels, unrotated tracks, declared BT.709 limited color, AAC
configuration, audio payloads and selected UTF-8 metadata. Unknown tracks,
fragments, external references, unsupported metadata/HDR/rotation and malformed
or overlapping samples refuse. No filename or provider statement proves a codec
or its decoded pixel/channel behavior; later native probing/decode is mandatory.

The profile catalog and observation grant no encoding, human quality approval,
replacement, physical-space reclamation, release or publication authority.
Audio copy and exact rational clock/metadata comparison remain later producer
acceptance requirements. Lossy video cannot claim decoded visual equivalence.

The closed `optiflow.mp4-delivery-request.v1` request explicitly selects quality
(CRF/preset) or target-size mode (maximum bytes, mux reserve and preset). Target
planning subtracts observed copied-AAC payload bytes and the selected mux reserve
before allocating a two-pass video bitrate. For example, a 100,000,000-byte
target, 2,000,000-byte reserve, 8,000,000 copied-audio bytes and 240 seconds
allocate 3,000,000 video bits/second. This is a rate estimate; final actual-byte
and strictly-smaller checks still decide acceptance. Impossible budgets refuse
without reducing audio, dimensions, frame rate or duration.

The read-only plan binds full source/launcher/tool pins and explicit whole-work
admission: source plus two candidate-size scratch allowances for faststart,
bounded logs/metadata, publication bytes, host payload buffers and provider
quota. Source, private work and artifact namespaces remain disjoint. Provider
memory, actual scratch/host support and full native decode are not established
by these numbers. No default lossy setting, automatic retry or shell is added.

Explicit `runtime::capture_unaccepted` uses the existing Linux service guard.
The separate MP4 policy admits up to two hours of caller-selected elapsed time
and larger private tmpfs scratch; legacy video policy still refuses these values.
The bounded worker receives pinned tools, exact source bytes and a closed job,
checks actual guard conditions and native codec/pixel/channel scope, and runs
both passes in the same private tmpfs so statistics cannot disappear between
passes. Source files and inherited environments never become arbitrary shell
commands. Version/build observations, job/output hashes, pass-log digests and
actual kernel resource receipts remain separately recorded.

Captured MP4 bytes are **unaccepted** and retained in a fresh private attempt.
No committed candidate or savings claim follows without the next independent
validation/publication step. Unsupported/degraded native guards, non-Linux hosts,
codec diagnostics, quota overflow and unknown partial state refuse. Total host
RSS, syscall preemption and physical reclamation are not claimed.

`mp4_delivery::produce` first preserves already-fitting target-size originals
without encoding. Otherwise it independently checks the final MP4 tables,
complete selected source/candidate native frame/sample decodes, copied AAC bytes
and codec configuration, presentation/edit clocks, supported metadata, actual
byte cap and strict size reduction. Source priming/tail scope is bounded;
unsupported decode/clock behavior refuses. Lossy video never claims exact pixel
equivalence, a perceptual score, musical review or an industry/platform approval.

Only passing technical candidates receive a private versioned artifact, committed
through the existing descriptor/fsync/no-replace primitives. The returned pin
includes an externally retainable marker digest. `artifact::inspect` rechecks
current candidate bytes/structure and retained native evidence with that pin;
it never reopens the original host paths, reruns native decode or upgrades
qualification/review authority. Partial attempts/staging remain preserved and
cannot be adopted as committed artifacts. Physical space reclamation, in-place
replacement, source disposal and public upload remain false.

## Explicit CLI and SDK

The `mp4-delivery` group routes before mutable configuration/state setup. Every
operation uses the same public `mp4_delivery` owner; command output uses the
existing command-result envelope. These are authored interfaces, not executed
examples or a released/qualified binary.

| Command | Scope |
| --- | --- |
| `mp4-delivery profiles` | Pure compiled profile description; no files/tools |
| `mp4-delivery inspect --source PATH --maximum-input-bytes BYTES` | Full bounded source hash and independent container observation |
| `mp4-delivery plan --request FILE` | Explicit source/tool pins, rate allocation and storage/memory admission; no provider invocation |
| `mp4-delivery run --request FILE` | Selected Linux guard, complete technical validation, private candidate pin |
| `mp4-delivery status --pin FILE` | Current private artifact bytes/structure and retained evidence; no original/tool reobservation |
| `mp4-delivery folder-preview --request FILE` | Bounded descriptor discovery, no provider/workspace |
| `mp4-delivery folder-run --request FILE` | Fresh fixed serial composition and immutable journal |
| `mp4-delivery folder-resume --request FILE` | Explicit input/tool reconciliation, then only never-started work |
| `mp4-delivery folder-status --run-directory PATH` | Retained results and uncertainty; no replay |

The standalone request shape is in
[`mp4-delivery-request-v1.schema.json`](../schemas/mp4-delivery-request-v1.schema.json).
Select normalized absolute source/tool/work/artifact paths, actual lowercase
Blake3 tool pins, exact FFmpeg/ffprobe 8.0.1 and the existing Linux user-manager
guard with its pinned helper. Choose every limit explicitly. Private work and
artifact roots must already exist with the existing private-root permissions;
there is no automatic tool acquisition, installation or host setup.

For a decimal 105 MB upload limit, select a target below that limit to retain
headroom, for example the following **control fragment** inside a complete
request. The copied audio and sample tables must still fit the budget:

```json
{"kind":"target_size","maximum_bytes":100000000,"mux_reserve_bytes":4000000,"preset":"slow"}
```

Once a separately qualified binary and real request are available, intended
invocations are:

```sh
optiflow --json mp4-delivery plan --request /private/mp4-request.json
optiflow --json mp4-delivery run --request /private/mp4-request.json > /private/mp4-result.json
# Only an accepted result contains an artifact pin; retain it separately.
jq -e '.result | select(.state == "accepted") | .artifact' /private/mp4-result.json > /private/mp4-pin.json
optiflow --json mp4-delivery status --pin /private/mp4-pin.json
```

An already-fitting source is unchanged without encoding. A validated candidate
that is not strictly smaller is also unchanged, with its generated attempt
identified for review. Standalone attempts remain retained; only folder mode
has the explicit generated-only cleanup policy described below. Neither result
replaces an input or proves available space was physically reclaimed. JSON
bytes in a pin are evidence identifiers, not signing or a human quality approval.

Task aliases expose profiles, plan/run and folder preview/run/resume/status.
For example, `task mp4:plan REQUEST=/private/mp4-request.json` delegates directly
to the same command; it does not build, install or qualify anything.

## Fixed serial folders

[`mp4-folder-request-v1.schema.json`](../schemas/mp4-folder-request-v1.schema.json)
contains a complete single-file request under `template`. Set `template.source`
to exactly `source_root`; it is a root placeholder, not an input file. Per-entry
composition changes only that source path and the remaining elapsed allowance.
Select `recursive`, a fresh `run_directory`, explicit batch bounds and
`scratch_cleanup: "remove_only_known_generated_candidate_after_durable_result"`.
The journal parent, private work and a dedicated **empty** artifact store must
already exist privately. Source/work/store/journal/tool namespaces are disjoint.

Discovery reuses the existing no-follow, device-aware, hardlink-refusing walker
with an explicit MP4 classifier. The complete bounded inventory and exact tool
baseline are frozen before work. Unsupported content receives a retained reason;
unknown metadata/codec/clock features never trigger a fallback codec or profile.
No more than one candidate is produced at a time. Batch caps additionally limit
entries (4,096), files (256), actions (32), depth (32), input/publication admission
(8 GiB each) and the explicit continuation window (24 hours). Per-file profile
and caller limits still apply. Publication admission reserves candidate bytes
plus 128 MiB and a marker allowance per started action for retained audit records.

Before each action, a durable start binds the frozen source and exact child
request. The producer checks those bindings and the frozen tool baseline. Only
after the owner result is durably recorded may folder mode remove its exact
known generated `candidate.mp4` duplicate. Accepted artifacts are re-inspected
first. Audit records, frame hashes and original inputs remain. Failed, uncertain,
unknown or modified scratch is retained; no generic recursive cleanup is added.
The cleanup receipt itself must be durable before the next entry may start.

Status verifies contiguous retained starts/results, artifact pins and their
source/request/tool joins, cleanup receipts, report prefixes and exact artifact
store membership. Current original/tool freshness is explicitly not observed by
status. Missing results/cleanup receipts, failed starts, unknown journal members,
partial staging and unaccounted artifacts block continuation. A crash between
an effect and its durable record requires reconciliation; absence is never used
to adopt an artifact, infer cleanup or replay an encoder invocation.

Explicit resume requires the same request, current full inventory identities
and exact current tool baseline, then proceeds only from never-started entries.
Each resume grants a new explicit bounded elapsed window; cumulative action and
publication admission caps do not reset. It cannot raise request budgets or
recover failed work by retrying it. The existing FFV1 v1/v2 folder defaults,
fingerprints and profile ceilings remain unchanged.

Eleven clock/container/budget/argv/worker/guard/preservation/folder/CLI cases are authored, uncompiled/unrun. All builds,
tests/dependencies/lint/format/schema/docs/drift/native/provider/media/CLI/SDK/CI
execution remains deferred. No actual private media, review, native/macOS
qualification, signing, release or posting occurred. Flow #88 remains last and
untouched; known alignment cancellation/start-marker failure unresolved, cause
unproven. GitHub Pages/live demonstration will be discussed after this sprint.
