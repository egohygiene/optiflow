---
title: PNG candidate contract
description: Contract-only preparation for source-preserving PNG candidate evaluation.
---

# PNG candidate contract

Issue [#78](https://github.com/egohygiene/optiflow/issues/78) defines
`optiflow.png-candidate-contract.v1`, a **contract-only** review packet for a
future lossless PNG provider. No encoder is selected or invoked. No PNG is
decoded, created, published, replaced, or deleted by this checkpoint. The
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

The only success result is `ConsistentForReview`. It is evidence-consistency
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

## Next execution checkpoint

After this contract is reviewed, scope one real source-preserving provider
with independently produced byte-validation evidence, a complete source-handle
window, bounded workspace/process behavior, and candidate-media publication.
That checkpoint must explicitly resolve the architecture and release boundary
before adding runtime execution. Existing Scan/Plan JSON artifact publication
does not already provide a media-output commit protocol.

Keep provider absence/failure, source changes, output corruption, no reduction,
budget exhaustion, cancellation, and cleanup as tested refusals. Provider exit
zero never substitutes for host validation. Transactional replacement, batch
execution, metadata stripping, and lossy/perceptual policies remain separate.
