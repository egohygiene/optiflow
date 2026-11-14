# Local CI consumer checkpoint — November 13, 2026

Relates to [Flow #137](https://github.com/egohygiene/flow/issues/137).
The maintainer selected focused local builds and smoke checks after the merged
source sprint. This consumer reuses reviewed Realm `75c1b2e1475b972a0ce1c8594e8daba2e1cf8686` and its
`realm.local-ci/v1` contract; it adds no new runner or hosted workflow.

The manifest requires the declared-compiler library check (1.89.0), the two
existing bounded budget commands, and a pinned act source-check fixture.
The other workflow/native/provider/distribution scopes are explicit unsupported
rows. Existing hosted CI/release conditions and publication authority are not
rewritten. Repository-specific setup and exact commands are in
[the local guide](../../local-ci.md).

## Observations

Manifest planning and the existing synthetic budget regressions may be run as
source admission checks. Actual product compile, act completion and scope-specific
smoke evidence belong to the final integrated candidate's local report. A planned
or authored command is not execution. This packet does not grant release or
whole-matrix qualification. Older dated source observations retain their scope;
Flow #88's original final-pass ledger remains unchanged and last.

## Executed source admission

The exact pinned Realm runner accepts this manifest and emits a plan with
`execution: not-performed`. All 12 existing budget regressions pass, and the
existing budget source planner succeeds. Actionlint 1.7.12 accepts both the new
act fixture and the existing workflow wrapper/scope regressions. Task YAML is
parsed with duplicate keys refused; all three aliases are distinct. The selected
compiler is compared with the owning Cargo minimum (Renderflow's workspace
minimum). These are source admission observations, not a Rust or act execution
result. Final integrated execution is recorded separately.

## Executed selected Linux run

The reviewed Realm runner completed all four required checks at local source
`b9fdd3840b5c7bf3de79497415bc07298584a220`, tree `6035f9a3849936ec79692cf690e14ab0d225bc93`, with a clean tracked checkout.
The selected manifest SHA-256 is `219635ee1b5851fcee461c0add9aa776e1fedd0028e59103002b1266bd6cb5c7`;
the retained private report SHA-256 is `87bfd163fd4171010986e044d1f3126b351c30a4a94de8f111bdaf964815ab99`.

The declared-compiler locked/offline library check, 12-case budget regression
suite, bounded budget plan and actual act budget-source job each pass. The act
job runs the same 12-case source suite in the pinned Linux image; it is not a
second product/native test result. Execution used checksum-verified act 0.2.97,
Docker 28.4.0 and the explicit managed proxy/CA adapter with no production
credentials or Docker socket inside the job. No hosted workflow was dispatched.

The report retains 5 explicit unsupported rows. `complete: true` covers only
the four required entries; full tests/Clippy, other toolchains/hosts/providers,
artifact/OIDC/services, native mutation and publication remain outside this
selection. The existing source budget plan still has product/execution authority
false. This dated observation is added after testing and changes no selected
command or Rust product source. Raw bounded logs remain private.
