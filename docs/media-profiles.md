---
title: Media-profile evidence
description: The versioned, read-only lossless PNG review contract and its evidence limits.
---

# Media-profile evidence

optiflow's first media-aware optimization analysis is deliberately narrower
than optimization execution. A scan can identify a current PNG as a candidate
for later lossless-recompression evaluation. It does not run an optimizer,
create a candidate output, modify the source, or estimate savings.

## Contract and profile identity

The evidence document uses `optiflow.media-profile-evidence.v1`, defined by
[`schemas/media-profile-evidence-v1.schema.json`](../schemas/media-profile-evidence-v1.schema.json).
New scans embed one such document in `optiflow.report.v6`. The first and only
profile in this contract is:

```text
optiflow.builtin.lossless-png-review@1.0.0
```

The complete accepted example is
[`examples/media-profile-evidence-v1.json`](https://github.com/egohygiene/optiflow/blob/main/examples/media-profile-evidence-v1.json).
The contract is intentionally closed around this one profile. Broader image,
animation, audio, and video profiles require separate versions or contracts.

## Selection and evidence

A filename suffix never selects the profile. The source observation must have
been classified from bytes as `image/png` and `image`. An opportunity also
requires all of the following:

- a stable, current, analyzed observation from the existing read-only handle
  window;
- media probing enabled by the effective evidence policy;
- one discovered `ffprobe` executable bound to its canonical native path,
  exact version line, BLAKE3-256 binary digest, and invocation fingerprint;
- a bounded, zero-exit invocation with empty stderr and valid JSON; and
- semantically valid PNG evidence: `png_pipe`, exactly one PNG video stream,
  and positive width and height with no audio observations.

The exact provider binary is checked before and after every inspection. Source
bytes are supplied through a clone of the already opened read-only file handle,
not by reopening the mutable pathname. A successful process exit alone is not
accepted as evidence.

The recorded executable digest covers the selected file's bytes; it does not
attest dynamically linked libraries, kernel behavior, or operating-system
containment. optiflow makes no filesystem- or network-sandbox claim for the
optional process.

## Coverage and limitations

Coverage applies only to byte-classified PNG inputs:

| Status | Meaning |
| --- | --- |
| `not_applicable` | The scan contained no byte-classified PNG input. |
| `not_requested` | PNG input existed, but media probing was explicitly disabled. |
| `complete` | Every in-scope PNG had current, semantically valid provider evidence. |
| `partial` | Some in-scope PNGs had complete evidence and some did not. |
| `unavailable` | PNGs were in scope, but none had enough evidence for an opportunity. |

Limited entries name one or more closed reason codes:
`media_probe_disabled`, `provider_unavailable`,
`provider_result_unavailable`, `source_evidence_not_current`, or
`provider_evidence_invalid`. Partial and unavailable requested coverage also
degrades the command-result coverage and exits with the existing partial-success
code `3`. Probe-disabled analysis remains an explicit `not_requested` result.

Stale observations are excluded. Missing providers, timeouts, non-zero exits,
stderr on a nominally successful run, malformed JSON, invalid stream facts,
and provider-binary replacement produce no opportunity. No fallback fabricates
or carries forward media facts.

## Opportunity meaning

An `opportunity` is a deterministic review candidate for a future
`evaluate_lossless_png_recompression` operation. It is not a plan action,
output artifact, execution request, or proof that the file can be made smaller.
Both `estimated_output_bytes` and `estimated_logical_savings_bytes` are always
`null`; `savings_claim` is always `not_estimated`.

The opportunity records validations that a separate future output-producing
checkpoint would have to satisfy, including complete decode, dimensions,
frames, alpha, color and metadata policy, decoded-pixel equivalence, source
revalidation, and a verified smaller output. Listing those requirements grants
no authority to create or commit that output.

Stable analysis, entry, and opportunity identifiers derive from the profile
configuration, effective evidence-policy fingerprint, native path, accepted
source facts, normalized media facts, and provider evidence. Run and
observation UUIDs remain provenance fields but do not perturb those derived
identifiers, so unchanged reruns are reviewably correlated.

## Compatibility and ownership

The report schema advances from v5 to v6 because it gains required profile
evidence. Run, plan, command-result, effective-policy, artifact-set, extension,
and SQLite contracts are unchanged. Historical report v1-v4 documents retain
their markerless compatibility. Report v5 and v6 documents require their
matching complete artifact-set marker.

This profile is built into optiflow and consumes the existing core `ffprobe`
adapter. It does not widen extension SDK authority. optiflow remains a
standalone file-inspection and optimization-analysis tool; flow may orchestrate
released contracts, renderflow produces artifacts, and aniflow owns temporal
media processing. No sibling source is imported.

## Separate follow-up checkpoints

The contract-only portion of candidate preparation is defined in
[PNG candidate contract](png-candidate-contract.md). It checks synthetic or
caller-supplied evidence consistency and does not execute an optimizer, decode
PNG bytes, or extend this profile's authority.

The following remain outside this profile:

- optimizer selection or invocation;
- candidate-output production and validation;
- measured or projected savings;
- source replacement, apply, quarantine, rollback, or recovery;
- animated PNG, metadata, color-profile, alpha-policy, or broad image coverage;
- remote or dynamically discovered providers; and
- cross-tool orchestration or temporal-media pipelines.
