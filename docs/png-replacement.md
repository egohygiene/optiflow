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

## Transaction and CLI source

`replacement review --source ABS --candidate-set ABS --retention ABS --output ABS`
creates a new immutable review document in an existing private output directory.
`replacement approve --plan ABS --approved-by NAME --output ABS` records exact
restorable authority. `replacement status --plan ABS` only reads recorded phases.
Human/JSON results use the existing command-result envelope.

`replacement apply` and `replacement restore` explicitly refuse before writable
journal access while #93/#95 qualification is pending. No JSON or bypass flag
can enable them. macOS replacement property preservation remains unavailable.

The Linux transaction source reuses the existing execution lock and SQLite
journal (additive migration 10). It stages a separately copied candidate, checks
properties, synchronizes it and journals each intent/result. Original retention
and publication use separate no-replace renames on the source volume. An
interruption may leave the source path temporarily absent; the retained original
and phase evidence must be inspected. Uncertain phases are never replayed.
Restore source retains the optimized object, refuses collisions and deletes
nothing. Original disposal is separately owned by #121 explicit batch authority.

This source is uncompiled/unrun and cannot establish native durability or crash
recovery qualification. Four further transition/gate/CLI cases are authored.
