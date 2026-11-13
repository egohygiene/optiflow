OPTIONAL PROPOSAL — requires maintainer compatibility decision, not adopted

# Concrete Rust 1.89 / patched SQLite compatibility candidate

This isolated fork starts from the primary checkpoint 6 source repair,
`1100e7b2c613deca7b7ad75dd32f1faca83c4185`. It is not the next dependency in
the primary sprint stack and must not be merged as routine CI cleanup. The
primary repair continues to promise Rust 1.85 and expose the unresolved advisory.
This candidate makes one alternative reviewable without adopting it.

## Proposed source change

| Concern | Primary repair | Optional candidate |
| --- | --- | --- |
| Product `rust-version` | 1.85 | 1.89 |
| Fuzz package `rust-version` | Unspecified; depends on primary product | Explicit 1.89 |
| `rusqlite`, bundled | 0.40 requirement; lock 0.40.2 | Exact `=0.41.1`; lock 0.41.1 |
| `libsqlite3-sys` | Lock 0.38.2 | Lock 0.39.1 |
| MSRV CI, release and dependency-policy toolchain | 1.85.0 | 1.89.0 |
| Canonical source formatter | 1.85.0 | **1.85.0, unchanged** |
| Development channel | `stable` | `stable`, unchanged |

Root/fuzz manifest requirements and both locks change together. The ordinary
bundled-library strategy remains selected; no system SQLite dependency,
vendored C replacement, source override, feature addition or advisory ignore is
introduced. Registry metadata declares Rust 1.89 for rusqlite 0.41.1 and Rust
1.88 for libsqlite3-sys 0.39.1.

The pinned RustSec [advisory revision c5ed556](https://github.com/RustSec/advisory-db/blob/c5ed55613b44ac8c641bb7e86120afb659930e58/crates/libsqlite3-sys/RUSTSEC-2026-0459.md)
classifies libsqlite3-sys `>=0.39.0` as patched for RUSTSEC-2026-0459 and describes
0.39.1 as bundling SQLite 3.54.1. This is upstream classification/metadata, not
an observed native SQLite version, exploitation regression or cargo-deny pass.

The proposed MSRV lane, release build/bundle environment and tool installation,
dependency-policy action input/override, local dependency-policy Task and release
toolchain fixture constant all select 1.89.0. The policy action's original base
image contains 1.85.0; its explicit version input would install 1.89.0 before the
matching environment override selects it. That proposed container path has not
run. Current public MSRV prose is updated for the optional candidate. Historical
receipts, native work packets and dated checkpoint records remain unchanged.

## Formatter policy remains independent

The four explicit `cargo +1.85.0 fmt` Task commands, canonical formatter install
step and two CI formatter commands remain unchanged. Rustfmt parses and lays
out source; it is not the supported compiler floor. Accepting this candidate
would not authorize a source-format style migration. Moving-stable compiler
lanes keep their existing selection and strict lint policy.

## Exact graph and metadata observations

The [machine-readable packet](06-optional-msrv-189.json) retains original and
candidate package records, checksums, package counts and metadata observations.
The exact package changes are the same in the root and isolated fuzz lock:

- `rusqlite`: **0.40.2 → 0.41.1**.
- `libsqlite3-sys`: **0.38.2 → 0.39.1**.

No other package version or package count changes. Targeted Cargo 1.89.0
metadata updates selected the rusqlite version in each owning workspace. The
following graph-resolution commands then exited 0 without changing either
candidate lock's SHA-256:

```sh
cargo +1.89.0 update --workspace --locked --offline
cargo +1.89.0 update --manifest-path fuzz/Cargo.toml --workspace --locked --offline
```

These operations resolve dependency metadata only. The task-private toolchain
was installed for those observations. No product compilation, build scripts,
Clippy, tests, cargo-deny, native execution, container build, hosted workflow,
signing, release or publication ran. Dependency/API/platform compatibility and
the other advisory/license/ban/source gates remain unverified. The 0.40→0.41
rusqlite API boundary must be checked during a separately authorized build pass.

## Adoption boundary

Maintainer acceptance must explicitly choose losing Rust 1.85–1.88 product
compiler compatibility in exchange for this patched bundled graph. Before a
release, qualify all advertised targets/features, database/migration/journal
behavior, root/fuzz policy results and release/installer contracts at exact
revisions. No existing native/mutation/APFS/creative acceptance gate is waived.

Until that decision and evidence exist, this remains a draft compatibility
proposal. It does not close checkpoint 6, resolve all security findings, adopt
a new supported MSRV, or establish a green release matrix. Flow #88 remains
unchanged and last.
