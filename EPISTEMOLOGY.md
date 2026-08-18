---
schema: aether.architecture-document/v1
id: optiflow-epistemology
title: OptiFlow Epistemology
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-08-18
governed_by:
  - architecture-epistemology
depends_on:
  - optiflow-purpose
  - optiflow-principles
related:
  - optiflow-ai-constitution
  - optiflow-ontology
  - optiflow-decisions
supersedes: []
---

# OptiFlow Epistemology

## Scope

This document defines how OptiFlow distinguishes observations, evidence,
claims, uncertainty, and decisions. It governs what the system may say it knows;
the wire shape of a particular artifact remains owned by its JSON Schema.

## Claim States

| State | Meaning |
| --- | --- |
| Observed | Captured directly from a named source during a bounded attempt. |
| Derived | Deterministically calculated from identified observations. |
| Inferred | Supported by a detector or heuristic but not logically proved. |
| Proposed | Recommended for review under a declared policy. |
| Unknown | Required evidence was not available or sufficient. |
| Invalidated | Previously recorded evidence no longer satisfies current preconditions. |

A claim never moves between states merely because it is repeated. New evidence,
a new derivation rule, or an explicit decision is required.

## Evidence and Source Evaluation

Evidence strength is claim-specific. For exact content identity, the current
ordering is:

1. current byte-for-byte comparison;
2. equal current logical size and equal complete cryptographic content hash;
3. equal logical size without a complete hash;
4. filename, extension, timestamps, media metadata, or partial signatures.

Only level 2 establishes an exact group during read-only analysis. A future
destructive operation requires level 1 immediately before commit. Lower levels
may narrow candidates but never inherit the meaning of a stronger level.

`ffprobe` output is direct evidence about what that adapter reported, not proof
that every decoder will interpret the file identically. Filesystem metadata is
an observation from a specific time and platform, not timeless content
identity.

## Provenance

Material claims preserve enough provenance to identify:

- source run and effective evidence policy;
- input and path observation;
- observation time and coverage;
- detector or adapter identity, version, parameters, and outcome;
- complete hash algorithm and value where applicable;
- schema version and producing OptiFlow version;
- derivation or planning rule that transformed evidence into a claim.

Provenance identifies how a result was obtained. It does not make weak evidence
stronger.

## Confidence and Uncertainty

Exact identity is a thresholded proof claim, not a confidence score. Future
perceptual, containment, or quality evidence may include scores, but each score
must remain scoped to its detector, version, inputs, and calibrated meaning.

The system represents uncertainty structurally through coverage, diagnostics,
stability, capability status, evidence completeness, and claim type. Human text
may summarize those facts but must not replace them.

## Conflict Resolution

When observations or claims conflict:

1. preserve both source records;
2. determine whether they describe different times, paths, policies, adapters,
   or schema versions;
3. prefer current direct evidence for current-action preconditions;
4. invalidate rather than rewrite immutable historical evidence;
5. return unknown or partial when the conflict cannot be resolved safely;
6. require an accepted decision before changing a durable interpretation.

Convenience, detector popularity, or infrastructure origin does not resolve an
evidence conflict.

## Canonical Working Knowledge

- SQLite is the queryable local working state for current implementations.
- Run, report, effective-policy, and plan artifacts are immutable evidence
  records for interchange and review.
- Checked-in schemas define the accepted wire shape.
- Specifications define intended behavior and non-goals.
- Tests provide executable evidence that an implementation satisfies selected
  cases.
- Architecture decisions define accepted trade-offs.

None of these sources silently overrides the others. A disagreement is a defect
or an explicitly recorded migration.

## Revision and Deprecation

Historical artifacts retain their original meaning. New evidence may supersede
their usefulness for a current action without rewriting them. Incompatible
claim semantics require a new schema identifier and migration guidance.

## Examples

- Two equal-size files with the same complete BLAKE3 hash are derived members of
  one exact group for the scan that observed them.
- A matching duration and resolution is candidate evidence only.
- A path missing during future apply invalidates that action's preconditions; it
  does not erase the historical report.
- Missing `ffprobe` can leave media descriptors unknown while exact byte
  evidence remains complete.

## Open Questions

- How should future detector calibration evidence be versioned?
- Which filesystem facts are sufficiently portable to participate in current
  content identity?
- How should distributed observers establish trust and clock-independent
  ordering?

## Validation

- Governing specification: `architecture-epistemology` version `2.0.0`.
- Claim states and evidence thresholds are explicit.
- Exact identity remains independent of perceptual confidence.
- Revision preserves immutable historical meaning.
