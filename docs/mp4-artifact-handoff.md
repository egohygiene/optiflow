# Typed MP4 artifact handoff

Relates to Optiflow #146/#135 and Flow #70. Authored interfaces; no native run,
release, installed route or qualification is recorded.

`mp4-delivery capabilities` is pure compiled metadata: supported input/output
container, codecs, pixel/color/audio/clock scope, geometry/frame/byte limits,
preservation, explicit lossy review and unchanged selection requirements. The
compiled release descriptor includes the same metadata and schemas. Its release
route and native qualification flags remain false. Flow retains ownership of
released-provider compatibility, pipeline placement, approvals and recovery;
Optiflow imports no sibling implementation and adds no orchestration engine.

An `optiflow.mp4-input-reference.v1` names exactly one selected path, complete
Blake3 bytes/size and origin. `input-reference --source PATH
--maximum-input-bytes BYTES` creates a selected-file reference using the same
independent bounded observer, without native tools. SDK callers may instead
supply an external-artifact origin with a bounded tool/contract/artifact ID and
manifest digest. Origin metadata is **caller-declared**: Optiflow does not open
guessed producer paths, verify external manifests/releases/signatures, infer
ownership or claim upstream provenance merely from those labels. Actual selected
file content must match the reference during preparation and execution.

The explicit consumer JSON contains:

```json
{"profile":"optiflow.mp4-h264-aac-delivery.v1","maximum_bytes":100000000,"accept_lossy_video":true}
```

Preparation requires the selected producer cap to fit that consumer cap and
explicit acceptance of the lossy profile. It binds the complete existing plan,
input reference, consumer and `refuse` or `keep_original` unchanged policy into
`optiflow.mp4-handoff-preparation.v1`. Source/tool observation and rate/resource
planning occur, but no encoder runs. No implicit consumer limit, lossy approval,
original fallback or broad format compatibility is selected.

Authored command examples, not executed:

```sh
optiflow --json mp4-delivery prepare-handoff --request /private/selection.json --input-reference /private/input-reference.json --consumer /private/consumer.json --unchanged-policy refuse
optiflow --json mp4-delivery run-handoff --request /private/prepared.json
```

Save the **inner `.result` preparation** from the command-result envelope as
`prepared.json`. Preparation honors the existing configuration and explicit
settings layers. `run-handoff` consumes that frozen record, refuses `--settings`
and explicit `--config`, and does not reread deployment configuration. It may explicitly select the same
private bounded progress log as ordinary run. Current source snapshot/digest,
tools and complete plan must match before the same producer can skip or encode.
Invocation uncertainty grants no retry/replay authority.

For acceptance, the pinned candidate must join the exact prepared plan.
`candidate_ready_for_human_review` selects the fixed member of that validated
artifact protocol with its exact bytes/digest. Human quality approval remains
unrecorded. An unchanged result with `refuse` selects no output. `keep_original`
can select the original only after a current exact input observation within the
consumer cap; otherwise the outcome is retained with no selected output. This
never replaces, disposes, uploads or automatically approves a source/candidate.
An unchanged original is not a new native decode or transformed-media proof.

The run result contains the versioned handoff and `handoff_digest`. Retain that
digest outside the handoff and save the inner `.result.handoff` JSON separately.
Readback uses:

```sh
optiflow --json mp4-delivery handoff-status --handoff /private/handoff.json --handoff-blake3 LOWERCASE_DIGEST_FROM_RESULT
```

This is the returned **semantic handoff digest**, not a hash of whitespace in
the saved JSON file. Readback checks that external pin, preparation/content/
policy joins and selected output. It reobserves a current pinned candidate or
the explicitly selected original; external origin/release, current native decode
and human review remain unverified. A changed selected output refuses. It never
searches for substitutes or replays the producer.

The prepared record reserves report headroom under the bounded 16 MiB metadata
ceiling. Typed side records do not widen the existing producer request, evidence,
artifact or legacy status protocols. Two consumer/input-reference cases are
authored, uncompiled/unrun. All compiler/build/tests/dependencies/lint/format/
schema/docs/drift, product/CLI/SDK/native/provider/media/model and CI execution
remains deferred. No Pages site, animation or real-media demo was created.
