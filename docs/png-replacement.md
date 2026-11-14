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

Manual source review follow-up: preparation inspects the committed candidate
again after independent source/pair validation and requires the same complete
manifest. The staged video handoff has the corresponding final commitment check.

Read-only replacement status now treats an unused state directory or a legacy
pre-migration-10 journal as having no replacement events. It creates no database
or migration and refuses newer journal versions and hard-link aliases. One
additional read-only refusal/side-effect boundary case is authored and unrun.

## PNG command wiring — November 10, 2026 source checkpoint

`replacement apply --plan ABS --approval ABS` and `replacement restore --plan
ABS --approval ABS` now have distinct typed delivery to the existing transaction
APIs, matching the video command pattern. The former shared placeholder is
removed. The closed bounded saved approval parser checks exact reviewed PNG
plan, authorization fingerprint and restorable-PNG-only scope. Video, disposal,
unknown fields, duplicate declarations and changed plans cannot substitute.

The unchanged application qualification gate still refuses before plan/approval
file reads, source observations or writable journal access. The transaction SDK
retains its independent gate before native state work. No flag/document can
qualify or bypass either boundary. Consequently this wiring does not enable
real replacements, qualify PNG properties/rollback, broaden the16MiB profile or
publish v0.3.0. Originals remain restorable under the selected future qualified
transaction semantics; irreversible disposal remains a separately owned action.

Two useful saved-declaration/scope/gate regressions are authored, uncompiled and
unrun. No test/compiler/build, dependency/lint/format/schema/native/media/CLI/SDK
or hosted CI operation ran. Feature #95 and #93's native/platform/release gates
remain open; Flow #88 stays last. No actual user-media mutation or approval.

## Held subject integrity — November 13, 2026 source checkpoint

The shared private Linux engine now reuses the execution hasher on the held
original and staged descriptors before retaining the original, before publishing
the replacement and before recording completion. Both restore moves have the
same checks, including the optimized object retained after the first move.
Each observation is bounded by the existing file ceiling and cancellation state.
The full byte pin, owner/mode/modification time and bounded extended attributes
must still agree; atime may advance during reads, and rename's new ctime belongs
to a freshly observed binding of the same object. A current path's bytes cannot
substitute for the held descriptor's bytes.

Current source/retention directory bindings must also match the held directory
handles. Restore requires the journal's original and published objects to match
the approved original and staged identities and the exact selected namespace.
The no-replace renames remain unchanged. Failure after an intent leaves that
intent pending and the objects available for inspection; the engine adds no
retry, rollback, unlink or orphan-adoption path. These observations do not provide
an atomic snapshot or isolation from concurrent writers.

Six private-engine regressions are authored, uncompiled and unrun: successful
retention/restore across rename ctimes, staged corruption before original
retention, changed held bytes despite a current metadata pin, late xattrs,
substituted directory handles and restore collisions. The generated raw byte
fixtures isolate transaction behavior; they are not PNG or native qualification.
No tests, compiler/build, dependency, lint/format/schema/docs/drift, product
CLI/SDK, native/provider/media, drive operation or CI execution/polling ran.

The application and SDK qualification gates still refuse before writable state.
This checkpoint leaves #95 open: #93's actual native release, qualified PNG
replacement/recovery/property trials and v0.3.0 install/upgrade/rollback evidence
remain required. macOS replacement dispatch is still unavailable. The current
package is still v0.1.1; no original replacement/disposal or release occurred.
