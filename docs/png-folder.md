# PNG-first serial low-space folder source

Issue #121 builds on #95 replacement and #93 native qualification. Source
checkpoints do not qualify original replacement/disposal or close those issues.

An explicit `png-folder-request.v1` selects source/provider and disjoint existing
private state, retained-original, artifact and run directories. Original
retention shares the source filesystem. Limits bound entries, files, depth,
actions, aggregate observed input, per-file encoded bytes, journal and elapsed
time. Recursion and hidden-entry handling are explicit. Discovery starts no
provider and creates no collection/staging files. It uses no-follow bindings,
skips symlinks/special files/device boundaries/hard links, and independently
validates the existing static PNG subset. Discovery is bounded, not a claim to
inventory the complete tree or attest same-device bind mounts.

Batch approval binds the exact request, inventory, maximum actions and one of
`retain_for_restore` or `discard_after_durable_replacement`. A PNG candidate or
restorable single-file approval cannot imply batch discard authority. Changing
policy, disposition or inventory requires another explicit approval.

One-file scheduling and recovery will join the same replacement event family.
The original must remain until replacement and recovery evidence are durable;
explicitly approved disposal and owned temporary cleanup must finish before the
next item. No zero-extra-bytes or physical-reclamation guarantee is made.

Five authority/coverage refusal cases are authored and unrun. Testing, compiler,
formatting, schema validation, CI and all native/provider/media execution remain
deferred. No originals have been processed during this sprint.

## Serial coordinator and recovery source

`optimize discover --request ABS --output ABS`, `optimize approve --request ABS
--inventory ABS --approved-by NAME --output ABS`, and `optimize status --request
ABS --inventory ABS --approval ABS` use immutable bounded private records.
`optimize run`/`resume` refuse before provider or writable mutation state while
native qualification is pending. No implicit discard or bypass flag exists.

The serial coordinator source freezes request/inventory/approval/tool bytes,
records each start before provider execution, and finishes replacement, explicit
disposal and owned candidate cleanup before recording a terminal item. Resume
checks exact saved basis/tool bytes and only continues never-started entries.
A missing terminal result exposes an uncertain start and refuses replay. Original
discard has a separate batch authorization pin in replacement events. It requires
a current retained original and committed replacement, then journals discard
intent/result and empty namespace cleanup. Cleanup enumerates exact owned
candidate members; it never recursively removes directories or unknown work.

Per-file reservation accounts for candidate/stage/journal bytes plus a reserve;
provider RSS/work usage and filesystem physical reclamation are not proven by
this admission calculation. An interrupted cleanup/disposal remains inspectable
and blocks advance. Four additional refusal/CLI cases are authored and unrun.
