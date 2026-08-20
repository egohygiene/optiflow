# ADR-0001 — Use Zensical + LaunchKit-Derived Static Composition for the Product Site

- **Status:** Accepted
- **Date:** 2026-08-20
- **Issue:** #25

## Context

OptiFlow has now shipped a reproducible documentation and product-site architecture across merged PRs #17, #18, #20, and #34. Earlier audit/planning material referenced mdBook as a possible documentation direction, but the implemented repository no longer needs two competing documentation systems.

The current static deployment composes independently owned producers:

- LaunchKit-derived static landing source at `/`;
- Zensical documentation at `/docs/`;
- generated architecture projection at `/architecture/`;
- canonical schema downloads at `/schemas/`;
- Relay-produced Repository Intelligence at `/intelligence/`.

## Decision

1. **Zensical is the canonical OptiFlow documentation build engine.**
2. **mdBook is superseded for this repository** and must not be introduced as a parallel documentation source/build path without a new ADR and concrete requirement.
3. **LaunchKit is a design/source reference, not an application runtime dependency.** The checked-in landing output remains lightweight static HTML/CSS/JS and must not inherit unsupported template claims.
4. Each public-site producer builds into an isolated staging directory.
5. A final deterministic composer owns the `dist/` tree, rejects mount collisions, and publishes only a verified complete artifact.
6. The architecture portal is a generated projection; root architecture documents remain canonical.
7. Repository Intelligence is consumed from Relay through a pinned producer contract rather than copied into OptiFlow.
8. Product-site deployment does not grant or imply source-media mutation authority.

## Consequences

### Positive

- One documentation engine and one navigation hierarchy.
- The website can evolve independently from the Rust domain model while staying evidence-aligned.
- Each producer has a visible ownership boundary.
- Site output can be regenerated and verified deterministically.
- Later Holon/Identity/Relay reuse can extract generic composition behavior without making OptiFlow the organization template owner.

### Costs

- Contributors must update Zensical rather than older mdBook assumptions.
- Landing changes need to preserve reserved mounts and composition tests.
- Production domain/TLS/rollback remains blocked on organization-owned dependencies rather than being solved locally.

## Superseded direction

Any roadmap/audit item whose only remaining outcome is “introduce mdBook for OptiFlow documentation” is superseded by this decision. A useful requirement hidden inside such an item—search, navigation, API docs, versioning, accessibility, etc.—should be routed to the accepted stack rather than preserving mdBook as an implementation constraint.

## Evidence

- PR #17 — pinned Zensical documentation foundation.
- PR #18 — LaunchKit-derived landing and deterministic site composition.
- PR #20 — generated architecture portal.
- PR #34 — pinned Relay Repository Intelligence producer and Pages deployment workflow.
- `docs/site-publication.md` — current surface/composition contract.
