---
schema: aether.architecture-document/v1
id: optiflow-system
title: OptiFlow System
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-08-18
governed_by:
  - architecture-system
depends_on:
  - optiflow-foundations
  - optiflow-ontology
related:
  - optiflow-architecture
  - optiflow-design
  - optiflow-methodology
supersedes: []
---

# OptiFlow System

## Purpose and Scope

This document identifies OptiFlow's logical systems, their responsibilities,
and their external relationships. [`ARCHITECTURE.md`](ARCHITECTURE.md) defines
the structural layers and dependency rules that implement them.

Current and target systems are labeled explicitly. A target relationship does
not imply that its integration or infrastructure is already shipped.

## System Inventory

### Invocation and Policy System

Parses CLI intent, captures recognized environment and working-directory
context, resolves configuration precedence, materializes locked safety
invariants, and calculates effective and evidence-policy identities.

- **Current evidence:** `src/cli.rs`, `src/configuration/`, configuration and
  effective-policy schemas.
- **State:** implemented for the public v1 configuration contract.

### Discovery System

Traverses explicit input roots under conservative symlink, hidden-tree, and
filesystem-boundary policy while preserving per-root failures and coverage.

- **Current evidence:** `src/discovery.rs` and filesystem integration tests.
- **State:** implemented for macOS and Linux.

### Inventory and Adapter System

Captures filesystem facts, classifies actual content, and normalizes optional
media descriptions returned by specialized tools.

- **Current evidence:** `src/inventory.rs`, `src/adapters/ffprobe.rs`.
- **Boundary:** adapters are invoked directly without a shell and cannot define
  domain relationship semantics.

### Evidence and Relationship System

Narrows candidate sets, streams complete BLAKE3 hashes, and derives exact groups
only from compatible current evidence.

- **Current evidence:** `src/hashing.rs`, `src/duplicates.rs`.
- **State:** exact byte-identity evidence implemented; perceptual, derivation,
  containment, and quality relationships deferred.

### State and Cache System

Persists run lifecycle, observations, exact groups, and reusable unchanged-path
analysis in local SQLite through forward-only migrations.

- **Current evidence:** `src/state.rs`, `migrations/`, state-model documentation.
- **State:** implemented for local single-process operation.

### Reporting and Planning System

Projects source-run evidence into immutable, schema-validated reports and
non-mutating plans with explicit preconditions and potential effects.

- **Current evidence:** `src/reports.rs`, `src/planning.rs`, artifact schemas.
- **State:** implemented through the review-only boundary.

### Outcome and Presentation System

Resolves typed diagnostics, coverage, results, artifacts, and signals into one
stable command outcome, then renders human or machine output with strict stream
ownership.

- **Current evidence:** `src/outcome.rs`, `src/render.rs`, `src/signals.rs`,
  command-result schema.
- **State:** implemented.

### Contract System

Owns checked-in JSON Schemas, validates generated documents at runtime, and
preserves compatibility boundaries independently from internal types.

- **Current evidence:** `src/contracts.rs`, `schemas/`, CLI contract tests.
- **State:** implemented for current artifacts.

### Publication System

Composes the product landing page, generated architecture portal, Zensical
documentation, and canonical schema downloads into one collision-checked
static artifact.

- **Current evidence:** `web/landing/`, `web/architecture/`, `docs/`,
  `scripts/site/`.
- **State:** build and verification implemented; hosting and domain activation
  remain separate work.

### Transactional Execution System

Will re-prove plan preconditions, create temporary artifacts, validate outcomes,
commit atomically where supported, record every attempt, and provide the stated
recovery behavior.

- **State:** target system; deliberately absent from `v0.1.x`.

## Responsibilities and Capability Ownership

| Capability | Primary owner | OptiFlow responsibility |
| --- | --- | --- |
| Media evidence and relationship semantics | OptiFlow | Define, implement, version, and explain |
| CLI and artifact contracts | OptiFlow | Publish stable subprocess and evidence boundaries |
| Specialized media operations | External adapter project | Expose bounded capability; OptiFlow normalizes and records use |
| Suite orchestration | `flow` | Invoke released OptiFlow contracts without embedding source |
| Workstation shell behavior | Mantle | Provide a portable operator environment; never alter evidence semantics |
| Reproducible development and local runtime | Realm | Package environment and future OCI execution profile |
| Reusable delivery automation | Relay | Build, verify, sign, publish, deploy, and observe released artifacts |
| Repository quality policy | Egolint | Validate source, contracts, supply chain, and architecture conformance |
| Canonical architecture specifications and skills | Aether target | Define reusable document and agent contracts |
| Repository creation and update materialization | Holon target | Install pinned universal/profile artifacts and report drift |
| Organization and runtime intelligence | Observatory and Pace targets | Aggregate evidence and conformance without becoming source truth |

Target owners describe the intended ecosystem boundary and remain provisional
until their released interfaces are consumed here.

## System Boundaries

- Invocation resolves policy once; domain systems do not reopen configuration
  files or reread environment variables.
- Discovery observes paths; it does not classify relationships.
- Inventory normalizes observations; adapters do not select plan actions.
- Relationship derivation consumes evidence; it does not grant execution
  authority.
- Reporting preserves source evidence; planning proposes without mutation.
- Presentation renders typed outcomes; it does not infer status from messages.
- Infrastructure schedules and observes commands; it does not reinterpret
  schemas or product policy.

## Major Interactions and Runtime Flows

### Read-Only Scan Flow

1. Capture invocation selectors and resolve one effective policy.
2. Preflight explicit inputs and begin a durable run.
3. Discover paths under conservative traversal policy.
4. Observe filesystem and content facts; use optional media adapters.
5. Reuse cache entries only when their identity remains valid.
6. Narrow equal-size candidates and collect complete hashes.
7. Derive exact groups and coverage.
8. Validate and atomically commit policy, run, and report artifacts.
9. Resolve and render one typed command result.

### Review Plan Flow

1. Load and validate an immutable source run or report.
2. Retain its historical effective-policy evidence.
3. Derive deterministic review defaults and potential reclaimable bytes.
4. Attach size, modification-time, and complete-hash preconditions.
5. Commit a plan declaring `mutates_files: false`.

### Future Platform Execution Flow

1. Flow or infrastructure selects a released OptiFlow image or binary.
2. Realm or a cloud runtime supplies explicit input and state mounts.
3. Relay-defined automation invokes the same CLI contract used locally.
4. Versioned result and evidence artifacts are retained.
5. OpenTelemetry-compatible runtime telemetry may describe execution health,
   but immutable OptiFlow artifacts remain the domain evidence source.
6. Observatory projects operational and repository intelligence without
   rewriting product state.

## External System Relationships

- The operating system supplies filesystem and signal behavior.
- SQLite supplies local durable query state.
- `ffprobe` supplies optional media-container and stream descriptions.
- GitHub supplies source hosting, review, CI, releases, and future Pages
  deployment.
- OCI registries and container runtimes may distribute and run packaged
  OptiFlow workloads.
- Kubernetes, Nomad, cloud batch services, or local schedulers may invoke those
  workloads later; none is required by the domain.
- CNCF projects provide an evaluation landscape for delivery, orchestration,
  observability, security, networking, and storage capabilities. Their use is
  selected by the surrounding Ego Hygiene platform rather than imported into
  OptiFlow indiscriminately.

See [`docs/cloud-native-placement.md`](docs/cloud-native-placement.md) for the
capability-layer mapping.

## Assumptions and Evidence Gaps

- No released Flow integration currently proves end-to-end suite composition.
- The OCI packaging, signing, SBOM, and deployment path is not implemented.
- No shared-state or distributed-run contract is accepted.
- Runtime telemetry has not been standardized and must not include media bytes
  or paths by default.
- The transaction system remains deferred.

## Open Questions

- What is the smallest portable job contract for local, CI, and cloud batch
  execution?
- Which operational signals belong in telemetry versus immutable artifacts?
- Should remote workers exchange content, evidence, or only scheduled work
  references?

## Validation

- Governing specification: `architecture-system` version `1.1.0`.
- Every logical system has a distinct responsibility and implementation state.
- External platform capabilities remain outside product-domain ownership.
- Current and target interactions are clearly separated.
