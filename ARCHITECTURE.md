---
schema: aether.architecture-document/v1
id: optiflow-architecture
title: OptiFlow Architecture
kind: architecture-document
version: 0.1.2
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-09-15
governed_by:
  - architecture-architecture
depends_on:
  - optiflow-foundations
  - optiflow-system
related:
  - optiflow-ontology
  - optiflow-methodology
  - optiflow-decisions
supersedes: []
---

# OptiFlow Architecture

## Purpose and Scope

This document defines how OptiFlow's logical systems are structurally organized,
how dependencies may cross boundaries, and how the product participates in the
larger Ego Hygiene platform. [`SYSTEM.md`](SYSTEM.md) owns the logical system
inventory; this document owns structural rules.

The current execution boundary converts immutable filesystem observations into
evidence-backed relationships, media-profile review opportunities, reports,
and review-only plans. Mutation, transactional
replacement, validation, quarantine, and recovery are future structural units
and must not be simulated inside the read-only pipeline.

## Structural Units or Layers

### Interface Layer

Owns CLI syntax, human and machine rendering, stream behavior, and operating
system exit status.

- Modules: `cli`, `render`, the binary entry point.
- Accepts and emits versioned application values.
- Does not derive relationship evidence or infer process outcome from text.

### Application Layer

Coordinates use cases, resolves one typed command outcome, handles cooperative
interruption, and defines artifact-commit boundaries.

- Modules: the `app` dispatcher and its `scan`, `reporting`, and
  `extension_commands` use-case modules; `outcome`; `signals`; and
  `configuration::resolver`.
- Receives explicit ports and fully materialized policy.
- Does not contain adapter parsing or filesystem-specific mutation.

### Domain Layer

Owns canonical concepts, evidence rules, effective policy values, inventory
classification, exact-group derivation, and plan semantics.

- Modules: `domain`, `configuration::policy`, `inventory`, `duplicates`,
  `media_profiles`, `planning`.
- Depends on data and behavior it owns rather than concrete external tools.
- Contains no shell, terminal, GitHub, container, or cloud-provider semantics.

### Evidence and Contract Layer

Owns complete hashing, checked-in machine contracts, runtime validation, and
the stable boundary between domain values and persisted or subprocess-visible
documents.

- Modules: `hashing`, `contracts`; files: `schemas/`.
- Schema identity is independent from the Rust crate version.
- Validation occurs before an artifact is reported as committed.

### Infrastructure Layer

Implements operating-system, SQLite, filesystem, external-adapter, and
artifact-set publication ports.

- Modules: `artifact_set`, `filesystem`, `state`, `adapters::ffprobe`.
- Migrations live in `migrations/` and move forward explicitly.
- External output is treated as untrusted until parsed and normalized.

### Publication Layer

Builds separate static product surfaces from `web/landing/`,
`web/architecture/`, `docs/`, and `schemas/`. It describes the application but
is not part of the CLI runtime.

- Build boundary: `scripts/site/build.sh`.
- Architecture projection: `scripts/site/generate_architecture.py` reads this
  corpus and repository-owned portal configuration without becoming a new
  architecture authority.
- Verification boundary: `scripts/site/verify.py` and strict Zensical build.
- Output: one collision-checked `dist/` tree.

## Runtime and Data Flow

```mermaid
flowchart TD
    caller[Person / flow / automation] --> cli[CLI interface]
    cli --> policy[Effective policy]
    policy --> discovery[Conservative discovery]
    discovery --> inventory[Content and media inventory]
    inventory --> state[(SQLite observations)]
    inventory --> evidence[Complete content evidence]
    evidence --> groups[Exact relationship groups]
    groups --> report[Immutable report]
    inventory --> profiles[Media-profile evidence]
    profiles --> report
    report --> plan[Review-only plan]
    plan --> outcome[Typed command result]
    outcome --> caller
    ffprobe[ffprobe adapter] -. normalized facts .-> inventory
    schemas[Checked-in schemas] -. validate .-> policy
    schemas -. validate .-> profiles
    schemas -. validate .-> report
    schemas -. validate .-> plan
    schemas -. validate .-> outcome
```

The `v0.1.x` authority boundary ends after plan generation. A future execution
flow begins by loading historical evidence and then creating new current
observations; it never treats the diagram's final plan node as direct write
permission.

## Component Map

| Component | Layer | Responsibility |
| --- | --- | --- |
| `cli` | Interface | Parse commands, output mode, selectors, and traversal overrides |
| `configuration::resolver` | Application | Select, snapshot, merge, validate, and explain configuration |
| `configuration::policy` | Domain | Hold values, provenance, locked invariants, and fingerprints |
| `app` | Application | Resolve configuration once and dispatch typed command use cases |
| `app::scan` | Application | Coordinate discovery, observation, exact evidence, persistence, and atomic scan publication |
| `app::reporting` | Application | Load compatible evidence and coordinate report and review-plan publication |
| `app::extension_commands` | Application | Coordinate explicit extension catalog inspection and typed availability outcomes |
| `outcome` | Application | Resolve diagnostics, coverage, artifacts, and exit semantics |
| `signals` | Application | Capture cooperative interruption without terminating library code |
| `render` | Interface | Enforce stdout/stderr ownership and buffered JSON rendering |
| `discovery` | Application/infrastructure boundary | Traverse explicit inputs under conservative policy |
| `inventory` | Domain | Normalize filesystem and media observations |
| `adapters::ffprobe` | Infrastructure | Bind one exact executable identity, invoke it from the read-only handle, and validate bounded evidence |
| `media_profiles` | Domain | Derive versioned, deterministic review opportunities and honest scoped coverage from accepted observations |
| `extensions` | Application/contract boundary | Validate explicit provider declarations and locks, resolve precedence, register typed embedded roles, and reject unsafe contributions |
| `extensions::process` | Infrastructure | Invoke one absolute, byte-pinned trusted provider with bounded JSON stdio and no inherited environment |
| `hashing` | Evidence | Stream complete BLAKE3-256 hashes with cancellation checkpoints |
| `duplicates` | Domain | Derive exact groups from equal size and complete hash evidence |
| `state` | Infrastructure | Persist lifecycle, observations, groups, and cache in SQLite |
| `planning` | Domain | Create non-mutating actions and future preconditions |
| `artifact_set` | Infrastructure | Stage, seal, publish, inspect, and recover related JSON artifacts |
| `reports` | Interface | Render typed reports and plans for human or JSON output |
| `contracts` | Evidence | Compile and enforce checked-in JSON Schemas at runtime |

## Boundary Rules

1. Configuration is resolved once before domain execution. Domain modules do
   not reopen configuration files or reread environment variables.
2. Source media is opened read-only. No source-write port exists in the current
   runtime.
3. External tools are invoked without a shell, with bounded arguments and
   normalized results.
4. Path observations remain distinct from content identity.
5. Cache reuse is explicit and must acquire missing evidence when a new claim
   requires it.
6. A report is accepted as source evidence only when its artifact set is
   committed, or when its historical contract predates artifact-set markers.
7. A plan is a new immutable projection and declares that it does not mutate
   files.
8. Renderers receive typed outcomes; message text never controls exit status.
9. Infrastructure orchestration consumes released CLI and artifact contracts;
   it does not embed default-branch source.
10. Publication producers build in isolation and reject final-path collisions.
11. Generated architecture projections remain disposable; canonical meaning
    changes only in the root architecture documents or their declared local
    presentation configuration.
12. Extension manifests only declare. Operator locks grant trust, effects,
    precedence, configuration, and fallback for exact provider bytes.
13. Extension results are proposed evidence. They cannot publish artifacts,
    execute plans, mutate source media, or bypass core revalidation,
    authorization, hashing, byte confirmation, provenance, and recovery.
14. Built-in media profiles consume accepted observations only. Missing,
    stale, partial, unavailable, or invalid provider evidence produces no
    opportunity.
15. Provider success is necessary but insufficient: the exact executable
    identity, bounded output, empty error stream, JSON shape, and expected
    semantic observations must all validate.

## Dependency Direction

```text
interface
    |
    v
application -----> evidence/contracts
    |
    v
domain <--------- normalized ports
    ^
    |
infrastructure implementations
```

Higher-level policy and domain language must not depend on GitHub Actions,
Zensical, Docker, Kubernetes, cloud SDKs, SQLite row layouts, or raw `ffprobe`
JSON. Infrastructure implements the boundaries required by application and
domain use cases.

The command coordinator is split by proven scan, report/plan, and extension
use cases. The wider module graph still predates a formal compile-time boundary
linter; new changes should move toward the declared direction without creating
indirection that has no demonstrated boundary value.

## Communication Patterns

- In-process components exchange typed Rust values.
- External adapters exchange bounded subprocess arguments and structured
  output.
- Embedded extensions register role-specific Rust traits; trusted process
  extensions exchange closed, bounded, independently versioned JSON documents.
- Local persistence uses transactional SQLite plus immutable JSON artifacts.
- Media-profile evidence binds profile configuration, evidence policy, native
  path facts, normalized observations, and exact provider identity.
- Automation exchanges `optiflow.command-result.v1` and referenced artifacts.
- `flow` invokes the binary as a subprocess and branches on complete schema
  identifiers and exit codes.
- Future telemetry describes runtime health through a separate observability
  port and does not replace domain artifacts.

## Deployment Topology

### Current standalone topology

```text
optiflow process
├── read-only source paths
├── optional ffprobe child process
├── local SQLite state
└── immutable artifact directory
```

### Reproducible environment topology

Realm may package the same process and its declared adapters in a devcontainer
or OCI image. Source and state enter through explicit mounts. The container
does not receive broader filesystem or network access by default.

### Future scheduled topology

Relay and infrastructure code may run the OCI artifact as a local, CI, cluster,
or cloud batch job. The job remains a contract consumer. Kubernetes, serverless
containers, and provider batch systems are interchangeable scheduling adapters
only where their storage, identity, interruption, and artifact-retention
semantics satisfy OptiFlow requirements.

## Significant Constraints

- SQLite rollback journal and `FULL` synchronization are used rather than
  assuming WAL behavior on arbitrary scanned filesystems.
- Related scan artifacts are validated, written and synchronized in a sibling
  staging directory, sealed by a content-addressed marker, and published by one
  directory rename. Plans use a recoverable file-and-marker handshake to retain
  their public file-path contract.
- Handled `SIGINT` and `SIGTERM` never finalize an active run as completed.
- Full hashes are calculated only when a candidate relationship requires them
  in the current MVP.
- The built-in PNG profile creates no output and makes no output-size, logical-
  savings, or physical-savings estimate.
- Direct byte confirmation remains deferred to the future destructive gate.
- The extension SDK grants no mutation, destructive, signing, or publication
  effect. Process trust is explicit but does not claim operating-system
  sandboxing; unknown or hostile binaries require external containment.
- OCI or cloud packaging must not introduce implicit source upload or telemetry
  containing paths or media content.

## Relationship to System Inventory

The Invocation, Discovery, Inventory, Relationship, Media Profile Analysis,
State, Reporting, Presentation, Contract, and Publication systems in
[`SYSTEM.md`](SYSTEM.md) map onto the layers above. The future Transactional
Execution System will add
explicit execution-domain and transaction-infrastructure units instead of
expanding `planning` into an implicit apply engine.

## Assumptions and Evidence Gaps

- Architecture boundaries are documented but not yet enforced by a dedicated
  dependency rule.
- The project has not published a signed OCI image or proved a remote topology.
- Shared-state concurrency and distributed trust are unspecified.
- Final visual identity assets and runtime telemetry contracts are not frozen.
- The architecture portal is a product-local proof; shared generator ownership,
  installation, and update semantics remain future Aether and Holon concerns.

## Open Questions

- Which ports should become explicit traits before transactional execution is
  added?
- Should artifact storage remain filesystem-only or gain a content-addressed
  abstraction for remote jobs?
- What sandbox boundary is appropriate for hostile or malformed media adapters?

## Validation

- Governing specification: `architecture-architecture` version `1.1.0`.
- Structural layers, direction, boundaries, communication, constraints, and
  deployment forms are explicit.
- The document reflects the current module and data model without claiming that
  future transaction or cloud systems exist.
- Product-domain code remains independent from a specific cloud-native stack.
