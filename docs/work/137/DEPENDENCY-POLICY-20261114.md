# Executed dependency policy — November 14, 2026

Relates to [Flow #137 checkpoint 6](https://github.com/egohygiene/flow/issues/137).
Both selected root and isolated-fuzz four-class scans pass with the existing
policy. The source adoption in [#215's receipt](215-sqlite-adoption.md) selected
Rust 1.89 and patched bundled SQLite; the older blocked-advisory discussion in
[checkpoint 6](06-dependency-policy.md) remains historical. No advisory ignore,
policy allowance, manifest or lock change was needed for these scans.

## Exact source and verified tool

- Evaluated source: `52ceb4b1611a2580ea3ba0207e841d8269bdee48`, tree
  `2f116a47d6040c257adad8a8b78640bde5c2e97b`. Its only difference from main
  `165d7f4b1e6f0750c6981032e44b4c63db3e0063` is the preceding compiler receipt.
- Cargo/Rustup selection: Rust **1.89.0**, isolated homes, two Cargo jobs.
- Policy tool: **cargo-deny 0.20.2**, obtained as the public
  `cargo-deny-0.20.2-x86_64-unknown-linux-musl.tar.gz` release asset. No global
  installation or tool compilation was performed.
- Archive SHA-256, independently matched against the GitHub release asset digest
  and its companion `.sha256` asset:
  `9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f`.
- Extracted executable SHA-256:
  `b329e25933d01c36dd7c47d84ea5716694f9b7caf53a5003d45674703a8ed54a`.

The tool's own help states that `--frozen` is `--locked` plus `--offline`, and
that offline disables fetching crates, advisory databases and
std-replacement-data. The existing Task runner was unavailable; the explicit
commands below select its same two manifests and four policy classes, with the
additional frozen boundary and stats/log output.

## Preparation and frozen execution

Separate online preparation obtained the public RustSec database, registry
package/index/license inputs and std-replacement-data using the pinned tool's
`fetch db index std-replacement`, followed by the isolated-fuzz
`--manifest-path fuzz/Cargo.toml --all-features --locked fetch index`.
The subsequent scans used the prepared private cache with
`RUSTUP_TOOLCHAIN=1.89.0`, `CARGO_NET_OFFLINE=true` and
`RAYON_NUM_THREADS=2`:

```sh
cargo deny -L info --manifest-path Cargo.toml --all-features --frozen check --show-stats advisories bans licenses sources
cargo deny -L info --manifest-path fuzz/Cargo.toml --all-features --frozen check --show-stats advisories bans licenses sources
```

RustSec database: `4ef2df8e806a65e2f1ecc14f56f51249062b23d6`, committed
`2026-11-13T12:33:03Z`, matching the live public branch readback. Its age at the
first scan was **111280.572 seconds** (30 hours, 54 minutes, 40.572 seconds).
The database revision and clean checkout were verified before and after both
scans. This is a dated observation, not a claim that future advisories are clear.

| Manifest | Start (UTC) | Duration | Selected crates | Result |
| --- | --- | --- | --- | --- |
| `Cargo.toml` | `2026-11-14T19:27:43.571715Z` | 0.565 s | 149 | Exit 0; all four classes pass |
| `fuzz/Cargo.toml` | `2026-11-14T19:27:44.144990Z` | 0.551 s | 134 | Exit 0; all four classes pass |

The graph keeps the policy's declared Linux x86-64, Intel macOS and Apple
Silicon target filters and all features. Graph filtering is not native compiler
or filesystem qualification. Each scan reports no advisory or source warnings,
and no policy errors. Both report the existing three permitted duplicate-version
warnings (`getrandom`, `miniz_oxide`, `syn`). Root additionally reports an unused
`NCSA` license allowance. These warnings were retained rather than suppressed.
The generic informational `fetching crates` log label is also retained; these
scan invocations select the tool's frozen/offline behavior.

Both lockfiles and the deny policy remained byte-identical across preparation
and scans:

| Input | SHA-256 |
| --- | --- |
| `Cargo.lock` | `a514fd4ef1f26c67ad62b6de8ce09b5165343f3d916b6bf9bd6589f0f2f3d509` |
| `fuzz/Cargo.lock` | `c02fd11d839b55119ad45ad87ee22694e48cee7d9ebd41bb64c4588c1dd34fca` |
| `deny.toml` | `8b12113fcca4edae4b55c36d8eba06b1348cf20bd53e3b0785f6c0d0facdfcfb` |

Complete retained log SHA-256 identities:

- Root: `eff73a2386b3525a02e1bb04d50166c0dc3fd1772f25cd231e0ac1faa6ea299b`.
- Isolated fuzz: `7f6d149de64f73de6023a991c27a6a9ef5826f5d36691019609326d19a151a8c`.
- Combined machine receipt: `11aa36b107f0e44ca1f909aaa63ee5a390940efe306d37bb9b68f1e22193d7f3`.

## Remaining qualification

This qualifies the selected local dependency-policy commands at the recorded
source, graph, tool and advisory revision. It does not execute the pinned
GitHub Action container, fuzz targets, product tests, strict Clippy, native
macOS/APFS/physical-volume tests or the full compiler/packaging matrix. No model,
real media, hosted CI, signing, release or publication occurred. Flow #137's
broader compatibility/workflow acceptance and Flow #88's final pass remain open;
Optiflow #95 and all mutation/native qualification gates are unchanged.
