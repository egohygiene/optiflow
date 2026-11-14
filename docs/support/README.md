# Local support diagnostics

Implements the metadata-only v1 pilot owned by
[Flow #21](https://github.com/egohygiene/flow/issues/21). This is a source
candidate; independently built CLI acceptance has not been run in this sprint.

Use `optiflow --json diagnostics preview --request FILE`, then explicitly select a fresh
output with `bundle --request FILE --output NEW`. Inspect that exact file with
`inspect --bundle FILE`; `doctor` describes compiled support without contacting
a collector. Task aliases are `diagnostics:preview`, `diagnostics:bundle`,
`diagnostics:inspect`, `diagnostics:doctor`. File selectors must be canonical
absolute paths on an applicable Unix host; Windows file I/O remains unsupported.

Only generated build/platform/contract/health metadata is eligible. There is no
raw-log, prompt, media, environment, argv or attachment input. Creation uses
no-clobber 0600 output; failure preserves partial files. Inspect performs bounded
structure/BLAKE3-256 checks with no extraction or embedded execution. These are
unsigned support claims, never domain acceptance, resumability or release proof.
Unknown queue/drop measurements remain null and no exporter is initialized.

Schemas, a four-category request and the shared conformance inventory are in
this directory. The operator runbook and cross-product conformance driver are
maintained in the Flow source branch for this checkpoint:

- [Profile and exact command table](https://github.com/egohygiene/flow/blob/codex/21-diagnostics-closeout-20261113/docs/observability/support-profile.md)
- [Operator runbook](https://github.com/egohygiene/flow/blob/codex/21-diagnostics-closeout-20261113/docs/observability/support-runbook.md)
- [Shared explicit-binary driver](https://github.com/egohygiene/flow/blob/codex/21-diagnostics-closeout-20261113/tools/check_support_clis.py)

No sibling code or observability package is imported. The protocol implementation
is source-local during this pilot; the separate #18 decision owns any future
package. #15–#20 still own real logging/exporter/stream adoption. No existing ADR
is ratified here. Retention, private storage and deliberate sharing are operator
choices; nothing uploads, cleans a run workspace or changes a domain result.

## Authored checks and deferred acceptance

The library contains canary/exclusion, malformed member/digest, bounded request,
cancellation, no-clobber and Unix alias tests. The shared driver covers preview,
creation/inspection, refused input, tampering, duplicates, oversize, aliases and
unknown health through the native CLI. These cases are authored and unrun.
All compilation, test, formatting/lint, schema and native/hosted qualification
remain deferred unless the maintainer explicitly enables focused local checks.
No passing evidence or issue closure is claimed from this source delivery.
