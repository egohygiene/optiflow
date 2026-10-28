# Video resource checkpoint handoff

Date: October 28, 2026 (America/New_York).
Owner: [#146](https://github.com/egohygiene/optiflow/issues/146).
Branch: `codex/146-video-resource-bounds`.
Base: draft [PR #149](https://github.com/egohygiene/optiflow/pull/149), head
`048a3ad43867405bad5ec0e96fc2bd6572b6df73`, tree
`f4f32ccaa60c38a762039e34cc50d3a5542478d2`.
This checkpoint does not merge itself or drafts #148/#149.

## Changes

Opt-in Linux/macOS nonblocking subprocess capture now aborts on output overflow,
borrows stdin without a full copy, uses no pipe worker threads, and cleans the
dedicated process group before reaping its leader. Legacy runner methods are
unchanged. The video adapter uses this mode for all calls. A shared operation
deadline covers source/provider IO, version queries, probing and reporting,
with cooperative guards around filesystem chunks and parsing. Invocation
evidence binds the changed capture semantics and records the actual remaining
child allowance.

The [resource guide](../../video-resource-bounds.md) details the exact contract.
The `rustix` process feature is enabled without a new crate/version or lockfile
change. This is source-only development and not a qualified native capability.

## Boundaries and continuation

Provider memory, total scratch, escaped-process containment and free-space
reserve remain unavailable. Host syscalls and parsing are not preemptible;
kernel spawn/reaping may overrun a deadline. The entire requested candidate
policy is still not admitted. FFprobe coverage remains incomplete; candidate
execution, independent acceptance/publication and replacement remain absent.

Next establish a supported-host launch policy for the remaining resource
requirements; then complete independent source/candidate decode and metadata
coverage, typed video publication and the synthetic walkthrough. Do not make
per-allocation, per-process or per-file controls stand in for aggregate limits.
Keep #146 open and preserve PNG #95/#121, native #111/#93, corpus #65/#136/#137
and Flow #70 scope.

## Evidence

Performed: source/issue/PR reads, implementation and independent source review,
primary Rust/rustix API inspection, Git metadata and content review.

Deferred by user request: tests, compiler/builds, dependency resolution/install,
format/lint, schema/corpus/drift checks, native/provider/media execution and CI.
No provider, synthetic media recipe or test fixture process was run. No Actions
were dispatched or polled. The new tests are authored only; no passing result
or physical storage savings is claimed.
