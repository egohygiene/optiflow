---
schema: aether.architecture-document/v1
id: optiflow-decisions
title: OptiFlow Decisions
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-08-18
governed_by:
  - architecture-decisions
depends_on:
  - optiflow-principles
  - optiflow-epistemology
  - optiflow-foundations
  - optiflow-system
  - optiflow-architecture
related:
  - optiflow-ai-constitution
  - optiflow-methodology
  - optiflow-roadmap
supersedes: []
---

# OptiFlow Decisions

## Purpose

This document is the canonical decision log for durable OptiFlow architecture
choices. It captures why a choice was accepted, its consequences, and what
evidence could cause reconsideration.

## Decision Governance

- IDs are stable and never reused.
- New decisions use `OFD-NNN` in ascending order.
- Accepted decisions describe current direction; proposed decisions grant no
  implementation authority.
- A changed durable choice adds a superseding record rather than rewriting the
  historical rationale.
- Breaking contract, safety, privacy, ownership, or deployment changes require
  explicit compatibility and migration consequences.

## Storage Mode

The initial record set remains in this index while the project is small. A
future migration may move detailed records to `docs/architecture/decisions/`
without changing IDs or history. This document will retain the index.

## Status Definitions

| Status | Meaning |
| --- | --- |
| Proposed | Under review and not authoritative. |
| Accepted | Governs current architecture. |
| Deprecated | Retained for history but discouraged. |
| Superseded | Replaced by a named later decision. |
| Rejected | Considered and deliberately not adopted. |

## Decision Index

| ID | Status | Decision |
| --- | --- | --- |
| OFD-001 | Accepted | End `v0.1.x` authority at review-only planning |
| OFD-002 | Accepted | Use local SQLite plus immutable JSON artifacts |
| OFD-003 | Accepted | Prove exact groups with complete BLAKE3 evidence |
| OFD-004 | Accepted | Version wire contracts independently from the binary |
| OFD-005 | Accepted | Invoke external adapters directly without a shell |
| OFD-006 | Accepted | Integrate through `flow` as a subprocess contract |
| OFD-007 | Accepted | Use a specification-schema-test evidence loop |
| OFD-008 | Accepted | Compose one static site from isolated producers |
| OFD-009 | Accepted | Treat cloud-native systems as replaceable platform capabilities |
| OFD-010 | Accepted | Adopt the Aether architecture-document metadata contract manually |

## Active Decisions

### OFD-001 — End `v0.1.x` authority at review-only planning

- **Status:** Accepted
- **Context:** Useful inventory and exact-duplicate evidence can ship before a
  complete transaction, validation, and recovery model exists.
- **Decision:** The current binary may write its own state and artifacts but has
  no source-media mutation command. Plans declare `mutates_files: false`.
- **Consequences:** Users receive immediate decision support; mutation work must
  cross a new explicit architecture and cannot hide in a patch release.
- **Revisit when:** Transactional exact resolution satisfies the safety model
  and its release exit criteria together.

### OFD-002 — Use local SQLite plus immutable JSON artifacts

- **Status:** Accepted
- **Context:** Scans need queryable cache and lifecycle state, while integrations
  need portable, reviewable evidence.
- **Decision:** SQLite owns local working state; schema-versioned JSON owns
  interchange and immutable run, report, policy, and plan evidence.
- **Consequences:** The system supports efficient repeat scans without making
  the database the only provenance record. Migrations and artifact schemas
  evolve independently.
- **Revisit when:** Shared or distributed execution demonstrates a concrete
  need that the current boundary cannot satisfy.

### OFD-003 — Prove exact groups with complete BLAKE3 evidence

- **Status:** Accepted
- **Context:** Metadata and partial signatures are efficient candidate filters
  but cannot prove complete byte identity.
- **Decision:** Read-only exact groups require equal logical size and equal
  complete BLAKE3-256 hashes. Future destructive resolution additionally
  requires current byte-for-byte confirmation.
- **Consequences:** Full hashing cost is paid only for candidate groups; exact
  language remains defensible and collision risk is addressed at the mutation
  boundary.
- **Revisit when:** Cryptographic or platform evidence changes materially.

### OFD-004 — Version wire contracts independently from the binary

- **Status:** Accepted
- **Context:** `flow`, shell scripts, and future services need stable semantics
  without coupling to internal Rust releases.
- **Decision:** Every machine document declares a complete schema identifier;
  schemas, database migrations, and the binary have separate version lifecycles.
- **Consequences:** Consumers branch on schema identity and tolerate only
  documented additive evolution. Semantic breaks require new identifiers.
- **Revisit when:** Never for the principle; individual compatibility policies
  may evolve through a superseding decision.

### OFD-005 — Invoke external adapters directly without a shell

- **Status:** Accepted
- **Context:** Source paths and adapter inputs may contain spaces, Unicode, or
  hostile shell characters.
- **Decision:** Spawn named executables with argument arrays, bounds, structured
  parsing, and no shell interpretation.
- **Consequences:** Adapter code carries explicit capability and error handling;
  pipelines cannot depend on shell composition shortcuts.
- **Revisit when:** A sandboxed adapter protocol replaces local subprocesses.

### OFD-006 — Integrate through `flow` as a subprocess contract

- **Status:** Accepted
- **Context:** OptiFlow must remain independently installable and releasable
  while participating in a suite.
- **Decision:** `flow` invokes released OptiFlow commands and consumes versioned
  JSON artifacts. Sibling tools do not embed OptiFlow source or depend on its
  default branch.
- **Consequences:** Release cycles and contract ownership remain independent;
  process startup is accepted in exchange for stronger isolation.
- **Revisit when:** A separately versioned SDK has multiple proven consumers and
  preserves the same boundary.

### OFD-007 — Use a specification-schema-test evidence loop

- **Status:** Accepted
- **Context:** Documentation-only, schema-only, and test-only development each
  leave blind spots in a safety-sensitive product.
- **Decision:** Material behavior evolves through intent, examples, schemas or
  migrations, failing proofs, implementation, observed evidence, and
  reconciliation.
- **Consequences:** Changes touch multiple coherent artifacts when required;
  disagreement becomes a defect rather than implicit precedence.
- **Revisit when:** The loop can be strengthened, not bypassed, by better tools.

### OFD-008 — Compose one static site from isolated producers

- **Status:** Accepted
- **Context:** The landing page, documentation, and schemas have separate source
  and build ownership but share one public domain.
- **Decision:** Producers build into isolated staging paths. A final composition
  rejects reserved-mount collisions and validates local links before replacing
  `dist/`.
- **Consequences:** No generator overwrites another surface; deployment remains
  a separate authorization and review boundary.
- **Revisit when:** Additional API, intelligence, or release surfaces join the
  composition contract.

### OFD-009 — Treat cloud-native systems as replaceable platform capabilities

- **Status:** Accepted
- **Context:** The CNCF ecosystem offers many mature solutions, but embedding
  each platform concern in a local media CLI would increase cognitive and
  operational load.
- **Decision:** OptiFlow remains a portable application workload. Realm, Relay,
  Flow, Observatory, Pace, and infrastructure code select packaging,
  orchestration, delivery, policy, and telemetry capabilities through explicit
  contracts.
- **Consequences:** Local operation stays complete. Kubernetes and cloud
  providers can be adopted incrementally without becoming domain dependencies.
- **Revisit when:** A platform capability changes product semantics rather than
  execution context.

### OFD-010 — Adopt the Aether architecture-document metadata contract manually

- **Status:** Accepted
- **Context:** Empathy has proven a reusable 18-document architecture graph, but
  Aether installation and organization-wide conformance are not yet released.
- **Decision:** OptiFlow adopts `aether.architecture-document/v1`, stable IDs,
  governing specification references, and an acyclic relationship graph now.
  Enforcement remains deferred to the later universal gate.
- **Consequences:** Humans and agents receive a complete canonical context set;
  the repository may later replace manual copies with a pinned Aether/Holon
  projection without changing document meaning.
- **Revisit when:** Aether publishes the installation and migration contract.

## Deprecated and Superseded Decisions

None.

## Historical Decisions

The detailed product roadmap and existing v1-v3 schemas predate this formal log
and remain valid implementation evidence. Future records should cite the
specific source artifacts that established an earlier choice.

## Evidence Gaps and Open Questions

- No accepted decision selects an OCI registry, orchestrator, observability
  stack, or cloud provider.
- Transaction and recovery strategies remain roadmap work.
- The architecture gate and Aether materialization mechanism are not released.

## Validation

- Governing specification: `architecture-decisions` version `2.0.0`.
- IDs, statuses, context, decisions, consequences, and review triggers are
  explicit.
- Accepted records align with current implementation or an explicitly stated
  architecture boundary.
