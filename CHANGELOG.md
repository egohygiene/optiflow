# Changelog

All notable changes to OptiFlow are documented here. The project is pre-1.0;
public contracts still receive explicit migration notes.

## Unreleased

### Added

- Added a source-pinned README media capability matrix and optimizer-strategy
  reference, separating inventory, conditional probing, review evidence, PNG
  byte validation, planned production and future transactions. Documents
  `image_optim` prior art without adding providers or runtime dependencies.

- Added a read-only PNG byte-pair validator with actual complete decoding,
  exact pixel/metadata comparison, observed content identities, explicit byte
  limits, and real synthetic success/refusal fixtures. Initial support is
  noninterlaced 8-bit RGB/RGBA with a documented metadata subset. The opaque
  result does not attest filesystem/process behavior or authorize artifacts;
  no encoder, CLI integration, source writes, or wire-schema changes are added.

- Added the separate `optiflow.png-candidate-contract.v1` preparation contract,
  pure consistency checker, and synthetic success/refusal fixtures for a
  future static RGB/RGBA PNG candidate. This does not produce or validate PNG
  bytes, select an encoder, measure savings, or change v0.1 CLI/write authority.

- Added `optiflow.media-profile-evidence.v1` and the built-in
  `optiflow.builtin.lossless-png-review@1.0.0` analysis profile. Current,
  handle-bound PNG observations can now produce deterministic review
  opportunities with exact provider and policy provenance, explicit coverage,
  required future-output validations, and no savings estimate.
- Added hermetic success, unavailable, invalid, timeout, deterministic-rerun,
  partial-coverage, artifact-validation, and source-preservation evidence for
  media-profile analysis, plus an installed-`ffprobe` PNG smoke fixture.
- Added a versioned synthetic read-only performance fixture with enforced cold
  discovery, complete hashing, warm-cache, peak-memory, and artifact-size
  ceilings plus retained CI measurement evidence.
- Added the independently versioned `optiflow.extension-sdk.v1` boundary with
  closed manifest, operator-lock, invocation, and result schemas; typed roles
  cover inspectors, analyzers, normalization/policy contributors, planners,
  validators, report/export providers, and read-only lifecycle observers.
- Added deterministic explicit provider selection, byte-pinned trusted process
  invocation, typed registration, result/provenance revalidation, honest
  partial coverage, `extensions list|inspect|doctor`, runnable SDK examples,
  generated capability reference checks, and adversarial failure fixtures.
- Added a deterministic `/architecture/` portal generated from the complete
  canonical document corpus, including an interactive dependency graph,
  system boundaries, structural layers, filtered document inventory, and a
  machine-readable architecture projection.
- Added versioned configuration and output schemas for the architecture portal
  while preserving root architecture documents as the sole canonical source.
- Added the complete 18-document Aether architecture corpus for OptiFlow,
  including canonical metadata, dependency relationships, decision records,
  design semantics, AI authority, and a meta-architecture inventory.
- Added cloud-native placement guidance that keeps OptiFlow a portable workload
  while mapping CNCF capability families onto the surrounding Ego Hygiene
  platform repositories and a need-driven adoption ladder.
- Added a responsive, accessible, LaunchKit-derived product landing page that
  presents OptiFlow's current evidence pipeline and read-only safety boundary
  without implying unavailable distribution or mutation capabilities.
- Added an isolated, collision-checked site composition build and structural
  link verifier for the landing page, Zensical documentation, and canonical
  schema downloads.
- Added a pinned Zensical documentation environment, branded documentation
  home and navigation, strict link validation, and a build-only documentation
  workflow.
- Added the static site publication contract for the future LaunchKit landing
  page, documentation, API reference, schemas, intelligence, and release
  surfaces.
- Added strict, typed `optiflow.config.v1` TOML with deterministic user,
  project, explicit-file, environment, and CLI precedence.
- Added `optiflow.effective-policy.v1`, per-leaf provenance and shadow traces,
  BLAKE3 effective-configuration and evidence-policy identities, and locked
  safety invariants.
- Added read-only `config validate`, `config show`, and `config explain`
  commands plus `--config`, `--no-config`, closed environment mappings, and
  explicit positive/negative scan-policy overrides.
- New scans persist a validated `effective-policy.json` sidecar; report and plan
  expose historical policy without reconstructing it from current defaults.
- Added the versioned `optiflow.command-result.v1` envelope and checked-in JSON
  Schema.
- Added centralized typed outcomes, diagnostic classifications and impacts,
  coverage resolution, committed-artifact references, and stable exit codes
  `0`, `1`, `2`, `3`, `4`, `5`, `130`, and `143`.
- Added `--output-format human|json`; the documented `--json` flag remains a
  compatible alias.
- Added cooperative `SIGINT` and `SIGTERM` handling. Interrupted scan rows are
  marked `interrupted` and are never finalized as complete.
- Added runtime validation of generated run, report, plan, and command-result
  documents before reporting success.
- Added `optiflow.artifact-set.v1`, which binds related artifacts to validated
  schemas, identities, byte lengths, and BLAKE3-256 content digests.
- Added staged scan-set publication, recoverable plan-marker publication, and
  startup reconciliation for crashes between filesystem and SQLite commits.
- Added deterministic crash and disk-exhaustion fault injection for artifact
  staging, visibility, and recovery boundaries.
- Added a cross-platform adversarial fault matrix, property-based serialization
  and planning checks, bounded parser/artifact-reader fuzz targets, and retained
  CI evidence logs.
- Added an enforced dependency admission policy, scheduled license,
  vulnerability, secret, and supply-chain checks, and deterministic release
  evidence contract tests.
- Added a full-SHA-pinned Relay binary release path for reproducible Linux and
  macOS archives with complete checksums, an artifact-bound SPDX SBOM, SLSA v1
  provenance, and a keyless Sigstore signature.
- Documented supported release targets, private security reporting,
  independent verification, release operations, and immutable rollback.

### Changed

- Reconciled the public `v0.1.0` release, post-release read-only source, and
  operator-ready roadmap. Documentation now records exact release and `main`
  pins, corrects published-binary availability, and assigns #88 through #96 to
  the dependency-gated `v0.1.1`, `v0.2.0`, and `v0.3.0` DAG without
  claiming candidate-production or mutation support.
- Made the synthetic performance guard resilient to isolated shared-runner
  stalls without raising any budget. CI now retains three independent trials,
  enforces median wall time, and keeps worst-case artifact-size and peak-memory
  evidence while validating scan correctness in every trial. The CI-only
  measurement evidence advances to `optiflow.performance-baseline.v2`; product
  JSON contracts are unchanged.
- Advanced the report contract to `optiflow.report.v6` to embed media-profile
  evidence. Run and plan contracts remain v5; both report v5 and v6 require a
  matching artifact-set marker.
- Bound built-in `ffprobe` evidence to one canonical executable path, exact
  version line, BLAKE3-256 binary digest, and invocation fingerprint. A zero
  exit status is insufficient: stderr, JSON shape, semantic observations, and
  provider identity must all validate.
- Split application command coordination into scan, report/plan, and extension
  modules, and removed repeated collection-wide lookup while classifying
  hard-link aliases.
- Documented the intentional absence of a live `v0.1.x` progress stream and
  added outcome-specific remediation guidance without changing JSON stdout.
- Pages pull requests now retain a reviewable site artifact, production
  deployment is restricted to `main`, and the site verifier enforces canonical
  metadata, accessibility structure, and reduced-motion baselines for public
  entry points.
- JSON command output is now a command-result envelope. The previous top-level
  scan/report/plan value is available under `result`, and committed artifacts
  are listed under `artifacts`. Consumers of the pre-1.0 `--json` output should
  select `.result` after checking `.schema` and `.outcome`.
- A valid result with incomplete coverage now exits `3` instead of claiming
  complete success. Empty complete scans and complete no-match results remain
  successful with exit code `0`.
- Human primary output is written to stdout; escaped diagnostics are written to
  stderr. JSON stdout is reserved for one buffered machine document.
- Aligned the `rusqlite` manifest requirement with the already resolved 0.40
  lockfile dependency and its checked integer-conversion behavior.
- Current run and plan contracts are v5, while the current report is v6. These
  documents carry artifact-set identity bindings, and readers distinguish
  committed, incomplete, and incompatible sets.

### Compatibility

- Issue #66 adds no command, database migration, source mutation, output
  production, extension authority, or cross-repository dependency. Report v6
  is the only changed product artifact identifier; report v1-v4 remain
  reviewable without markers and report v5 remains reviewable only with its
  original complete marker.
- Issue #29 does not change commands, options, exit codes, product JSON schemas,
  state migrations, extension authority, or the read-only source boundary.
- Existing CLI, configuration, run, report, plan, command-result, database, and
  artifact-set contracts are unchanged. The extension commands and four
  extension wire identifiers are additive; no new crate dependency was added.
- Existing command names and the `--json` alias remain available.
- Existing database migrations are unchanged.
- Immutable `optiflow.run.v1`–`v4`, `optiflow.report.v1`–`v4`, and
  `optiflow.plan.v1`–`v4` contracts are unchanged and remain reviewable without
  artifact-set markers.
- No source-file mutation or apply behavior was introduced.
