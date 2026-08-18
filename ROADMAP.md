# optiflow roadmap

> Observe first. Prove relationships. Plan explicitly. Mutate transactionally.

This document defines the product and release roadmap for `optiflow` itself.
It covers the CLI, domain model, state, safety, media adapters, validation,
distribution, and release lifecycle required to make the tool dependable.

It intentionally excludes organization-wide automation, shared repository
infrastructure, websites, dashboards, and work owned by other repositories.
Repository-local automation required to test, package, secure, and release
`optiflow` remains in scope.

## Roadmap contract

This roadmap is:

- Ordered by safety and dependency, not by calendar date.
- A statement of intent rather than a compatibility guarantee.
- Expected to evolve as implementation evidence exposes better boundaries.
- Complete only when each release satisfies its exit criteria.
- Designed so each unchecked item can become one or more focused GitHub issues.

Release scope may move, but safety invariants must not be weakened to meet a
version target.

### Status legend

- [x] Implemented in the current codebase.
- [ ] Planned but not yet implemented.

## Product vision

`optiflow` will be a local-first media decision engine for disorganized image,
audio, and video collections. It will inventory real content, prove or score
relationships, estimate meaningful storage outcomes, generate explainable
plans, and execute approved changes with durable validation and recovery.

The mature tool should answer five questions:

1. What media is present, regardless of filename extension?
2. Which files are provably identical, related, derived, or potentially
   contained within one another?
3. What storage can actually be reclaimed without overstating savings from
   hard links, sparse files, clones, or filesystem behavior?
4. Which transformations satisfy an explicit preservation, archive, delivery,
   or pipeline policy?
5. Can every approved mutation be explained, validated, interrupted safely,
   audited, and recovered when recovery was requested?

## Product boundaries through `v1.0.0`

`optiflow` owns:

- Filesystem discovery and collection policies.
- Content inventory and normalized media descriptors.
- Exact and perceptual relationship evidence.
- Durable local state, immutable artifacts, and cache invalidation.
- Explainable plan generation and approval boundaries.
- Transactional execution, validation, commit, recovery, and reporting.
- Capability-driven adapters around specialized media tools.
- Stable human and machine-readable CLI contracts.
- Built-in profiles with explicit preservation and quality policies.

`optiflow` delegates:

- Codec implementations and media decoding.
- Specialized image encoders and optimizers.
- Container and stream inspection.
- Perceptual quality metrics and audio fingerprint generation.

`optiflow` is not intended to become:

- A media library, catalog browser, or digital asset manager.
- A photo editor, nonlinear video editor, or audio workstation.
- A cloud synchronization or backup service.
- A codec or encoder implementation.
- A general-purpose disk cleaner.
- An opaque automatic deletion system.
- A promise that one format or profile is universally best.

## Non-negotiable safety invariants

Every release must preserve these rules:

- Scanning is read-only with respect to source media.
- Detection, planning, approval, execution, and commit remain separate phases.
- A plan is immutable evidence, not implicit permission to mutate files.
- No perceptual score, filename, duration, dimension, or partial fingerprint
  can prove byte identity.
- Exact identity requires equal logical size and a complete cryptographic hash;
  destructive resolution additionally requires apply-time byte comparison.
- Every mutating action re-proves its preconditions immediately before use.
- Changed, missing, replaced, or ambiguous inputs cause the affected action to
  fail closed.
- Generated output is written away from the source, synchronized, validated,
  and only then committed.
- A failed validation never replaces the source.
- Recovery guarantees are stated per action and never implied.
- Perceptual, containment, and quality judgments remain explainable and
  detector-specific.
- Metadata, color, animation, streams, chapters, subtitles, attachments,
  sidecars, paired assets, and filesystem attributes are preserved or changed
  only by explicit policy.
- External tools are invoked without a shell and with recorded versions,
  capabilities, arguments, timeouts, and outcomes.
- Machine-readable artifacts remain schema-versioned independently from the
  binary version.

## Release map

| Release | Theme | Primary outcome |
| --- | --- | --- |
| `v0.1.0` | Read-only proof | Inventory and exact duplicate plans. |
| `v0.1.x` | Foundation | Accuracy, diagnostics, security, and packaging. |
| `v0.2.0` | Transactions | Recoverable exact duplicate resolution. |
| `v0.3.0` | Lossless images | Transactional image optimization. |
| `v0.4.0` | Image profiles | Archive, delivery, and pipeline conversions. |
| `v0.5.0` | Audio and video | Stream-aware optimization and validation. |
| `v0.6.0` | Relationships | Reviewable similarity and containment evidence. |
| `v0.7.0` | Collections | Scale, identity, and portable state. |
| `v0.8.0` | Policy and UX | Understandable and automation-safe workflows. |
| `v0.9.0` | Stable beta | Freeze and prove intended `v1` contracts. |
| `v1.0.0` | General availability | Stable macOS and Linux tool. |

Versions after `v0.1.0` describe intended sequencing. A release may be split
when doing so produces a smaller independently safe increment.

## Cross-cutting workstreams

These workstreams span multiple milestones and should be considered whenever a
feature is designed.

### Safety and transactions

- Immutable observations and plans.
- Apply-time preconditions and time-of-check/time-of-use defenses.
- Temporary artifacts, synchronization, validation, and atomic commit where
  the filesystem supports it.
- Explicit behavior when an operation crosses filesystem boundaries.
- Durable execution journals and recovery after process interruption.
- Idempotent resume behavior and safe cleanup of abandoned temporary files.
- Recoverable defaults and separately authorized irreversible actions.

### Evidence and explainability

- Detector name, version, parameters, inputs, and evidence recorded together.
- No universal interpretation of a perceptual score.
- Human reports explain why each relationship or action exists.
- JSON artifacts expose the same material evidence as human output.
- Storage estimates distinguish logical bytes, allocated bytes, and estimated
  reclaimable bytes when the platform provides enough evidence.

### State and compatibility

- Forward-only, tested SQLite migrations.
- Explicit schema identifiers for run, report, plan, execution, and recovery
  artifacts.
- Compatible additive changes within a schema major version.
- Intentional migration commands or release notes for breaking changes.
- Cache keys include every input that can change an analysis result, including
  adapter version and parameter hashes.

### Adapter architecture

- Capability discovery instead of assumptions based on executable presence.
- Typed requests and normalized results around every external tool.
- Version and build-feature provenance stored in plans and executions.
- Configurable timeouts, output limits, and cancellation.
- Clear distinction between unavailable, unsupported, failed, and invalid
  adapter results.
- Deterministic software paths by default where reproducibility matters;
  hardware acceleration must be explicit and recorded.

### Quality engineering

- Unit tests for domain rules and policy evaluation.
- Contract tests for schemas, migrations, and adapter parsing.
- Integration tests with controlled tool fixtures.
- Synthetic end-to-end tests for every supported action class.
- Failure-injection tests for interruption, disk exhaustion, stale plans,
  permission changes, adapter hangs, malformed media, and disconnected volumes.
- Property and fuzz testing for parsers, manifests, paths, and state machines.
- Performance regression fixtures for large collections and large files.

### Release engineering

- Reproducible release builds for supported platforms and architectures.
- Checksums, provenance, software bill of materials, and signed release
  artifacts when the release platform supports them.
- Automated changelog and release-note generation from conventional commits.
- Installation, upgrade, rollback, and uninstall documentation.
- A release checklist that proves schemas, migrations, CLI help, licenses,
  packaging, and smoke tests before publication.

## `v0.1.0` — read-only exact-duplicate MVP

**Status:** implemented in the current codebase; publication may still be
pending.

### Goal

Deliver useful storage intelligence without granting the tool authority to
change source media.

### Implemented capabilities

- [x] Create a Rust library and CLI with `unsafe_code` forbidden.
- [x] Support macOS and Linux code paths.
- [x] Add `doctor`, `scan`, `report`, `plan exact-duplicates`, and
  `cache status` commands.
- [x] Discover multiple file and directory inputs recursively.
- [x] Avoid symbolic links, hidden trees, and filesystem crossings by default.
- [x] Classify actual content independently from filename extension.
- [x] Collect optional normalized media metadata from `ffprobe` JSON.
- [x] Persist runs, observations, exact groups, and unchanged-path cache entries
  in local SQLite state.
- [x] Narrow exact candidates by size before complete hashing.
- [x] Calculate complete streaming BLAKE3-256 hashes for candidates.
- [x] Produce evidence-backed exact duplicate groups and potential logical
  reclaimable-byte totals.
- [x] Write immutable run, report, and review-plan artifacts atomically.
- [x] Include stale-plan, re-hash, and byte-confirmation requirements in each
  planned group.
- [x] Publish versioned JSON schemas for run, report, and plan artifacts.
- [x] Provide strict formatting, lint, unit, integration, and smoke gates.

### `v0.1.0` exit criteria

- [x] Identical files produce a single exact group.
- [x] Same-length files with different content remain separate.
- [x] Missing `ffprobe` does not block exact duplicate analysis.
- [x] Spaces and Unicode paths are covered by the end-to-end smoke test.
- [x] Scanning and planning leave all source files unchanged.

## `v0.1.x` — public-release hardening

**Goal:** make the read-only foundation accurate enough to trust and simple
enough to install before mutation is introduced.

### Inventory correctness

- [x] Record stable platform file identity where available: device, inode or
  file ID, link count, and filesystem identifier.
- [x] Detect multiple paths to the same hard-linked file and avoid presenting
  them as independent physical copies.
- [x] Report logical size separately from allocated size when supported.
- [x] Detect or conservatively disclose sparse files, copy-on-write clones, and
  reflinks whose physical savings cannot be inferred from logical size alone.
- [x] Refuse to claim exact reclaimable physical bytes when the filesystem does
  not expose enough information.
- [x] Detect files that change during hashing and mark their evidence stale.
- [ ] Define duplicate path normalization for case-insensitive filesystems,
  relative paths, Unicode, and repeated inputs.
- [ ] Add structured warnings for unsupported filesystem metadata.
- [ ] Make partial and interrupted runs visible through CLI status/reporting.

### CLI and diagnostics

- [ ] Publish a stable exit-code taxonomy for success, partial success, invalid
  input, unavailable capability, stale state, and internal failure.
- [ ] Add `--quiet`, verbosity, and structured diagnostic behavior without
  contaminating JSON standard output.
- [ ] Add command examples and remediation guidance to `doctor`.
- [ ] Report binary version, schema support, state location, database status,
  filesystem assumptions, and discovered adapter capabilities.
- [ ] Add shell completion generation and manual-page generation.
- [ ] Add a privacy-safe diagnostic bundle command with an explicit preview of
  included data.

### Configuration

- [ ] Define precedence for built-in defaults, configuration file, environment
  variables, and CLI flags.
- [ ] Add `config show`, `config paths`, and `config validate` commands.
- [ ] Keep configuration optional; the default scan remains conservative.
- [ ] Version configuration files and reject unknown safety-critical values.
- [ ] Record the effective policy hash in every run.

### Release and supply-chain foundation

- [ ] Add `CHANGELOG.md`, `SECURITY.md`, and a repository-local release
  checklist.
- [ ] Enforce the declared minimum supported Rust version in CI.
- [ ] Check dependency licenses, advisories, duplicate versions, and source
  policies.
- [ ] Generate optimized binaries for supported macOS and Linux architectures.
- [ ] Produce release archives, SHA-256 checksums, build provenance, and an
  SBOM.
- [ ] Validate a clean install and first-run smoke test from packaged artifacts.
- [ ] Document source installation and prebuilt binary installation.
- [ ] Define patch-release and security-fix procedures.
- [ ] Decide whether and when to publish the Rust package; do not make the Rust
  library API stable merely by publishing the CLI package.

### Test expansion

- [ ] Add fixtures for empty files, huge sparse files, hard links, symbolic
  links, permission failures, disappearing files, invalid media, and files that
  change during hashing.
- [x] Test Linux and macOS path and filesystem behavior independently.
- [ ] Add schema validation for every emitted artifact in CI.
- [ ] Test SQLite migrations from every previously released schema.
- [ ] Establish a performance baseline for file discovery, candidate hashing,
  cache reuse, memory use, and artifact size.

### `v0.1.x` exit criteria

- [ ] A public release can be installed from a versioned artifact and verified
  by checksum.
- [ ] Duplicate and reclaimable-space reporting is hard-link aware and honest
  about unknown physical allocation.
- [ ] All emitted JSON validates against its declared schema.
- [ ] The CLI has documented exit codes and keeps machine output clean.
- [ ] Supported platforms pass clean-install and upgrade smoke tests.

## `v0.2.0` — transactional exact-duplicate resolution

**Goal:** introduce the first source mutation through one narrow,
evidence-complete, recoverable workflow.

### Execution domain

- [ ] Add schema-versioned `ExecutionRun`, `Attempt`, `Artifact`,
  `ValidationResult`, `CommitRecord`, and `RecoveryRecord` entities.
- [ ] Add an explicit `apply --plan <path-or-id>` boundary.
- [ ] Require plans to declare their action type, mutation authority, policy,
  source run, evidence, preconditions, and expected storage outcome.
- [ ] Verify plan schema, source identity, supported action versions, and policy
  compatibility before execution begins.
- [ ] Re-stat, re-hash, and byte-compare exact candidates immediately before
  each destructive commit.
- [ ] Detect path replacement and link substitution between verification and
  commit as far as supported by the host platform.
- [ ] Write a durable execution journal before the first source mutation.
- [ ] Make execution resumable and idempotent at action boundaries.
- [ ] Handle termination signals without claiming an in-flight action was
  committed.

### Resolution policies

- [ ] Provide a non-mutating `apply --dry-run` that performs all feasible
  validation and space checks.
- [ ] Make a recoverable quarantine policy the default mutating strategy.
- [ ] Require the quarantine location and whether it shares a filesystem with
  the source to be explicit in the plan.
- [ ] Use atomic same-filesystem moves when available.
- [ ] For cross-filesystem quarantine, copy to a temporary destination,
  synchronize it, verify full content, commit the destination, and only then
  remove the source.
- [ ] Add `restore` with collision-safe destination handling and preconditions.
- [ ] Add separately authorized permanent deletion with prominent dry-run and
  no implied recovery guarantee.
- [ ] Never treat the deterministic review `keep_path` as a quality decision;
  require a declared keeper-selection policy or reviewed plan.
- [ ] Support skip, fail-fast, and continue-independent-actions policies.

### Space and filesystem safety

- [ ] Estimate temporary, quarantine, database, and safety-margin space before
  execution.
- [ ] Refuse actions when required space is unavailable or cannot be measured
  safely.
- [ ] Distinguish projected logical savings from physical savings.
- [ ] Detect read-only volumes, disconnected inputs, permission changes, and
  unsupported atomicity.
- [ ] Preserve configured ownership, permissions, timestamps, and extended
  attributes or report why preservation is impossible.

### Recovery and audit

- [ ] Add `execution status`, `execution report`, `resume`, and `restore`
  commands.
- [ ] Record every attempted action and its terminal state.
- [ ] Retain enough provenance to explain what changed, when, why, with which
  binary, and under which effective policy.
- [ ] Provide explicit cleanup for completed or abandoned temporary artifacts.
- [ ] Never clean an artifact whose ownership or execution state is ambiguous.

### Failure testing

- [ ] Inject stale plans, modified files, renamed paths, permission changes,
  insufficient space, failed synchronization, interrupted moves, interrupted
  copies, database contention, and volume disconnects.
- [ ] Prove that validation failure leaves the source unchanged.
- [ ] Prove that resuming does not repeat a committed mutation.
- [ ] Prove restore behavior for same- and cross-filesystem quarantine.

### `v0.2.0` exit criteria

- [ ] No source mutation occurs without an approved, versioned plan.
- [ ] Every exact candidate is re-proven and byte-confirmed before commit.
- [ ] Interrupted executions can be inspected and safely resumed or recovered.
- [ ] Quarantined files can be restored according to the recorded guarantee.
- [ ] Permanent deletion is explicit, separately authorized, and covered by
  destructive-path tests.

## `v0.3.0` — transactional lossless image optimization

**Goal:** reuse the execution engine to reduce supported image sizes without an
intentional loss of decoded image content.

### Image inventory

- [ ] Normalize image format, dimensions, bit depth, color model, alpha,
  animation, frame count, orientation, embedded profiles, and metadata facts.
- [ ] Detect multi-frame and animated images before selecting an optimizer.
- [ ] Inventory RAW files and paired assets without modifying them by default.
- [ ] Identify sidecar relationships conservatively and keep them attached to
  the plan.
- [ ] Distinguish encoded-byte losslessness from decoded-pixel equivalence.

### Adapter framework

- [ ] Define typed adapter capability, request, result, error, and tool-record
  contracts.
- [ ] Implement capability discovery and version capture for OxiPNG.
- [ ] Add a general image inspection/decoding adapter where needed for
  validation.
- [ ] Permit optional platform-specific adapters only behind the same contract;
  they must not become foundational dependencies.
- [ ] Record executable identity, version, capabilities, arguments, runtime,
  exit status, and bounded standard-error output.

### Profiles and planning

- [ ] Add named, versioned optimization profiles.
- [ ] Begin with conservative lossless PNG optimization.
- [ ] Make metadata, color-profile, animation, alpha, and filesystem-attribute
  behavior explicit in each profile.
- [ ] Skip files with no expected benefit unless the user requests validation
  only.
- [ ] Include predicted output size, required temporary space, adapter
  capability requirements, and validation policy in each action.
- [ ] Never label a lossy transformation as lossless because it appears
  visually unchanged.

### Validation and commit

- [ ] Write optimized output to a same-filesystem temporary artifact when
  replacement atomicity is required.
- [ ] Validate output existence, type, dimensions, frames, alpha, orientation,
  color information, metadata policy, and complete decode.
- [ ] Compare decoded pixels where the profile promises pixel equivalence.
- [ ] Require the output to be smaller by a configured threshold before commit.
- [ ] Preserve the original or quarantine it according to the approved plan.
- [ ] Record the rejected output when configured for debugging, otherwise clean
  it safely.

### `v0.3.0` exit criteria

- [ ] Supported lossless profiles produce completely decodable outputs that
  satisfy every declared invariant.
- [ ] An output that is larger, incomplete, or policy-invalid never replaces
  its source.
- [ ] Interrupted optimization is safely resumable or recoverable.
- [ ] Reports distinguish encoded-byte savings from decoded-content guarantees.

## `v0.4.0` — image transformation and delivery profiles

**Goal:** support intentional image conversion and lossy compression without
pretending one output format is correct for every use case.

### Profile model

- [ ] Define profile intents such as preservation, archive derivative, balanced
  delivery, aggressive delivery, and pipeline intermediate.
- [ ] Version every built-in profile independently from the binary.
- [ ] Allow custom profiles with strict schema validation.
- [ ] Make source retention, metadata, privacy, color, animation, alpha,
  compatibility, quality, size, and extension policies explicit.
- [ ] Record why a codec and container were selected for each input.
- [ ] Treat normalization as a policy choice, not a universal default.

### Encoders and formats

- [ ] Add capability-driven JPEG optimization and encoding.
- [ ] Add palette-reduction support as an explicitly lossy PNG profile.
- [ ] Add modern delivery formats only after compatibility and metadata
  behavior are defined and tested.
- [ ] Preserve animated inputs unless the selected profile explicitly permits
  flattening or animation conversion.
- [ ] Preserve originals by default for lossy and format-changing actions.

### Quality validation

- [ ] Define detector-specific visual quality thresholds per profile.
- [ ] Combine structural invariants with quality metrics; neither alone is
  sufficient.
- [ ] Detect unintended resizing, cropping, orientation changes, alpha loss,
  gamut changes, banding, and animation loss.
- [ ] Store quality tool version, parameters, and results with the execution.
- [ ] Add review-required outcomes for borderline results instead of silently
  accepting them.

### Metadata and privacy

- [ ] Add explicit `preserve`, `strip-private`, and custom metadata policies.
- [ ] Protect orientation, capture time, copyright, color, and paired-asset
  metadata unless the profile says otherwise.
- [ ] Report metadata that cannot be represented in the target format.
- [ ] Require confirmation when a requested conversion would discard a feature
  the source contains.

### `v0.4.0` exit criteria

- [ ] Every lossy or format-changing action names the intended tradeoff.
- [ ] Originals are retained unless the plan explicitly authorizes replacement.
- [ ] Quality and metadata decisions are reproducible from recorded provenance.
- [ ] Unsupported source features fail closed or require explicit approval.

## `v0.5.0` — audio and video optimization

**Goal:** add stream-aware audio and video transformations using the same
plan/apply/validate/commit model.

### Media model

- [ ] Represent containers, programs, video streams, audio streams, subtitles,
  attachments, chapters, cover art, dispositions, language, time bases,
  duration, and rotation.
- [ ] Distinguish container conversion, stream copy, re-encoding, remuxing,
  resampling, channel changes, and loudness processing as separate actions.
- [ ] Detect variable frame rate, high dynamic range, interlacing, and unusual
  channel layouts before selecting a profile.
- [ ] Preserve masters and project intermediates by default.

### FFmpeg adapter

- [ ] Discover the installed FFmpeg/ffprobe version, build configuration,
  decoders, encoders, formats, filters, and hardware capabilities.
- [ ] Generate arguments from typed requests rather than interpolated command
  strings.
- [ ] Record complete stream mapping and prevent accidental stream omission.
- [ ] Add progress parsing, cancellation, bounded logs, and configurable
  timeouts.
- [ ] Make hardware encoding opt-in and record the hardware and driver context
  needed to interpret reproducibility.

### Profiles

- [ ] Add versioned archive-derivative, balanced, space-saver, web-delivery,
  and pipeline profiles only after their invariants are specified.
- [ ] Support stream-copy plans when conversion does not require re-encoding.
- [ ] Keep audio loudness normalization separate from transparent compression.
- [ ] Define subtitle, attachment, chapter, cover-art, language, metadata, and
  timestamp policies per profile.
- [ ] Refuse unsupported profiles before starting expensive work.

### Validation

- [ ] Validate container structure, expected streams, codec properties,
  dimensions, duration, timestamps, frame/sample counts where reliable, and
  complete decode.
- [ ] Detect truncation, missing streams, desynchronization, invalid timestamps,
  unexpected duration drift, and silent audio.
- [ ] Add reference-based visual quality measurement where the profile requires
  it.
- [ ] Add audio quality and loudness checks appropriate to the selected action.
- [ ] Define tolerances explicitly and record all deviations.
- [ ] Compare final size and measured quality against plan thresholds before
  commit.

### Long-running execution

- [ ] Persist progress without making partial output appear committed.
- [ ] Resume at action boundaries; only claim within-file resume when the
  underlying operation provides a verifiable continuation contract.
- [ ] Re-estimate disk requirements as outputs are produced.
- [ ] Handle thermal, power, and cancellation signals conservatively when the
  platform exposes them.

### `v0.5.0` exit criteria

- [ ] No expected stream or media feature is discarded implicitly.
- [ ] Every committed output completes structural and decode validation.
- [ ] Lossy outputs meet the declared profile thresholds and preserve their
  source according to policy.
- [ ] Adapter provenance is sufficient to reproduce or explain the result.

## `v0.6.0` — perceptual relationships and containment

**Goal:** surface related media for review while maintaining a hard boundary
between similarity evidence and exact identity.

### Evidence framework

- [ ] Add versioned `RelationshipEvidence` with detector-specific kinds,
  parameters, scores, thresholds, and confidence explanations.
- [ ] Support multiple independent evidence items without collapsing them into
  one opaque universal score.
- [ ] Record preprocessing steps so fingerprints can be invalidated correctly.
- [ ] Keep detector upgrades from silently reinterpreting historical evidence.
- [ ] Add explicit `exact`, `near-identical`, `similar`, `derived`,
  `contained`, `unrelated`, and `unknown` semantics where evidence supports
  them.

### Image similarity

- [ ] Add perceptual fingerprints suitable for resized, recompressed, and
  mildly edited images.
- [ ] Account for crop, rotation, mirroring, alpha, animation, and color changes
  through detector-specific strategies.
- [ ] Generate contact sheets or review manifests without requiring a GUI.
- [ ] Never auto-delete an image based only on perceptual similarity.

### Audio similarity

- [ ] Add an optional Chromaprint adapter for near-identical recordings.
- [ ] Normalize duration and offset evidence without treating metadata matches
  as content proof.
- [ ] Distinguish alternate encodes from edits, excerpts, remixes, and unrelated
  recordings when possible; otherwise report `unknown`.
- [ ] Never infer semantic similarity from an acoustic fingerprint designed for
  recording identity.

### Video similarity and containment

- [ ] Combine sampled visual fingerprints, audio evidence, timing, and stream
  facts through explainable detectors.
- [ ] Detect alternate encodes and resized or watermarked variants.
- [ ] Detect time-aligned excerpts and candidate subsection containment.
- [ ] Confirm containment candidates with denser evidence around proposed
  boundaries.
- [ ] Report confidence and blind spots for videos lacking audio or stable
  visual content.

### Review planning

- [ ] Add review-only plans for all perceptual and containment relationships.
- [ ] Let users declare keeper preferences without describing them as objective
  quality truth.
- [ ] Require a later exact or user-approved resolution plan before mutation.
- [ ] Preserve every detector result used to justify a review decision.

### `v0.6.0` exit criteria

- [ ] Similarity results identify their detector and explain their meaning.
- [ ] Thresholds are calibrated against documented fixture corpora.
- [ ] False-positive and false-negative behavior is measured and published.
- [ ] No perceptual or containment result directly authorizes deletion.

## `v0.7.0` — collection scale, identity, and portable state

**Goal:** make repeated analysis dependable across large collections, moved
files, removable volumes, and evolving tool capabilities.

### Collection model

- [ ] Add named collections with roots, filesystem identity, policy, and scan
  history.
- [ ] Register removable volumes without placing the primary database on them
  automatically.
- [ ] Detect volume identity changes and path reuse.
- [ ] Add portable, schema-versioned collection manifests with explicit export
  and import.
- [ ] Merge imported evidence without trusting unverified source paths.

### Content identity and cache

- [ ] Recognize moved or renamed content through content-addressed evidence.
- [ ] Separate path observations from reusable content objects in persistent
  state.
- [ ] Include detector, adapter, profile, and parameter versions in cache keys.
- [ ] Add cache inspection, pruning, verification, export, and rebuild commands.
- [ ] Define retention policies for historical runs, logs, fingerprints,
  temporary artifacts, and reports.

### Performance and scale

- [ ] Add bounded worker pools and explicit I/O, CPU, memory, and external-tool
  concurrency controls.
- [ ] Avoid excessive seek contention when several files share one physical
  device.
- [ ] Stream or page large reports instead of requiring all observations in
  memory.
- [ ] Add incremental scans and change-aware collection refresh.
- [ ] Support cancellation with durable partial-run state.
- [ ] Benchmark representative SSD, spinning-disk, removable, and large-file
  workloads without claiming universal numbers.

### State durability

- [ ] Add database integrity verification and supported repair/export paths.
- [ ] Test migration rollback strategy even when migrations themselves remain
  forward-only.
- [ ] Detect unsupported network-filesystem state locations and fail with
  actionable guidance.
- [ ] Document database backup and restoration.

### `v0.7.0` exit criteria

- [ ] Large scans stay within documented memory and concurrency bounds.
- [ ] Moved files can reuse valid content evidence without confusing path
  history.
- [ ] Removable collection state can be exported and re-verified safely.
- [ ] Cache lifecycle operations cannot silently erase authoritative execution
  or recovery records.

## `v0.8.0` — policy, review, and automation-safe UX

**Goal:** make advanced use cases approachable while preserving deterministic
CLI and pipeline behavior.

### Policy system

- [ ] Add schema-versioned reusable policies for discovery, evidence, keeper
  selection, optimization, metadata, validation, storage, recovery, and
  failure handling.
- [ ] Provide a command that resolves and prints the effective policy before a
  run.
- [ ] Reject conflicting or incomplete safety-critical policies.
- [ ] Allow per-collection overrides without hidden global behavior.
- [ ] Include the complete effective policy or immutable reference in every
  plan.

### Review experience

- [ ] Add plan summaries grouped by risk, action type, media type, projected
  outcome, and review requirement.
- [ ] Add include/exclude decisions without mutating the original plan; produce
  a derived approved plan with provenance.
- [ ] Add stable diff output between reports, plans, and executions.
- [ ] Add paginated or interactive terminal review only if it remains usable
  through a non-interactive equivalent.
- [ ] Make dangerous actions visually and structurally distinct.

### Automation contract

- [ ] Guarantee one complete JSON result on standard output in JSON mode and
  send diagnostics elsewhere.
- [ ] Support non-interactive execution only when every required decision is
  encoded in a validated plan and policy.
- [ ] Add machine-readable progress and event streams without breaking the
  final-result contract.
- [ ] Add explicit lock, concurrency, and already-running behavior.
- [ ] Define stable retryability and error categories.
- [ ] Ensure automation cannot bypass approvals required by the selected
  policy.

### `v0.8.0` exit criteria

- [ ] A complex plan can be reviewed and narrowed without hand-editing JSON.
- [ ] The effective policy is reproducible and attached to resulting evidence.
- [ ] Interactive and non-interactive flows produce equivalent domain actions.
- [ ] Automation receives stable events, errors, and terminal results.

## `v0.9.0` — stable-contract beta

**Goal:** freeze the intended `v1` surface and prove that real upgrades and
failures do not compromise user data.

### Interface freeze

- [ ] Inventory every CLI command, option, environment variable, configuration
  key, exit code, artifact schema, profile identifier, and state migration.
- [ ] Mark each surface stable, experimental, internal, or deprecated.
- [ ] Freeze `v1` run, report, plan, execution, and recovery schema semantics.
- [ ] Publish a deprecation policy and minimum support window.
- [ ] Preserve unknown additive fields when artifacts are read and re-emitted
  where round-tripping is promised.

### Compatibility matrix

- [ ] Define supported macOS versions and architectures.
- [ ] Define supported Linux architecture, libc, and distribution assumptions.
- [ ] Document filesystem guarantees and limitations for APFS, common Linux
  filesystems, removable media, and network filesystems.
- [ ] Test supported external-tool version ranges and capability differences.
- [ ] Verify configuration, database, and artifact upgrades from every public
  minor release.

### Reliability campaign

- [ ] Run destructive-path tests exclusively against disposable fixture
  filesystems.
- [ ] Add randomized state-machine testing across plan, apply, interrupt,
  resume, restore, and cleanup transitions.
- [ ] Fuzz adapter parsers, artifact readers, configuration, and path handling.
- [ ] Exercise multi-terabyte logical fixture sets without requiring equivalent
  physical test storage.
- [ ] Run long-duration soak tests for scan, hashing, and media transformation.
- [ ] Audit every filesystem mutation and external process invocation.
- [ ] Complete an independent threat-model and security review.

### Documentation and operations

- [ ] Publish a complete user guide, command reference, profile reference,
  safety guide, recovery guide, troubleshooting guide, and JSON contract.
- [ ] Provide migration and rollback guidance for the beta.
- [ ] Document performance expectations and known limitations honestly.
- [ ] Define bug-report data collection that defaults to local inspection and
  avoids exposing media paths without consent.

### `v0.9.0` exit criteria

- [ ] No unresolved critical data-loss or recovery defects remain.
- [ ] Every stable contract has compatibility tests.
- [ ] Upgrade, interruption, resume, restore, and rollback exercises pass on
  all supported platform classes.
- [ ] Release artifacts pass clean-machine installation and signature/checksum
  verification.

## `v1.0.0` — trustworthy general availability

**Goal:** ship a stable, supportable CLI that can inventory, deduplicate, and
optimize media under explicit policies without sacrificing evidence or
recoverability.

### Required product capabilities

- [ ] Read-only inventory and exact duplicate proof.
- [ ] Honest logical and physical storage reporting where measurable.
- [ ] Transactional exact-duplicate resolution with explicit recovery choices.
- [ ] Transactional supported image, audio, and video profiles.
- [ ] Structural, decode, metadata, and configured quality validation.
- [ ] Review-only perceptual and containment evidence.
- [ ] Named collection state, cache lifecycle, and portable evidence export.
- [ ] Human review and deterministic machine integration.

### Required release qualities

- [ ] Stable `v1` CLI, configuration, exit-code, profile, and JSON contracts.
- [ ] Tested forward migrations and documented backup/rollback procedures.
- [ ] Supported macOS and Linux installation artifacts with checksums,
  provenance, and SBOMs.
- [ ] Published security and vulnerability-response policy.
- [ ] Complete license attribution for bundled and external dependencies.
- [ ] Performance and resource-use documentation.
- [ ] Release notes that call out compatibility, migration, and safety changes.

### General-availability exit criteria

- [ ] Every `v0.9.0` beta exit criterion remains satisfied.
- [ ] No critical or high-severity unresolved security or data-integrity issue
  is known.
- [ ] The stable documentation matches the shipped binary and schemas.
- [ ] A clean install can scan, plan, dry-run, apply to disposable fixtures,
  interrupt, resume, restore, and verify resulting artifacts successfully.
- [ ] The maintainers can reproduce the release from its source revision.

## Post-`v1.0` candidates

These are deliberately uncommitted. They should be promoted only after the
stable core demonstrates demand and sufficient safety evidence.

- Windows support with a native filesystem and transaction model.
- Optional local graphical review client built on stable CLI contracts.
- Plugin or adapter SDK after more than one independent adapter needs it.
- Sandboxed or isolated media adapters for hostile inputs.
- Distributed analysis with explicit trust and content-access boundaries.
- Watch mode for continuously changing collections.
- Policy packs for well-defined devices or delivery targets.
- Optional duplicate-content replacement with hard links or reflinks where the
  filesystem semantics and rollback behavior are safe and explicit.
- Filesystem-native snapshot integration.
- Additional quality metrics and domain-specific similarity detectors.

None of these candidates should delay a coherent `v1.0.0`.

## Release lifecycle

Every release should follow the same local lifecycle.

### 1. Define

- Confirm the release goal and non-goals.
- Create or update the feature specification and safety analysis.
- Identify affected CLI, configuration, schema, state, and adapter contracts.
- Define fixtures and failure cases before implementation.

### 2. Implement

- Land changes in independently reviewable increments.
- Update migrations and schemas with their producers and consumers.
- Keep external tool calls typed, bounded, and provenance-aware.
- Add user-facing documentation with the behavior it describes.

### 3. Validate

- Run formatting, strict linting, unit, integration, contract, migration,
  end-to-end, and relevant failure-injection tests.
- Validate emitted artifacts against their schemas.
- Test clean installation and upgrade from the previous public release.
- Verify that no test relies on user media or uncontrolled external state.

### 4. Package

- Build supported release targets from a tagged source revision.
- Generate archives, checksums, provenance, SBOMs, notices, and release notes.
- Verify packaged binaries rather than only build-tree binaries.
- Confirm version output, CLI help, completions, and manual pages.

### 5. Publish and observe

- Publish the tag and immutable release artifacts.
- Verify installation instructions on clean supported environments.
- Triage regressions by data-integrity and recovery risk first.
- Use patch releases for compatible fixes; move new behavior to the next minor
  release while the major version is zero.

## Versioning and compatibility policy

Before `v1.0.0`:

- Minor releases may change unstable interfaces when release notes and
  migrations make the change explicit.
- Patch releases should remain backward compatible and focus on fixes,
  documentation, packaging, and security updates.
- Artifact schema versions, profile versions, and database migrations evolve
  independently from the binary version.
- Experimental fields and commands must be marked as such.

At and after `v1.0.0`:

- Semantic Versioning applies to documented stable interfaces.
- Breaking stable CLI, configuration, artifact, profile, or state behavior
  requires a major release or a documented deprecation cycle.
- Additive schema changes preserve existing meaning within the same schema
  major version.
- Security or data-integrity fixes may disable unsafe behavior immediately;
  release notes must explain the compatibility impact.

The Rust crate's internal library API is not stable unless a future release
explicitly documents it as a supported public interface.

## Definition of done for every roadmap issue

A roadmap item is complete only when all applicable conditions are met:

- The behavior and non-goals are documented.
- Safety invariants and failure modes are identified.
- Domain, CLI, configuration, state, schema, and adapter changes are coherent.
- Tests cover the success path, boundaries, and material failures.
- Machine-readable output is schema-valid and human output is actionable.
- State migrations are forward-tested from every supported prior version.
- External capabilities and versions are discovered and recorded.
- No source mutation happens before validation and explicit authority.
- Documentation and examples match the final CLI.
- Formatting, strict linting, tests, smoke tests, and packaging checks pass.
- The change is represented in release notes or the changelog.

## Prioritization rules

When roadmap work competes, choose in this order:

1. Prevent or repair data loss, silent corruption, and false safety claims.
2. Preserve recovery, transaction, and schema integrity.
3. Correct false duplicate, relationship, quality, and storage evidence.
4. Maintain deterministic CLI and automation contracts.
5. Improve installation, diagnostics, and user comprehension.
6. Improve performance without weakening correctness.
7. Add new formats, profiles, detectors, and convenience features.

The central product rule remains simple: `optiflow` must earn mutation authority
through evidence, explicit policy, validation, and recoverability.
