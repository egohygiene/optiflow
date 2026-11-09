---
title: Public Repository Intelligence source feeds
description: Bounded public Work, Health and Releases capture handoff for OptiFlow's existing Pages build.
---

# Public Repository Intelligence source feeds

This implementation checkpoint starts from OptiFlow main
`fa4347e0a660412b82cd31c07e46d571a52d1749`, which merged native Journey and
attributed Search in [#162](https://github.com/egohygiene/optiflow/pull/162).
The new producer and current generator select the same final reviewed immutable
Relay revision, staged here as `REVIEWED_INTELLIGENCE_RELAY_SHA` until the
cross-repository implementation review finishes. That marker must be replaced
with the full exact SHA before PR publication; it is not an available release.
Refs [organization #41](https://github.com/egohygiene/.github/issues/41),
[Relay #146](https://github.com/egohygiene/relay/issues/146),
[#149](https://github.com/egohygiene/relay/issues/149), and
[#150](https://github.com/egohygiene/relay/issues/150).

## Existing Pages job integration

On push and manual dispatch, the existing complete-history checkout precedes a
pinned Relay source producer. It establishes public repository identity through
unauthenticated GitHub reads before and after acquisition and verifies the
exact represented Git/API commit. It writes bounded public issue/PR inventories,
matching check runs and published release metadata into the existing transport
contracts. No consumer script, scanner or provider mutation is executed.

The next ordinary artifact upload retains the closed same-run/head/attempt
capture bundle for seven days. The existing native generator receives its
exact immutable artifact ID/digest and the producer's actual UTC `as-of`
boundary. A separate seven-day ordinary artifact retains the bounded producer
receipt outside the site. Captures and receipts stay in runner-temporary
directories; raw responses and capture files are not copied into `dist/`.

The build job adds only the read permissions needed for these operations:
`actions`, `issues`, `pull-requests`, and `checks`. The existing repository token
is used only by the pinned source producer and artifact intake. Repository
visibility, published releases, public tags and assets are always read without
credentials; draft release details cannot enter through token visibility.
Public titles/check names are explicitly selected and remain bounded and
sanitized. Bodies, comments, actors, emails, labels, app objects and logs are
excluded from the public source handoff.

PRs retain the existing no-capture path. Empty outputs from skipped source
steps select no artifact or later evaluation clock; native Journey and
attributed Search keep their previous source boundary. No workflow event,
existing step guard, job dependency, timeout, Pages branch gate, deployment
environment, existing artifact retention, or single-deployer authority changes.
The native action still runs before OptiFlow installs Python 3.13 through `uv`.

## Truthful source states

Work keeps issues and PRs distinct and preserves original states, closure
reasons and pagination. It derives no acceptance or completion from merges and
does not infer relationships from bodies. Health selects exact-head check runs,
preserves execution state/conclusion, and keeps the applicable producer inventory
partial. Releases resolves exact public tag refs, peels annotated tags, and
requires represented historical Git objects before admitting public asset names,
sizes and supplied digests. Missing, rate-limited, truncated or malformed
observations remain unavailable or partial rather than empty, complete or healthy.

Deployment and install evidence remain uncollected. Generic GitHub metadata
cannot invent an exact consumer run attempt or deployment receipt. Existing
normalized report summaries keep their original scanner commits, execution,
findings and freshness; this adoption does not relabel them current.

Native Journey and attributed Search remain enabled. Captured push/manual paths
also select `collect-dependencies` using the actual capture evaluation boundary.
The exact pinned Hygiene public roster admits only declared public consumption
and permitted boundary evidence; it proves no runtime/package adoption. The
existing nine-domain coverage vocabulary stays unchanged, and missing inventory
domains remain uncollected. PRs select no dependency source; their no-capture evaluation uses a
commit-time clock that may predate the owner-catalog observation. No
canonical ADR/Roadmap collector, external snapshot/comparison or retained Compare
baseline is selected. Actual provider observations and later owner normalization
determine useful source evidence; this commit does not assert completed checks,
release delivery, publication, lifecycle ratification or issue acceptance.

## Review and recovery boundary

[ADR-0001](decisions/ADR-0001-publication-stack.md) governs isolated producer
composition and pinned Relay consumption. ADR not required: this supplies data
to that existing architecture without changing ownership or deployment policy.
The composer, verifier, product assets, unrelated release-workflow Relay pin and
the [reviewed-revert recovery procedure](site-publication.md#rollback) retain
their original bytes. No historical rollback selector, source, digest or order
is rewritten and no replacement rollback point is claimed.

Static YAML/shell parsing, source/input inspection and comparison of preserved
event/job/deployment gates are the scoped review evidence. Native collection,
tests, builds, dependency installation, CI, API/artifact canaries, browser review,
publication and recovery qualification remain deferred at the maintainer's
request. The existing fifteen-minute build budget remains unqualified for the
additional capture/normalization work. There is no root `AGENTS.md` or
`CONTINUITY.md` in the starting checkout; this bounded handoff records the
integration and its limits without inventing prior operational state.
