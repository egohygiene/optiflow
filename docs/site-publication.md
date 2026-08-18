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
Zensical build -----------> staging/docs/
rustdoc build ------------> staging/api/
schema publication -------> staging/schemas/
intelligence generation --> staging/intelligence/
release projection -------> staging/releases/
                                  |
                                  v
                               dist/
```

The implemented composer currently owns the landing, documentation, and
checked-in schema producers. It builds Zensical into `.site/staging/docs/`,
copies each static source into an isolated stage, rejects pre-existing `docs/`
or `schemas/` mounts in the landing source, and only then replaces `dist/` with
the verified composition. A failed build never publishes a partially composed
tree.

Run the same contract used by CI:

```bash
task site:build
task site:serve
```

## Current scope

The repository now establishes the Zensical source, a LaunchKit-derived landing
shell, canonical schema downloads, and their deterministic composition
contract. Generated API reference, repository intelligence, GitHub Pages
deployment, custom-domain activation, and final identity or motion assets
remain separate reviewable changes.

That sequencing keeps the documentation contract independently verifiable and
prevents the first visual pass from silently defining release or API promises.
