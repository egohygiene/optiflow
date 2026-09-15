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

## Delivery modes

The pinned `Publish site` workflow uses one build path for review and
production:

| Event | Artifact | Deployment |
| --- | --- | --- |
| Pull request | Compose, verify, and retain a reviewable `github-pages` artifact for seven days | Never |
| Push to `main` | Compose, verify, and retain the exact artifact | Deploy to the protected `github-pages` environment |
| Manual dispatch on `main` | Recompose and verify current `main` | Deploy to the protected `github-pages` environment |

The workflow refuses to configure or deploy Pages from any ref other than
`main`. Pull-request artifacts therefore exercise the production composer
without acquiring production publication authority.

## Production contract

The canonical production endpoint is
`https://optiflow.egohygiene.io/`. The product hostname is a DNS CNAME to
`egohygiene.github.io`; GitHub Pages terminates TLS and serves the artifact
created by `.github/workflows/pages.yml`. The default Pages URL redirects to
the canonical hostname, and plain HTTP redirects to HTTPS.

Every public HTML entry point must publish one canonical URL. The site verifier
enforces that contract for the landing, architecture, and documentation entry
points together with a non-empty description, English language declaration,
viewport metadata, one main landmark, one primary heading, and one skip link.
It also requires reduced-motion overrides for both hand-authored visual
surfaces.

After every production deployment, verify the public route and its critical
mounts without disabling certificate checks:

```bash
curl --fail --silent --show-error --location \
  --output /dev/null \
  --write-out "url=%{url_effective} status=%{http_code} tls=%{ssl_verify_result}\n" \
  "http://optiflow.egohygiene.io/"

for route in / /architecture/ /docs/ /intelligence/ \
  /schemas/config-v1.schema.json; do
  curl --fail --silent --show-error --location \
    --output /dev/null \
    --write-out "$route status=%{http_code} start-transfer=%{time_starttransfer}s bytes=%{size_download}\n" \
    "https://optiflow.egohygiene.io$route"
done

curl --fail --silent --show-error \
  "https://optiflow.egohygiene.io/" \
  | rg '<link rel="canonical" href="https://optiflow.egohygiene.io/">'
```

For a release review, also inspect the landing and architecture surfaces at
320, 768, and 1440 CSS pixels; traverse navigation and controls by keyboard;
confirm the skip link receives focus; and repeat with reduced motion enabled.
Record the workflow run, deployed commit, DNS answer, redirect chain, response
timings, and review result on the delivery issue.

## Rollback

Production is commit-derived, so the durable rollback is a reviewed revert on
`main`:

1. Identify the last known-good `Publish site` run and its full commit SHA.
2. Revert the faulty merge in a pull request; do not patch generated `dist/`
   output or mutate a deployed artifact.
3. Merge the revert after its pull-request artifact passes verification.
4. Confirm that the resulting `main` run deploys successfully, then repeat the
   production checks above and attach the run and commit to the incident or
   delivery issue.

If the source is already correct but a deployment needs to be replayed, run
`Publish site` manually from `main`. A DNS or certificate failure is outside
this repository's authority: the organization-domain owner restores the CNAME
and Pages domain settings under ORG-03, while OptiFlow maintainers leave the
last known-good artifact intact.

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
