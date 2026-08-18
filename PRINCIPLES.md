---
schema: aether.architecture-document/v1
id: optiflow-principles
title: OptiFlow Principles
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-08-18
governed_by:
  - architecture-principles
depends_on:
  - optiflow-purpose
  - optiflow-vision
related:
  - optiflow-pillars
  - optiflow-foundations
  - optiflow-methodology
supersedes: []
---

# OptiFlow Principles

## Introduction

These principles guide choices when multiple implementations appear viable.
They do not replace accepted decisions, schemas, safety invariants, or release
criteria.

## Principle 1 — Evidence Before Action

Acquire and preserve the evidence needed to justify a conclusion before
proposing or executing a consequential action.

- Record coverage and failures alongside successful observations.
- Do not convert a heuristic into proof through repetition or confidence alone.
- Make every proposal traceable to the source evidence and policy that created
  it.

## Principle 2 — Observation Is Not Authority

Keep detection, planning, approval, execution, validation, and commit as
separate phases.

- A report describes; a plan proposes; neither authorizes mutation.
- Future apply behavior must be explicit in command, policy, and artifact.
- Re-prove current preconditions at the last responsible moment.

## Principle 3 — Exact Means Byte-for-Byte

Use precise relationship names and evidence thresholds.

- Filename, duration, dimensions, metadata, and partial fingerprints never
  prove exact identity.
- Exact groups require equal logical size and complete content hashes.
- Destructive exact resolution additionally requires current byte comparison.

## Principle 4 — Uncertainty Is a Result

Unknown, partial, unsupported, stale, interrupted, and unavailable outcomes are
valid information that must remain visible.

- Prefer an honest partial result to fabricated completeness.
- Do not silently drop unreadable or unstable observations.
- Human and machine renderers communicate the same semantic outcome.

## Principle 5 — Contracts Outlive Implementations

Version persisted and subprocess-visible meaning independently from binary and
internal code versions.

- Schemas, migrations, examples, producers, and consumers evolve together.
- Additive compatibility never permits silent semantic reinterpretation.
- Integrations depend on releases and declared contracts, not default-branch
  source layouts.

## Principle 6 — Local First, Composable Everywhere

Keep the core useful on one supported workstation and compose richer execution
forms around that boundary.

- Hosted infrastructure may schedule or observe the tool but does not redefine
  its domain rules.
- OCI, Kubernetes, and cloud providers are deployment options rather than
  mandatory internal abstractions.
- Private source media is not uploaded implicitly.

## Principle 7 — Specialized Adapters, Stable Domain

Use specialized tools behind typed, bounded, provenance-aware ports.

- Invoke executables directly without a shell.
- Distinguish unavailable, unsupported, failed, malformed, and successful
  adapter outcomes.
- Prevent adapter-specific output from becoming the domain language.

## Principle 8 — Conservative and Cross-Platform by Default

Prefer behavior that is safe and explainable across supported macOS and Linux
filesystems.

- Do not follow symbolic links, hidden trees, or mount boundaries implicitly.
- Treat paths as observation locations, not content identity.
- State platform assumptions explicitly and fail closed when they do not hold.

## Principle 9 — Close the Specification Loop

Develop behavior through a continuous specification, schema, test,
implementation, and evidence loop.

- Every machine contract has executable validation.
- Safety-sensitive behavior includes success, refusal, interruption, and
  recovery proofs at appropriate layers.
- Reconcile discoveries back into documentation rather than letting code become
  an undocumented exception.

## Principle Conflicts and Precedence

When principles conflict, apply this order:

1. Prevent data loss, privacy violations, corruption, and false safety claims.
2. Preserve evidence truthfulness, explicit authority, and recoverability.
3. Preserve contract meaning and compatibility.
4. Preserve portability, composability, and maintainability.
5. Optimize speed, resource use, and convenience.

No performance or platform benefit justifies weakening a higher-order rule
silently.

## Exceptions

An exception identifies the principle, scope, owner, rationale, evidence,
review trigger, and compensating control. Exceptions do not become defaults
through repeated use.

## Open Questions

- Which future detectors need additional evidence tiers or confidence language?
- Which supported filesystem features justify platform-specific acceleration?
- How should profile-specific exceptions be represented in effective policy?

## Validation

- Governing specification: `architecture-principles` version `2.0.0`.
- Each principle describes a durable trade-off rather than one tool choice.
- Conflict precedence and exception handling are explicit.
- The principles derive from [`PURPOSE.md`](PURPOSE.md) and
  [`VISION.md`](VISION.md).
