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
