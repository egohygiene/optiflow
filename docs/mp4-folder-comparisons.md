# MP4 folder outcomes and comparisons

Relates to #146/#135. Authored source only; no native/media run or qualification.

`mp4-delivery folder-summary --run-directory PATH` and the SDK
`folder::comparison(PATH)` first reconcile the existing journal, exact candidate
store, pins and retained source/request/tool/accounting joins. They create no
workspace, invoke no providers and do not reopen original media or tool paths.
The Task alias is `mp4:folder:summary RUN_DIRECTORY=PATH`.

The separate `optiflow.mp4-folder-comparison.v1` view reports discovery entries,
files seen/inspected/eligible, technically optimized/unchanged files, unsupported/
skipped entries, failures, recorded results, pending entries and uncertain
attempts. Unsupported/skipped counts refer to recorded results; undispatched
entries remain pending. Traversal directories and non-file exclusions are not
mislabelled as optimized files. Per-entry local paths and reasons remain local
report data, not portable telemetry or metric labels.

Accepted-source and candidate bytes sum only the technically accepted subset.
Their exact difference must equal reconciled saved encoded bytes. Reduction is
an exact numerator/denominator; when no candidate is accepted it is unknown,
not a fabricated zero-percent measurement. Nonshrinking candidate measurements
are retained separately and never contribute savings. An already-fitting source
records that no encoding occurred. None of these counts selects an unchanged
original for downstream use or records actual human quality approval.

Original objects remain. Logical encoded-byte difference does not prove lower
allocated space, shared/snapshot reclamation, whole-collection savings or a
current free-space change. Pinned candidates are freshly reobserved; original
freshness, native decode and review are not refreshed by this view.

New `optiflow.mp4-folder-report.v2` records embed the same closed validated v1
result-prefix/accounting record and add the observed continuation-window elapsed
time and actual stop cause: complete, cancellation, elapsed window, cumulative
action/publication limit, or reconciliation required. Readers validate the
prefix, totals, current request/inventory membership and applicable budget/time
witness. The original v1 reports remain readable with `not_recorded` cause and
no elapsed observation. No legacy report is rewritten or inferred to have new
evidence. Host I/O/cleanup is cooperative, so elapsed observation can exceed the
selected window; it is not a hard syscall deadline guarantee.

Current continuation guidance distinguishes complete, manual reconciliation,
report-history exhaustion, cumulative budget exhaustion and explicit resume of
never-started work. Resume refuses exhausted cumulative budgets before another
no-op action/report. A completed resume still reconciles current inputs/tools
but does not append unnecessary report history. A new time window never resets
actions/publication limits, adopts scratch, repeats an uncertain start, raises
the frozen request budget or performs automatic original disposal.

Existing request, inventory, result and status v1 identities/shapes remain
unchanged. Two outcome-count/legacy-report negative cases are authored,
uncompiled/unrun. No compiler/build/tests/dependencies/lint/format/schema/docs/
drift, product/CLI/SDK/native/provider/media/model or CI execution occurred.
