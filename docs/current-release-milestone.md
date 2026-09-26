# Current Release Milestone — Operator-ready Path

This is the compact execution view from OptiFlow's shipped read-only product to
safe external-drive space reclamation.
[`ROADMAP.md`](https://github.com/egohygiene/optiflow/blob/main/ROADMAP.md)
remains the long-horizon product roadmap; live issue acceptance criteria and
merged evidence remain authoritative.

## Exact reconciliation pins

The starting evidence snapshot was captured on 2026-09-25 before this change.

| Evidence | Exact pin |
| --- | --- |
| Starting default branch | [`7de8483b64387542a05214004c19a9cd05628908`](https://github.com/egohygiene/optiflow/commit/7de8483b64387542a05214004c19a9cd05628908) |
| Public release | [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0), published 2026-09-12 |
| Released source | [`f04c82a0b0c677a2939ea351c4219602cd7181af`](https://github.com/egohygiene/optiflow/commit/f04c82a0b0c677a2939ea351c4219602cd7181af) |
| Annotated tag object | `1e0381cb9e92616fabda2828f201ef1c1f2d4688` |
| Release workflow | [Run 34698618575](https://github.com/egohygiene/optiflow/actions/runs/34698618575) |

The release bundle contains checksums, an SPDX SBOM, SLSA provenance, and a
keyless signature for its release-subject manifest. The annotated Git tag
object reports as unsigned, so this documentation claims signed and verified
release evidence, not a cryptographically signed Git tag.

At the starting `main` revision, the latest push runs for
[CI](https://github.com/egohygiene/optiflow/actions/runs/35542085075),
[adversarial tests](https://github.com/egohygiene/optiflow/actions/runs/35542085042),
[documentation](https://github.com/egohygiene/optiflow/actions/runs/35542085113),
[site publication](https://github.com/egohygiene/optiflow/actions/runs/35542085069),
[security](https://github.com/egohygiene/optiflow/actions/runs/35542085155),
and [Identity validation](https://github.com/egohygiene/optiflow/actions/runs/35542085082)
all succeeded.

## Capability truth

| State | What it means now |
| --- | --- |
| Released in `v0.1.0` | Read-only inventory, exact-duplicate proof, conservative reclaimable-byte evidence, and immutable review planning from the released source pin. |
| Merged after `v0.1.0` | Extension, media-profile, bounded PNG-validation, documentation, and performance evidence on `main`. These additions are not retroactively present in the `v0.1.0` binary. |
| Planned | The issue-owned operator chain below. A plan or closed design issue does not make behavior available. |
| Unsupported | Candidate production, optimization execution, apply, replace, quarantine, restore, irreversible finalization, and measured physical-space reclamation. |

Version `0.1.0` and current `main` are read-only with respect to source media.
A review plan is evidence for an operator; it is never write authorization.

## Completed evidence chain

| Issue | Delivered evidence |
| --- | --- |
| #21 — NativePath v4 | Lossless native path identity across state and artifact boundaries |
| #23 — bounded subprocess runner | Direct argv execution with time, output, concurrency, cancellation, and typed failure bounds |
| #22 — handle-bound observations | One stable read handle binds identity, allocation, content, and optional probe evidence |
| #24 — artifact-set commit protocol | Staged, digest-verified publication with incomplete/incompatible refusal and recovery |
| #26 — adversarial matrix | Property, filesystem-fault, parser, and artifact-reader evidence |
| #27 — release policy | Dependency policy plus the published and verified `v0.1.0` release path |
| #51 — safe extension SDK | Explicit declarations and locks, typed read-only roles, deterministic resolution, and bounded providers |
| #29 — CLI and performance | Focused command coordination, stable remediation guidance, and enforced synthetic performance budgets |
| #66 — lossless PNG review profile | Deterministic, provider-bound review evidence with explicit limitations and no savings estimate |
| #28 — production site | Canonical production publication, live validation, and rollback evidence |
| #75 / PR #76 | Bounded batching, perceptual-validation, and metadata-policy planning; no mutation authority |
| #78 / PR #79 | Static PNG candidate preparation contract; no encoder or candidate output |
| #80 / PR #82 | Read-only validation of actual PNG source/candidate bytes within a documented subset |
| #83 / PR #84 | Media capability matrix and optimizer-strategy documentation |
| #85 / PR #86 | Deterministic cold-discovery performance evidence on the pinned starting `main` |

The live GitHub records remain authoritative for the full acceptance evidence.
Closed planning, contract, validation, documentation, or CI work is not evidence
of an optimizer or transaction engine.

## Operator-ready execution chain

Work proceeds by dependency. Rows 1–7 are the primary operator chain; #94 is a
parallel candidate-production lane, and #95 is their `v0.3.0` join. Each row
has exactly one capability owner.

| Gate | Owning issue | Bounded outcome |
| --- | --- | --- |
| 1 | [#88](https://github.com/egohygiene/optiflow/issues/88) | Filesystem, path, state, and removable-volume corpus foundations |
| 2 | [#89](https://github.com/egohygiene/optiflow/issues/89) | External-drive read-only pilot and signed `v0.1.1` |
| 3 | [#90](https://github.com/egohygiene/optiflow/issues/90) | Execution/approval contracts and non-mutating dry-run preflight |
| 4 | [#91](https://github.com/egohygiene/optiflow/issues/91) | Bounded exact-duplicate quarantine apply |
| 5 | [#92](https://github.com/egohygiene/optiflow/issues/92) | Status, resume, restore, cleanup, and fault recovery |
| 6 | [#96](https://github.com/egohygiene/optiflow/issues/96) | Separately authorized quarantine finalization |
| 7 | [#93](https://github.com/egohygiene/optiflow/issues/93) | Removable-volume qualification and signed exact-deduplication `v0.2.0` |
| Parallel after relevant #65 fixtures | [#94](https://github.com/egohygiene/optiflow/issues/94) | Bounded, source-preserving OxiPNG candidate production |
| Join after #93 and #94 | [#95](https://github.com/egohygiene/optiflow/issues/95) | Transactional validated lossless PNG replacement and signed `v0.3.0` |

The strict dependency shape is:

1. #88 → #89 qualifies real read-only use before mutation.
2. #88 → #90 → #91 → #92 → #96, with #89 also complete, enables #93.
3. The relevant provider/media/candidate fixtures under
   [#65](https://github.com/egohygiene/optiflow/issues/65) enable #94.
4. #93 and #94 together enable #95.

The first honest space-reclamation release is #93's `v0.2.0`, not the #89
read-only pilot. #94 may produce validated candidate artifacts only in
OptiFlow-owned storage; it grants no replacement authority. #95 is the first
lossless-PNG replacement release.

## Separately gated work

- [#60](https://github.com/egohygiene/optiflow/issues/60) remains blocked on
  the shared ADR system. It is not a prerequisite for starting #88.
- [#61](https://github.com/egohygiene/optiflow/issues/61) remains the deferred
  post-roadmap repository, backlog, and Identity audit. It does not replace an
  active implementation issue.
- #65 remains the corpus umbrella after #88 for media, provider,
  candidate-comparison, perceptual, compatibility, and resource-stress fixture
  families. Those later families do not block the exact-duplicate `v0.2` chain
  unless a consuming issue explicitly depends on them.
- Later image and audio/video profiles remain long-horizon roadmap work. Once
  #61's Flow-suite completion gate is satisfied, that audit owns turning
  confirmed gaps into bounded issues; no later format is supported merely
  because it appears in the roadmap.

## Next checkpoint

[#88](https://github.com/egohygiene/optiflow/issues/88) is complete through
[PR #101](https://github.com/egohygiene/optiflow/pull/101), merged at
`ddc4b010caf5ad51c3d3f4b04e6b967a87ff8eea`. [#89](https://github.com/egohygiene/optiflow/issues/89)
is active: the [pilot guide](external-drive-pilot.md) and mandatory native
qualification prepare `v0.1.1`. The issue remains open until all three targets
qualify and the signed release is published and independently verified.
Then #90 adds execution/approval contracts and a non-mutating dry run.

The Flow-suite coordinator is
[`flow#11`](https://github.com/egohygiene/flow/issues/11).
