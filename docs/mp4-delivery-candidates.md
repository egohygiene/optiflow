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

Four clock/container/budget/argv cases are authored, uncompiled/unrun. All builds,
tests/dependencies/lint/format/schema/docs/drift/native/provider/media/CLI/SDK/CI
execution remains deferred. No actual private media, review, native/macOS
qualification, signing, release or posting occurred. Flow #88 remains last and
untouched; known alignment cancellation/start-marker failure unresolved, cause
unproven. GitHub Pages/live demonstration will be discussed after this sprint.
