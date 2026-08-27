# Identity v1 consumer integration

OptiFlow is the technical, evidence-oriented Identity v1 pilot. It inherits the
same pinned organization defaults as Empathy and overrides only its primary
accent plus its action alias. Its selected profiles deliberately omit `web`,
`pwa`, and `social`: OptiFlow currently ships a local CLI and repository
surfaces rather than a deployed application shell.

## Immutable package contract

Identity is consumed through an exact reviewed commit recorded in
`.config/identity/consumer-lock.json`. A consumer uses only the published v1
validator and generated `assets/identity/` packages; it never imports or copies
Identity implementation modules. The compiler manifest records the exact
selected profile build and package checksums record portable entries.

## Check, upgrade, and rollback

```bash
task identity:v1:check
task identity:v1:verify
```

`verify` is read-only and fails on missing, stale, or drifted generated state.
After reviewing a compatible immutable Identity update or a source change, run
`task identity:v1:generate` followed by `task identity:v1:verify`, then commit
the source, gitlink, lock, manifest, and package together.

Rollback restores the preceding reviewed consumer revision (or its matching
gitlink, lock, and generated output set). It never reconstructs or hand-edits
canonical source assets. Incompatible profile versions, unapproved overrides,
bad source digests, and compiler drift all fail with stable Identity diagnostics
and an explicit recovery path.
