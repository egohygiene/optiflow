# Changelog

All notable changes to OptiFlow are documented here. The project is pre-1.0;
public contracts still receive explicit migration notes.

## Unreleased

### Added

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

### Compatibility

- Existing command names and the `--json` alias remain available.
- Existing database migrations are unchanged.
- Immutable `optiflow.run.v1`–`v3`, `optiflow.report.v1`–`v3`, and
  `optiflow.plan.v1`–`v3` contracts are unchanged and remain reviewable.
- No source-file mutation or apply behavior was introduced.
