# Video candidate planning checkpoint

Issue: [#146](https://github.com/egohygiene/optiflow/issues/146).
Date: October 28, 2026 (America/New_York).

## Source and authority

User approved merging PR #147 and beginning the next video issue. #147 was
merged at `e8c70b381a072dbcb55f0c2930d01c5c507d4aa7`, the base of this branch,
`codex/146-video-candidate-plan`. #136 remains open for its deferred execution
acceptance; the merged source repair is not a passing corpus result.

The current request authorizes this source checkpoint and saving a draft PR.
Tests, builds, dependency resolution/install, schema/drift/corpus checks,
format/lint, provider/native/media execution and CI remain deferred. No
Actions were dispatched or polled. No private media, replacement, deletion,
release, or merge of this new checkpoint is performed.

## Checkpoint boundary

The first code is a pure typed request and deterministic FFmpeg recipe for a
declared, bounded video subset. It separates remuxing from explicitly chosen
lossy H.264 encoding and binds source, probe, policy, provider and resource
declarations. It neither acquires these observations nor authenticates them.
`PreparedForReview` is not a candidate acceptance or execution authorization.

The schema, synthetic request, library tests, preview example and disposable
input recipe evolve together. The video module is registered through the
existing contract mechanism, without a new dependency or executor framework.
PNG candidate production/publication, reports, product CLI, extension SDK,
transaction semantics and the fixture corpus remain unchanged.

The [planning guide](../../video-candidate-planning.md) describes the proposed
subset, explicit quality policy, and the outstanding producer work. The
synthetic example does not contain measurements, provider hashes or source
observations from a real invocation. No input fixture or optimized candidate
was generated. Eight focused test functions have been authored but not run or
compiled. Independent source review found no blocker; it covered schema/type
parity, explicit disposition and timing intent, recipe binding and separation
of cumulative decoded samples from peak memory. This is source review, not
execution evidence.

## Next implementation checkpoint

1. Acquire complete bounded video/timing/metadata observations from stable
   source handles; do not promote the existing minimal ffprobe descriptor to
   preservation evidence.
2. Bind and qualify the selected FFmpeg/ffprobe build and capabilities, then
   wire the recipe through the existing subprocess boundary with enforceable
   resource limits and cancellation.
3. Validate actual candidate bytes and timeline against the selected policy,
   refuse larger/changed/invalid outputs, and publish a separately versioned
   video candidate artifact with source and provider provenance.
4. Complete the synthetic inspect → candidate → compare walkthrough before
   presenting a fresh-checkout optimization trial. Recursive candidates and
   qualified replacement/disposal remain later stages of #146.

The first recipe stays useful independently of PNG replacement and the Flow
product CLI. [Flow #70](https://github.com/egohygiene/flow/issues/70) owns
configurable before/after composition once released capabilities exist.
Existing #111/PR #122, #65, #93 and PNG #95/#121 retain their own scopes.
