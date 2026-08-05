# JSON Contract

## Stability

Every document contains a `schema_version`. Consumers must branch on that value
rather than the installed binary version.

Within a schema major version:

- Existing field meaning will not change.
- Enum values will not be silently reinterpreted.
- New fields may be added.
- Consumers should ignore unknown fields.
- Paths are serialized as lossy UTF-8 strings where the host path cannot be
  represented directly in JSON.

## Standard output

Pass global `--json` before or after the subcommand. On success, standard output
contains one complete JSON document and a trailing newline. Do not parse human
output.

Example:

```bash
optiflow \
  --json \
  --state-directory "/tmp/optiflow-state" \
  scan "/data/media" > "report.json"
```

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

Aniflow and RenderFlow should retain both the report and plan as provenance
artifacts when invoking OptiFlow.

