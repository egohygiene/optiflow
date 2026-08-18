# JSON Contract

## Stability

Every machine document declares its schema. Command results use `schema`; domain
artifacts use `schema_version`. Consumers must branch on that identifier rather
than the installed binary version.

The current artifact identifiers are:

| Artifact | Identifier | Schema |
| --- | --- | --- |
| Command result | `optiflow.command-result.v1` | `schemas/command-result.schema.json` |
| Run | `optiflow.run.v3` | `schemas/run.schema.json` |
| Report | `optiflow.report.v3` | `schemas/report.schema.json` |
| Plan | `optiflow.plan.v3` | `schemas/plan.schema.json` |

Within an artifact schema major version:

- Existing field meaning will not change.
- Enum values will not be silently reinterpreted.
- New fields may be added.
- Consumers should ignore unknown fields.
- Native paths in command diagnostic context and artifact references use a
  lossless tagged UTF-8 or Unix-byte representation. Historical artifact path
  fields retain their existing compatibility representation.

## Standard output

Pass global `--output-format json` or its compatible `--json` alias before or
after the subcommand. When structured rendering is possible, standard output
contains exactly one complete `optiflow.command-result.v1` document and a
trailing newline for success, partial success, and blocking failures. Do not
parse human output.

Example:

```bash
optiflow \
  --output-format json \
  --state-directory "/tmp/optiflow-state" \
  scan "/data/media" > "optiflow-result.json"
```

The process status exactly matches `outcome.exit_code`. JSON stdout never
contains banners, progress text, warnings, color, or logs. Diagnostics remain
inside the envelope; JSON-mode stderr stays empty during ordinary rendering.
The renderer buffers the full document before writing it.

Before this contract, `--json scan`, `report`, and `plan` wrote the domain value
directly. They now wrap that value in `result`, with committed outputs listed in
`artifacts`. This is a pre-1.0 machine-output compatibility change. Immutable
`v1`, `v2`, and `v3` artifact schemas were not changed.

See [CLI Outcome Contract](cli-contract.md) for the exit-code matrix, typed
diagnostics, partial-run semantics, stream ownership, and signal behavior.

## Reports

Reports embed:

- The immutable run manifest
- Aggregate counts and reclaimable bytes
- Exact duplicate groups and evidence
- Complete observations, warnings, cache facts, and optional media descriptors

## Plans

Plans contain no executable mutation in `v0.1.0`. Each action includes:

- Exact relationship evidence
- A deterministic review default
- Candidate paths
- Potential reclaimable bytes
- Size, time, and hash preconditions
- Required future apply-time re-hashing and byte confirmation

`flow` should retain both the report and plan as provenance artifacts when it
invokes `optiflow`. Other consumers should do the same when reproducibility
matters.
