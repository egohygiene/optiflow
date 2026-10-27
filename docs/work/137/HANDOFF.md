# Candidate-artifact fixture root checkpoint

Issue: [#137](https://github.com/egohygiene/optiflow/issues/137), under the
existing filesystem corpus owner #65.

## Source and scope

Reviewed main: `d03e53f2e397af03920070d9bb6c076d7827e12b`.
Branch: `codex/137-canonical-test-root`.
Observed on October 27, 2026: #137 is open; the separate #111 work remains in
draft [PR #122](https://github.com/egohygiene/optiflow/pull/122) at
`ac381942e4202d7221cbd1d8832d65058e146081`.
That PR does not change `src/candidate_artifact.rs` or
`tests/png_production.rs`; this checkpoint does not adopt or modify its
transaction or ACL work.

The ten current candidate-artifact unit tests share `private_root()`. The audit
in #137 records eight failing cases at its older source. Their ordinary
fixture setup previously used `tempfile::tempdir()`, which can retain macOS's
symlinked `/var` ancestor. The production directory walk deliberately refuses
symlink components, so that host alias could prevent a fixture from reaching
the assertion it was meant to exercise.

The test-only helper now creates its private temporary root beneath the
canonical system temporary directory. This follows the existing
`tests/png_production.rs::SyntheticRun::new` setup, uses a recognizable unique
prefix, and retains the explicit `0700` permissions and `TempDir` cleanup.
It changes no production code, dependencies, schemas, receipts or workflow
gates. No new fixture framework or duplicated behavioral test is needed.

The adversarial root/member symlinks are still created after ordinary setup
and passed directly to production operations. They are not canonicalized.
Exact committed membership, evidence/content binding, missing-member refusal,
unknown-stage preservation and all existing assertions remain unchanged.

## Evidence and limits

Only live issue/PR reads, source review and Git inspection were performed.
Independent source review found no implementation blocker and confirmed the
test-only boundary; the handoff distinguishes ten current tests from the
audit's eight reported failures.
No tests, compiler/build, formatter/linter, schema or corpus checks, native
execution, dependency resolution or CI were run. No Actions were dispatched
or polled. The historical audit remains historical; this source correction
does not establish a passing macOS or Linux result.

#137 remains open for native supported-host qualification. At the deferred
testing checkpoint, record the exact revision, OS/architecture, toolchain,
resolved temporary-root path and command results for the current unit module
on macOS and the focused Linux regression run. Run all ten current cases,
including all eight originally reported cases. Confirm that successful
publication reaches its assertions and deliberate symlink refusals and
unknown-stage preservation still hold. Do not substitute Linux evidence for
macOS evidence, relax the production no-follow walk, or claim #111/#93 native
mutation or release qualification from this fixture repair.
