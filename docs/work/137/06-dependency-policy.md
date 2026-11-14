# Flow #137 checkpoint 6 — November 13, 2026

Status: container/toolchain source repair authored; SQLite advisory remains
unresolved. This checkpoint is **not accepted or green**.

## Toolchain selection repair

The pinned `cargo-deny-action` revision
`3c6349835b2b7b196a839186cb8b78e02f7b5f25` uses an Alpine image containing
Rust 1.85.0 and cargo-deny 0.20.2. Its entrypoint accepts a `rust-version` input
and calls `rustup default` when supplied, but a checkout's `rust-toolchain.toml`
can still override that default. The observed container failure selected an
uninstalled `stable-x86_64-unknown-linux-musl` channel.

The owning security step now supplies both the supported `rust-version: 1.85.0`
input and `RUSTUP_TOOLCHAIN: 1.85.0`. The environment override takes precedence
over the checkout's floating channel and selects the version already represented
by the pinned image. `task security:dependencies` uses the same explicit policy
toolchain. The existing root/fuzz matrix, four policy classes, action pin and
failure behavior remain intact. Product compiler lanes and the declared MSRV
are unchanged.

This is source inspection of the exact upstream action, Dockerfile and
entrypoint, not a container execution result. The action was not rebuilt or run.

## Advisory and compatibility evidence

Both owning locks still contain `rusqlite 0.40.2` and `libsqlite3-sys 0.38.2`,
with bundled SQLite selected by the unchanged root manifest. The RustSec
[advisory at revision c5ed556](https://github.com/RustSec/advisory-db/blob/c5ed55613b44ac8c641bb7e86120afb659930e58/crates/libsqlite3-sys/RUSTSEC-2026-0459.md)
classifies libsqlite3-sys 0.6.0 through 0.38.2 as affected by
RUSTSEC-2026-0459 / CVE-2026-119375. Its patched range begins at 0.39.0 and
bundles SQLite 3.54.0 or later. The issue concerns a database and sibling WAL
whose page sizes differ; no claim is made that an existing state-directory
check makes the advisory inapplicable.

The inspected registry metadata contains no 0.38.3 backport. The compatible
requirement of rusqlite 0.40.2 selects libsqlite3-sys 0.38.x, while the inspected
patched candidates have this compiler requirement:

| Candidate | Declared Rust requirement | Relationship to current graph |
| --- | --- | --- |
| libsqlite3-sys 0.39.0 / 0.39.1 | 1.88.0 | Outside rusqlite 0.40's 0.38.x requirement |
| rusqlite 0.41.0 / 0.41.1 | 1.89.0 | Selects patched libsqlite3-sys 0.39.x |

Optiflow promises Rust 1.85. Updating the ordinary registry graph to those
candidates would change that compatibility contract. The adjacent JSON retains
the exact observed locks and candidate metadata. Metadata observation is not
compiler/platform qualification.

## Concrete decision retained for reconciliation

The source default remains bundled SQLite and Rust 1.85, with the advisory
finding still enforced. No advisory ignore, registry/advisory rewrite, unsafe
version substitution or implicit system-library selection is introduced.

The remaining decision has these bounded paths:

1. Obtain an upstream-supported patched backport compatible with Rust 1.85 and
   rusqlite 0.40, including an authoritative advisory classification for that
   patched release. No such registry graph was observed. A private source patch
   cannot be called cargo-deny-clean merely because its C payload was replaced.
2. Explicitly revise the supported compiler contract to at least Rust 1.89,
   then update the rusqlite/sys graph in both locks and qualify every advertised
   host/target, feature and packaging lane. That contract change is not selected
   by this patch.
3. Design an explicit, qualified system-SQLite profile requiring a patched
   native library and visible availability/version checks on each host. Removing
   `bundled` without that installation/packaging contract is not a repair and
   is not selected here.

For this sprint, preserve 1.85 and retain the fail-closed advisory result until
one supported remediation is chosen and evidenced. Do not approve a release or
mark checkpoint 6 complete from the container fix alone.

## Validation boundary

Read the exact action revision, the pinned advisory and the local registry
metadata; inspected both lockfiles and the root requirement. The source diff
changes only the workflow/task toolchain selection and this handoff. Cargo
requirements, locks and deny policy are unchanged. No product compilation,
tests, cargo-deny execution, container/native work, hosted CI, release or signing
occurred. No passing advisory, license, ban or source-policy result is asserted.

After execution is authorized, require both manifest matrix jobs to select the
declared toolchain and produce the actual four-class policy result. Advisory
acceptance remains blocked on the compatibility decision above. Flow #88 stays
unchanged and last; native mutation/APFS/media gates are unaffected.
