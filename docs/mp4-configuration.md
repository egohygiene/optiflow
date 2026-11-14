# Explicit MP4 settings

Relates to #146/#135. Source checkpoint only; no execution or qualification.

The existing configuration resolver now accepts optional MP4 settings. Its
normal source selection is unchanged: user configuration followed by project
`optiflow.toml`, or one explicitly selected `--config` / `OPTIFLOW_CONFIG` file
instead of those layers. `--no-config` disables configuration-file selection.
Existing presentation environment/CLI precedence remains unchanged. No new
MP4 environment variables, configuration search, state store or resolver exist.

The order for processing settings is configuration → explicit request values →
the JSON document explicitly selected by `--settings`. Control is an atomic
choice; individual limits merge by name. There are **no compiled lossy/control
or resource defaults**. Missing control or any of the six limits refuses.

```toml
schema = "optiflow.config.v1"

[mp4_delivery.control]
kind = "target_size"
maximum_bytes = 100000000
mux_reserve_bytes = 4000000
preset = "slow"

[mp4_delivery.limits]
input_bytes = 268435456
candidate_bytes = 104857600
elapsed_ms = 3600000
memory_admission_bytes = 4294967296
working_bytes = 2147483648
minimum_free_bytes = 134217728
```

These are explicit example choices, not observed capacity or recommended quality
for every video. The complete request still selects real exact pinned tools,
its Linux guard/resource policy, source and disjoint private output roots.
Settings cannot change those paths/pins, audio copy, metadata preservation,
source mutation, validation, upload, replacement or disposal authority.

A complete `optiflow.mp4-delivery-request.v1` retains all explicit settings and
therefore shadows file settings. The new
[`optiflow.mp4-delivery-selection.v1`](../schemas/mp4-delivery-selection-v1.schema.json)
uses the same required source/tool/guard/output/preservation fields but allows
omitted control and partial or omitted limits. Use it to inherit file settings.
The selected CLI patch uses
[`optiflow.mp4-settings.v1`](../schemas/mp4-settings-v1.schema.json), for example:

```json
{"schema":"optiflow.mp4-settings.v1","settings":{"limits":{"elapsed_ms":1800000}}}
```

Authored command examples, not executed:

```sh
optiflow --config /private/optiflow.toml --json mp4-delivery resolve --request /private/selection.json
optiflow --config /private/optiflow.toml --json mp4-delivery plan --request /private/selection.json --settings /private/override.json
```

`resolve` reads only configuration/request/settings records. It does not open
media/tool paths, inspect storage, invoke providers or create mutable state.
The result exposes effective control/limits, provenance, shadowed settings and
the digest of the complete producer request. Plans and production bind that
same effective request through existing plan/evidence digests. Resolution
provenance is a separate local view, not a new proof or a change to the legacy
scan evidence-policy identity. Retain the resolution output separately if its
source-selection history is needed.

For folders, a distinct
[`optiflow.mp4-folder-selection.v1`](../schemas/mp4-folder-selection-v1.schema.json)
allows a complete request or selection under `template`. Other folder fields
stay explicit, including the root placeholder and cleanup/batch policies.
`mp4-delivery folder-resolve --request FILE [--settings FILE]` reveals the final
folder request without traversal. Preview/run/resume use the identical resolved
request. A changed effective request invalidates retained continuation rather
than silently rewriting its frozen policy.

Pure profiles, independent source inspection, artifact status and folder status
continue without configuration/state setup. Existing plan/run result shapes
remain unchanged. Two meaningful resolution/identity cases are authored,
uncompiled/unrun. No product/CLI/SDK/native/provider/media/model, compiler/build,
tests/dependencies/lint/format/schema/docs/drift or CI execution occurred.
