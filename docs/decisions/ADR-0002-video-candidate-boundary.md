# ADR-0002 — Bound development video candidates without collection mutation

- **Status:** Proposed; source implementation/merge does not ratify or qualify it.
- **Date:** 2026-11-06
- **Issue:** #146

The first video producer remains a typed FFmpeg/ffprobe adapter with a closed
Matroska/FFV1 stream-copy subset. It adds no codec engine, generic process/job
framework or cross-suite orchestrator. Flow owns before/after placement through
released contracts. The historical read-only review/observation APIs stay distinct.

Selected Linux user-manager services and a safe internal exec guard provide
observed cgroup provider-subtree memory limits and aggregate private tmpfs
scratch. Other writable mounts, unavailable/degraded limits and unsupported
hosts refuse. Total Rust host RSS, general escaped-process containment and hard
preemption of host syscalls/parsing are not claimed; these scopes remain explicit.
Resource numbers, exact launch/provider bytes and actual guard receipts bind
candidate evidence. No privileged manager or installation is implicit.

Host-owned independent EBML/payload/timestamp observation and complete source/
candidate decode comparison precede strictly-smaller acceptance. Descriptive or
unknown features refuse. Lossy execution and quality approval remain separate.
Source bytes are read only and re-observed at precommit.

Video uses its own versioned candidate protocol over the existing descriptor,
private staging, no-follow, fsync and no-replace publication primitives. It does
not expand PNG's closed member vocabulary or add replacement/disposal authority.
Partial staging and failed/unconfirmed cleanup stay failures without adoption.

All source/compiler/tests/lint/schema/native/provider/CI qualification remains
deferred by maintainer; no native-supported-host acceptance or release is recorded.

November 7 source extension: bounded serial folder discovery and per-entry
start/result evidence compose this same producer. A single candidate runs at
a time; failed/unresolved states stop and read-only recovery grants no replay.
See [folder scope](../video-folder-candidates.md). Proposed status, native/host
qualification and the separate future mutation/disposal boundary are unchanged.

November10 proposed source extension: an explicitly selected v2 profile raises
geometry/frame/encoded/decoded and artifact bounds within the same FFV1 stream
copy lane. One internal profile owns admission, source/provider capture,
independent parser/decode and publication/readback limits. V1 wire/fingerprint
meaning and folder defaults remain unchanged; there is no failed-v1 fallback.
The same existing Linux guard owns provider-subtree/scratch evidence. Larger
payload estimates are not full host RSS or native qualification. Distinct v2
request/evidence/marker identities prevent cross-profile relabeling; source
replacement/disposal and broader media remain separate. See [v2 scope](../video-candidate-profile-v2.md). Proposed status is unchanged.
