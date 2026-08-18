---
schema: aether.architecture-document/v1
id: optiflow-foundations
title: OptiFlow Foundations
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-08-18
governed_by:
  - architecture-foundations
depends_on:
  - optiflow-purpose
  - optiflow-principles
  - optiflow-epistemology
related:
  - optiflow-pillars
  - optiflow-system
  - optiflow-architecture
  - optiflow-methodology
supersedes: []
---

# OptiFlow Foundations

## Foundational Assumptions

- Filesystems expose changing observations, not a perfectly stable global
  truth.
- Paths locate observations but do not identify content permanently.
- Real media may be private, malformed, partially readable, aliased, sparse,
  cloned, hard-linked, remotely mounted, or modified during analysis.
- Specialized external tools are useful but fallible and version-dependent.
- A valid partial result can be more useful than an all-or-nothing failure when
  its limitations are explicit.
- The person or automation invoking OptiFlow needs stable outcomes and artifacts
  more than it needs access to internal Rust types.
- Infrastructure portability is valuable only when it preserves semantic and
  privacy boundaries.

## Invariants

1. Scanning does not mutate source media.
2. Detection, planning, approval, execution, validation, commit, and recovery
   remain distinct concepts.
3. Exact identity requires equal logical size and a complete content hash;
   future destructive resolution additionally requires current byte comparison.
4. A plan is immutable evidence and never implicit execution authority.
5. Changed or insufficient preconditions fail closed.
6. Shell execution is not part of an adapter boundary.
7. Persisted and subprocess-visible artifacts declare their schema.
8. Artifact commits are validated and atomic at the supported filesystem
   boundary.
9. Coverage loss and interruption never masquerade as complete success.
10. Platform automation does not redefine product evidence semantics.

## Baseline Constraints

- Supported targets are macOS and Linux; Windows behavior is not currently
  guaranteed.
- The current state model uses local SQLite with rollback journaling and full
  synchronization.
- The first cache identity is path, logical size, and modification nanoseconds.
- `ffprobe` is optional for the current exact-duplicate workflow.
- The `v0.1.x` binary contains no apply, delete, move, replace, quarantine, or
  transcode command.
- Source files are opened read-only and external tools are invoked directly.
- JSON contracts, database migrations, and binary versions evolve
  independently.
- OCI images, Kubernetes, and cloud services are future execution substrates,
  not prerequisites for local correctness.

## Mental Models

### Evidence ledger

Think of a run as a bounded evidence ledger. It records what was requested,
which policy governed observation, what succeeded, what failed, and which
claims were derived.

### Compiler pipeline

Think of planning as compilation:

```text
inputs + policy
  -> observations
  -> normalized evidence
  -> relationship model
  -> immutable report
  -> review plan
```

Each stage accepts a narrower, validated language and does not grant a later
stage's authority.

### Transaction boundary

Future execution is a new transaction over current facts, not the final step of
an old scan. Historical evidence selects candidates; current preconditions
decide whether an action may proceed.

### Product inside a platform

OptiFlow is an independently useful workload. Realm, Relay, Flow, Observatory,
and infrastructure systems may package, schedule, invoke, and observe it, but
they do not become its domain core.

## Falsified or Revised Foundations

- **Rejected:** filename extension is a reliable media classification source.
- **Rejected:** equal size alone is enough to identify duplicates.
- **Rejected:** a cryptographic hash alone is sufficient authority for
  destructive resolution.
- **Rejected:** a successful report implies complete requested coverage.
- **Rejected:** all infrastructure portability must be implemented inside the
  application.
- **Revised:** a cached observation may be reused only under an explicit cache
  identity and must acquire additional evidence when a later candidate set
  requires it.

## Assumptions and Evidence Gaps

- Current platform tests do not prove behavior on every supported filesystem.
- State portability across removable volumes is not implemented.
- No production evidence yet validates distributed execution or shared state.
- Recovery semantics for mutation remain design targets rather than shipped
  guarantees.

## Open Questions

- Which filesystem identity hints should supplement future content-addressed
  cache recognition?
- What is the narrowest safe state interface for concurrent or remote jobs?
- Which foundations need to become machine-enforced organization policy in
  Aether and Egolint?

## Validation

- Governing specification: `architecture-foundations` version `1.1.0`.
- Assumptions, invariants, constraints, mental models, and rejected foundations
  are distinguishable.
- Current limitations are not presented as future guarantees.
- The foundations align with [`docs/safety-model.md`](docs/safety-model.md).
