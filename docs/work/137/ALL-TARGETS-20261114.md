# Optiflow all-target compiler observation — November 14, 2026

Relates to [Flow #137 checkpoint 2](https://github.com/egohygiene/flow/issues/137).
The exact selected Linux compiler check passes. This records an executed local
observation; it does not complete the supported native host matrix.

## Source and command

- Source: `165d7f4b1e6f0750c6981032e44b4c63db3e0063`.
- Source tree: `9ff3ce42ba60fa3437d753dadb971a56ca485ea8`.
- Compiler: `rustc 1.89.0 (29483883e 2025-08-04)`, Linux x86-64.
- Explicit isolated Cargo/Rustup homes and a reused target cache; two jobs.

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo check --all-targets --all-features --locked --offline --jobs 2
```

The command exited **0** in 26.175 seconds, from
`2026-11-14T19:25:17.653960+00:00` to `2026-11-14T19:25:43.829186+00:00`.
It checks the root package's library, binary, test/example/benchmark targets and
all declared features on the current Linux host. It does not compile the
separate isolated fuzz workspace or cross-compile the macOS targets.

Both lockfiles remained unchanged:

| Lockfile | SHA-256 before and after |
| --- | --- |
| `Cargo.lock` | `a514fd4ef1f26c67ad62b6de8ce09b5165343f3d916b6bf9bd6589f0f2f3d509` |
| `fuzz/Cargo.lock` | `c02fd11d839b55119ad45ad87ee22694e48cee7d9ebd41bb64c4588c1dd34fca` |

The retained complete compiler log SHA-256 is
`a3c0da41dd67f0613d6fda64d1774aa888708f1e4e295d1cef1769ec07fd9f42`.
No product source, manifests, locks or policy changed for this observation.

## Remaining limits

Four library warning diagnostics and two library-test warning diagnostics remain
in the compiler output, including repeated diagnostics. This is not a strict
Clippy result. The check compiles tests; it does not execute them.

The previously accepted CLI executable was copied and verified before cache
reuse; its SHA-256 remains
`419f0356b07efac1e8d646b0fc81388cb6ef17e8bff8b4bbd3913968eab2f0f2`.
No native macOS/APFS/physical-volume, provider/model/media, hosted CI, signing,
release or publication qualification occurred. Existing qualification and
mutation gates remain intact. Flow #88 remains the final deep acceptance owner.
