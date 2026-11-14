# Recursive PNG discovery and authority proof

November 14, 2026 (America/New_York). Relates to
[Optiflow #121](https://github.com/egohygiene/optiflow/issues/121).

This checkpoint follows the selected [#95 Linux proof](../95/LINUX-INTEGRITY-20261114.md)
on the draft #217 → #218 source stack. The recursive folder owner, explicit
disposition protocol and coordinator already exist. This adds repeatable proof
through their public CLI and selects the existing folder unit group; it creates
no parallel planner, optimizer or transaction implementation.

## Repeatable public boundary

Build the current development CLI explicitly with its declared Rust 1.89
compiler. Supply its independently recorded SHA-256; no executable is selected
by discovery or installed by the proof:

```sh
cargo +1.89.0 build --bin optiflow --locked --offline --jobs 2
python3 -B scripts/prove-png-folder-cli.py \
  --executable /absolute/path/to/optiflow \
  --expected-executable-sha256 ACTUAL_SHA256
```

The driver accepts Linux only, creates its own ordinary disposable directories,
suppresses user/project configuration and selects a private state directory. It
generates two valid PNGs (one nested Unicode/spaced name), invalid/unsupported
files, an excluded hidden PNG, a symlink to its own outside canary and two
hard-linked paths. The declared provider path does not exist. No real collection,
model, network service, provider command or mounted test volume is selected.

The 13 actual CLI cases pass against the development binary built from #218
head `6f0821bdd968d5f907ae575f575f8126b0a7ab1e`, tree
`6bc599d061fb37ef5838d7de935e2c9fd9dc6ddf`. Its SHA-256 is
`419f0356b07efac1e8d646b0fc81388cb6ef17e8bff8b4bbd3913968eab2f0f2`:

- effective compiled policy is read without creating state;
- recursive discovery produces two eligible, two unsupported and four skipped
  entries, with exact nested membership and truthful reason codes;
- explicit nonrecursive selection omits nested work;
- retained-original authority records the exact inventory and action ceiling;
- existing inventory output cannot be overwritten;
- an entry-budget breach publishes no inventory;
- changing retain intent into discard cannot reuse the old inventory;
- separately requested discard discovery/authority can be recorded, solely as
  synthetic review declarations, without enabling any application;
- a discard approval cannot be substituted into retained-original status;
- an unknown request field cannot publish an inventory;
- both run and resume refuse at the public qualification gate before attempting
  to read deliberately missing request/inventory/approval files.

Source and outside-canary membership, bytes, identities, modes, links and
modification/change times remain unchanged (legitimate read atimes are excluded).
State, retention, candidate and run directories remain empty. The executable hash
is checked before/after. The bounded receipt contains fixed case IDs/counts and
generated names, rather than private temporary paths. It explicitly retains
`physical_space_reclaimed: false`, `native_qualified: false` and synthetic-only
authority. These are discovery/authority/refusal outcomes, not optimization.

## Existing unit group and expanded local selection

```sh
cargo +1.89.0 test --lib execution::png_folder:: --locked --offline --jobs 2
```

All 11 existing folder cases pass on the selected Linux source, with 293 other
library cases filtered out. No assertion or qualification gate is weakened and
no production defect was exposed by this selection. Existing warning debt is
not relabeled a strict-lint pass.

The Realm manifest now selects seven required checks: the declared-compiler
library target, two replacement/counter groups, this folder group, two budget
commands and the pinned act budget job. Five unsupported rows retain the
unselected full-Rust, native/provider, hosted-service, publication and security
scopes. The complete expanded run must use the reviewed existing Realm runner:

```sh
python3 -B /path/to/realm/tools/realm_local_ci.py \
  --repository /path/to/optiflow --output /private/local-ci-output run
```

The older four-entry #218 report does not cover this expanded manifest. The
follow-on complete run's exact source/manifest hashes and private report remain
separate task receipts and are recorded in the parent issue/PR handoff. Planning
alone is not completion, and selected completion is not full CI/native parity.

## Executed complete expanded selection

The reviewed Realm runner then completed all seven required entries at clean
local revision `e8f5335612ba9cc306b1ecea4de3da1f338b3cdd`, source tree
`cd243f2cfd3871def4749e70820683827d22cfa4`. It ran from
`2026-11-14T05:56:36Z` to `2026-11-14T05:57:39Z`, approximately 64 seconds.
The selected manifest SHA-256 is
`0c1e3342bf96760b798cc7d66cb3474e1a0c88eba5eaeef14342688a7b505f48`;
the retained private report SHA-256 is
`623d931e6be684568d69e5fc6503703b34ef60e788c564490eb1006af227ec05`.

The Rust 1.89 library check, two SQLite cases, 20 replacement cases, 11 folder
cases, 12 budget cases, budget plan and actual pinned act budget-source job all
pass. The act job executes the bounded budget commands in the reviewed Linux
image; it is not hosted execution or another product-test result. The existing
explicit managed proxy/CA adapter was selected, with no production credentials
forwarded and no Docker socket inside the job.

The report's `complete: true` applies to those seven required entries. All five
unsupported rows remain unsupported. No full Rust/security/native/provider/
publication matrix is inferred. The source budget plan still keeps product and
execution authority false. Raw logs and report remain in the private task output,
not the repository or a public release artifact.

The first result append was documentation-only after the named tested revision.
The selected manifest and Rust code are still identical to that tested source.
The CLI observer was subsequently strengthened under independent review below,
with its own focused rerun; the seven-entry selection does not exercise that
observer. No additional product execution is claimed for writing a receipt.
Both versions of the 13-case CLI proof use the exact #218 binary and its
independent digest above.

## Observer correction after independent review

Review found that the first observer accepted some unrelated same-exit failures
and incompletely checked saved approval and inventory membership. That earlier
CLI receipt is retained privately as historical evidence; it is superseded for
the strengthened observer claims below. No product/qualification gate changed.

Every case now checks the exact command-result schema, command name and declared
outcome/exit status. Refusals require the exact blocking diagnostic code,
classification, severity, impact and selected reason. Native run/resume require
the complete existing qualification message, not a matching keyword. The closed
request case compares the exact known schema-failure representation of its
generated request. Duplicate JSON members refuse rather than silently selecting
the final value.

Both saved approvals must equal their emitted command result and independently
match the closed approval schema, synthetic actor, inventory's request and
inventory fingerprints, selected disposition and action ceiling. Recursive
membership must have exactly eight unique sorted paths. The nonrecursive case
requires exactly `a.png` eligible and no nested child. No-clobber checks preserve
the inventory record's device/inode, mode, link count, size, modification/change
times and digest, so byte-identical file substitution also refuses.

The corrected observer passes all 13 real CLI cases against the same pinned
#218 executable. Seven focused Python negative-observer methods also pass:

```sh
python3 -B -m unittest discover --start-directory tests \
  --pattern test_png_folder_cli_observer.py
```

The tested observer SHA-256 is
`955f9a12d019d5d7ae1371d3b215a437d654217dd130f57026aaf2aca8da9b49`;
the negative-test source SHA-256 is
`d89c4df7344d2a8458f9c3821013372796014c4e62490d566247f4b383db8a99`.
The corrected private CLI receipt SHA-256 is
`d9efd02c4d4650161744e9050fb88732a64d405463d8f63be37b03c4190ee5ed`.

Only the changed observer checks and CLI proof were rerun. The seven-entry Realm
selection and Rust code are unchanged; its earlier exact-source report above
does not claim execution of these new Python observer tests. No redundant Rust
or act execution, native scope expansion or hosted workflow was selected.

## Remaining acceptance

The simple user-facing in-place optimization outcome still depends on #95/#93
and applicable #111 qualification. Public run/resume remain deliberately blocked.
This proves neither lossless replacement nor irreversible disposal, crash/power
loss behavior, APFS/removable volumes, actual storage reclamation, provider
limits, installed releases or user-media safety. No real approval, source-media
mutation, signing, publication, hosted CI or #88 ledger change occurs. #121 stays
open for its complete outcome and acceptance criteria.
