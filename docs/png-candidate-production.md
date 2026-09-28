---
title: Bounded PNG candidate production
description: Source-preserving OxiPNG invocation, independent validation, candidate publication, and recovery in development source.
---

# Bounded PNG candidate production

OptiFlow [#94](https://github.com/egohygiene/optiflow/issues/94) adds an
explicit candidate-producing path after the [synthetic #112 fixture
tranche](png-corpus.md). This is development source, **not part of the signed
read-only v0.1.1 release**. The candidate is a new file in private OptiFlow
state. This command does not replace, move, quarantine, or delete the source;
it grants no approval for the separate [#95 replacement
work](https://github.com/egohygiene/optiflow/issues/95).

```bash
optiflow candidate png --source "/absolute/path/to/source.png" \
  --oxipng "/absolute/path/to/oxipng"

optiflow candidate status --id "<candidate-set-uuid>"
optiflow candidate recover --id "<candidate-set-uuid>"
```

Use `--state-directory` to select a local state location. The candidate root
must exclude group and other access. The CLI resolves relative
source paths into absolute, normalized paths and refuses parent traversal;
all source components must be free of symlinks and the source must be a regular
file. The executable is an explicitly selected path resolved to a canonical
regular non-symlink binary. OptiFlow does not search `PATH` or download it.
Use synthetic or disposable
test images for this development command. It does not inherit authorization
from an old review plan, extension declaration, or PNG opportunity.

## Provider and supported profile

The adapter requires **OxiPNG v10.2.1**, separately installed by the operator.
The [upstream project](https://github.com/oxipng/oxipng/tree/v10.2.1) uses the
MIT license; this repository does not bundle or redistribute its executable.
Building this OxiPNG version from source requires Rust 1.88; OptiFlow's
separate Rust 1.85 minimum is unchanged.
OptiFlow observes the complete executable bytes with BLAKE3-256, checks its
version, binds the canonical path and an invocation fingerprint, and rechecks
the file around invocation and before candidate publication. A path swap
between the last check and the operating system's exec remains a host race;
use a private, stable executable installation. A binary digest does not attest
its publisher, dynamic dependencies, or isolation.

The fixed direct invocation uses `--opt 2 --nx --interlace keep --threads 1
--max-raw-size <bound> --quiet --stdout -`. The final `-` requests standard
input; OptiFlow supplies its already read, bounded source bytes through a
pipe, captures bounded standard output as **untrusted** candidate bytes, and
clears the child environment. The working directory is private. It does not
pass the source path to OxiPNG, select metadata stripping, or trust the
provider's result as validation. OxiPNG may exit successfully with unchanged
bytes; a candidate must still be strictly smaller.

The current [`optiflow.png-idat-preserve.v1` byte
profile](png-candidate-contract.md#actual-byte-validation) accepts static,
noninterlaced, 8-bit RGB/RGBA PNGs with its listed supported metadata subset.
It refuses interlace, animation, indexed/grayscale images, other bit depths,
and unsupported metadata. Before invoking the provider, the host completely
validates the source. Afterward it independently validates both byte streams,
their complete decodes and exact samples (including invisible RGB under zero
alpha), ordered non-IDAT chunks and placement, and a positive encoded-byte
reduction. The provider cannot turn an unsupported input into an accepted
candidate.

## Bounds and observations

The current per-invocation ceilings are fixed by the coordinator; the library
API can request **lower** values. All values are byte counts unless shown
otherwise.

| Scope | Maximum |
| --- | ---: |
| Source encoded input | 16 MiB |
| Candidate captured output | 16 MiB |
| Decoded bytes per image and decoder allocation allowance | 64 MiB each |
| Executable bytes hashed | 64 MiB |
| Captured standard error | 64 KiB |
| Published evidence JSON | 1 MiB |
| PNG chunks per image | 256 |
| Elapsed time per child invocation (version query and optimize separately) | 30 seconds |
| Concurrent provider children per runner | 1 |

The production coordinator serializes candidate work within its process, and
the runner allows one child at a time; separate OptiFlow processes have no
shared candidate-production limit. The evidence `elapsed_ms` records the
optimize child stage, not the version query or whole command. The OxiPNG
`--max-raw-size` value is computed from the checked filtered scanline
size and bound source input. The host limits source reads, stdout/stderr
capture, process time and validator allocations; it kills and waits for a
timed-out or cancelled direct child. **Peak provider RSS and private workdir
usage are unavailable**, recorded as `null`, and not represented as measured
or enforced caps. A provider could create temporary files or descendant
processes outside these direct-child bounds; an independent OS sandbox and
full process-tree resource accounting are outside this issue. Do not treat
the declared decoder allowance as whole-process peak memory.

The host snapshots source device/inode, type/mode, owner/group, link count,
logical length, mtime and ctime, and directly rereads and compares all source
bytes after provider execution and immediately before publication. On a
changed source, changed provider, invalid/larger output, nonzero exit,
unexpected stderr, timeout, cancellation, overflow or unsafe state directory,
the command refuses. The source is never passed as an output target. A hostile
concurrent filesystem writer is beyond an atomic compare-and-write guarantee.

## Evidence and durable publication

Successful production emits
[`optiflow.png-candidate-evidence.v1`](../schemas/png-candidate-evidence-v1.schema.json)
and a separate
[`optiflow.png-candidate-artifact-set.v1`](../schemas/png-candidate-artifact-set-v1.schema.json)
marker. The evidence binds the source path and before/after snapshots, complete
source and candidate digests/lengths, effective policy fingerprint, producer
path/version/binary digest/invocation, exact argv and input/output binding,
configured limits, observed lengths/time, independent PNG facts and
preservation checks. It records **encoded logical reduction** separately from
`physical_savings_bytes: null`. Keeping the source and candidate occupies
additional storage and proves no reclaimed physical space.

The candidate set is distinct from scan/plan `artifact-set.v1`, whose closed
member vocabulary is JSON-only. A private same-filesystem staging directory
receives `candidate.png`, `evidence.json`, and
`candidate-artifact-set.json`. Each member and directory is synchronized; the
marker binds exact sizes and BLAKE3-256 digests. After final source/provider
rechecks, publication renames staging to the UUID namespace without replacing
an existing path, then synchronizes its parent directory. A reader accepts
only the final UUID directory with all three regular, singly linked members
matching the marker. The candidate and marker are evidence for review, not a
replacement authorization or a guarantee that every process/OS resource was
contained.

## Inspection and interruption

`candidate status` inspects the committed namespace read-only. Its result is
`committed`, `incomplete`, or `incompatible`; a marker in a staging directory
alone is **never committed**. `candidate recover` inspects an existing final
set, or discards only the exact abandoned private staging namespace if no
final set exists. It never promotes staging, overwrites an existing namespace,
or changes a source file. After an abrupt stop, inspect the state candidate
root for a leftover `.provider-work-UUID` directory; recovery for that UUID
returns `incompatible` and retains the directory for manual inspection. A
simultaneous final/staging namespace,
altered members, unsafe paths, or another ambiguous state likewise requires
manual inspection.
Filesystem publication and CLI result rendering cannot be assumed to commit
atomically; a failed parent-directory sync can leave a visible result whose
durability needs inspection. Recovery does not infer source-replacement
authority from this candidate evidence.

Tests for this issue use only original synthetic fixture media and controlled
fake providers. The [#65 corpus](https://github.com/egohygiene/optiflow/issues/65)
remains open for broader media families and stress cases. Native macOS and
external-drive qualification for the separate mutation release remain gated
by [#111](https://github.com/egohygiene/optiflow/issues/111) and
[#93](https://github.com/egohygiene/optiflow/issues/93).
