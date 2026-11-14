# Flow #137 checkpoint 2 — optiflow, November 13, 2026

The observed compiler failures have three owning repairs:

- SQLite replacement-journal count/byte reads use an i64-to-u64 checked bridge,
  and event-sequence writes use u64-to-i64 checked conversion. Negative,
  noninteger or out-of-storage-range values fail rather than wrapping or
  saturating. Existing count/byte budgets and event validation remain intact.
- The process-group signal uses rustix 1.1.4's actual `Signal::KILL` constant.
  The wait-owned/PID-reuse protections and cleanup decisions remain intact.
- Video controller construction accepts the existing `PinnedLauncher` type and
  selects its path field; the helper argv uses its path field too. Pin/digest
  admission and the bound executable observations remain unchanged. There is
  no unpinned launcher fallback and no shell command construction.

Two in-memory SQLite boundary/refusal cases are authored. Existing video argv
coverage now includes a selected helper path with spaces, Unicode and shell
metacharacters, checking that it remains one literal argument after `--`.
No real database, source-media mutation, process, model or service is exercised
by this source authoring. The native mutation qualification gates remain closed.

## Source evidence and acceptance

Repairs are grounded in the failed jobs linked from
[Flow #137](https://github.com/egohygiene/flow/issues/137). The rustix 1.1.4 signal
constant and rusqlite 0.40.2 error-conversion variants were additionally read
from their exact upstream source tags; no dependency was installed or resolved.
No Cargo manifest/lock, MSRV, feature support, journal schema or native authority
gate changes here. The separate SQLite security-advisory/toolchain conflict is
checkpoint 6; these type repairs do not resolve it.

This branch is stacked after its owning checkpoint 1 workflow-lint draft.
No tests/compiler/build, formatting/lint, schema/drift, native/provider/media or
hosted check ran under the maintainer's existing execution deferral. Authored
checks and source review are not evidence of successful compilation.

Acceptance still requires the declared all-target/MSRV/host/feature checks to
run, the affected refusal/range/argv cases to pass, and the relevant CI workloads
to get past library compilation. Do not mark checkpoint 2 or the whole CI issue
complete from this source repair. Preserve the unresolved alignment/start-marker
failure and the separate native/APFS qualification requirements with their
current owners; this patch makes no claim about those causes or results.
