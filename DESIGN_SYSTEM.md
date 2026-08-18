---
schema: aether.architecture-document/v1
id: optiflow-design-system
title: OptiFlow Design System
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-18
updated: 2026-08-18
governed_by:
  - architecture-design-system
depends_on:
  - optiflow-personal-model
  - optiflow-design
related:
  - optiflow-ontology
  - optiflow-architecture
supersedes: []
---

# OptiFlow Design System

## Purpose and Scope

This document defines semantic design roles shared by the landing page,
documentation, terminal output, diagrams, and future review clients. It does not
freeze final brand assets or require one frontend framework.

## Relationship to DESIGN.md

[`DESIGN.md`](DESIGN.md) owns the intended experience and aesthetic direction.
This document translates that direction into reusable language, tokens,
patterns, states, and handoff rules.

## Design-Language Foundations

- Evidence has hierarchy: outcome, coverage, claim, support, provenance, detail.
- Safe boundaries feel stable, not celebratory.
- Review states are visible without appearing failed.
- Unknown and unavailable have explicit neutral treatment.
- Dense evidence is grouped into inspectable structures rather than hidden.

## Semantic Roles

| Role | Meaning | Typical use |
| --- | --- | --- |
| Canvas | Primary quiet background | Page or terminal surface |
| Surface | Grouped evidence region | Card, panel, code block |
| Primary text | Current conclusion or heading | Titles and essential values |
| Secondary text | Explanation | Body and descriptions |
| Muted text | Supporting provenance | IDs, timestamps, auxiliary detail |
| Verified | Completed evidence or preserved safety invariant | Exact proof, unchanged source state |
| Active | Work currently being attempted | Analysis progress |
| Review | Human decision boundary | Proposal, warning with no failure |
| Partial | Valid result with reduced coverage | Coverage summary |
| Blocking | Invalid, stale, or unsafe continuation | Errors and refusals |
| Unknown | Evidence not established | Missing descriptor or unsupported capability |

Semantic roles are not tied permanently to one color value.

## Typography

- Use a readable system sans-serif stack for narrative and interface text.
- Use a system monospace stack for commands, paths, schema identifiers,
  fingerprints, evidence values, and stage labels.
- Headings use tight but legible spacing and avoid all-caps paragraphs.
- Terminal output respects user environment and does not require a bundled font.

The landing implementation currently uses system fonts to avoid a remote font
dependency.

## Color and Contrast

The current dark theme starts from these implementation values:

| Token | Current value | Semantic role |
| --- | --- | --- |
| `--background` | `#080b10` | Canvas |
| `--surface` | `#111720` | Surface |
| `--text` | `#f4f7f6` | Primary text |
| `--text-secondary` | `#aab4bd` | Secondary text |
| `--mint` | `#7ce7c5` | Verified and safe boundary |
| `--blue` | `#73a7ff` | Active analysis |
| `--amber` | `#f4c66c` | Review boundary |

Status never relies on color alone. Text labels, icons, position, border, or
pattern carry the same meaning. New combinations target WCAG 2.2 AA contrast
for ordinary text.

## Spacing and Density

- Base spacing follows a 4-pixel rhythm with common steps at 8, 12, 16, 24,
  32, 48, 64, and 96 pixels.
- Narrative surfaces use generous whitespace; evidence tables may be denser but
  retain clear row and group boundaries.
- Small screens reduce margins before reducing readable font size.
- Terminal density is controlled through summaries and explicit detail commands,
  not horizontal clipping.

## Shape, Surface, Border, and Elevation

- Corners are moderately rounded on web surfaces and absent from semantic text
  output.
- Thin low-contrast borders define evidence groups.
- Elevation distinguishes navigation or a focused evidence window, not every
  card.
- Dashed or patterned boundaries may represent review or deferred authority.

## Iconography and Imagery

- Icons supplement labels and carry accessible names where interactive.
- The current emblem represents two evidence paths meeting at a proved center.
- Architecture imagery favors pipeline, boundary, and provenance diagrams over
  generic cloud art.
- Generated or AI-authored imagery is a candidate until explicitly promoted by
  the Identity source contract.

## Motion and Transition

- Motion explains state transition or focus; it is never required to understand
  content.
- Loops are restrained and avoid suggesting analysis when no work is running.
- Interactive transitions are short and interruptible.
- `prefers-reduced-motion` disables non-essential animation.
- Terminal progress remains separate from final output and is absent from JSON
  stdout.

## Interaction States

Interactive controls support default, hover, focus-visible, active, disabled,
busy, success, and error states where applicable. Focus is never removed
without an accessible replacement. Disabled future capability includes a reason
or is omitted rather than presented as a dead promise.

## Feedback, Errors, and Recovery

Feedback pairs state with consequence and next action:

```text
what happened -> affected scope -> evidence/diagnostic -> safe next step
```

Blocking errors do not erase valid artifacts already committed. Partial results
show what remains usable. Future recovery messages state the actual guarantee
instead of generic reassurance.

## Content and Voice Patterns

- Product identity uses lowercase `optiflow`; prose may use `OptiFlow`.
- Prefer **observe**, **prove**, **report**, **plan**, **review**, **authorize**,
  **validate**, and **recover**.
- Avoid **clean automatically**, **safe to delete**, **best file**, **AI knows**,
  and unsupported production claims.
- Buttons use direct verbs such as **Get started**, **Open the guide**, and
  **View source**.
- Diagnostics use stable codes for machines and concise explanations for people.

## Accessibility Requirements

- Semantic headings and landmarks.
- Keyboard access and visible focus.
- Skip navigation on long web pages.
- Accessible names for controls and meaningful images.
- Non-color status indicators.
- Reduced motion support.
- Reflow at narrow widths without loss of information.
- Lossless machine path representation and escaped human terminal display.
- SVG diagrams include title and description when generated as standalone
  artifacts.

## Responsive and Cross-Platform Behavior

Web surfaces support phone through desktop widths. Terminal output degrades to
plain text and never requires Unicode symbols for semantic understanding.
Commands and examples use macOS/Linux-portable syntax within the supported
product boundary.

## Product Identity, Themes, and Variation

The current dark theme is the implemented baseline, not a prohibition on an
accessible light theme. Identity variations preserve the emblem geometry,
lowercase name, evidence-led meaning, and semantic status roles.

Ego Hygiene shared tokens may be consumed after their canonical ownership and
versioning are defined. OptiFlow retains product-specific extensions.

## Governance and Contribution

- Semantic role changes require design and accessibility review.
- Token changes update every implemented surface or document intentional
  divergence.
- Product identity candidates require explicit human approval before promotion.
- New components demonstrate keyboard, contrast, reduced-motion, and responsive
  behavior.

## Deprecation and Migration

Deprecated tokens or patterns remain aliased for a bounded migration when
consumed by more than one released surface. Final removal is recorded in the
changelog. Machine contract semantics never depend on visual tokens.

## Implementation Handoff

- Landing tokens and components: `web/landing/assets/site.css`.
- Landing behavior: `web/landing/assets/site.js`.
- Documentation overrides: `docs/stylesheets/extra.css`.
- CLI semantics: `src/render.rs` and `docs/cli-contract.md`.
- Canonical product language: [`ONTOLOGY.md`](ONTOLOGY.md).
- Final identity sources: future `.identity/` contract and Identity workflow.

## Coverage Gaps and Open Questions

- No automated accessibility or visual-regression gate is installed.
- Final favicon, social image, and full identity package are not promoted.
- Light-theme and graphical review-client tokens are not implemented.
- Terminal color policy is not public configuration in v1.

## Validation

- Governing specification: `architecture-design-system` version `2.0.0`.
- Semantic roles, accessibility, content, responsive behavior, governance, and
  implementation sources are explicit.
- Current tokens are recorded without misrepresenting provisional identity as
  final.
