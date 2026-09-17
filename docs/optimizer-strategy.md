---
title: Media capabilities and optimizer strategy
description: Evidence behind the format matrix, planned provider ownership, and image_optim prior art.
---

# Media capabilities and optimizer strategy

The [README capability matrix](https://github.com/egohygiene/optiflow#media-capability-matrix)
separates inventory, probing, review evidence, byte validation, production,
and apply. This page records the evidence behind those cells and the existing
optimizer direction; it adds no runtime capability or execution authority.

## Evidence snapshot

Reviewed on **2026-09-17** against OptiFlow
[`5be461c413fc34515cd70f41d4178eb8243525d9`](https://github.com/egohygiene/optiflow/tree/5be461c413fc34515cd70f41d4178eb8243525d9),
after [PR #82](https://github.com/egohygiene/optiflow/pull/82) merged. The matrix
describes merged source, not the installed-release surface. The
[`v0.1.0` tag](https://github.com/egohygiene/optiflow/tree/f04c82a0b0c677a2939ea351c4219602cd7181af)
predates the media-profile and PNG candidate modules, while current
[`Cargo.toml`](https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/Cargo.toml)
still declares `0.1.0`. A version string alone does not prove these later
capabilities are present in a binary; consult its release/source provenance.

**✓** means implemented at this source pin, **~** means conditional or a
limited subset, **P** means roadmap-only, and **—** means no implementation or
selected format-specific profile. Planned evaluation does not promise support.

## Inventory and exact duplicates

Any readable regular file can participate, including unknown formats and
files with missing or misleading extensions. Discovery policy, readability
and stable observation still apply; a failed observation is not accepted
evidence. Format recognition is unnecessary for inventory and exact grouping.

Current exact groups require equal logical byte length and equal complete
BLAKE3-256 hashes. This is review evidence, not a byte-by-byte comparison made
at deletion time. A future destructive action must revalidate identity/content
and directly compare bytes. Hard links, independent copies, logical bytes,
allocated bytes and uncertain reclaimability remain distinct.

Pinned implementation: [discovery][discovery], [handle observations][observation],
[inventory][inventory-source], [duplicate analysis][duplicates], and
[review planning][planning]. The [safety model](safety-model.md) describes the
future apply gate.

## Classification and optional probing

The scanner classifies an up-to-8192-byte prefix with the locked `infer`
dependency. MIME classification must be image, audio or video before the
built-in `ffprobe` path is considered. Probing must also be enabled, the
provider must be available, and its bounded invocation/result must succeed.
Unknown/other classifications remain inventory candidates but are not probed.
Changing a filename extension does not bypass this gate. See the pinned
[classification and dispatch][inventory-source], [lockfile][lockfile], and
[`ffprobe` adapter][ffprobe].

The locked `infer 0.22.0` implementation includes recognizers for PNG, JPEG,
GIF, WebP, JPEG XL, AVIF, HEIF and TIFF, plus some RAW signatures such as Canon
CR2. This is eligibility evidence, not a stable format-support registry:

- SVG has no built-in image recognizer, so its README row is inventory-only.
- RAW is a family, not one supported codec; recognizing CR2 or a TIFF-like
  container does not establish general RAW support.
- AVIF/HEIF recognition depends on container brands and available prefix
  bytes; the suffix is insufficient.
- Audio/video labels cover recognized containers, not every possible codec
  or stream combination.
- Even recognized files may be unsupported or invalid for the installed
  `ffprobe` build. A successful metadata probe is not a complete decode,
  candidate validation, or optimization guarantee.

The recognizer evidence is the
[versioned matcher table](https://docs.rs/crate/infer/0.22.0/source/src/map.rs)
and [image matchers](https://docs.rs/crate/infer/0.22.0/source/src/matchers/image.rs).
Do not turn that dependency's extension list into an OptiFlow support promise.

## Current boundaries

Only PNG has a built-in versioned optimization-review profile. Its
[selection rules](media-profiles.md#selection-and-evidence) require current,
provider-bound observations, `png_pipe`, one PNG video stream with positive
dimensions and no audio facts. That profile does **not** enforce the later
byte validator's RGB/RGBA/noninterlaced subset. An opportunity means that
recompression may be evaluated later, with no estimated savings or output.
See [profile implementation][profiles] and [fixtures][profile-tests].

The [candidate contract](png-candidate-contract.md) checks consistency of
supplied declarations; it does not authenticate those declarations. Separately,
[`png_validation::validate_png_pair`][png-validation] compares actual immutable
source/candidate slices. Its supported subset is static noninterlaced 8-bit
RGB/RGBA with explicitly supported metadata. It checks complete streams,
exact samples (including invisible RGB), ordered non-IDAT bytes and placement,
and strictly smaller encoded length. [Real synthetic fixtures][png-tests]
exercise success and refusal. Its byte limits and best-effort decoder
allocation accounting are not process-memory or wall-clock enforcement.

This is a **library-only** validator. The [CLI command definitions][cli] and
[application dispatch][app] do not invoke it, generate candidates, optimize,
apply, replace, delete or quarantine source media. No format currently has a
candidate producer or transaction engine. Other formats have no built-in
optimization-review profile or candidate byte validator; ordinary inventory,
probe metadata and extension roles are not substitutes for these capabilities.

## Format plans

The existing [image roadmap][image-roadmap] begins with conservative lossless
PNG and OxiPNG capability discovery. OxiPNG remains the **first planned
candidate producer after #80**; no executable version, invocation or working
adapter is selected by this documentation checkpoint. A later bounded issue
must prove its configuration fits the validator's exact preservation subset,
or explicitly refuse unsupported inputs. The wrapper's default behavior or an
encoder's “lossless” label cannot establish profile conformance.

The later [image transformation roadmap][image-transformations] includes JPEG
optimization/encoding and compatibility-tested modern delivery formats.
WebP, AVIF and JPEG XL are **evaluation candidates under that roadmap**, not
promised adapters. In the combined AVIF/HEIF row, the production plan refers
to AVIF evaluation, not a commitment to encode every HEIF codec. GIF, SVG,
TIFF and RAW have no separately selected producers here; animation and RAW
preservation requirements do not imply optimization support for them.

[Audio/video work][av-roadmap] belongs to the later FFmpeg adapter/profile and
validation phase. [Perceptual checks][quality-roadmap], configured
[ExifTool metadata passes][metadata-roadmap], and bounded
[batch/temporary-space handling][space-roadmap] remain planned. Metadata
stripping is a separate explicit policy, not an optimization default.

The **P** apply/replace column names the shared
[transactional exact-duplicate milestone][transaction-roadmap]. It is
format-independent duplicate resolution, not a promise to optimize every
format. Optimized-media replacement additionally depends on its own validated
profile and the later commit/recovery gates. Candidate generation, candidate
publication and replacement are distinct checkpoints; #80 delivered none of
those operations. Resolve the architecture/release boundary before adding
execution to the CLI, preserving [OFD-001 and OFD-005][decisions].

## Rust core and mature providers

OptiFlow owns capability discovery, typed requests/results, policy, evidence,
validation coordination, reporting and CLI contracts. Its roadmap also assigns
scheduling, staging, transactions and recovery to that core; those future
parts are not implemented merely because they are owned here. Flow owns
cross-tool orchestration through released contracts.

Codec, encoder, decoder and quality-metric implementations should remain
mature tools or libraries behind bounded adapters. OptiFlow's existing PNG
validator already uses a pinned decoder with independent structural and
preservation checks; it is not a new PNG codec. Prefer direct typed adapters
for OxiPNG and later format-specific tools. Rust is the control layer, not a
reason to rewrite established compression algorithms.

This follows the [existing adapter roadmap][image-roadmap],
[architecture][architecture], [extension trust boundary](plugin-sdk.md), and
[candidate contract](png-candidate-contract.md). It introduces no new provider
role, dependency, mutation model or format policy.

## What image_optim contributes

Source review pin:
[`toy/image_optim@574276eb8e2f9ccfb468b74b18e3e5883e8445a1`][upstream-tree].
The latest version tag observed was
[`v0.32.0` at `b1029755959d2105f775eb70d121122696d5d0c0`][upstream-tag].
This was source inspection, not an executed optimizer benchmark.

Its [Ruby gem declaration][upstream-gem] and [worker registry][upstream-workers]
show an orchestration layer over external JPEG, PNG, GIF and SVG utilities.
The inspected built-in worker set has no WebP, AVIF, JPEG XL, audio or video
producer. It is useful prior art for provider inventory and behavior, not a
foundation that defines OptiFlow's format coverage.

| Prior art | OptiFlow use and boundary |
| --- | --- |
| [Content detection][upstream-sniff] and [worker registry][upstream-workers] | Discover capabilities explicitly; recognize content before selecting an applicable typed profile. |
| [Per-tool benchmarking][upstream-readme] | Compare safe synthetic fixtures and recorded outcomes; no performance claim from this review. |
| [Per-image timer/process handling][upstream-cmd] | Inform deadline/cancellation tests; OptiFlow still needs explicit limits, cleanup and measured evidence. |
| [Content/options cache with optional worker digests][upstream-cache] | Inform invalidation cases; OptiFlow must retain its own complete identities, policy/tool bindings and current-source checks. |
| [Default worker size predicate][upstream-worker] | Useful accept-smaller pattern; nonempty/smaller output is insufficient without independent decode and preservation validation, and worker overrides must be examined. |
| [In-place API][upstream-api] and [metadata-stripping defaults][upstream-oxipng] | Do not inherit replacement authority or stripping policy. The first PNG profile preserves non-IDAT bytes exactly. |

Do not port `image_optim` line-for-line or add its Ruby gem as a foundational
runtime dependency. A future optional compatibility provider could be considered
only through the same untrusted-candidate boundary as other providers: exact
version and executable-byte provenance, typed invocation, resource limits,
explicit metadata policy, independent byte validation and a tool/license
inventory. Its nested utilities must also be identified; pinning the wrapper
alone would not bind their behavior. No such provider is implemented here.

The wrapper's [MIT license][upstream-license] does not establish the licensing
or redistribution terms of all invoked tools or binary packs. Provider
admission must inventory those separately under OptiFlow's
[dependency policy](release-policy.md). This note makes no redistribution
approval or tool-bundling decision.

## Maintaining the matrix

The README is a human-readable projection, not a second capability registry.
Update it with the implementing PR and evidence, preserving the distinction
between source availability, a released binary, and observed provider behavior.

| Matrix column | Source of truth |
| --- | --- |
| Inventory / exact dedup | Discovery, stable observation, hashing, grouping and planning code; safety/observation fixtures. |
| Optional probe | Locked recognizers, MIME dispatch, effective probe policy and the exact provider result; never suffixes alone. |
| Review evidence | Implemented versioned profile, schema, selection rules and fixtures. |
| Candidate validation | Actual validator entry point and byte fixtures, with its supported subset and invocation surface. |
| Candidate production | Implemented provider adapter plus independently validated outputs; until then, only the precise roadmap or explicit deferral. |
| Apply / replace | CLI authority, approved immutable plan, transaction/recovery implementation and failure tests; currently roadmap-only. |
| Maturity / evidence | Immutable source/release links and execution evidence for the claimed scope. |

Every README status links to these explanations or a precise roadmap section.
The snapshot is intentionally dated and pinned. A future canonical provider-
capability registry should generate or validate this projection; this issue
does not invent that registry or install a second discovery mechanism.

Decision impact: **Reference / ADR not required**. This records existing
implementation and roadmap boundaries without accepting a new durable
decision or changing the gated [ADR migration](https://github.com/egohygiene/optiflow/issues/60).

[discovery]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/src/discovery.rs
[observation]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/src/observation.rs
[inventory-source]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/src/inventory.rs
[duplicates]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/src/duplicates.rs
[planning]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/src/planning.rs
[lockfile]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/Cargo.lock
[ffprobe]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/src/adapters/ffprobe.rs
[profiles]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/src/media_profiles.rs
[profile-tests]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/tests/media_profiles_cli.rs
[png-validation]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/src/png_validation.rs
[png-tests]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/tests/png_byte_validation.rs
[cli]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/src/cli.rs
[app]: https://github.com/egohygiene/optiflow/tree/5be461c413fc34515cd70f41d4178eb8243525d9/src/app
[architecture]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/ARCHITECTURE.md
[decisions]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/DECISIONS.md
[transaction-roadmap]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/ROADMAP.md#v020--transactional-exact-duplicate-resolution
[image-roadmap]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/ROADMAP.md#v030--transactional-lossless-image-optimization
[image-transformations]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/ROADMAP.md#encoders-and-formats
[quality-roadmap]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/ROADMAP.md#quality-validation
[metadata-roadmap]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/ROADMAP.md#metadata-and-privacy
[space-roadmap]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/ROADMAP.md#space-and-filesystem-safety
[av-roadmap]: https://github.com/egohygiene/optiflow/blob/5be461c413fc34515cd70f41d4178eb8243525d9/ROADMAP.md#ffmpeg-adapter
[upstream-tree]: https://github.com/toy/image_optim/tree/574276eb8e2f9ccfb468b74b18e3e5883e8445a1
[upstream-tag]: https://github.com/toy/image_optim/tree/b1029755959d2105f775eb70d121122696d5d0c0
[upstream-gem]: https://github.com/toy/image_optim/blob/574276eb8e2f9ccfb468b74b18e3e5883e8445a1/image_optim.gemspec
[upstream-workers]: https://github.com/toy/image_optim/blob/574276eb8e2f9ccfb468b74b18e3e5883e8445a1/lib/image_optim/worker/class_methods.rb
[upstream-sniff]: https://github.com/toy/image_optim/blob/574276eb8e2f9ccfb468b74b18e3e5883e8445a1/lib/image_optim/image_meta.rb
[upstream-readme]: https://github.com/toy/image_optim/blob/574276eb8e2f9ccfb468b74b18e3e5883e8445a1/README.markdown#benchmark
[upstream-cmd]: https://github.com/toy/image_optim/blob/574276eb8e2f9ccfb468b74b18e3e5883e8445a1/lib/image_optim/cmd.rb
[upstream-cache]: https://github.com/toy/image_optim/blob/574276eb8e2f9ccfb468b74b18e3e5883e8445a1/lib/image_optim/cache.rb
[upstream-worker]: https://github.com/toy/image_optim/blob/574276eb8e2f9ccfb468b74b18e3e5883e8445a1/lib/image_optim/worker.rb
[upstream-api]: https://github.com/toy/image_optim/blob/574276eb8e2f9ccfb468b74b18e3e5883e8445a1/lib/image_optim.rb
[upstream-oxipng]: https://github.com/toy/image_optim/blob/574276eb8e2f9ccfb468b74b18e3e5883e8445a1/lib/image_optim/worker/oxipng.rb
[upstream-license]: https://github.com/toy/image_optim/blob/574276eb8e2f9ccfb468b74b18e3e5883e8445a1/LICENSE.txt
