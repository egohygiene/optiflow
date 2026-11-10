# Explicit bounded video folder v2

Issue #146 source checkpoint; no native invocation or qualification occurred.

`candidate video --request ABS` selects the existing v1/v2 single-file producer
by exact schema. `video-folder discover|run|resume --request ABS` selects an
explicit folder request. `video-folder status --run-directory ABS` reads recovery.
No provider installation, PATH search, replacement or original disposal is added.

`video-folder-request.v2` carries the existing FFV1/SDR closed Matroska v2 profile
through discovery, production, artifact readback and recovery: 128 MiB encoded
input/output, 1280×720, up to 3000 frames, 512 MiB decoded samples, 2 GiB memory
admission, 30 s per file, and bounded aggregate input/publication up to 1 GiB.
Existing Linux provider guard and serial bounds remain required. V2 publication
reserves its own 8 MiB evidence/marker overhead. V1 schema/byte ceilings and PNG
limits are unchanged; unknown schemas and mixed artifact profiles refuse.

Inventory/start/report/recovery use explicit v2 identities. Resume requires the
exact original request and tool baseline and refuses uncertain attempts. This is
source-preserving candidate work; strict encoded reduction does not prove
physical reclaim, native usefulness, arbitrary codec/HDR/audio support or
replacement eligibility. Four new boundary/CLI cases are authored and unrun.

## Read-only replacement handoff

`replacement review-video --source ABS --candidate-set ABS --artifact-schema
optiflow.video-candidate-artifact-set.v2 --video-policy PIN --retention ABS
--output ABS` creates a separately versioned staged review. `--video-policy`
means the candidate request's exact video policy pin, including its sandbox and
limits, rather than the unrelated global scan policy. The host rebinds source,
marker/evidence, candidate, current closed container structure and complete
stream-copy packet payloads. It records existing private state/retention and
strict encoded reduction. No native provider is called by review.

Historical saved decode receipts do not become a live decoder observation.
Flags explicitly deny live decode, preserved native properties, replacement
qualification, apply and disposal authority. `admit_to_transaction` refuses
before writable state. The existing PNG transaction parser rejects this type;
video evidence never bypasses PNG limits or reuses its approval. A future live
validator/property adapter and native qualification must explicitly complete the
shared transaction seam. Four further refusal cases are authored and unrun.

## Current native validation source checkpoint (#146)

`replacement validate-video --request ABS --output ABS` explicitly selects a
review plus its exact guarded production profile. It invokes current complete
source/candidate probe and decode through the existing Linux guard, verifies
full sample-byte equality and exact reported frame timing, and rechecks current
subjects, original resource policy and selected executable/build identities.
Each invocation must have an observed guard receipt. No optimization or writable
transaction-state access occurs in validation.

A non-deserializable live token retains all subject/tool bindings and the same
cooperative deadline. Saved validation output cannot become a live token or
mutation permission. Provider subtree quotas and conservative host payload
admission retain existing limits; total host RSS and blocked syscalls are not
preempted. Native qualification and macOS property backend gates remain open.
Five cases are authored, uncompiled/unrun; no native command or check ran here.
