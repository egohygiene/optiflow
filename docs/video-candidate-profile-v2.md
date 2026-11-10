# Explicit broader lossless video candidate profile (#146)

This source checkpoint selects `optiflow.video-production-request.v2` through the same typed `VideoProductionRequest` and `video_production::produce` library/explicit development example. The schema identifier selects the complete fixed v2 profile; it is never inferred from file size, installed tools or a failed v1 request. Source is uncompiled/unrun and absent from released v0.1.1. No execution or installation occurred.

| Bound | Existing v1 | Explicit v2 |
| --- | --- | --- |
| Geometry, even square pixels | up to640×360 | up to1280×720 |
| Complete frames / clusters |250 |3000 |
| Source/candidate encoded bytes each |16MiB |128MiB |
| Complete decoded YUV420 bytes per subject |64MiB |512MiB |
| Selected temporary admission maximum |32MiB |512MiB |
| Selected host-payload plus provider memory admission maximum |256MiB |2GiB |
| Candidate evidence / marker bytes |512KiB /16KiB |8MiB /16KiB |
| Shared cooperative operation deadline |at most30seconds |at most30seconds |

All bounds intersect; they are not simultaneously achievable media dimensions/duration. At1280×720,512MiB permits at most388 decoded frames (15.52seconds). At very small geometry,3000 frames represent120seconds at the unchanged25fps clock. A file within those limits can still refuse on selected memory, temporary storage, provider quota, free reserve or deadline. This is not a general full-length AMV optimizer or a promise that remuxing saves space.

The lane remains one progressive FFV1 Matroska video track, yuv420p8-bit, explicit limited-range BT.709, exact25fps/40ms intervals,1/1000 time base and zero origin. The same strict EBML allowlist, independently observed packet payloads/PTS/keyframe state, codec-private bytes, color/aspect/track contract, complete ffprobe frame inventory and full source/candidate decoded-sample equality remain required. Audio, other codecs, HDR, rotation, non-square pixels, variable frame rate, lacing, extra streams, unknown/CRC/descriptive features and lossy execution still refuse. The old v1 lossy review-only planner is unchanged; v2 is stream-copy-only even for preparation.

One internal closed profile selects limits throughout pure request admission, no-follow source capture, guarded provider input/output pipes, probe parsing, independent container parsing, full decode and durable artifact publication/readback. V1 public observation/parsing/retained-read APIs stay tiny. V2 has explicit `video_probe::parse_ffv1_observation_v2` and `video_container::inspect_v2` pure structural adapters; neither authenticates provider reports or establishes native decode. Normal SDK configuration resolution, tool identities and cleared environments are unchanged.

V2 emits distinct `video-candidate-request.v2`, `matroska-ffv1-sdr-candidate.v2`, request/preparation fingerprint domains, `video-candidate-evidence.v2` and `video-candidate-artifact-set.v2` marker identities. Marker/evidence/request/container profile joins must match before readback. New evidence rejoins declared geometry/frame count/decoded-byte count with the complete packet inventory and selected memory/temporary admission. It cannot relabel a larger set as v1 or adopt partial staging. Marker member names remain `candidate.mkv` and `evidence.json`; source bytes are never replaced. Old v1 serialized fields, schemas, domain prefixes, limits and folder defaults retain their meanings. No database migration or automatic old-root upgrade occurs.

The existing explicitly pinned Linux user-systemd/cgroup/tmpfs guard is reused with unchanged policy and receipt semantics. It already admits finite provider-subtree memory/scratch values; v2 changes payload admission, not the isolation authority. Actual receipt checks remain mandatory, with no unguarded fallback. The v2 memory calculation includes source, four candidate-buffer allowances, two complete decoded subjects,32MiB metadata allowance and the selected provider quota. These are conservative payload estimates, not full Rust host RSS enforcement, allocator/DOM overhead quotas, hardware qualification or atomic free-space reservation. The shared30second deadline and separate at-most2second cleanup per invocation remain explicit; blocking host syscalls/parsing are not preempted. Unsupported/degraded hosts refuse.

The [v2 example](../examples/video-production-request-v2.json) contains deliberately nonexistent paths/pins. Operator-managed explicit setup must provide actual pinned FFmpeg/ffprobe8.0.1 and guard launchers, disjoint private working/publication roots, user bus, budgets and reserves. Use the existing development example with an explicit request after execution is authorized; no supported product CLI/release or private-media trial is claimed. The recursive folder v1 protocol still selects tiny v1; extending folder selection is remaining #146 work.

Only independently validated strictly smaller candidates are published, with current source/provider checks immediately before durable no-replace publication. Failed, cancelled, changed, unsupported or non-shrinking work preserves originals. Output evidence reports encoded-byte reduction, `physical_space_reclaimed:false` and `replacement_authority:false`. There is no source disposal, approved replacement, lossy quality target, macOS/APFS production support or physical savings claim. #95/#121 remain PNG-first and #93/#111 native mutation/release gates stay separate.

Eleven synthetic source/schema/parser/admission/IO cases are authored/uncompiled/unrun. Their fake EBML payloads are deliberately not native-qualified FFV1 media. No actual guard/provider/decoder, CLI/SDK or filesystem mutation workflow ran. Native larger-profile resource/cancellation/recovery/decoder/host qualification, useful savings, broader codecs/media and mutation remain open under #146 and flow #88 last. ADR-0002 remains proposed.
