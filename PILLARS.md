---
schema: aether.architecture-document/v1
id: optiflow-pillars
title: OptiFlow Pillars
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-08-18
governed_by:
  - architecture-pillars
depends_on:
  - optiflow-purpose
  - optiflow-vision
  - optiflow-principles
related:
  - optiflow-manifesto
  - optiflow-foundations
  - optiflow-roadmap
supersedes: []
---

# OptiFlow Pillars

## Introduction

The pillars group OptiFlow's durable commitments into initiative-scale themes.
A roadmap item should advance at least one pillar without undermining another.

## Pillar 1 — Faithful Observation

Build a conservative, content-aware inventory that preserves the distinction
between paths, filesystem facts, media descriptors, content evidence, and
coverage limitations.

Success means the system can explain what it attempted, what it observed, what
it reused, and what remained unreadable or unstable.

## Pillar 2 — Defensible Relationships

Name each relationship class precisely and bind it to detector-specific
evidence. Exact duplicates, perceptual similarity, derivation, containment, and
quality suitability are different claims and never share an implicit threshold.

Success means another implementation can inspect an artifact and reproduce the
reason a relationship was asserted.

## Pillar 3 — Bounded Authority and Recovery

Separate evidence from permission. Introduce mutation only through explicit,
transactional, preconditioned, validated, durable, and recoverable execution.

Success means a failure cannot be disguised as completion and every authorized
change has a stated recovery guarantee.

## Pillar 4 — Versioned Interoperability

Expose stable CLI outcomes, effective policy, schemas, migrations, and immutable
artifacts so people and tools can integrate without embedding internal source.

Success means OptiFlow can evolve independently while consumers branch on
declared contracts and retain provenance.

## Pillar 5 — Reproducible Operation

Make supported workstation, devcontainer, CI, and future OCI execution paths
consume the same semantic configuration and validation rules.

Success means infrastructure changes where and when OptiFlow runs, not what an
evidence claim means.

## Pillar 6 — Understandable Experience

Present complex storage evidence with calm hierarchy, explicit status,
accessible output, and progressive disclosure for both human and machine
consumers.

Success means a user can identify the conclusion, supporting evidence,
limitations, proposed next step, and safety boundary without reverse-engineering
implementation details.

## Relationships Between Pillars

Faithful observations feed defensible relationships. Relationships inform
plans, while bounded authority constrains what plans can become. Versioned
interoperability preserves those meanings across consumers. Reproducible
operation expands where the product can run, and understandable experience
makes the complete chain reviewable.

No pillar stands alone: faster operation without evidence fidelity, or polished
experience without truthful limitations, is not progress.

## Initiative Alignment

| Initiative | Primary pillars |
| --- | --- |
| `v0.1.x` read-only foundation | 1, 2, 4, 5, 6 |
| Transactional exact resolution | 2, 3, 4 |
| Media optimization adapters | 1, 3, 5 |
| Perceptual and containment evidence | 1, 2, 6 |
| `flow` orchestration | 4, 5, 6 |
| OCI and cloud execution | 4, 5 |

## Open Questions

- Which measurable signals best represent understandable decision support?
- When should distributed execution become an initiative rather than a future
  option?
- How should recovery guarantees be compared across action classes?

## Validation

- Governing specification: `architecture-pillars` version `2.0.0`.
- Every pillar is durable, distinct, and traceable to the principles.
- Initiative alignment is directional and does not replace
  [`ROADMAP.md`](ROADMAP.md).
