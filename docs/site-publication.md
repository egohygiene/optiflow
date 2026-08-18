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

## Current scope

This repository pass establishes the Zensical source, navigation, brand layer,
and strict documentation build. LaunchKit import and customization, the final
composition script, GitHub Pages deployment, and custom-domain activation are
separate reviewable changes.

That sequencing keeps the documentation contract independently verifiable and
prevents the first visual pass from silently defining release or API promises.

