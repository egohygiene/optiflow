# Checkpoint 12: actual-workload release admission

Source repair authored November 13, 2026 (America/New_York). This packet describes
source policy and mocked checks. It does not record a hosted run, native execution,
release qualification, artifact verification or publication.

## Why the extra gate exists

A workflow conclusion can look successful when product jobs were skipped. The
cheap budget-policy/formatting lane and deferred local plan therefore cannot
authorize release. The budget generator preserves manual release workflows; this
separate adjunct intentionally tightens two existing consumers with read-only job
admission while preserving triggers, permissions, environments, approval gates,
artifact checks and publication steps.

## Four repository audit

- **Aniflow:** the existing candidate-run guard now requires authorize, all three
  declared native target jobs and seal, each successful with the exact meaningful
  qualification/build/sealing steps successful. Both exact default matrix display
  orders (triple, runner) and (runner, triple) map to one target identity; duplicate
  aliases cannot satisfy a missing target. Existing current-main, dependency and
  immutable-tag gates remain prerequisites.
- **Renderflow:** manual release gains a read-only precondition requiring complete
  CI, docs and manual conformance job inventories on the exact release source, plus
  the latest scheduled maximal-conformance workload within the existing eight-day
  window. The scheduled source may differ; its actual SHA is validated and reported.
  Its conformance workflow bytes are fetched read-only at that exact SHA and must
  have the same SHA-256 as the reviewed policy source. A changed historical workflow
  requires fresh qualification, even when job/step display names stayed identical.
  The scheduled fast job must be explicitly skipped. Existing run-status checks
  remain in place as additional prerequisites.
- **Optiflow:** current-source publication is already denied by the historical
  version/commit allowlist in its release authorization step. It has no current-main
  consumer that admits publication based only on a cheap CI conclusion. No allowlist,
  native-mutation or #93/#111 qualification gate is changed.
- **Flow:** the source-only handoff packaging path has no live workflow-success
  publication consumer. Its separate publication gates are unchanged; no consumer
  is invented and no release qualification is claimed.

## Read-only evidence contract

The policy binds reviewed workflow bytes (and Aniflow's target declaration) by
SHA-256. Source drift refuses admission until the expected inventory/critical steps
are reviewed and policy is explicitly re-authored. This prevents silently treating
a job renamed to formatting-only work as the previously reviewed qualification job.

Run lists and latest-attempt job collections are read with explicit 100-record
pages, a ten-page/1,000-record ceiling and consistent total counts. Empty required
inventories, short incomplete pages, changed counts, duplicate IDs, missing matrix
members and unexpected jobs refuse. API read failure, missing fields or oversized
payloads refuse rather than supplying a successful fallback.

Each run binds its repository/head repository, full SHA, main branch, workflow path,
event, completed/success state and positive run/attempt identity. Each job binds run
ID, full source SHA and the selected attempt; every required job and critical named
step must have completed successfully. Historical scheduled workflow content must
match the reviewed source bytes, with bounded size and strict base64 decoding. A
final run read rejects a changed attempt
or state during collection. Partial reruns that retain jobs from earlier attempts
are intentionally refused: obtain a complete qualifying attempt. Latest-run failure
cannot fall back to an older green run. The bounded JSON report records the exact
admitted runs, attempts, workload names and source revisions without publishing.

## Focused verification and limits

Run only the new mocked contracts when authorized:

    python3 -B -m unittest discover -s tests/ci_release_evidence -p 'test_*.py'

These checks cover complete and truncated pagination, source drift, all-skipped,
empty and source-policy-only successes, incomplete/duplicate matrix identities,
skipped/failed workload steps, wrong run/SHA/attempt, latest failed runs and stale
scheduled evidence and changed historical workflow bytes. Fixtures are synthetic
API records; no actual GitHub job-name
inventory, token permission, native workload or publication has been observed here.
Unexpected real API/matrix naming fails closed and requires review. Mock success
does not establish product qualification, signed artifact validity or native APFS
behavior. Optiflow #93/#111 and the Flow final native qualification issue remain
unchanged and deferred.
