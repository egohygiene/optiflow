# Read-only MP4 preflight

Relates to #146/#135. Authored interfaces; no execution/qualification recorded.

`mp4-delivery preflight --request FILE [--settings FILE]` resolves settings through
the existing configuration owner, then observes explicit local prerequisites.
The Task alias is `mp4:preflight REQUEST=FILE`; `OPTIFLOW_CONFIG` keeps its existing
selection meaning. No native tool, version command, process, service, installer,
socket connection or mutable workspace is started by preflight.

The versioned report has twelve named checks: host family, supported source,
five exact executable pins, private work/artifact roots, explicit user-manager
socket, plan admission and the unverified actual runtime guard. Pin failures
identify their selected tool and explain exact-byte/regular-file/mode needs;
root failures describe private no-follow directories; storage checks show the
same required allowance as production and available bytes when observable.
Source-specific admission reuses the planner's copied-audio/mux, scratch,
candidate, host-payload and provider-quota arithmetic.

Missing prerequisites produce `prerequisites_unavailable`. When the read-only
observations pass, the state is `prerequisites_observed_guard_unverified`.
Producing a preflight report is not an encoder success. Available bytes are
point observations, not reservations; a socket is not a working/delegated user
manager. Provider version/build/codec behavior, actual cgroup/tmpfs confinement,
full decode, quality review and supported-host qualification remain separate.
Every report keeps provider invocation, service connection, space reservation,
guard/native qualification, mutation, replacement and upload flags false.

Observation uses a cooperative window no longer than the selected elapsed limit
or 30 seconds. Cancellation/deadline interrupts between bounded operations;
individual host syscalls cannot be preempted by this contract. A bound plan is
included only after source/tool observations and selected admission succeed.
The ordinary producer reobserves its source/tools/storage before and during
actual execution. Preflight does not grant reuse of earlier observations.

One missing-prerequisite/no-write/no-native-success case is authored,
uncompiled/unrun. No tests/builds/compiler/dependencies/lint/format/schema/docs/
drift, product/CLI/SDK/native/provider/media/model or CI execution occurred.
