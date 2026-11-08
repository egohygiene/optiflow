# Produce serial source-preserving folder candidates

The #146 development library adds `video_folder::{discover,run,resume,inspect_recovery}`
and [an explicit trial example](../examples/video_folder.rs) over the existing
[guarded video producer](video-candidate-production.md). This is uncompiled/unrun
source, absent from published v0.1.1. No private collection or native trial was
processed. It does not replace/dispose of originals or reclaim their space.

The source subset remains exactly the existing tiny Linux FFV1/Matroska lane:
one video-only progressive yuv420p stream, even dimensions up to 640×360,
explicit limited-range BT.709/square pixels, 25 fps/40 ms, zero origin and
at most 250 frames. Existing byte/decode limits may admit fewer frames. Unknown
elements, CRC/lacing, chapters, attachments, audio, HDR/rotation/descriptive
metadata and unsupported preservation requirements refuse. Every actual
candidate still needs the existing independent container/packet/exact timing,
full source/candidate YUV decode equality, strictly-smaller gate, observed Linux
kernel guard and private durable publication/readback. No lossy profile or
second encoder/acceptance engine is introduced.

Use the [request shape](../examples/video-folder-request-v1.json). All tool/bus/
helper paths, synthetic digests and host settings must be replaced with actual
qualified local values. Source, work, store and journal are separate existing
canonical directories. Work/store/journal require private permissions; store
and journal must be empty for a new run. Recursion defaults to true; false
records skipped subdirectories. Existing content classification and closed
container inspection decide eligibility; a `.mkv` extension grants nothing.

Discovery is bounded before native work: at most 4,096 directory entries,
256 regular files, depth 32, 4 MiB retained inventory and 256 MiB admitted source
subject bytes, within the caller's tighter limits. Open directory iterators are
bounded by depth; a large directory is not fully buffered before count checks.
Collected paths are sorted before content-byte admission, so filesystem
enumeration order cannot change which paths fit the read budget. Directory
namespace/signature changes or discovery count overflow stop without starting
a candidate. This is an observed bounded inventory, not an atomic filesystem
snapshot. `hashed_input_bytes` counts admitted source logical bytes once;
existing bounded hashing/reobservation reads them multiple times.

No symlink is followed. Hard-link aliases and special files are skipped.
Device boundaries are skipped; same-device bind mounts are not attested.
Every eligible source gets the existing full no-follow descriptor, inode/device,
metadata and BLAKE3 binding and is rebound before production. Concurrent changes
refuse or stop the batch. Native non-UTF-8 paths remain lossless `NativePath`
records and are explicitly skipped because the current production request
requires UTF-8 locators. Nothing is converted through a lossy display name.

Only one producer invocation is active at a time. The caller caps actions
(at most 32), cumulative new publication admission (at most 256 MiB) and one
cooperative discovery/processing deadline (at most 10 minutes after static
admission). Each file gets at most the remaining deadline and the existing
30-second operation ceiling; existing cleanup keeps its separately bounded
allowance. Host syscalls/parsing/schema/readback are not preemptible. The kernel
guard still covers provider-subtree memory/scratch; total host RSS, escaped
processes, macOS support and resource calibration remain unqualified.

The batch never stages a second full source tree. Accepted candidates remain
in their individual private UUID sets; cumulative admission charges actual
candidate bytes plus the maximum 540,672-byte evidence/marker envelope per
publication. `publication_admission_bytes` is that conservative admission
account, not measured allocated storage. The private journal is capped at
16 MiB and 67 immutable members. Existing per-file temporary/scratch bounds and
free-space reserve apply independently. Availability is observed, not reserved;
concurrent non-cooperating writers can change space. Folder owners lock journal
and store directories and stop when unowned store members appear. These locks
do not turn unrelated single-file producer calls into a shared global scheduler.

`report.json` records inspected/eligible/optimized/unchanged/unsupported/skipped/
failed counts and a reason per entry. Producer failure or uncertain readback
stops further candidates; the private detail is bounded to 4,096 characters.
Only a readback-verified accepted result contributes encoded-byte savings.
`physical_space_reclaimed` and `replacement_authority` stay false. No physical
free-space improvement, percentage reduction, visual approval or release claim
follows from creating a copy.

Before each attempt the private journal synchronizes an immutable start tied
to exact request, inventory, source and effective limits; afterwards it records
the result. A candidate may already exist if cancellation/publication/durability
fails, so a missing result remains an unfinished attempt. `inspect_recovery`
rechecks bounded journal joins and recorded accepted candidate/source/artifact
bindings, returns unfinished/unstarted entries and preserves everything. It is
read-only, grants no replay authority and explicitly does not freshly validate
the original source. Repeated inspection cannot adopt scratch or launch work.
Reusing a nonempty journal/store for `run` refuses. [Reconciled continuation](video-folder-continuation.md) now handles exact resolved
starts and never-started work; uncertain recovery and reviewed cleanup remain #146 work. Inspect unresolved
producer/store state with its existing owner before a new explicitly scoped run.

After the current execution deferral ends and supported-host qualification,
the manual example flow is:

```sh
cargo run --locked --example video_folder -- discover /absolute/folder-request.json
cargo run --locked --example video_folder -- run /absolute/folder-request.json
cargo run --locked --example video_folder -- recover /absolute/private-folder-journal
```

These commands are authored instructions and were not executed. The JSON output
contains private source locators and local diagnostics; keep it local. No tools
are installed or discovered through PATH, no model/network/upload/Flow adapter
or private-media approval is added. Flow #70 still owns released placement.

Eight meaningful source/policy/discovery/journal regression cases are authored,
uncompiled/unrun. All tests/compiler/build/dependency acquisition, lint/format,
schema/corpus/drift/docs validators, native/provider/media/CLI/SDK and hosted CI
execution remain deferred. ADR-0002 remains proposed. #146/#65 retain native/
platform/adversarial/resource qualification, wider formats/lossy policies,
broader recovery qualification and later video replacement/disposal under #93/#111.
#95/#121 remain PNG-first. Full original issue/history stays open.
