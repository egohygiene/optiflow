# SQLite source adoption — November 13, 2026

The maintainer requested merging all open Flow-suite PRs, including the concrete
Optiflow #215 compatibility option. The primary repair stack was reconciled
first; this source change then selects Rust 1.89 and patched bundled SQLite.
The containing #215 revision identifies the resulting source. Its reviewed
main input is `8451abf9e32b7c5c133fa833f12fea6da9275b28`.

## Selected compatibility contract

- Product and isolated fuzz packages require Rust 1.89; Rust 1.85–1.88 product
  compiler support is no longer promised for this development source.
- Both owning locks select rusqlite 0.41.1 and libsqlite3-sys 0.39.1. Bundled
  SQLite remains selected; no system-library substitution or advisory ignore
  is added. The inspected advisory classifies this sys version range as patched;
  that source classification is not an executed cargo-deny or native result.
- MSRV CI, release build/evidence, dependency-policy action input/environment,
  local security Task and release fixture constant select Rust 1.89.0.
- Canonical source formatting remains Rustfmt 1.85.0 and the development channel
  remains stable. Checkpoint 12 hosted/scheduled opt-ins and original workload
  conditions remain intact. Publication allowlists and native authority do not
  change; published historical v0.1.1 assets are not replaced.

## Source reconciliation and evidence

The original proposal is retained in
[the historical decision packet](06-optional-msrv-189.md) and its adjoining JSON.
The current active manifests, README and policy prose describe the selected
contract rather than continuing to label it unadopted. Earlier dated checkpoint
receipts remain history.

The corpus companion refresh is restricted to the reviewed Cargo.lock digest.
Recipes, expectations, payloads, catalog, generator and proof-source bindings
must stay byte-identical to the already reconciled primary main input. A current
binding does not mean its Rust proof passed. Actual bounded local observations
are recorded below after execution; no missing result is inferred.

## Remaining qualification

No product compilation/build/test/Clippy, cargo-deny scan/container, native
macOS/APFS/physical-volume work, installer or hosted CI is performed by this
adoption. The rusqlite API boundary, database/migration behavior and all declared
hosts/features still need their actual exact-source qualification. No signing,
release, model/provider execution or publication occurs. Optiflow #95 stays open;
Flow #88's original final-pass ledger remains untouched and last.

## Executed bounded local observations

- Cargo 1.89.0 root and isolated-fuzz `update --workspace --locked --offline`
  both exit 0, with each lock SHA-256 unchanged after metadata resolution.
  Only the rusqlite/sys package records differ from the reconciled main input;
  root/fuzz package counts remain 191/176.
- The dedicated source comparison confirms all 15 workflow job metadata rows,
  existing conditions/timeouts, formatter selections and publication authority
  are preserved. Release workflow changes are restricted to its four compiler
  selections. No Rust product source is edited by this adoption.
- The lock-only corpus companion changes only `cargo_lock_sha256`, from
  `268f92574d8b244da608ccc980fa7b5a529af40bd7d5e125c2146fc0178b10c5` to
  `a514fd4ef1f26c67ad62b6de8ce09b5165343f3d916b6bf9bd6589f0f2f3d509`.
  Every non-lock binding remains identical; the source check reports all 17
  fixture bindings current. No Rust proof or corpus tier is executed.
- The pinned Actionlint wrapper and the source-only budget plan pass; the plan
  retains heavy qualification as deferred with execution/product authority false.
- The focused Python release-toolchain contract passes one case, checking the
  installed compiler pins from source. The complete product/release/native
  matrix is not executed or qualified by that single source regression.
