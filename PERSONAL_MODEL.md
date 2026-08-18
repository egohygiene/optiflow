---
schema: aether.architecture-document/v1
id: optiflow-personal-model
title: OptiFlow Personal Model
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-08-18
governed_by:
  - architecture-personal-model
depends_on:
  - optiflow-purpose
  - optiflow-vision
  - optiflow-principles
  - optiflow-epistemology
  - optiflow-ontology
related:
  - optiflow-ai-constitution
  - optiflow-design
  - optiflow-design-system
supersedes: []
---

# OptiFlow Personal Model

## Introduction

OptiFlow affects files that may carry personal history, creative labor, legal
obligations, or irreplaceable context. This document defines the deliberately
limited model of people used when designing product authority and experience.
It is not a user profile schema.

## Human Assumptions

- People have limited attention and may review plans under time pressure.
- Technical expertise, risk tolerance, accessibility needs, and storage goals
  vary.
- A person may not remember why a file exists or whether another system depends
  on it.
- Deterministic automation can still be wrong because its inputs, policy, or
  model were incomplete.
- The person running a command may not be the only person affected by its
  result.

## Person and Representation

The architecture recognizes roles, not inferred identities:

| Role | Responsibility |
| --- | --- |
| Operator | Selects inputs, policy, and command authority. |
| Reviewer | Evaluates evidence and proposed actions. |
| Automation caller | Invokes versioned contracts and handles every outcome. |
| Orchestrator | Composes OptiFlow with other independently released tools. |
| Affected owner | Has a legitimate interest in source media or resulting decisions. |
| Maintainer | Evolves implementation and contracts under repository governance. |

One person may occupy several roles. Possession of a filesystem path does not
prove sole ownership or consent from every affected person.

## Agency and Autonomy

People retain authority over consequential source-media changes. Defaults may
reduce effort but must not erase the distinction between a recommendation and
approval. Future unattended execution requires explicit policy, bounded scope,
current preconditions, and a stated recovery contract.

## Identity and Self-Description

OptiFlow does not need demographic, psychological, or social identity to
inventory bytes. User-facing text should avoid assumptions about profession,
ability, family, ownership structure, or why media matters.

## Context and Relationships

Media may participate in project files, sidecar sets, Live Photos, RAW pairs,
editing timelines, legal records, backups, and relationships not visible to
content analysis. The system treats absent context as unknown rather than
evidence that a file is disposable.

## Needs, Intentions, and Motivations

Likely goals include understanding a collection, reducing storage, finding
exact copies, preparing an archive, improving delivery formats, or integrating
evidence into another workflow. The system asks for or receives explicit policy
instead of inferring a person's goal from the collection.

## Growth and Change

Preferences, tools, and collection meaning change. Immutable artifacts record
what was known under a policy at a time; they do not freeze a person's future
intent.

## Consent

- Scanning authority is not future mutation authority.
- One approved action is not approval for an entire group or later run.
- Cloud upload, remote inference, or external sharing is opt-in and outside the
  local-first default.
- Automation must retain a review boundary appropriate to its action class.

## Privacy and Inference Boundaries

OptiFlow minimizes collection to operational and evidence needs. It does not
analyze faces, relationships, emotions, location histories, or content meaning
unless a future explicitly selected capability defines a privacy model for that
purpose. Media bytes and paths are not telemetry by default.

## Correction and Contestability

Users can inspect policies, provenance, diagnostics, reports, plans, and
preconditions. A contested claim should be reproducible or invalidated through
new evidence. Human disagreement with a proposed disposition stops the action;
the system does not reinterpret disagreement as noise.

## Limits of the Model

This model cannot establish legal ownership, copyright, emotional value,
archival importance, or informed consent for other affected people. Product
documentation must not imply otherwise.

## Architectural Implications

- Read-only operation is the initial universal authority.
- Human-readable and machine-readable evidence are both required.
- Plans expose why, limitations, potential effect, and future preconditions.
- Destructive defaults require stronger review and recovery than reversible
  actions.
- Telemetry and remote execution remain explicit capability choices.
- Accessibility and low cognitive load are safety characteristics.

## Assumptions and Open Questions

- How should multi-owner review be represented without turning OptiFlow into a
  collaboration platform?
- Which plan summaries best support informed review of large collections?
- What consent boundary should apply to organization-managed unattended jobs?

## Validation

- Governing specification: `architecture-personal-model` version `2.0.0`.
- The model is minimal, role-based, contestable, and privacy-preserving.
- It does not infer personal worth or media value from content evidence.
- Architectural implications are traceable to product behavior.
