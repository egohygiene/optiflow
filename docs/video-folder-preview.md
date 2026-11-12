# Preview video folders before preparing tools

The existing v1 and broader v2 Matroska/FFV1 candidate profiles now have a
tool-free entry point. `video-folder profiles` describes their actual internal
geometry/frame/byte limits. `video-folder preview --request ABS` uses the same
bounded discovery walker and closed container classifier as production, without
FFmpeg, sandbox setup, private work/store/journal directories or source writes.
Both early commands bypass normal mutable state setup.

The closed `optiflow.video-folder-preview-request.v1` selects `source_root`,
`profile` (`tiny_v1` or `bounded_v2`), `recursive`, per-file `input_bytes` and
the existing `batch` limits. The selected profile is explicit: v1 never silently
upgrades to v2. Existing cross-field and profile-specific batch bounds apply.
The source root must be canonical and this implementation remains Linux-only.

Preview lists exact content/identity observations, eligible/unsupported/skipped
entries and refusal reasons. It never classifies by extension alone, follows
links, crosses an observed device boundary or starts candidate production.
Eligibility is closed-container preflight, not full decode or quality acceptance.
Same-device bind mounts and total process RSS remain unqualified; cancellation
and deadline checks are cooperative and cannot preempt a blocked syscall.

`maximum_actions` and `maximum_publication_bytes` describe the selected later
serial budget; preview consumes neither. No output/savings/physical-reclaim
estimate is fabricated. Use a separately prepared real v1/v2 production request
with exact tools, Linux guard and independent writable namespaces for a later run.
The preview inventory has a separate schema and request fingerprint. It cannot
be adopted as a production journal or resumed; native run/resume fingerprints
and historical v1/v2 inventories retain their original meanings.

This exposes the existing broader v2 subset rather than adding another codec.
Audio tracks, MP4/HDR/lossy operations, native/resource qualification and broader
video profiles remain unfinished #146 scope. Source replacement/disposal gates
are unchanged. Two meaningful profile-boundary/tool-free filesystem cases are
authored, uncompiled/unrun. No native/media/SDK/CLI/task, tests/builds/validators
or CI were run; no actual approval, replacement/disposal, release or publication
occurred. Proposed ADR-0002 and final Flow #88 retain their prior status.
