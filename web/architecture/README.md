# OptiFlow architecture portal source

This directory contains repository-owned presentation configuration and static
assets for the generated `/architecture/` surface.

The canonical architecture remains the 18 root Markdown documents inventoried
by [`META.md`](../../META.md). `architecture.config.json` groups those stable
document IDs and describes product-specific system layers and platform
boundaries. It does not copy or replace the canonical document metadata.

`scripts/site/generate_architecture.py` resolves the document graph and emits
deterministic HTML and JSON into the isolated architecture staging directory.
`scripts/site/build.sh` then composes that projection with the landing page,
documentation, and schemas.

This is an OptiFlow-local proof of the future Aether and Holon contract:

- Aether may eventually own the shared configuration and output schemas,
  generator behavior, and reusable presentation assets.
- Holon may install and update those inputs through visible, idempotent plans.
- Repository-owned descriptions, boundaries, and document contents remain
  local and must survive generator upgrades.

Generated files under `.site/` and `dist/` are disposable and are not committed.
