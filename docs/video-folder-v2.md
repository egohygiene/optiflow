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
