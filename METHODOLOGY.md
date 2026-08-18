---
schema: aether.architecture-document/v1
id: optiflow-methodology
title: OptiFlow Methodology
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-08-18
governed_by:
  - architecture-methodology
depends_on:
  - optiflow-principles
  - optiflow-epistemology
  - optiflow-ai-constitution
  - optiflow-foundations
  - optiflow-architecture
related:
  - optiflow-decisions
  - optiflow-roadmap
supersedes: []
---

# OptiFlow Methodology

## Purpose and Scope

OptiFlow combines specification-driven, schema-driven, and test-driven
development into one evidence loop. This document defines how work moves
through that loop and how architecture, product, and infrastructure choices are
evaluated.

## Working Method

Every material change begins by identifying the concern it changes and the
canonical artifact that owns that concern.

| Concern | Canonical source |
| --- | --- |
| Purpose, principles, systems, and boundaries | Root architecture document set |
| Product behavior and non-goals | `docs/mvp-spec.md` and `ROADMAP.md` |
| Safety authority | `docs/safety-model.md` and accepted decisions |
| Machine semantics | `docs/json-contract.md` and `schemas/` |
| State evolution | `docs/state-model.md` and `migrations/` |
| CLI outcomes | `docs/cli-contract.md` and command-result schema |
| Executable proof | Unit, component, integration, contract, and smoke tests |
| Release evidence | Changelog, package metadata, provenance, and CI artifacts |

No source silently wins a contradiction. The change resolves the disagreement
and records compatibility consequences.

## Workflow Stages or Loops

### 1. Frame

- State the user or operator outcome.
- Name authority, privacy, safety, and compatibility implications.
- Declare non-goals and the evidence that would falsify the proposal.

### 2. Model

- Update canonical concepts and boundaries when necessary.
- Define accepted, rejected, partial, interrupted, and stale examples.
- Record an architecture decision when the trade-off is durable or costly to
  reverse.

### 3. Contract

- Update schemas, migrations, CLI behavior, adapter ports, and examples.
- Assign complete identifiers and compatibility behavior.
- Define provenance and current-precondition requirements.

### 4. Prove

- Write the smallest failing tests that demonstrate missing behavior.
- Place safety-sensitive proof at the lowest practical layer and repeat it at
  an end-to-end boundary.
- Prefer synthetic fixtures over private or uncontrolled media.

### 5. Implement

- Change production code narrowly until the specified proofs pass.
- Preserve layer direction and avoid importing infrastructure semantics into
  the domain.
- Keep unsupported future capability absent rather than simulated.

### 6. Reconcile

- Feed implementation discoveries into specifications, schemas, examples,
  tests, decisions, and documentation.
- Record assumptions and evidence gaps rather than hiding them.
- Confirm that human and machine output express the same outcome.

### 7. Release or Defer

- State additive, breaking, migratable, experimental, or unsupported impact.
- Verify clean installation and packaged behavior when release scope applies.
- Defer a capability when its authority, recovery, or validation chain remains
  incomplete.

## Validation Loops

```text
architecture intent
  -> behavior specification
  -> schemas and examples
  -> executable tests
  -> implementation
  -> local and CI evidence
  -> architecture and specification refinement
```

The verification layers are domain tests, filesystem and SQLite component
tests, CLI integration tests, schema tests over real artifacts, synthetic
end-to-end safety tests, macOS/Linux CI, and release-candidate package tests.

## Technology Evaluation

External tools and cloud-native projects are evaluated as capability providers,
not collected as status symbols.

1. Identify a concrete user or operator need.
2. Decide whether OptiFlow, another Ego Hygiene repository, a hosted provider,
   or no system should own it.
3. Compare maturity, portability, security, operational load, data boundaries,
   community health, and exit cost.
4. Prefer established standards and graduated or incubating projects for
   production foundations; use sandbox projects for bounded experiments.
5. Introduce the smallest interface that permits replacement.
6. Record the decision, fallback, validation, and removal conditions.

The CNCF Landscape is a taxonomy and discovery aid, not an installation plan.

## Feedback and Improvement

- Production and user findings become reproducible fixtures or explicit
  evidence gaps.
- Repeated manual work is a candidate for automation only after its semantics
  are understood.
- Repeated exceptions trigger architecture review rather than silent policy
  drift.
- Metrics may inform priorities but do not override safety or truthfulness.

## Human and AI Collaboration Patterns

- Humans define consequential intent, authority, and acceptance.
- AI may scan, compare, draft, implement bounded changes, and run validation.
- Pull requests are the default boundary for material repository changes.
- Agents label assumptions, preserve provenance, and stop at the risk boundaries
  in [`AI_CONSTITUTION.md`](AI_CONSTITUTION.md).
- Review evaluates the reasoning and evidence, not only whether CI is green.

## Boundaries and Exclusions

- The methodology does not require speculative abstractions before a behavior
  needs them.
- It does not require Kubernetes, microservices, or distributed systems for a
  local CLI.
- It does not permit weakening an invariant to reach a version target.
- It does not treat generated documentation as a substitute for source review.
- It does not define organization-wide materialization; Aether and Holon are the
  target owners of that future capability.

## Assumptions and Evidence Gaps

- The 18-document architecture contract is currently adopted manually in this
  repository; the Aether installation and conformance gate are not yet released.
- Release-candidate package verification is incomplete.
- Cloud execution lacks production evidence.

## Open Questions

- Which architecture relationships should the eventual universal gate enforce
  first?
- How should contract examples be generated without hiding handwritten intent?
- Which cloud-native experiments belong in Realm rather than OptiFlow?

## Validation

- Governing specification: `architecture-methodology` version `1.1.0`.
- Workflow, validation, evaluation, feedback, and human/AI boundaries are
  explicit.
- The methodology preserves the closed loop documented in
  [`docs/development-model.md`](docs/development-model.md).
