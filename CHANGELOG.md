# Changelog

All notable changes to OptiFlow are documented here. The project is pre-1.0;
public contracts still receive explicit migration notes.

## Unreleased

### Added

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

### Changed

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
- Current run, report, and plan contracts are v5 and carry artifact-set identity
  bindings. Readers distinguish committed, incomplete, and incompatible sets.

### Compatibility

- Existing command names and the `--json` alias remain available.
- Existing database migrations are unchanged.
- Immutable `optiflow.run.v1`–`v4`, `optiflow.report.v1`–`v4`, and
  `optiflow.plan.v1`–`v4` contracts are unchanged and remain reviewable without
  artifact-set markers.
- No source-file mutation or apply behavior was introduced.
