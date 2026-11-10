# Restorable PNG replacement source checkpoints

Issue #95 depends on #93 qualification; this source does not satisfy that gate.

`execution::replacement::review_png` binds an explicit current source, committed
candidate set, evidence policy and existing private state/retention directories.
It independently decodes and compares the current PNG pair with the existing
validator, rechecks both bindings and marker/evidence bytes, and refuses stale
provenance, another source or a non-smaller candidate. Source/provider mutation
is not performed during review. Original retention must be on the source volume.

A distinct v1 replacement plan/approval cannot be accepted by byte-identical
quarantine. Approval grants restorable PNG replacement only, never disposal.
`recheck_png` repeats the current review and compares the exact approved body.
Saved self-consistency is not current eligibility or a live validation token.
Encoded reduction does not establish physical space reclamation; two retained
copies initially use more space. Native runtime and #93/#95 proof remain deferred.

Seven refusal/authority boundary cases are authored, uncompiled and unrun. No
compiler, formatter, schema validation, provider/media execution or CI ran.
