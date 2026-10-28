# Video provider observation

This second development checkpoint of
[#146](https://github.com/egohygiene/optiflow/issues/146) acquires source and
provider identities and parses the richer FFmpeg-reported view needed for
video work. It builds on the review planner in
[PR #148](https://github.com/egohygiene/optiflow/pull/148).
The code and examples are authored but have not been compiled or executed.

## What is implemented

`video_observation::observe_video_source` captures at most 16 MiB from a
regular source using no-follow directory/file handles. It binds complete
BLAKE3 content, filesystem identity, permissions, ownership, link count and
timestamps, then re-observes the source before reporting. These checks detect
observed changes; they do not lock a filesystem against concurrent writers.
Reads can update access times according to the host's filesystem policy.

The provider adapter requires explicit absolute FFmpeg and ffprobe paths,
binds their complete bytes (up to 128 MiB each), verifies the exact `8.0.1`
version token, and records the complete version/build stdout digest. It
rechecks the executable before and after invocation. The hash-to-exec path
race, dynamically loaded libraries and kernel behavior are not attested by
the binary digest. Use a stable, trusted provider installation. There is no
PATH discovery, download, install, shell invocation or automatic fallback.

The full probe invocation includes streams, format, programs, stream groups,
chapters, decoded-frame observations and frame counts. It uses bounded,
immutable captured bytes on stdin and emits JSON on stdout. The parser checks
the first FFV1 subset's stream count, dimensions, per-frame rational timing,
color fields, chroma location, crop flags, dispositions, reported side data and
metadata. It refuses unsupported or missing required fields and unexpected
reported features. Source and provider hashes are acquired by the coordinator;
the pure parser alone authenticates neither.

The returned development review includes invocation arguments, content
digests, captured byte counts, source identity and stated coverage limits.
It sets `candidate_execution_available` to false. It is not a versioned
accepted-media artifact for Flow or a source-replacement authorization.

## Coverage and runtime limits

Full ffprobe output describes features exposed by that provider. It does not
prove the absence of every container metadata carrier. Nonseekable input can
also omit container information. FFprobe's successful exit and reported frames
are not independent complete-decode proof. Consequently the observed
`media.feature_observation_complete` remains false; these observations cannot
be promoted directly into an eligible planner request.

The adapter uses the existing subprocess runner with separate capture limits:

| Limit | Current behavior |
| --- | --- |
| Encoded source | At most 16 MiB retained; runner also copies stdin bytes |
| Provider executable | At most 128 MiB hashed with a bounded streaming buffer |
| Version stdout | At most 64 KiB retained |
| Probe stdout | At most 4 MiB retained |
| Internal encoder stdout | At most 16 MiB retained |
| Stderr | At most 256 KiB retained; diagnostics refuse success |
| Elapsed time | At most 30 seconds per child invocation; excludes hashing and the complete workflow |
| Provider memory, scratch, process tree, free space | Not enforced or claimed |

The output cap limits retained bytes. The existing runner drains excess output
until process exit, then refuses truncation; it does not terminate the child
as soon as the cap is crossed. Cancellation and timeout terminate the direct
child, without claiming containment of descendants. Host file IO is bounded
by byte count but has no wall-time deadline or in-read cancellation.

The work directory must already be empty, have exact mode 0700, and contain no
symlink components. Permission checks do not attest ownership or provide an OS
sandbox. Before/after emptiness checks detect unexpected scratch; they are not
a scratch quota. Unexpected entries are retained for inspection and refused,
never recursively deleted by this adapter.

An internal `capture_untrusted` method can invoke the shared stream-copy/lossy
recipe and capture untrusted bytes. It has no public optimizer command and no
candidate validator or publisher. It deliberately does not accept the
planner's `RequestedLimits`: those require enforcement this runner cannot
provide. The full optimizer coordinator remains unavailable until that
enforcement and independent candidate validation exist. A low-level successful
process exit is not a successful optimization.

## Deferred synthetic observation walkthrough

After authorizing the later execution/testing checkpoint, generate a disposable
input using the [synthetic recipe](video-synthetic-input.md). Then select the
exact separately installed provider pair and an empty private work directory:

```bash
(
  optiflow_work_dir="$(mktemp -d "${TMPDIR:-/tmp}/optiflow-video-probe.XXXXXX")" || exit 1
  optiflow_work_dir="$(cd "$optiflow_work_dir" && pwd -P)" || exit 1
  cargo run --locked --example video_observe -- \
    /absolute/canonical/path/to/source.mkv \
    /absolute/canonical/path/to/ffmpeg \
    /absolute/canonical/path/to/ffprobe \
    "$optiflow_work_dir"
)
```

The example installs cancellation handlers and prints observation JSON. It
invokes provider version queries and ffprobe, but never the encoder. It writes
no optimized media. Source and executable paths must also have no symlink
components; resolve the disposable fixture location intentionally before use.
If a provider writes unexpected files, inspect the work directory. It is safe
to remove it with `rmdir` only when empty.

This walkthrough has not been run. The checked-in JSON under
`tests/fixtures/video/` is manually authored parser input, not a captured
provider report or qualified media fixture. Native provider output may reveal
additional refusal cases during the deferred qualification work.

## Next checkpoint

Add a launch boundary that can enforce the requested resource policy before
connecting the encoder to the full candidate workflow. Add strict independent
decode, payload/timing checks for stream copy and an explicit lossy validation
policy. Resolve container/metadata coverage and H.264 encoder-side data instead
of silently accepting them. Then publish a versioned video candidate and
complete the inspect → candidate → compare walkthrough. Recursive low-space
operation and original disposal remain later stages.

Source references:
[pinned ffprobe implementation](https://github.com/FFmpeg/FFmpeg/blob/n8.0.1/fftools/ffprobe.c)
and [FFprobe documentation](https://ffmpeg.org/ffprobe.html). Source inspection
informs this design; it does not qualify the adapter on a native host.
