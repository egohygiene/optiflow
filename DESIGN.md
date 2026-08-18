---
schema: aether.architecture-document/v1
id: optiflow-design
title: OptiFlow Design
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-08-18
governed_by:
  - architecture-design
depends_on:
  - optiflow-purpose
  - optiflow-vision
  - optiflow-principles
  - optiflow-personal-model
related:
  - optiflow-design-system
  - optiflow-ontology
  - optiflow-system
supersedes: []
---

# OptiFlow Design

## Design Philosophy

OptiFlow makes complex evidence feel calm without making it look simpler than
it is. The experience emphasizes orientation, truthfulness, progressive
disclosure, and meaningful control over spectacle or urgency.

## Intended Experience

An operator should be able to answer, in order:

1. What did OptiFlow attempt?
2. What did it observe and what could it not observe?
3. What relationship or outcome is being claimed?
4. Which evidence and policy support that claim?
5. What artifact was committed?
6. What is proposed next, and what authority does it not have?

Machine consumers receive the same semantics through typed results rather than
parsing human language.

## Experience Qualities

- **Calm:** no fear-driven cleanup language, artificial urgency, or destructive
  primary action.
- **Precise:** exact, candidate, partial, stale, and unknown retain canonical
  meanings.
- **Traceable:** important conclusions link to policy, run, artifact, and
  evidence identity.
- **Progressive:** the first view communicates outcome and safety; deeper views
  expose diagnostics and proof.
- **Consistent:** CLI, documentation, landing page, and future graphical clients
  use one domain vocabulary.
- **Portable:** human output remains useful in ordinary terminals and machine
  output remains stable in automation.

## Interaction Philosophy

Commands express explicit verbs and scopes. Defaults constrain traversal and
authority. A future consequential action requires a deliberate command,
approved plan identity, current precondition checks, and clear recovery mode.

The current CLI avoids an interactive terminal UI so results remain composable
and reproducible. A future review interface should consume the same released
artifacts instead of owning a parallel evidence model.

## Communication Philosophy

- Lead with the semantic outcome, not internal activity.
- Separate primary result, diagnostic, and progress channels.
- State coverage limitations next to affected conclusions.
- Prefer concrete evidence language over confidence theater.
- Never say **safe to delete** when the system has proved only byte identity.
- Describe unavailable features honestly; do not use disabled controls as a
  marketing promise.

## Accessibility Philosophy

Accessibility is part of evidence review safety. Interfaces use semantic
structure, keyboard operation, readable hierarchy, non-color status cues,
adequate contrast, reduced-motion support, lossless path representation, and
plain-language explanations of specialized terms.

Machine-readable output enables alternate presentations but does not excuse an
inaccessible default experience.

## Agency and Meaningful Control

- The operator chooses inputs and effective policy.
- Inspection commands expose resolved values, provenance, and shadows.
- Plans separate deterministic review defaults from quality or value judgments.
- Future mutation presents effect, preconditions, recovery, and refusal reasons
  before execution.
- Cancellation and interruption produce honest recoverable states.

## Cognitive Load

Summaries group repeated evidence while preserving drill-down paths. Stable
identifiers allow a reviewer to correlate CLI, report, plan, and automation
without copying long paths or guessing which run is current.

The design avoids presenting every available adapter, CNCF project, policy leaf,
or schema field in the primary path. Advanced capability is discoverable at the
point it becomes relevant.

## Trust, Feedback, and Recovery

Trust is built through visible invariants and accurate outcomes:

- read-only commands say what they wrote and where;
- progress never replaces final coverage;
- validation failures block artifact promotion;
- interruption never claims completion;
- future actions explain how recovery works and when it cannot be guaranteed;
- command exit status and structured outcome agree exactly.

## Aesthetic Direction

The current visual direction is dark, spacious, technical, and evidence-led:
near-black surfaces, restrained mint for verified or safe boundaries, blue for
active analysis, amber for review boundaries, and monospaced detail inside a
clear sans-serif hierarchy.

The visual metaphor is a measured evidence pipeline rather than a dashboard of
scores. Motion may communicate state transitions, but never conceal content or
create false activity.

## Design Anti-Goals

- Gamified deletion or reclaimed-byte competition.
- Fake customer proof, pricing, download availability, or production claims.
- Dense enterprise dashboards before the underlying operational need exists.
- Color-only status, uncontrolled animation, or terminal formatting that
  contaminates machine output.
- Cloud branding that makes local-first operation look secondary.
- AI language that suggests the system understands personal media value.

## Product Identity and Shared Commitments

The name is written `optiflow` in lowercase in product identity contexts and
`OptiFlow` in prose when conventional readability benefits. The emblem and
current landing theme are provisional implementation sources until the Identity
workflow promotes final canonical assets.

Shared Ego Hygiene aesthetics may create family resemblance, but product
vocabulary, evidence diagrams, and safety messaging remain OptiFlow-owned.

## Evidence and Assumptions

- CLI output and the landing shell provide the current implemented experience.
- No formal usability study or accessibility audit has been completed.
- The documentation information architecture is implemented with Zensical.
- Final identity, social imagery, and polished hero motion remain future work.

## Open Questions

- What report summary supports large collections without hiding rare failures?
- Which future review interactions belong in Flow versus an OptiFlow-specific
  client?
- How should perceptual evidence be visualized without implying a universal
  score?

## Downstream Implications

- [`DESIGN_SYSTEM.md`](DESIGN_SYSTEM.md) defines reusable visual and content
  semantics.
- CLI and schema changes preserve the information hierarchy above.
- Landing, documentation, terminal, and future clients share canonical status
  vocabulary.
- Identity generation consumes this direction but requires human promotion of
  canonical sources.

## Validation

- Governing specification: `architecture-design` version `2.0.0`.
- Experience, interaction, communication, accessibility, agency, trust, and
  anti-goals are explicit.
- Design remains grounded in current product authority and evidence language.
