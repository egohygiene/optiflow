---
title: Bounded subprocess contract
description: The execution boundary for ffprobe and future external media tools.
---

# Bounded subprocess contract

OptiFlow treats every external executable as an untrusted capability boundary.

Adapters must not call a shell, collect unbounded process output, or wait
indefinitely. They invoke programs through `SubprocessRunner` with an explicit
argv vector and bounded execution policy.

## Contract

The runner owns five limits:

| Limit | Default | Purpose |
| --- | ---: | --- |
| Runtime | 15 seconds | Prevent a hung direct child from blocking a scan indefinitely. |
| Poll interval | 10 ms | Bound cancellation/timeout detection latency without busy waiting. |
| Captured stdout | 4 MiB | Prevent successful or hostile tool output from consuming unbounded memory. |
| Captured stderr | 256 KiB | Keep diagnostics useful while bounding failure-output memory. |
| Concurrent children | 2 per shared runner | Prevent one adapter from spawning an unbounded process fan-out. |

Cloned runners share the same concurrency permit pool. Independently created
runners have independent pools and therefore must be created intentionally.

## Invocation rule

Production adapters construct a `SubprocessCommand`:

```rust
let command = SubprocessCommand::new("ffprobe")
    .args(["-v", "error", "-of", "json"])
    .arg(path.as_os_str().to_owned());
```

`std::process::Command` receives that program and argv directly. The runner does
not invoke `sh -c`, interpolate strings, expand globs, perform variable
substitution, or parse command text.

Tests may launch a shell as a fixture process in order to synthesize hostile
output, sleeping children, and exit codes; that does not change the production
execution contract.

## Output bounds

stdout and stderr are drained concurrently so a child cannot deadlock merely
because one pipe fills. Each reader:

1. counts all bytes read;
2. stores at most the configured capture limit;
3. continues draining after the in-memory limit is reached; and
4. returns a typed `Truncated` error after the child exits.

This bounds memory without hiding the fact that the child produced more output
than OptiFlow was willing to trust.

A truncated JSON response is never parsed as though it were complete.

## Runtime and cancellation

The timeout budget begins before the concurrency permit is acquired. Queue time
therefore counts against the caller's bounded execution budget.

While the direct child is running, the runner polls `try_wait()` and checks the
provided cancellation callback. On timeout or cancellation it:

1. requests termination of the direct child with `Child::kill()`;
2. waits for that child to exit;
3. drains/joins the output readers; and
4. returns a typed `Timeout` or `Cancelled` result.

The current v0.1.x contract bounds the **direct child process**. Cross-platform
process-group/tree termination is not promised by this implementation. An
adapter that requires a subprocess tree must record that capability explicitly
and must not assume child-tree cleanup from this contract.

## Typed failures

Adapters can distinguish:

- `InvalidConfiguration` — nonsensical runner limits;
- `Spawn` — executable could not be launched;
- `Wait` — process status could not be observed reliably;
- `Timeout` — the configured runtime/queue budget expired;
- `Cancelled` — the caller requested cancellation;
- `Exit` — the process completed unsuccessfully;
- `Truncated` — stdout or stderr exceeded its capture contract;
- `Parse` — a successful bounded response did not satisfy the expected JSON contract;
- `OutputRead` — a pipe could not be drained safely;
- `Internal` — runner synchronization/invariant failure.

Adapters may add domain context with `anyhow::Context`, but they should preserve
the typed source error instead of collapsing every failure into an unavailable
tool or parse error.

## ffprobe migration

`src/adapters/ffprobe.rs` is the first consumer of this contract.

Both media inspection and `ffprobe -version` now run through the same bounded
runner. `ffprobe` remains optional for the read-only exact-duplicate workflow;
a failed optional probe must not weaken exact byte-identity evidence.

Future adapters should reuse this boundary rather than introducing another
`Command::output()` path.

## Validation

The runner has unit coverage for:

- successful stdout capture;
- typed non-zero exits and bounded stderr diagnostics;
- hostile oversized stdout;
- timeout and child termination;
- mid-run cancellation;
- invalid JSON parse behavior; and
- invalid concurrency configuration.

Repository validation remains:

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
```

The GitHub Linux/macOS matrix is authoritative for platform-specific process
behavior.
