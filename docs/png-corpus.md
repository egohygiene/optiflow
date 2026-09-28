---
title: Synthetic PNG candidate corpus
description: Reproduce the bounded PNG and contract fixture tranche for OptiFlow #112.
---

# Synthetic PNG candidate corpus

[#112](https://github.com/egohygiene/optiflow/issues/112) contributes the
PNG candidate portion of the open [#65](https://github.com/egohygiene/optiflow/issues/65)
corpus. These are original synthetic bytes, not downloaded images or
observations of user media. They exercise the read-only byte validator and
the declaration-only candidate-contract checker before
[#94](https://github.com/egohygiene/optiflow/issues/94) implements an actual
OxiPNG runner. No provider is invoked and no media is replaced.

## Reproduce and review

```sh
python3 scripts/png-corpus.py --check
cargo test --locked --test png_corpus --test png_provider_corpus
```

`tests/fixtures/media/cases.json` defines stable source/candidate recipe
IDs, a zero seed, media type, validity class, MIT provenance, and exact typed
expected results. `scripts/png-corpus.py` uses Python's standard library
and emits its small PNGs from original pixels and explicit PNG/zlib recipes;
the fixed DEFLATE blocks are constructed directly, independent of the host
zlib encoder version. `tests/fixtures/media/index.json` binds the generator,
catalog, proof sources, provider recipe catalog, and each emitted PNG's
SHA-256 and size. `--check` regenerates every fixture in memory and
compares the complete bytes and index; it never rewrites canonical files.
After reviewing an intentional change:

```sh
python3 scripts/png-corpus.py --write-index
git diff -- scripts/png-corpus.py tests/fixtures/media/ tests/png_corpus.rs tests/png_provider_corpus.rs
```

The Rust test reads the committed bytes under a 64 KiB encoded-byte ceiling
per image, a 1 MiB decoded-byte and decoder-allocation ceiling per image,
and a 128-chunk ceiling. Individual boundary cases may tighten those limits.
It calls `validate_png_pair`, checks the named `ByteRefusal` or a strictly
positive observed encoded-byte reduction, observes BLAKE3 content identities
and equal PNG facts on success, and rereads the original and candidate after
each case. These ceilings are API limits, not a measured peak RSS or a general
resource sandbox. The CI job has its own wall-clock timeout. No scheduled
heavy PNG tier is claimed by this small tranche.

## Contract coverage

| Fixture family | Proof here | Remaining responsibility |
| --- | --- | --- |
| RGB/RGBA encoded reduction | Actual synthetic PNG bytes decode with equal samples and preserved chunks, including zero-alpha RGB and metadata before/after IDAT | #94 must produce a candidate with a locked real executable and reobserve source and output |
| Equal/larger or changed candidate | Typed byte refusal; encoded logical bytes are never described as physical savings | #94 must reject and clean its staged output |
| Corrupt, trailing, incomplete, unsupported PNG | Typed checksum, structure, stream, or profile refusal | #94 must enforce process/output/temporary-space limits and cleanup on all failures |
| Source corruption or unsupported metadata | Refusal before accepting the candidate | #94 must bind current source identity before and after execution |
| Provider/host disagreement | `provider_cases.json` applies synthetic mutations to the v1 example and asserts named `ContractRefusal` results | #94 must actually launch, observe, limit, and recover a real provider transaction |

The provider catalog's example values are **declared test claims**. A failed
status, changed executable digest, missing host evidence, stale source,
candidate alias, failed check, or bound violation here proves the pure
checker refuses that declaration. It does not prove a subprocess timed out,
was cancelled, emitted bounded output, or ran OxiPNG. #94 must add those
runtime and candidate-artifact proofs, including provider absence, nonzero
exit, timeout, cancellation, output overflow, malformed success, and cleanup.
The grayscale fixture independently decodes as a complete PNG but is outside
the current RGB/RGBA profile. Indexed, interlaced, APNG, and iCCP examples are
early-refusal sentinels, not full conformance samples for those features.

The broader #65 umbrella still owns other media profiles, perceptual pairs,
released compatibility, and heavier resource-stress fixtures. A minimized
fuzz finding should receive a stable ID, a small deterministic recipe, named
typed expected behavior, and a reviewed index update before joining this
corpus. This finite sample does not certify arbitrary PNGs or other formats.
