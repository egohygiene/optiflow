# Video pipe capture and shared deadline

This source checkpoint advances [#146](https://github.com/egohygiene/optiflow/issues/146)
on top of [PR #149](https://github.com/egohygiene/optiflow/pull/149). It closes
specific output, cancellation and elapsed-budget gaps in the development
video observation path. It does not complete the planner's `RequestedLimits`
contract or expose a usable video optimizer. All code and tests are unrun.

## Implemented boundary

The existing `SubprocessRunner` gains an opt-in
`run_with_bounded_pipes_and_cancel` method for Linux/macOS. It pumps stdin,
stdout and stderr without blocking pipe calls or background worker threads.
The caller's input is borrowed rather than cloned. Output allocation requests
and retained byte lengths stay within the configured caps, with a fixed read
buffer. Kernel pipe capacity and allocator overhead are outside that claim.

The first observed byte beyond either output cap returns `Truncated` and
triggers cleanup; excess output is not drained until normal process exit.
Each IO turn is bounded so one busy stream cannot indefinitely postpone the
other stream, cancellation or deadline checks. Empty input closes stdin;
incomplete input delivery cannot produce a successful result.

Every child starts in a dedicated process group. `waitid` with `WNOWAIT`
observes exit without reaping the leader. The runner signals that group before
reaping the direct child, including a successful leader whose descendants
still hold pipe descriptors. This ordering avoids signalling a group by a
reused leader PID after reaping. The caller must own child reaping; a competing
SIGCHLD reaper or automatic-reaping policy invalidates that ownership. A
detected loss of wait ownership refuses capture. Group cleanup is not a sandbox:
descendants can leave the group, and kernel signal delivery/reaping can be
delayed. No new worker thread survives a timeout or failed capture.

The adapter refuses setuid/setgid executable mode bits before launching a
provider. Provider installations must be trusted, must not change privileges
and must not daemonize; this source boundary does not attest file capabilities
or make arbitrary executable code safe to run.

Existing inventory/PNG subprocess entry points keep their previous behavior.
The video adapter uses the new mode for both version queries, probing and its
internal untrusted encoder primitive. Unsupported targets refuse before IO;
there is no fallback to weaker capture.

## One operation clock

`CaptureLimits.elapsed_ms` is now one shared session allowance, up to 30 seconds,
rather than an allowance renewed for each tool call. `observe_video_source`
starts it before opening the source; direct `VideoProviders::lock` starts it
before binding providers. Copies of the internal budget retain the same
monotonic deadline. Later use of an expired provider instance refuses.

Source reads, path walks, metadata checks and executable hashing call a guard
between operations and 64 KiB hash/read chunks. Probe parsing is checked before
and after. Each subprocess gets only the remaining duration. The invocation
records that duration exactly in nanoseconds, rounded up in its display
milliseconds, and records the configured operation allowance. The invocation
fingerprint also binds the new pipe-capture and shared-clock semantics.

These checks are cooperative. They cannot interrupt an individual filesystem
syscall, parser, allocator, process spawn or kernel wait. They prevent accepted
late results at the checked boundaries, not every possible wall-time overrun.
Caller JSON serialization and printing occur outside this operation.

On a failed child invocation the adapter does not start another host-file
inspection. The capture error is returned and any work-directory entries are
retained. Successful captures still require provider/source identity checks
and an empty work directory before a report can be returned.

## Remaining enforcement and candidate work

Provider memory, total scratch, escaped descendants and free-space reserve
are still unenforced. A private empty directory is not a disk quota. FFmpeg
`-max_alloc` limits an individual allocation, not process RSS. A per-process
address-space limit is not an aggregate RSS limit; a per-file size limit is not
a total scratch limit. None of those weaker controls should silently satisfy
the planner's requested policy. A later launch boundary needs an explicit
resource meaning and supported-host contract before full candidate execution.

After that boundary: independent decode, stream-copy payload/timing checks,
declared lossy comparison, container/metadata/chroma coverage and versioned
video artifact publication. Then the synthetic inspect → candidate → compare
walkthrough can become executable. Recursive batches and qualified replacement
remain later stages. The current observation still reports incomplete feature
coverage and `candidate_execution_available: false`.

## Deferred qualification

Authored cases cover bidirectional pipe pressure, exact/zero/overflow bounds,
infinite stdout, blocked stdin with infinite stderr, cancellation, timeout,
same-group descendant pipe holders, nonzero exits, pre-spawn cancellation,
shared-budget expiration and cancellation during guarded source reads.
They have not run, and no Linux/macOS behavior is qualified by this source work.

No tests, compiler/build, dependency resolution/install, lint/format, schema,
corpus/drift validators, native/provider/media execution or CI were performed.
The existing `rustix` dependency enables its `process` feature; `Cargo.lock`
is unchanged. Future locked-build and native-host checks remain required.

Primary implementation references:

- [Rust 1.85 Unix CommandExt](https://github.com/rust-lang/rust/blob/1.85.0/library/std/src/os/unix/process.rs)
- [rustix 1.1.4 wait API](https://github.com/bytecodealliance/rustix/blob/v1.1.4/src/process/wait.rs)
- [rustix 1.1.4 process signals](https://github.com/bytecodealliance/rustix/blob/v1.1.4/src/process/kill.rs)
- [rustix 1.1.4 resource limits](https://github.com/bytecodealliance/rustix/blob/v1.1.4/src/process/rlimit.rs)
