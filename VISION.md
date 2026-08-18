---
schema: aether.architecture-document/v1
id: optiflow-vision
title: OptiFlow Vision
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-08-18
governed_by:
  - architecture-vision
depends_on:
  - optiflow-purpose
related:
  - optiflow-principles
  - optiflow-pillars
  - optiflow-roadmap
supersedes: []
---

# OptiFlow Vision

## Vision Statement

Any media collection should be understandable through a portable evidence model
that runs consistently on a workstation, in a reproducible development
environment, or as an automated job—and no consequential change should occur
without current proof, explicit policy, validation, and an appropriate recovery
contract.

## Desired Future State

In the desired future:

- one command can inventory mixed image, audio, and video collections while
  reporting exactly what it could and could not observe;
- exact, perceptual, derived, and containment relationships are distinct claim
  types with detector-specific evidence;
- reports and plans remain reproducible through versioned schemas, effective
  policy identities, adapter provenance, and immutable source-run references;
- approved changes execute through durable transactions that re-prove every
  precondition, validate every result, and fail closed;
- the same binary and contracts work as a standalone CLI, a `flow` subprocess,
  a devcontainer capability, an OCI workload, or a scheduled infrastructure
  job without changing the product's semantic rules;
- local operation remains complete and private by default, while remote
  execution is an explicit deployment choice;
- operators can start with a narrow safe workflow and progressively adopt more
  capable profiles without inheriting unrelated platform complexity.

## Intended Impact

OptiFlow should reduce wasted storage and manual investigation while increasing
confidence in what each proposed decision means. Its larger impact is to make
storage optimization behave like a verifiable engineering process rather than
an opaque cleanup action.

Within Ego Hygiene, OptiFlow demonstrates how an independently useful tool can
participate in a broader platform through contracts instead of source coupling.

## Directional Signals

The product is moving toward the vision when:

- a claim can be traced from policy and observations through evidence, tests,
  reports, plans, and any later execution record;
- unknown, partial, stale, unavailable, and invalid states remain
  machine-distinguishable;
- clean installations behave consistently on supported macOS and Linux targets;
- containerized and cloud execution reuse the same CLI and artifact contracts;
- adapter additions do not leak shell behavior or tool-specific data into the
  domain model;
- users can understand why an action is proposed and what would make it unsafe;
- future mutation expands only after recovery and failure evidence are proved.

## Boundaries and Anti-Vision

OptiFlow is not intended to become:

- an automatic deletion daemon;
- a proprietary cloud service required to inspect local media;
- a universal quality oracle that hides detector assumptions;
- a media editor, catalog browser, or codec implementation;
- a Kubernetes operator masquerading as the product core;
- a platform bundle that forces every CNCF capability into a single-user tool;
- an AI system that infers the personal value of media from its content.

## Assumptions

- OCI packaging and cloud-native orchestration can remain deployment adapters
  around a portable application boundary.
- Stable contracts provide more durable integration than embedding OptiFlow as
  an unversioned library.
- The first safe implementation of a capability should be narrower than the
  final desired experience.

## Open Questions

- Which execution forms beyond workstation and devcontainer use have genuine
  demand?
- Should large distributed collections use one central evidence store or
  portable per-volume state projections?
- At what scale does remote or distributed analysis justify its additional
  privacy and coordination model?

## Validation

- Governing specification: `architecture-vision` version `2.0.0`.
- The desired state follows from [`PURPOSE.md`](PURPOSE.md) without prescribing
  one infrastructure vendor.
- Directional signals can evaluate roadmap and architecture proposals.
- Anti-vision statements prevent platform adoption from redefining the product.
