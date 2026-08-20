---
title: Site publication architecture
description: The contract between the LaunchKit landing page, Zensical documentation, generated references, and release artifacts.
---

# Site publication architecture

The public product experience is one static deployment with independently
owned surfaces:

```text
optiflow.egohygiene.io/
├── /                  LaunchKit landing page
├── /architecture/     Generated architecture portal and graph
├── /docs/             Zensical documentation
├── /api/              Curated rustdoc reference
├── /schemas/          Versioned machine contracts
├── /intelligence/     Repository evidence dashboard
└── /releases/         Install and verification guidance
```

## Ownership contract

| Surface | Source | Responsibility |
| --- | --- | --- |
| `/` | `web/landing/` | Product narrative, visual demonstration, installation call to action, and links into deeper evidence |
| `/architecture/` | root architecture documents + `web/architecture/` | Generated system boundaries, structural layers, document graph, inventory, and machine-readable projection |
| `/docs/` | `docs/` + `zensical.toml` | Tutorials, concepts, operational guidance, architecture, and contract explanations |
| `/api/` | `cargo doc` | Curated Rust API only after the library stability decision is explicit |
| `/schemas/` | `schemas/` | Downloadable canonical JSON Schemas and representative examples |
| `/intelligence/` | generated evidence | CI, dependency, security, contract, and release health |
| `/releases/` | release metadata | Supported targets, checksums, provenance, SBOMs, and verification commands |

## Composition rule

Each producer builds into an isolated staging directory. A final composition
step assembles one `dist/` tree and rejects path collisions before GitHub Pages
receives the artifact. No producer writes into another producer's output.

```text
LaunchKit build ----------> staging/landing/
architecture generation --> staging/architecture/
Zensical build -----------> staging/docs/
rustdoc build ------------> staging/api/
schema publication -------> staging/schemas/
intelligence generation --> staging/intelligence/
release projection -------> staging/releases/
                                  |
                                  v
                               dist/
```

The implemented composer owns the landing, architecture, documentation,
repository-intelligence, and checked-in schema producers. It generates the
architecture portal from the 18 canonical root documents and repository-owned
presentation configuration, builds Zensical into `.site/staging/docs/`, copies each static
source into an isolated stage, accepts Relay's public-only dashboard bundle
from `.site/producers/intelligence/`, rejects pre-existing `architecture/`,
`docs/`, `intelligence/`, or `schemas/` mounts in the landing source, and only
then replaces `dist/` with the verified composition. A failed build never
publishes a partially composed tree.

Build and preview the checked-in surfaces locally:

```bash
task site:build
task site:serve
```

This local build intentionally omits the CI-generated Intelligence producer
and passes the verifier's explicit `--allow-missing-intelligence` waiver. Set
`INTELLIGENCE_SOURCE` to a generated directory inside `.site/producers/` to
exercise the published contract; every Pages build requires the complete
`/intelligence/` mount.

## Current scope

The repository now publishes the Zensical source, a LaunchKit-derived landing
shell, generated architecture portal, canonical schema downloads, and a
commit-scoped repository-intelligence dashboard through one deterministic
GitHub Pages artifact. Generated API reference, release projection, and final
identity or motion assets remain separate reviewable changes.

## Architecture projection rule

The `/architecture/` surface is a projection, not an additional source of
architecture truth. Its generator:

1. reads the complete `aether.architecture-document/v1` metadata set;
2. rejects missing documents, unresolved relationships, duplicate IDs, and
   dependency cycles required to render the graph;
3. combines that graph with product-specific group, layer, and boundary
   configuration;
4. emits deterministic HTML and `optiflow.architecture-portal.v1` JSON; and
5. exposes the configuration and output contracts under `/schemas/`.

This build-time integrity check does not establish the future organization-wide
repository conformance gate. Aether and Holon must define that installation,
migration, exception, and drift contract independently.

That sequencing keeps the documentation contract independently verifiable and
prevents the first visual pass from silently defining release or API promises.
