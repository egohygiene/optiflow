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
