# Video provider observation checkpoint

Date: October 28, 2026 (America/New_York).
Owner: [#146](https://github.com/egohygiene/optiflow/issues/146).

## Source and scope

Base: draft [PR #148](https://github.com/egohygiene/optiflow/pull/148), head
`481489529c13e40b73ee40559cf42394f2827829`, tree
`8b4fd6d422bcecb1ff21aa648c18b07e6abda94f`.
Branch: `codex/146-video-observation`.
Main remains `e8c70b381a072dbcb55f0c2930d01c5c507d4aa7` after merged #147.
The user approved continuation; this checkpoint does not merge #148 or itself.

This adds read-only source/provider byte binding, exact provider version/build
observation, a bounded parser for the pinned provider's complete reported view,
and a development source-observation example. An internal encoder-capture
primitive reuses the planner's argument recipe; it returns untrusted bytes and
is not connected to a product optimizer or candidate publisher.

The existing runner cannot enforce provider memory/scratch/process-tree/free
space policy. Its output bound is retained capture, not immediate child
termination. The new adapter reports those limits explicitly and does not
consume the full planner `RequestedLimits`. The full candidate coordinator is
unavailable. No field or default converts the unavailable enforcement into a
passing result.

FFprobe output also does not establish complete container-feature coverage or
independent complete decode. Parsed `feature_observation_complete` stays false.
The observation result remains review evidence, with no accepted candidate,
physical savings or replacement authority. The separate richer source parser
does not change inventory probing, PNG validation/publication, SDK authority,
transaction semantics, dependencies or the existing filesystem corpus.

## Evidence

Performed: live source/PR/issue reads, source authoring, primary FFmpeg source
and documentation inspection, and Git metadata/content review. Independent
source review found no concrete blocker in the file bindings, parser, adapter,
coordinator and example. This is inspection, not compile/runtime evidence.

Deferred: all tests, compiler/builds, dependency resolution/install,
schema/drift/corpus checks, format/lint, provider/native/media execution and CI.
No example, fixture generator or provider was invoked. No Actions were
dispatched or polled. The JSON fixture is manually authored synthetic provider
input, not an observed report. Seventeen new test functions (six file-binding,
three provider/recipe and eight parser cases) are authored only and have no
passing result yet.

## Continue from here

The [observation guide](../../video-provider-observation.md) records exact
capture limits, incomplete coverage and the deferred walkthrough. Next work
must establish resource enforcement before full candidate execution, strict
decode and metadata coverage, candidate-policy validation and versioned video
publication. Preserve the distinction between reported observations, untrusted
captured bytes and independently accepted candidate artifacts.

#146 stays open. #136/#137 still need their own deferred qualification;
#111/PR #122, #65, #93 and PNG #95/#121 retain scope. Flow #70 remains the owner
of before/after orchestration through released compatible artifacts.
