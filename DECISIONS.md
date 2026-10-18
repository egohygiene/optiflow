---
schema: aether.architecture-document/v1
id: optiflow-decisions
title: OptiFlow Decisions
kind: architecture-document
version: 0.1.1
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-09-28
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
| OFD-0001 | Accepted | End `v0.1.x` authority at review-only planning |
| OFD-0002 | Accepted | Use local SQLite plus immutable JSON artifacts |
| OFD-0003 | Accepted | Prove exact groups with complete BLAKE3 evidence |
| OFD-0004 | Accepted | Version wire contracts independently from the binary |
| OFD-0005 | Accepted | Invoke external adapters directly without a shell |
| OFD-0006 | Accepted | Integrate through `flow` as a subprocess contract |
| OFD-0007 | Accepted | Use a specification-schema-test evidence loop |
| OFD-0008 | Accepted | Compose one static site from isolated producers |
| OFD-0009 | Accepted | Treat cloud-native systems as replaceable platform capabilities |
| OFD-0010 | Accepted | Adopt the Aether architecture-document metadata contract manually |
| OFD-0011 | Accepted | Separate extension declaration, operator trust, and core acceptance |
| OFD-0012 | Proposed | Bind explicit selections and approval to a dry-run-only execution contract |
| OFD-0013 | Proposed | Separate quarantine mutation evidence from dry-run v1 |
| OFD-0014 | Proposed | Require pre-mutation authority and append-only recovery evidence |
| OFD-0015 | Proposed | Independently authorize irreversible retained-copy finalization |
| OFD-0016 | Proposed | Publish source-preserving PNG candidates under an independent artifact protocol |

## Active Decisions

### OFD-0001 — End `v0.1.x` authority at review-only planning

- **Status:** Accepted
- **Context:** Useful inventory and exact-duplicate evidence can ship before a
  complete transaction, validation, and recovery model exists.
- **Decision:** The current binary may write its own state and artifacts but has
  no source-media mutation command. Plans declare `mutates_files: false`.
- **Consequences:** Users receive immediate decision support; mutation work must
  cross a new explicit architecture and cannot hide in a patch release.
- **Revisit when:** Transactional exact resolution satisfies the safety model
  and its release exit criteria together.

### OFD-0002 — Use local SQLite plus immutable JSON artifacts

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

### OFD-0003 — Prove exact groups with complete BLAKE3 evidence

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

### OFD-0004 — Version wire contracts independently from the binary

- **Status:** Accepted
- **Context:** `flow`, shell scripts, and future services need stable semantics
  without coupling to internal Rust releases.
- **Decision:** Every machine document declares a complete schema identifier;
  schemas, database migrations, and the binary have separate version lifecycles.
- **Consequences:** Consumers branch on schema identity and tolerate only
  documented additive evolution. Semantic breaks require new identifiers.
- **Revisit when:** Never for the principle; individual compatibility policies
  may evolve through a superseding decision.

### OFD-0005 — Invoke external adapters directly without a shell

- **Status:** Accepted
- **Context:** Source paths and adapter inputs may contain spaces, Unicode, or
  hostile shell characters.
- **Decision:** Spawn named executables with argument arrays, bounds, structured
  parsing, and no shell interpretation.
- **Consequences:** Adapter code carries explicit capability and error handling;
  pipelines cannot depend on shell composition shortcuts.
- **Revisit when:** A sandboxed adapter protocol replaces local subprocesses.

### OFD-0006 — Integrate through `flow` as a subprocess contract

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

### OFD-0007 — Use a specification-schema-test evidence loop

- **Status:** Accepted
- **Context:** Documentation-only, schema-only, and test-only development each
  leave blind spots in a safety-sensitive product.
- **Decision:** Material behavior evolves through intent, examples, schemas or
  migrations, failing proofs, implementation, observed evidence, and
  reconciliation.
- **Consequences:** Changes touch multiple coherent artifacts when required;
  disagreement becomes a defect rather than implicit precedence.
- **Revisit when:** The loop can be strengthened, not bypassed, by better tools.

### OFD-0008 — Compose one static site from isolated producers

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

### OFD-0009 — Treat cloud-native systems as replaceable platform capabilities

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

### OFD-0010 — Adopt the Aether architecture-document metadata contract manually

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

### OFD-0011 — Separate extension declaration, operator trust, and core acceptance

- **Status:** Accepted
- **Context:** The Flow suite roadmap now needs a native OptiFlow provider
  boundary before transactional work, while OptiFlow must remain read-only and
  independently releasable. A declaration alone cannot safely confer runtime
  authority, and a plugin result cannot be allowed to bypass core evidence or
  planning gates.
- **Decision:** Publish `optiflow.extension-sdk.v1` as additive typed Rust and
  bounded JSON contracts. Provider manifests declare capabilities and effects;
  operator locks pin bytes and grant trust, configuration, effects, precedence,
  replacement, and fallback. Embedded handlers use role-specific registration.
  Process handlers require an absolute pinned executable and bounded direct
  invocation. Every result is revalidated by the core, planners only describe
  operations that retain all core preconditions, and lifecycle observers return
  no contributions. No SDK v1 extension receives mutation, destructive,
  signing, or publication authority.
- **Consequences:** OptiFlow can host inspectors, analyzers, normalization and
  policy contributors, planners, validators, report/export providers, and
  read-only observers without sibling source coupling or executable discovery.
  Process providers are explicitly trusted and bounded but are not represented
  as OS-sandboxed. Flow may adapt released contracts without importing OptiFlow
  source. Existing CLI and artifact contracts remain unchanged; four new wire
  identifiers and three inspection commands are additive.
- **Revisit when:** A real containment backend can enforce declared network,
  filesystem, and resource effects, or a semantic SDK change requires new wire
  identifiers and a migration.

### OFD-0012 — Bind explicit selections and approval to a dry-run-only execution contract

- **Status:** Proposed for #90 review
- **Context:** Review-plan keep defaults and plan existence cannot authorize the
  first source mutation. Files and volumes can change after inventory, while
  partial transaction implementations must not imply completed recovery.
- **Decision:** Capture explicit operator-selected paths in a new immutable
  execution plan and require a separate fingerprint-bound approval. Revalidate
  scoped identities, metadata, full hashes, bytes, permissions and capacity
  without source writes. Persist versioned run/attempt/validation/commit/recovery
  evidence through an additive SQLite migration and an exclusive execution lock.
- **Consequences:** Historical plans gain no authority. Native Linux/macOS
  filesystem evidence is required; unsupported identities/flags/topology fail
  closed. Canonical fingerprints give deterministic local identity but do not
  authenticate the operator. Commit v1 can represent only `not_started`, and
  recovery v1 claims only `no_source_mutation`. #91 requires explicit contract
  evolution and fresh proofs before any source mutation can be represented.
- **Evidence:** [Execution dry-run protocol](docs/execution-dry-run.md),
  `schemas/execution-v1.schema.json`, migration 0006 and synthetic execution tests.
- **Revisit when:** #91 earns bounded mutation and property-preserving recovery.

### OFD-0013 — Separate quarantine mutation evidence from dry-run v1

- **Status:** Proposed for #91 review
- **Context:** v1 commit/recovery records explicitly forbid source mutation.
  Cross-filesystem copy and same-filesystem rename require different recovery
  evidence and can stop between filesystem and journal steps.
- **Decision:** Keep v1 immutable. Add migration 0007 and a strict v2 mutation
  record under the existing exclusive, fully synchronized journal. Persist
  phases before and after every consequential step, use no-replace Linux
  renames, and verify content and required properties before cross-filesystem
  source removal. Refuse hosts or properties whose guarantees are unavailable.
  Never automatically retry an occupied namespace or represent an ambiguous
  interruption as committed.
- **Consequences:** #91 supplies a manual inspection/recovery path; #92 owns
  idempotent resume, status and restore. A crash may leave a durable copy,
  original or both. The last durable phase is evidence, not an atomic
  compare-and-mutate against hostile concurrent writers. Physical savings
  remain unknown and the v0.1.1 release remains read-only.
- **Evidence:** [Quarantine protocol](docs/execution-quarantine.md),
  `schemas/execution-mutation-v2.schema.json`, migration 0007 and synthetic
  same/cross-filesystem plus interruption tests.
- **Revisit when:** #92 defines idempotent restore and fault-injected recovery.

### OFD-0014 — Require pre-mutation authority and append-only recovery evidence

- **Status:** Proposed for #92 review
- **Context:** v2 mutation records cannot express new operator recovery
  authority, and a pending filesystem step may have persisted after its last
  journal update.
- **Decision:** Bind effective configuration, evidence policy and binary
  version before new mutations, preserve v1/v2 records, and append v3 events
  under migration 0008. Record bounded ownership/mode/mtime/xattr fingerprints
  at commit. Permit only verified untouched actions to resume, committed
  quarantine actions to restore without replacement, and empty owned
  namespaces to be removed. A pending transition remains ambiguous.
- **Consequences:** Historical v2 runs remain inspection-only. Cross-filesystem
  restore retains the quarantine copy; cleanup refuses it. Read-only status is
  a journal report, not a current filesystem proof. Physical savings remain
  unknown and the released v0.1.1 remains read-only.
- **Evidence:** [Recovery protocol](docs/execution-recovery.md),
  `schemas/execution-recovery-*-v3.schema.json`, migration 0008 and synthetic
  interruption/collision/metadata tests.
- **Revisit when:** #96 defines independently authorized finalization.

### OFD-0015 — Independently authorize irreversible retained-copy finalization

- **Status:** Proposed for #96 review
- **Context:** A cross-filesystem restore retains a verified quarantine copy;
  v3 recovery authority cannot express permanent removal or its interruption.
- **Decision:** Restrict v4 finalization to explicitly selected, completed,
  v3-bound `restored_retained` actions. Require a read-only fingerprinted
  preview, manifest-bound separate local authorization, revalidation of the
  restored source, keeper and quarantine file, and durable pending/removed
  events around unlink and directory sync. A pending event stays inspect-only.
- **Consequences:** v1-v3 records remain historical and no original source is
  removed. Observed allocation and available-space samples are separate from
  logical bytes; physical reclaimed bytes remain unknown. The filesystem and
  SQLite journal cannot atomically commit together, so interrupted removal
  needs operator investigation.
- **Evidence:** [Finalization protocol](docs/execution-finalization.md),
  `schemas/execution-finalization-*-v4.schema.json`, migration 0009 and
  synthetic cross-filesystem refusal/interruption tests.
- **Revisit when:** #93 qualifies the release and reviews native target evidence.

### OFD-0016 — Publish source-preserving PNG candidates under an independent artifact protocol

- **Status:** Proposed for #94 review
- **Context:** The #78 declaration packet and #80 library byte validator do
  not execute an optimizer. Scan/plan artifact-set v1 cannot represent binary
  candidate media, and a provider result cannot establish profile preservation
  or mutation authority. The relevant #65 fixtures enter through #112.
- **Decision:** Require an explicitly selected, separately installed OxiPNG
  v10.2.1 executable, observe its complete binary and fixed direct invocation,
  supply bounded source bytes by standard input and accept its standard output
  only after independent source stability and PNG byte validation. Publish
  source-preserving `candidate.png` plus strict v1 evidence under a separate
  marker-sealed candidate set with private staging, no-replace publication and
  inspection/recovery. A candidate never authorizes source replacement.
- **Consequences:** Published v0.1.1 remains read-only and immutable. A #94
  result records encoded logical reduction while physical savings remain
  unknown; full provider RSS and private workdir usage are unmeasured. The
  provider is not bundled or OS-sandboxed, and binary hashing cannot eliminate
  the final path-swap/exec race. No old contract or artifact is promoted into
  runtime authority. #95 must separately bind approved replacement to the #93
  release-qualified transaction engine.
- **Evidence:** [PNG production protocol](docs/png-candidate-production.md),
  `schemas/png-candidate-evidence-v1.schema.json`,
  `schemas/png-candidate-artifact-set-v1.schema.json`, and #94 synthetic
  provider/CLI interruption tests.
- **Revisit when:** A second provider, full process containment, or approved
  PNG replacement requires a new protocol version or stronger isolation.

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

## Identifier format migration (2026-10-18)

The maintainer selected four-digit numbering in [Hygiene #73](https://github.com/egohygiene/hygiene/issues/73). This padding-only migration changes current labels and anchors; logical numbers, decision content, dates and disposition remain unchanged. Git retains prior versions. New canonical records use `ADR-NNNN` under Hygiene policy v1.2.0.

| Previous ID | Current ID | Record |
| --- | --- | --- |
| `OFD-001` | `OFD-0001` | [DECISIONS.md](DECISIONS.md#ofd-0001--end-v01x-authority-at-review-only-planning) |
| `OFD-002` | `OFD-0002` | [DECISIONS.md](DECISIONS.md#ofd-0002--use-local-sqlite-plus-immutable-json-artifacts) |
| `OFD-003` | `OFD-0003` | [DECISIONS.md](DECISIONS.md#ofd-0003--prove-exact-groups-with-complete-blake3-evidence) |
| `OFD-004` | `OFD-0004` | [DECISIONS.md](DECISIONS.md#ofd-0004--version-wire-contracts-independently-from-the-binary) |
| `OFD-005` | `OFD-0005` | [DECISIONS.md](DECISIONS.md#ofd-0005--invoke-external-adapters-directly-without-a-shell) |
| `OFD-006` | `OFD-0006` | [DECISIONS.md](DECISIONS.md#ofd-0006--integrate-through-flow-as-a-subprocess-contract) |
| `OFD-007` | `OFD-0007` | [DECISIONS.md](DECISIONS.md#ofd-0007--use-a-specification-schema-test-evidence-loop) |
| `OFD-008` | `OFD-0008` | [DECISIONS.md](DECISIONS.md#ofd-0008--compose-one-static-site-from-isolated-producers) |
| `OFD-009` | `OFD-0009` | [DECISIONS.md](DECISIONS.md#ofd-0009--treat-cloud-native-systems-as-replaceable-platform-capabilities) |
| `OFD-010` | `OFD-0010` | [DECISIONS.md](DECISIONS.md#ofd-0010--adopt-the-aether-architecture-document-metadata-contract-manually) |
| `OFD-011` | `OFD-0011` | [DECISIONS.md](DECISIONS.md#ofd-0011--separate-extension-declaration-operator-trust-and-core-acceptance) |
| `OFD-012` | `OFD-0012` | [DECISIONS.md](DECISIONS.md#ofd-0012--bind-explicit-selections-and-approval-to-a-dry-run-only-execution-contract) |
| `OFD-013` | `OFD-0013` | [DECISIONS.md](DECISIONS.md#ofd-0013--separate-quarantine-mutation-evidence-from-dry-run-v1) |
| `OFD-014` | `OFD-0014` | [DECISIONS.md](DECISIONS.md#ofd-0014--require-pre-mutation-authority-and-append-only-recovery-evidence) |
| `OFD-015` | `OFD-0015` | [DECISIONS.md](DECISIONS.md#ofd-0015--independently-authorize-irreversible-retained-copy-finalization) |
| `OFD-016` | `OFD-0016` | [DECISIONS.md](DECISIONS.md#ofd-0016--publish-source-preserving-png-candidates-under-an-independent-artifact-protocol) |
