# Current Release Milestone — v0.1.x Foundation

This is the compact execution view for optiflow's current release foundation.
[`ROADMAP.md`](https://github.com/egohygiene/optiflow/blob/main/ROADMAP.md)
remains the long-horizon product roadmap; this file records which existing
issues supplied the read-only baseline and what is dependency-ready next.

## Milestone objective

Maintain a trustworthy read-only `v0.1.x` product whose path identity,
observations, external-tool execution, artifact publication, fault behavior,
extension boundary, release evidence, CLI semantics, and performance budgets
are explicit before source mutation is introduced.

## Completed dependency chain

| Issue | Delivered evidence |
| --- | --- |
| #21 — NativePath v4 | Lossless native path identity across state and artifact boundaries |
| #23 — bounded subprocess runner | Direct argv execution with time, output, concurrency, cancellation, and typed failure bounds |
| #22 — handle-bound observations | One stable read handle binds identity, allocation, content, and optional probe evidence |
| #24 — artifact-set commit protocol | Staged, digest-verified publication with incomplete/incompatible refusal and recovery |
| #26 — adversarial matrix | Property, filesystem-fault, parser, and artifact-reader evidence |
| #27 — signed release policy | Dependency policy and the verified `v0.1.0` release path |
| #51 — safe extension SDK | Explicit declarations and locks, typed read-only roles, deterministic resolution, and bounded providers |
| #29 — CLI boundaries and performance | Focused command coordination, stable remediation guidance, and enforced synthetic performance budgets |
| #66 — lossless PNG profile evidence | Versioned, deterministic, provider-bound review evidence with explicit limitations and no savings estimate |
| #28 — production site | Canonical production publication, live validation, and rollback evidence |

The live GitHub issue and pull-request records remain authoritative for exact
merge commits and workflow runs.

## Completed #29 — CLI boundaries and performance

Issue #8 and PR #15 already established the stable exit-code, JSON-envelope,
signal, stream-ownership, and typed-diagnostic contract that #29 depends on.
The remaining #29 work is now represented by:

- command coordination separated into scan, report/plan, and extension modules;
- an optimized hard-link alias path without collection-wide repeated lookup;
- a versioned synthetic fixture and enforceable wall-time, memory, and artifact
  size ceilings;
- retained Linux CI measurements from the optimized release binary; and
- explicit progress and caller-remediation guidance that preserves one clean
  terminal JSON result.

This checkpoint does not add a live progress stream, source-media mutation,
optimization execution, new product JSON schema, database migration, or
extension authority.

## Completed #66 — first media-profile evidence

The first bounded OPT-Q04 media profile selects files by inspected
`image/png` content and evaluates only current, handle-bound observations with
semantically validated `ffprobe` evidence. The report embeds the independently
versioned `optiflow.media-profile-evidence.v1` contract under the new
`optiflow.report.v6` identifier.

The built-in `optiflow.builtin.lossless-png-review@1.0.0` profile produces
deterministic review-candidate identifiers and records exact provider version,
executable path, executable digest, invocation fingerprint, evidence-policy
fingerprint, normalized PNG facts, coverage, and limitations. It creates no
candidate output, makes no savings estimate, and never changes source files.
Missing, failed, invalid, or stale evidence produces no opportunity. Run and
plan schemas, SQLite migrations, artifact-set membership, extension authority,
and existing commands remain unchanged.

## Current evidence checklist

- [x] Native paths round-trip losslessly through supported artifact/state boundaries.
- [x] Observation evidence cannot silently combine incompatible file states.
- [x] External tools use the typed bounded subprocess and provider paths.
- [x] Related artifacts are distinguishable as committed, incomplete, or incompatible sets.
- [x] Adversarial and fault tests exercise the combined read-only invariants.
- [x] Public release artifacts carry the documented dependency, checksum, SBOM, provenance, and signing evidence.
- [x] CLI exits, JSON stdout, diagnostics, and interruption behavior have a stable documented contract.
- [x] Representative discovery, candidate-hashing, cache-reuse, memory, and artifact-size budgets are enforced.
- [x] Production documentation and site claims preserve the read-only boundary.
- [x] The first media profile preserves current observation, provider, policy,
  path, artifact-set, and deterministic identity evidence.

Unchecked items in the detailed `v0.1.x` roadmap that lack a current issue are
future issue-planning inputs, not implicit additions to #29.

## What comes after

OPT-Q04 is complete through issues #51, #29, and #66. Broader format support,
candidate-output production, savings measurement, and optimization execution
remain separate checkpoints.

OPT-Q05 then introduces reviewed transactions only after preview, explicit
authorization, current precondition proof, output validation, durable commit,
interruption, and rollback behavior are specified together. flow may
orchestrate released optiflow commands; it does not own optiflow's file-safety
semantics or import its source.
