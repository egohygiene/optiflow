---
schema: aether.architecture-document/v1
id: optiflow-ontology
title: OptiFlow Ontology
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-08-18
governed_by:
  - architecture-ontology
depends_on:
  - optiflow-purpose
  - optiflow-vision
  - optiflow-principles
  - optiflow-epistemology
related:
  - optiflow-personal-model
  - optiflow-system
  - optiflow-architecture
supersedes: []
---

# OptiFlow Ontology

## Domain Scope

OptiFlow models filesystem observations, media descriptions, evidence-backed
content relationships, review plans, and the policy and provenance required to
interpret them.

## Domain Boundaries

The domain begins with explicitly selected filesystem inputs and ends, in the
current release, with immutable reports, plans, and command outcomes. Editors,
catalogs, codecs, backup systems, cloud storage, and orchestration platforms are
external systems.

Future execution entities belong to the domain only after the transactional
apply boundary is specified and implemented.

## Canonical Concepts

| Concept | Meaning |
| --- | --- |
| Input root | A caller-selected file or directory at which discovery begins. |
| Effective policy | Fully materialized values, provenance, locked invariants, and semantic fingerprints for one invocation. |
| Scan run | Immutable identity and lifecycle of one bounded observation attempt. |
| Path observation | Facts observed for content at one filesystem location during a run. |
| Filesystem identity hint | Platform facts that help detect aliasing or change but do not replace content evidence. |
| Media descriptor | Normalized container, stream, format, and media facts reported by an adapter. |
| Content evidence | Size, complete hash, or future detector output used to evaluate a relationship. |
| Coverage | Whether the requested scope was completely, partially, or not meaningfully observed. |
| Diagnostic | Typed information about a condition, classification, impact, and context. |
| Candidate set | Items narrowed for additional evidence collection; not a proven relationship. |
| Exact duplicate group | Two or more observations with equal size and complete content hash in one run. |
| Report | Immutable presentation of a source run, observations, evidence, relationships, and aggregate outcomes. |
| Plan | Immutable review artifact that proposes actions and preconditions without authorizing execution. |
| Plan action | One proposed operation over identified evidence and candidate paths. |
| File precondition | A fact that a future execution must re-prove against current state. |
| Command result | Versioned process outcome containing result, coverage, diagnostics, artifacts, and exact exit status. |
| Capability | A named, discoverable function supplied by OptiFlow or an external adapter. |
| Artifact | A schema-versioned, durably committed document produced by a command. |

Future concepts include `ExecutionRun`, `Attempt`, `GeneratedArtifact`,
`ValidationResult`, `CommitRecord`, and `RecoveryRecord`. Their names do not
imply that the current binary implements them.

## Relationship Model

```text
effective policy ──governs──> scan run
input root ──is traversed by──> scan run
scan run ──records──> path observation
path observation ──may include──> media descriptor
path observation ──contributes──> content evidence
content evidence ──supports──> exact duplicate group
scan run ──is projected as──> report
report ──may inform──> plan
plan ──contains──> plan action
plan action ──requires──> file precondition
command ──returns──> command result
command result ──references──> artifact
```

Reports and plans refer to immutable source identities. A later execution must
create new current observations rather than treating historical relationships
as current facts.

## Ubiquitous Language

- Use **exact duplicate group** when byte identity has been proved for a run.
- Use **candidate** when more evidence is required.
- Use **retained candidate** or `keep_path` for the deterministic review default;
  it is not a quality judgment.
- Use **partial result** when a valid artifact exists with reduced coverage.
- Use **unavailable capability** when required functionality cannot be invoked.
- Use **stale state** when supplied evidence no longer satisfies a command's
  prerequisites.
- Use **source media** for files under observation or future action authority.
- Use **state** for queryable local operational data and **artifact** for
  versioned interchange evidence.

## Aliases and Deprecated Terms

- Avoid **duplicate file** without the exact, candidate, or perceptual qualifier.
- Avoid **best copy**; OptiFlow does not currently score personal or creative
  value.
- Avoid **safe to delete**; exactness alone does not establish authorization or
  recovery policy.
- Avoid **optimizer** when describing `v0.1.x` behavior; it inventories and
  plans but does not transform source media.
- Avoid **success with warnings** when typed coverage and diagnostic impact can
  express the actual outcome.

## Conceptual Invariants

- A path is an observation location, not content identity.
- A plan is not an execution record or permission token.
- Equal metadata is not exact content evidence.
- Historical evidence remains historical after current state changes.
- Adapter output is normalized before crossing into domain relationships.
- Every machine document identifies its schema.

## Open Questions

- When should content identity become independent from a path-based cache key?
- What canonical concepts are needed for paired assets and compound media?
- Should policy profiles be domain entities or external configuration packages?

## Migration Notes

The current v1-v3 run, report, and plan artifacts remain authoritative. This
ontology names their shared concepts but does not rewrite historical schemas.
Future incompatible concepts require new schema identifiers.

## Validation

- Governing specification: `architecture-ontology` version `2.0.0`.
- Concepts have distinct ownership and avoid implementation-only types.
- Relationship language aligns with current schemas and safety documentation.
- Future concepts are explicitly identified as unimplemented.
