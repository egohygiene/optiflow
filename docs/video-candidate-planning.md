# Video candidate planning

This is the first development-source checkpoint of
[#146](https://github.com/egohygiene/optiflow/issues/146). It prepares a typed,
review-only FFmpeg recipe from supplied declarations. It does not inspect
media, invoke FFmpeg, produce an optimized file or grant replacement authority.
The published Optiflow release and product CLI gain no video command here.

The follow-on [provider observation checkpoint](video-provider-observation.md)
adds host-acquired source/provider identities and richer reported video facts.
Its coverage remains incomplete, and its internal encoder capture does not
execute this planner's full resource policy or publish an accepted candidate.

## First declared subset

Start with a bounded Matroska input containing one FFV1 video stream:
progressive 8-bit `yuv420p`, limited-range BT.709 SDR, square pixels and a
zero-based, constant-rate timeline. Audio, subtitles, attachments, data,
chapters, rotation, HDR and unknown/descriptive metadata are outside this
first subset. Stream disposition must be explicitly observed as either none
or default and retained in the recipe. The policy explicitly permits
regeneration of encoder/duration bookkeeping only. Unknown observations cannot
be represented as an assumption that an unsupported feature is absent.

This first planning profile is limited to even dimensions up to 640x360,
25/1 fps, a 1/1000 time base and at most 250 frames (10 seconds). Requested
ceilings may be reduced from the following maxima; the planner does not
enforce them on a process:

| Requested budget | Maximum |
| --- | --- |
| Source or candidate bytes | 16 MiB each |
| Decoded sample bytes per video | 64 MiB |
| Temporary bytes | 32 MiB |
| Peak memory | 256 MiB |
| Elapsed time | 30 seconds |

The decoded limit counts the summed 8-bit YUV420 frame payload separately for
the source and candidate. It does not measure allocator overhead or encoder
working memory. Cumulative decoded samples and peak memory are different
budgets; a streaming validator should not retain all decoded frames in memory.

The [synthetic request](../examples/video-candidate-request-v1.json) describes
320x180, 25 fps, 50 frames over two seconds, with a 1/1000 time base. Its
digests and provider paths are synthetic declarations, not observed files or
installed binaries. The [input recipe](video-synthetic-input.md) describes how
to create disposable input later; it has not been executed for this checkpoint.

Two explicit operations are proposed:

- Stream-copy/remux to Matroska, retaining the selected encoded stream.
  Container bytes can change and savings are not guaranteed. A future
  validator must still establish payload and timeline preservation.
- Lossy H.264 encoding through `libx264`, also to Matroska. CRF and preset
  must be supplied. There is no default quality target, exact-pixel promise
  or inferred user acceptance of a quality change.

The producer and probe declarations target FFmpeg/ffprobe 8.0.1, following the
existing suite demonstration pin. Each records executable identity and
invocation evidence; an exact version declaration does not qualify a build,
its linked libraries or encoder availability. No provider is installed or
discovered by this planner.

## Review a request

The library offers `parse_request` and `prepare_candidate`. The closed
[request schema](../schemas/video-candidate-request-v1.schema.json) and typed
checks reject unsupported profiles, missing policy, incompatible declared
features, invalid timing or limits, and extra fields. The result is
`PreparedForReview`: request and preparation fingerprints, a proposed argument
array and outstanding validation requirements. It is not an accepted candidate
or an executable authorization.

The `preparation_fingerprint` additionally binds the derived arguments,
descriptor bindings and environment requirements to the request fingerprint. Both are
BLAKE3 digests over domain-separated typed serialization, following the
module's documented field order. Provider build fingerprints are declarations
for the complete captured `-version` stdout bytes, including configuration and
library-version lines, without trimming. No fingerprint here authenticates
its supplied input. The first profile accepts normalized absolute UTF-8 Unix
provider paths only; no filesystem lookup occurs.

At the later execution/testing checkpoint, the development example can print
that review without running a media provider:

```bash
cargo run --locked --example video_candidate_plan -- \
  examples/video-candidate-request-v1.json
```

This command has not been run here; it requires compiling the development
source. Edit the sample only as a review exercise. Supplying new hashes or
claims does not authenticate a source or make it eligible for optimization.
The sample selects stream copy. To review a lossy recipe, explicitly replace
its `operation` with, for example,
`{"kind":"h264_lossy","crf":23,"preset":"medium"}`. Those numbers are
an illustrative policy choice, not a recommended setting or an application
default. This packet is a development contract; the eventual user command
should acquire observations and construct it without asking users to write
hashes or media declarations by hand.

The recipe's descriptor/stdout bindings belong to a future coordinator; do
not paste its argument array into a shell and treat it as the product workflow.

## What the producer still needs

The next checkpoint must acquire richer observations from a stable, read-only
source handle, verify the exact provider/build and supported capabilities,
enforce the declared budgets, and produce one candidate into owned staging.
It must independently inspect and fully decode both sides; check stream
count, timestamps, frame coverage, aspect, color and metadata policy; and
re-observe the source before publishing a strictly smaller candidate.
Remux and lossy validation have different acceptance rules. Lossy encoding
cannot establish exact decoded equivalence, and a quality metric alone does
not authorize replacement.

The existing scan descriptor lacks the observations needed for this profile.
The existing candidate artifact publisher validates PNG-specific members and
evidence; video needs its own versioned publication contract. Reuse existing
subprocess, identity and staging patterns without changing PNG semantics.
The subprocess runner currently bounds direct-child time and captured bytes;
that is not enforcement of process-tree, RSS or scratch-space ceilings.

Flow will bind explicit upstream and downstream artifact identities through
released adapters. It can place eligible optimization before rendering or
after artifact generation, as tracked in
[Flow #70](https://github.com/egohygiene/flow/issues/70). This pure planner does
not expose a Flow adapter, an extension producer role or folder traversal.
Restorable replacement and explicit irreversible original disposal remain
later, separately qualified steps of #146.

## Evidence

Only source review and authoring were performed. Tests, builds, schema/drift
checks, formatter/linter, provider/media execution and CI remain deferred.
There is no measured size reduction, generated video or native qualification
from this checkpoint. See the [handoff](work/146/HANDOFF.md).

Primary FFmpeg references used for the proposed recipe:
[stream selection, mapping and timestamps](https://ffmpeg.org/ffmpeg.html),
[libx264 options](https://ffmpeg.org/ffmpeg-codecs.html#libx264_002c-libx264rgb),
and [Matroska muxing](https://ffmpeg.org/ffmpeg-formats.html#matroska).
These document provider behavior; they are not executed evidence for this
Optiflow source or the selected build.
