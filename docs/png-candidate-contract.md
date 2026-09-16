---
title: PNG candidate contract
description: PNG candidate declarations, actual byte validation, and remaining execution boundaries.
---

# PNG candidate contract

Issue [#78](https://github.com/egohygiene/optiflow/issues/78) defines
`optiflow.png-candidate-contract.v1`, a **contract-only** review packet for a
future lossless PNG provider. Issue
[#80](https://github.com/egohygiene/optiflow/issues/80) adds a separate read-only
byte validator described below. No encoder is selected or invoked; the
existing CLI and read-only v0.1 authority are unchanged.

The [schema](../schemas/png-candidate-contract-v1.schema.json), Rust
`png_candidate` module, and [synthetic example](https://github.com/egohygiene/optiflow/blob/main/examples/png-candidate-contract-v1.json)
form one specification and conformance boundary. All paths, tools, repeated
hexadecimal content digests, PNG facts, sizes, and resource measurements in the
example are **invented test declarations**, not observed files or measurements.
Only its request fingerprint is computed from the declared request.

## Ownership and compatibility

The packet is separate from scan reports, review opportunities, executable
plans, extension SDK v1, and artifact-set v1. It does not add a new extension
role or permission. Run v5, report v6, plan v5, command-result v1, effective
policy v1, and SQLite are unchanged. OFD-001's review-only boundary remains in
force; no accepted architecture decision is superseded.

The provider result and host evidence have separate shapes. A provider may
report its status and claimed candidate content identity. It cannot place
validation or host-observation fields inside that result. A future coordinator
must acquire the host observations itself rather than copying provider stdout.
Deserializing a `HostEvidence` value does not authenticate its origin.

The packet checker's success result is `ConsistentForReview`. It is evidence-consistency
feedback, not an accepted media artifact, runtime safety attestation, creative
approval, or permission to execute. No current application path consumes it.

## Narrow preservation profile

`optiflow.png-idat-preserve.v1` covers static **8-bit RGB or RGBA** PNGs only.
Indexed, grayscale, other bit depths, APNG, and unavailable facts are refused.
This is a boundary for initial implementation, not a claim about all PNGs.

An eventual byte validator must establish:

- One complete frame with positive, unchanged dimensions and the same IHDR.
- Full PNG structural/decode validation, including chunk CRCs, critical-chunk
  validity, stream completion, and no unaccounted trailing bytes.
- No `acTL`, `fcTL`, or `fdAT` animation chunks.
- Exactly equal decoded samples, including otherwise invisible RGB samples
  beneath zero alpha. Do not premultiply alpha, color-convert, strip precision,
  or use a perceptual comparison.
- Exact ordered preservation of every non-IDAT chunk, including metadata,
  color/transfer declarations, transparency, and its placement before or after
  IDAT. Unknown unsafe-to-copy ancillary chunks must cause refusal before
  recompression; recognized chunks and unknown safe-to-copy chunks retain
  their exact bytes and placement.
- Current source identity/content before and after execution and validation;
  an independently observed regular, non-symlink candidate distinct from the
  source filesystem object and path.
- A candidate with strictly fewer **encoded logical bytes** than the source.

The canonical decoded sample stream is the unfiltered, deinterlaced image in
top-to-bottom row order, left-to-right pixel order, with packed 8-bit RGB or
straight RGBA samples and no row padding. Its length must equal width × height
× channels. BLAKE3-256 hashes exactly those bytes. The IHDR digest hashes the
13-byte IHDR payload. The preservation digest hashes the following unambiguous
stream: UTF-8 `optiflow.png-preservation.v1` and a zero byte, followed in file
order by tagged records. Each non-IDAT chunk contributes byte `0x00`, its
complete chunk byte length as an unsigned 64-bit big-endian integer, and its
original length/type/data/CRC bytes. The one consecutive IDAT run contributes
exactly one byte `0x01` instead of any IDAT bytes. Include IHDR and IEND records.
This preserves the position of chunks relative to IDAT while permitting IDAT
repartitioning/recompression. No other chunk bytes or placement may change.
The selected validator must identify unknown unsafe-to-copy chunks and report
their presence; that observation is always refused by this first profile.
These requirements follow the [PNG editing rules](https://www.w3.org/TR/png-3/#14Editors).
Digest equality is a compact record; the runtime must
perform actual complete comparisons and validation before reporting a pass.

Keeping the original and candidate increases occupied storage. The contract
reports only `reported_encoded_byte_reduction`, calculated from the supplied
lengths. It makes no physical-space, reclaimability, or measured-savings claim.

## Bindings and checks

The request binds one source observation ID, canonical native path, filesystem
identity, observation fingerprint, **complete source-content digest**, logical
length, exact profile, effective policy, producer, validator, and limits.
An existing #66 opportunity fingerprint is not a complete content hash and
cannot substitute for one.

Tool bindings include name, version, canonical executable path, executable-byte
digest, and invocation fingerprint. Future implementations must define and
record their immutable argv/configuration/input-binding recipe. A binary digest
does not authenticate its publisher, dynamic dependencies, or sandboxing.
Host evidence records both tools before and after the observation window.

The provider result and host evidence each bind the full request fingerprint.
One host record scopes all eight required checks to exactly its source,
candidate, tools, and policy:

| Check | Required result |
| --- | --- |
| `source_stable` | Same current source before and after |
| `complete_decode` | Both complete decodes and structural validation pass |
| `dimensions` | Same positive dimensions and IHDR |
| `frame_count` | Exactly one static frame each |
| `decoded_samples` | Exact canonical sample comparison |
| `alpha_preserved` | Alpha representation and samples preserved |
| `color_preserved` | Color representation and declarations preserved |
| `metadata_preserved` | Ordered non-IDAT bytes preserved |

Each check must occur exactly once and be `passed`. Failed, unsupported,
not-run, missing, duplicate, and unknown checks cannot satisfy the contract.
The pure checker also compares the supplied PNG facts; passing flags cannot
override contradictory digests, sizes, frames, dimensions, or preservation.

Limits cover input bytes, candidate bytes, decoded bytes per image, peak
temporary bytes, peak process memory, and elapsed milliseconds. Declared usage
must fit the limits. These are future enforcement requirements, not enforcement
provided by this pure checker. A future runner must state its peak-memory
measurement/enforcement scope and refuse claims it cannot substantiate.

## Deterministic request identity

The request fingerprint is BLAKE3-256 over the UTF-8 domain separator
`optiflow.png-candidate-request.v1` followed by a zero byte and the compact
`serde_json` serialization of the typed `CandidateRequest`. Struct field order
is fixed by the v1 model and the checked-in example; nested domain structs use
their declared field order. There are no floating values or unordered maps.
This is not a hash of arbitrary incoming JSON formatting. Consumers must parse
and validate first; reordering input JSON keys does not change the identity.

Canonical paths use the existing NativePath encoding, absolute paths without
dot segments or repeated separators, and its canonical UTF-8/base64 choice.
The checker does not resolve filesystem symlinks. Runtime handle/path binding
and complete source/candidate observations remain mandatory later.

Use `parse_contract` for wire input: it validates the raw schema before typed
deserialization so shared legacy domain types cannot discard unknown fields.
`check_candidate_contract` checks a typed packet and returns a closed
`ContractRefusal` code on disagreement. It performs no IO. Invalid-shape and
semantic-refusal tests are separate because a schema-valid claim can still be
inconsistent.

## Actual byte validation

The library function `png_validation::validate_png_pair` accepts immutable
source/candidate byte slices and explicit `ByteValidationLimits`. It opens no
files, invokes no provider, and creates no output. Its opaque
`ValidatedPngPair` has private fields and no deserializer: declarations from
the contract example cannot be converted into observed byte evidence.

It uses the pinned [`png` 0.18.1 decoder](https://docs.rs/png/0.18.1/png/struct.Decoder.html)
(declared MSRV 1.73, below OptiFlow's 1.85), with identity transformations and
checksum checking explicitly enabled. A separate chunk scanner verifies all
CRCs, framing, ordering, and supported metadata before decoding. A bounded
`flate2` zlib pass requires the complete stream, correct Adler-32, exact
filtered image length, and consumption of all IDAT data. This compensates for
the decoder's intentional tolerance of unused compressed bytes and malformed
ancillary metadata. PNG permits readers to ignore unused final IDAT bytes;
this validator's refusal is a
[stricter profile rule](https://www.w3.org/TR/png-3/#11IDAT), not a claim that
every refused file violates PNG.

The current implementation supports a **subset** of
`optiflow.png-idat-preserve.v1`:

| Input | Current behavior |
| --- | --- |
| Static, noninterlaced 8-bit RGB/RGBA | Complete decode and exact samples |
| `PLTE`, RGB `tRNS`, `gAMA`, `sRGB`, `pHYs` | Check shape, multiplicity and placement; preserve exact bytes |
| Uncompressed `tEXt` | Check keyword/text structure; preserve exact bytes and order |
| Unknown private safe-to-copy ancillary chunks | Preserve opaque bytes and placement; no semantic interpretation |
| Interlace, grayscale, indexed color, other bit depths, APNG | Explicit refusal |
| Other public/registered metadata, including `iCCP`, `zTXt`, `iTXt`, `cHRM`, `eXIf` | Explicit refusal; no compressed-metadata expansion |
| Unknown critical or unsafe-to-copy chunks | Explicit refusal |

Private safe-to-copy chunks have lowercase first, second and fourth type
letters, and an uppercase reserved third letter. Supported metadata validation
is structural; this is not an ICC/colorimetric or general PNG conformance
certifier. Adding metadata/interlace support requires its own bounded tests.

Both actual canonical sample buffers and every ordered preserved raw chunk
are compared directly, with one marker retaining the IDAT run's position.
This catches alpha changes, invisible RGB changes, metadata additions/removals,
and movement across IDAT even when visible rendering is identical. Only then
does the validator expose `ContentIdentity`, `PngFacts` and a strictly positive
`encoded_byte_reduction`, computed from the observed slices with the digest
recipes above. No serialized schema changes are required. The existing pure
contract checker retains its declaration-only semantics.

Limits independently bound source bytes, candidate bytes, decoded bytes per
image and chunks per image. Checked dimensions are refused before image-sized
allocation. Allocation failures in caller-owned sample/chunk/IDAT buffers
return a limit refusal. `decoder_allocation_bytes` configures the PNG library's
**best-effort internal allocation accounting**, excluding caller input, output
samples, IDAT copies, zlib state and bookkeeping. Two decoded images are held
for comparison. These ceilings are not peak RSS, wall-clock enforcement,
process isolation or a substitute for `CandidateLimits`. The API produces no
`ResourceUsage` measurements or `HostEvidence` packet.

Run the generated, original synthetic PNG proofs with:

```sh
cargo test --locked --test png_byte_validation
```

These tests include different compression/filter/IDAT layouts, transparency,
metadata placement, changed samples, corrupt chunk CRCs and zlib checksums,
truncation, extra compressed/decompressed bytes, unsupported input, and exact
budget boundaries. They are functional fixtures, not the broader redistribution
corpus tracked by [#65](https://github.com/egohygiene/optiflow/issues/65).

## Next execution checkpoint

After this validator is reviewed, scope one real source-preserving provider
using independently produced byte-validation evidence, a complete source-handle
window, bounded workspace/process behavior, and candidate-media publication.
That checkpoint must explicitly resolve the architecture and release boundary
before adding runtime execution. Existing Scan/Plan JSON artifact publication
does not already provide a media-output commit protocol.

Keep provider absence/failure, source changes, output corruption, no reduction,
budget exhaustion, cancellation, and cleanup as tested refusals. Provider exit
zero never substitutes for host validation. Transactional replacement, batch
execution, metadata stripping, and lossy/perceptual policies remain separate.
