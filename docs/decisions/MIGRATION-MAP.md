# OptiFlow ADR history migration map

## Purpose and authority boundary

This document inventories OptiFlow's pre-existing architecture-decision
history and proposes a future migration cutover. It is the preparatory record
for [issue #77](https://github.com/egohygiene/optiflow/issues/77), a bounded
child of [issue #60](https://github.com/egohygiene/optiflow/issues/60).

The inventory is based on the complete reachable Git history, public issues,
pull requests, the `v0.1.0` release, workflows, and the repository sources at
[`d67250ca6dc71637672ad6ed5f49d46da5cbdde0`](https://github.com/egohygiene/optiflow/commit/d67250ca6dc71637672ad6ed5f49d46da5cbdde0).
That revision's [CI](https://github.com/egohygiene/optiflow/actions/runs/35155272752),
[adversarial](https://github.com/egohygiene/optiflow/actions/runs/35155272706),
[security](https://github.com/egohygiene/optiflow/actions/runs/35155272744),
and [Pages](https://github.com/egohygiene/optiflow/actions/runs/35155272709)
workflows passed. The map is point-in-time evidence, not a substitute for
checking live state before the cutover.

This document does **not**:

- make this map or the current root log a policy-valid ADR index;
- approve, reject, deprecate, or supersede any decision;
- convert a merge, release, implementation, agent conclusion, or legacy status
  label into human disposition evidence;
- split the combined log, change the root `DECISIONS.md`, add policy or agent
  guidance, install validation, or alter publication; or
- resolve the relationship between `OFD-008` and `ADR-0001`.

The current root [`DECISIONS.md`](https://github.com/egohygiene/optiflow/blob/d67250ca6dc71637672ad6ed5f49d46da5cbdde0/DECISIONS.md)
remains canonical until a separately reviewed atomic cutover.

## Evidence method

The inventory follows the repository's
[`EPISTEMOLOGY.md`](https://github.com/egohygiene/optiflow/blob/d67250ca6dc71637672ad6ed5f49d46da5cbdde0/EPISTEMOLOGY.md)
vocabulary:

- **Observed** means the named repository artifact or public GitHub resource
  was inspected.
- **Derived** means the statement follows mechanically from observed Git
  history, such as the first commit that introduced a file.
- **Inferred** means the evidence supports a likely relationship but does not
  prove intent; inferences remain labeled and require review.
- **Proposed** means this map recommends a later migration action without
  granting authority to perform it.
- **Unknown** means the public evidence inspected here does not establish the
  claim.

Three independent axes are recorded for every decision:

1. **Legacy lifecycle label** is the verbatim status in the historical source.
2. **Disposition evidence** asks whether the current organization policy's
   explicit human-approval requirement is satisfied.
3. **Implementation evidence** records delivery and verification separately;
   it never determines lifecycle status.

Hygiene policy v1.1.0 was reviewed at
[`f598ed659a43dd759d4ede41c27f9e5daf991aa7`](https://github.com/egohygiene/hygiene/commit/f598ed659a43dd759d4ede41c27f9e5daf991aa7),
[explicitly approved by the maintainer](https://github.com/egohygiene/hygiene/issues/15#issuecomment-5647398908),
and activated by
[`bae230ba92fd231e8f26e24f85667c7117e4021b`](https://github.com/egohygiene/hygiene/commit/bae230ba92fd231e8f26e24f85667c7117e4021b).
The [activated policy](https://github.com/egohygiene/hygiene/blob/bae230ba92fd231e8f26e24f85667c7117e4021b/docs/decisions/POLICY.md)
requires every non-proposed disposition to identify a human authority, date,
and durable approval URL. No such approval was found in the source issues,
pull-request reviews, or comments for the records below. Every `Accepted`
value is therefore recorded as a **legacy claim awaiting human disposition**,
not carried forward as accepted by this map.

## Identity and numbering constraints

| Existing surface | Observed identity | Proposed migration treatment |
| --- | --- | --- |
| Combined root log | `OFD-001` through `OFD-011` | Preserve every ID and create one canonical file per record only after an approved alternate-prefix exception exists. Never issue a new `OFD-*` ID. |
| Detailed site record | `ADR-0001` | Preserve the four-digit ID and existing filename as migration history under an approved width exception. |
| Standard local namespace | `ADR-NNN` | Treat numeric slot `001` as occupied by historical `ADR-0001`; the first available new standard ID is `ADR-002`. |
| Canonical index | None under the accepted contract | Later generate `docs/decisions/README.md`; this map is preparatory and noncanonical. |

Holon's immutable
[index implementation](https://github.com/egohygiene/holon/blob/de5047ee5d1515ece0c2c38993f3cefdf51faa69/tools/architecture_decision_blueprint.py#L345-L394)
uses `(prefix, integer value)` as decision identity while preserving the
original digit width in output. `ADR-0001` and `ADR-001` therefore collide as
the same `ADR` number; the existing four-digit record occupies slot `001`.

The accepted policy permits a historical prefix under a documented exception,
but the activated
[decision schema](https://github.com/egohygiene/hygiene/blob/bae230ba92fd231e8f26e24f85667c7117e4021b/schemas/architecture-decision.v1.schema.json)
restricts decision IDs and lineage references to `ADR-NNN` or `ADR-NNNN`.
Policy exceptions cannot locally override the global schema. The prefix and
width exceptions are therefore necessary but not sufficient: canonical
cutover also requires a Hygiene-compatible legacy-ID and lineage mechanism or
schema revision, followed by matching EgoLint and Relay support. An OptiFlow
extension must not invent that contract.

The legacy root log anticipated a future
[`docs/architecture/decisions/`](https://github.com/egohygiene/optiflow/blob/d67250ca6dc71637672ad6ed5f49d46da5cbdde0/DECISIONS.md#L46-L50)
location. The repository subsequently established
`docs/decisions/ADR-0001-publication-stack.md`, and the accepted organization
policy standardizes `docs/decisions/`. The cutover should record that divergence
and use `docs/decisions/`; it must not create a third decision location.

The proposed filenames below preserve the stable ID. Slugs are navigation aids
and may be adjusted during human review without changing identity.

### Record-source provenance

| Records | First record commit | Delivery pull request and merge | Source issue |
| --- | --- | --- | --- |
| `OFD-001` through `OFD-010` | [`c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f`](https://github.com/egohygiene/optiflow/commit/c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f) | [PR #19](https://github.com/egohygiene/optiflow/pull/19), merged by [`43ead53b859c1bb21ea4ea66e654ca6e2e5f19b2`](https://github.com/egohygiene/optiflow/commit/43ead53b859c1bb21ea4ea66e654ca6e2e5f19b2) | No dedicated issue located in public history. |
| `ADR-0001` | [`5f3d57eb3d2daa6a9c9f80187238581c2940170d`](https://github.com/egohygiene/optiflow/commit/5f3d57eb3d2daa6a9c9f80187238581c2940170d) | [PR #35](https://github.com/egohygiene/optiflow/pull/35), merged by [`b5ca9e44da7440fc9146b9c1b5299c619997cb97`](https://github.com/egohygiene/optiflow/commit/b5ca9e44da7440fc9146b9c1b5299c619997cb97) | [Issue #25](https://github.com/egohygiene/optiflow/issues/25) |
| `OFD-011` | [`fdbe629e82fcd53ec42c5fabf2323f35f45ddc6f`](https://github.com/egohygiene/optiflow/commit/fdbe629e82fcd53ec42c5fabf2323f35f45ddc6f) | [PR #62](https://github.com/egohygiene/optiflow/pull/62), merged by [`c9cf7c8572591c0ebcf72e2fbe3ccf1fcdfc5d69`](https://github.com/egohygiene/optiflow/commit/c9cf7c8572591c0ebcf72e2fbe3ccf1fcdfc5d69) | [Issue #51](https://github.com/egohygiene/optiflow/issues/51) |

The first ten `OFD-*` entries were reconstructed together in the architecture
corpus rather than captured contemporaneously one by one. Their implementation
links below therefore establish delivery history, not proof of the original
rationale or lifecycle authority.

The [`v0.1.0` release](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0)
resolves to source commit
[`f04c82a0b0c677a2939ea351c4219602cd7181af`](https://github.com/egohygiene/optiflow/commit/f04c82a0b0c677a2939ea351c4219602cd7181af).
It includes `OFD-001` through `OFD-010` and `ADR-0001`; it predates
`OFD-011`. Release inclusion is delivery evidence only.

## Existing-record summary

| ID | Legacy label | Disposition evidence | Observed implementation state | Proposed canonical filename | Human question |
| --- | --- | --- | --- | --- | --- |
| `OFD-001` | Accepted | Missing | Implemented and repeatedly exercised | `OFD-001-review-only-planning-authority.md` | Retain or correct the lifecycle label? |
| `OFD-002` | Accepted | Missing | Implemented and strengthened by artifact-set recovery | `OFD-002-local-state-and-json-artifacts.md` | Retain or correct the lifecycle label? |
| `OFD-003` | Accepted | Missing | Implemented and tested | `OFD-003-complete-blake3-exact-evidence.md` | Retain or correct the lifecycle label? |
| `OFD-004` | Accepted | Missing | Implemented across versioned schemas and migrations | `OFD-004-independent-wire-contract-versioning.md` | Retain or correct the lifecycle label? |
| `OFD-005` | Accepted | Missing | Implemented; bounded execution was strengthened later | `OFD-005-direct-bounded-adapter-invocation.md` | Retain or correct the lifecycle label? |
| `OFD-006` | Accepted | Missing | OptiFlow producer implemented; Flow integration remains in progress and is broader than subprocess-only wording | `OFD-006-flow-subprocess-integration.md` | Amend, relate, or supersede this boundary against Flow's current adapter contract? |
| `OFD-007` | Accepted | Missing | Documented and exercised; no dedicated architecture gate | `OFD-007-specification-schema-test-loop.md` | Retain or correct the lifecycle label? |
| `OFD-008` | Accepted | Missing | Implemented and publicly deployed | `OFD-008-static-site-producer-composition.md` | Related to, duplicated by, or superseded by `ADR-0001`? |
| `OFD-009` | Accepted | Missing | Implemented as an architectural boundary | `OFD-009-replaceable-platform-capabilities.md` | Retain or correct the lifecycle label? |
| `OFD-010` | Accepted | Missing | Manual metadata adoption implemented; managed adoption deferred | `OFD-010-aether-document-metadata-adoption.md` | Does released Aether guidance require amendment or supersession? |
| `OFD-011` | Accepted | Missing | Implemented and tested after `v0.1.0`; not in a later release | `OFD-011-extension-trust-and-core-acceptance.md` | Retain or correct the lifecycle label? |
| `ADR-0001` | Accepted | Missing | Implemented and publicly deployed | Existing `ADR-0001-publication-stack.md` | Related to, duplicate of, or superseding `OFD-008`? |

“Implemented” in this table describes repository evidence only. It is not a
proposed front-matter value, and “exercised” does not imply policy-valid
acceptance. The cutover must select formal implementation statuses from the
accepted policy after reviewing the named evidence.

## Detailed record evidence

### `OFD-001` — End `v0.1.x` authority at review-only planning

- **Origin:** The record first appears in
  [`c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f`](https://github.com/egohygiene/optiflow/blob/c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f/DECISIONS.md#L79-L90),
  delivered by [PR #19](https://github.com/egohygiene/optiflow/pull/19).
  No dedicated source issue was located.
- **Implementation:** The read-only plan boundary exists from the
  [bootstrap commit](https://github.com/egohygiene/optiflow/commit/539a9a427ee25cc73218a78384f90a38d442e7cd)
  and remains explicit in the snapshot
  [safety model](https://github.com/egohygiene/optiflow/blob/d67250ca6dc71637672ad6ed5f49d46da5cbdde0/docs/safety-model.md).
- **Delivery and verification:** The source is included in
  [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0).
  Current-main CI passed at the inventory snapshot. This is implementation and
  verification evidence only.
- **Uncertainty:** The record says `Accepted`; PR #19 has no human review or
  lifecycle-approval comment. Human disposition remains required.

### `OFD-002` — Use local SQLite plus immutable JSON artifacts

- **Origin:** The record first appears in
  [`c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f`](https://github.com/egohygiene/optiflow/blob/c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f/DECISIONS.md#L91-L103)
  through [PR #19](https://github.com/egohygiene/optiflow/pull/19), with no
  dedicated source issue located.
- **Implementation:** SQLite state and JSON artifacts begin in the
  [bootstrap commit](https://github.com/egohygiene/optiflow/commit/539a9a427ee25cc73218a78384f90a38d442e7cd).
  [Issue #24](https://github.com/egohygiene/optiflow/issues/24) and
  [PR #39](https://github.com/egohygiene/optiflow/pull/39) later made related
  artifact sets atomic and recoverable without changing the two-authority
  boundary.
- **Delivery and verification:** The original boundary is included in
  [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0);
  PR #39 names crash, disk-full, schema, and recovery validation.
- **Uncertainty:** No policy-valid human disposition evidence was found.

### `OFD-003` — Prove exact groups with complete BLAKE3 evidence

- **Origin:** The record first appears in
  [`c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f`](https://github.com/egohygiene/optiflow/blob/c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f/DECISIONS.md#L104-L116)
  through [PR #19](https://github.com/egohygiene/optiflow/pull/19), with no
  dedicated source issue located.
- **Implementation:** Complete BLAKE3 relationship evidence exists from the
  [bootstrap commit](https://github.com/egohygiene/optiflow/commit/539a9a427ee25cc73218a78384f90a38d442e7cd)
  and was hardened with filesystem identity and hard-link accounting in
  [PR #5](https://github.com/egohygiene/optiflow/pull/5). The later
  handle-bound observation work in
  [PR #38](https://github.com/egohygiene/optiflow/pull/38) binds hashes and
  identity evidence to one accepted observation window.
- **Delivery and verification:** The behavior is included in
  [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0) and is
  covered by the repository's current CI and adversarial suites.
- **Uncertainty:** No policy-valid human disposition evidence was found.

### `OFD-004` — Version wire contracts independently from the binary

- **Origin:** The record first appears in
  [`c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f`](https://github.com/egohygiene/optiflow/blob/c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f/DECISIONS.md#L117-L128)
  through [PR #19](https://github.com/egohygiene/optiflow/pull/19), with no
  dedicated source issue located.
- **Implementation:** Versioned schemas start in the
  [bootstrap commit](https://github.com/egohygiene/optiflow/commit/539a9a427ee25cc73218a78384f90a38d442e7cd)
  and were strengthened by [PR #13](https://github.com/egohygiene/optiflow/pull/13).
  NativePath v4 and later report contracts provide additional implementation
  evidence but are assessed separately below.
- **Delivery and verification:** The original boundary is included in
  [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0);
  schema and compatibility checks run in current CI.
- **Uncertainty:** No policy-valid human disposition evidence was found.

### `OFD-005` — Invoke external adapters directly without a shell

- **Origin:** The record first appears in
  [`c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f`](https://github.com/egohygiene/optiflow/blob/c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f/DECISIONS.md#L129-L139)
  through [PR #19](https://github.com/egohygiene/optiflow/pull/19), with no
  dedicated source issue located.
- **Implementation:** Direct `ffprobe` invocation begins in the
  [bootstrap commit](https://github.com/egohygiene/optiflow/commit/539a9a427ee25cc73218a78384f90a38d442e7cd).
  [Issue #23](https://github.com/egohygiene/optiflow/issues/23) and
  [PR #36](https://github.com/egohygiene/optiflow/pull/36) later added typed,
  bounded subprocess behavior.
- **Delivery and verification:** The boundary and bounded runner are included
  in [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0);
  PR #36 records Linux/macOS, CLI, and contract checks.
- **Uncertainty:** No policy-valid human disposition evidence was found.

### `OFD-006` — Integrate through `flow` as a subprocess contract

- **Origin:** The record first appears in
  [`c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f`](https://github.com/egohygiene/optiflow/blob/c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f/DECISIONS.md#L140-L152)
  through [PR #19](https://github.com/egohygiene/optiflow/pull/19), with no
  dedicated source issue located.
- **Implementation:** OptiFlow exposes the CLI, typed exit behavior, and
  versioned artifacts described by
  [PR #15](https://github.com/egohygiene/optiflow/pull/15) and
  [PR #16](https://github.com/egohygiene/optiflow/pull/16). Flow's later
  [federated contract](https://github.com/egohygiene/flow/blob/8688d1949fb6ca0cc4b5a034086f9ec7d620ae8f/docs/architecture/governance/decisions/ADR-0004-federated-suite-contracts.md#L37-L59)
  permits either a stable library or a versioned CLI, which is broader than
  the subprocess-only wording here. Flow's current
  [bounded transport checkpoint](https://github.com/egohygiene/flow/blob/a2cd59fe9937aa55ebece5c6dd86c7a34046f3be/docs/architecture/governance/decisions/ADR-0006-bounded-process-transport.md#L172-L177)
  validates supplied process transcripts but explicitly has no supported
  launcher or real OptiFlow adapter.
- **Delivery and verification:** The OptiFlow side is included in
  [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0).
- **Uncertainty:** Cross-repository implementation remains in progress. Human
  review must reconcile the narrower `OFD-006` direction with Flow's current
  contract; external-consumer conformance and policy-valid disposition remain
  separate evidence gaps.

### `OFD-007` — Use a specification-schema-test evidence loop

- **Origin:** The record first appears in
  [`c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f`](https://github.com/egohygiene/optiflow/blob/c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f/DECISIONS.md#L153-L164)
  through [PR #19](https://github.com/egohygiene/optiflow/pull/19), with no
  dedicated source issue located.
- **Implementation:** [PR #13](https://github.com/egohygiene/optiflow/pull/13)
  introduced the detailed development loop; PR #19 placed it in the canonical
  architecture corpus. Current schemas, fixtures, Rust tests, smoke tests, and
  adversarial tests demonstrate continued use.
- **Delivery and verification:** The decision source is included in
  [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0).
- **Uncertainty:** The practice is exercised, but no dedicated architecture
  dependency gate or policy-valid human disposition evidence was found.

### `OFD-008` — Compose one static site from isolated producers

- **Origin:** The record first appears in
  [`c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f`](https://github.com/egohygiene/optiflow/blob/c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f/DECISIONS.md#L165-L177)
  through [PR #19](https://github.com/egohygiene/optiflow/pull/19), with no
  dedicated source issue located.
- **Implementation:** [PR #18](https://github.com/egohygiene/optiflow/pull/18)
  created isolated site composition, [PR #20](https://github.com/egohygiene/optiflow/pull/20)
  added the architecture producer, [PR #34](https://github.com/egohygiene/optiflow/pull/34)
  added Repository Intelligence and Pages, and
  [PR #63](https://github.com/egohygiene/optiflow/pull/63) reconciled production
  delivery and rollback.
- **Delivery and verification:** The original decision source is included in
  [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0).
  The snapshot [Pages run](https://github.com/egohygiene/optiflow/actions/runs/35155272709)
  successfully composed and deployed the site.
- **Uncertainty:** This record overlaps `ADR-0001`; their relationship and both
  lifecycle dispositions require human resolution.

### `OFD-009` — Treat cloud-native systems as replaceable platform capabilities

- **Origin:** The record and the detailed placement analysis first appear in
  [`c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f`](https://github.com/egohygiene/optiflow/blob/c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f/DECISIONS.md#L178-L192)
  through [PR #19](https://github.com/egohygiene/optiflow/pull/19), with no
  dedicated source issue located.
- **Implementation:** The snapshot
  [cloud-native placement guide](https://github.com/egohygiene/optiflow/blob/d67250ca6dc71637672ad6ed5f49d46da5cbdde0/docs/cloud-native-placement.md)
  keeps platform capability ownership outside the product domain. No cloud or
  orchestrator dependency was added to the core.
- **Delivery and verification:** The boundary is included in
  [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0).
- **Uncertainty:** Architectural absence is observable; future platform
  conformance and policy-valid human disposition remain unresolved.

### `OFD-010` — Adopt the Aether architecture-document metadata contract manually

- **Origin:** The record and the 18-document metadata adoption first appear in
  [`c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f`](https://github.com/egohygiene/optiflow/blob/c4e2c255e7c1a5bc3905e0a4637fd06515f8af1f/DECISIONS.md#L193-L206)
  through [PR #19](https://github.com/egohygiene/optiflow/pull/19), with no
  dedicated source issue located.
- **Implementation:** The snapshot
  [`META.md`](https://github.com/egohygiene/optiflow/blob/d67250ca6dc71637672ad6ed5f49d46da5cbdde0/META.md)
  indexes the manually adopted corpus and the architecture generator validates
  its local graph.
- **Delivery and verification:** The manual adoption is included in
  [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0).
- **Uncertainty:** Managed Aether/Holon adoption was explicitly deferred. The
  cutover must compare released guidance before deciding whether to amend or
  supersede this record. No policy-valid human disposition evidence was found.

### `OFD-011` — Separate extension declaration, operator trust, and core acceptance

- **Origin:** The record first appears in
  [`fdbe629e82fcd53ec42c5fabf2323f35f45ddc6f`](https://github.com/egohygiene/optiflow/blob/fdbe629e82fcd53ec42c5fabf2323f35f45ddc6f/DECISIONS.md#L207-L234),
  implementing [issue #51](https://github.com/egohygiene/optiflow/issues/51)
  through [PR #62](https://github.com/egohygiene/optiflow/pull/62).
- **Implementation:** PR #62 added the versioned extension SDK, manifests,
  locks, direct process boundary, core revalidation, examples, and adversarial
  fixtures described by the record.
- **Delivery and verification:** PR #62 records 145 tests plus contract,
  adversarial, fuzz, documentation, package, release-evidence, and Identity
  checks. The change was merged after the `v0.1.0` tag; no later OptiFlow
  release was found at the snapshot.
- **Uncertainty:** The issue and PR establish implementation intent and
  delivery, not policy-valid human lifecycle approval.

### `ADR-0001` — Use Zensical and LaunchKit-derived static composition

- **Origin:** The existing file was created by
  [`5f3d57eb3d2daa6a9c9f80187238581c2940170d`](https://github.com/egohygiene/optiflow/blob/5f3d57eb3d2daa6a9c9f80187238581c2940170d/docs/decisions/ADR-0001-publication-stack.md),
  under [issue #25](https://github.com/egohygiene/optiflow/issues/25) and
  [PR #35](https://github.com/egohygiene/optiflow/pull/35).
- **Implementation:** The record cites
  [PR #17](https://github.com/egohygiene/optiflow/pull/17),
  [PR #18](https://github.com/egohygiene/optiflow/pull/18),
  [PR #20](https://github.com/egohygiene/optiflow/pull/20), and
  [PR #34](https://github.com/egohygiene/optiflow/pull/34). PR #63 later added
  production evidence and rollback documentation.
- **Delivery and verification:** The record is included in
  [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0), and
  the snapshot Pages run passed.
- **Uncertainty:** Issue #25 says its issue-creation scope was approved, but no
  durable comment or review explicitly approves this ADR's lifecycle
  disposition. The record also overlaps `OFD-008`.

## `OFD-008` and `ADR-0001` resolution gate

The records directly share isolated producer staging, collision-checked final
composition, and separation of composition from deployment authority.
`ADR-0001` additionally names the documentation stack, canonical architecture
sources, Relay consumption, and the explicit non-mutation boundary. The
evidence does not establish which of these human interpretations was intended:

1. **Related records:** `OFD-008` owns the general composition boundary while
   `ADR-0001` owns the selected documentation and presentation stack.
2. **Duplicate records:** both encode one decision, requiring selection of one
   canonical identity while preserving the other only as migration history or
   a discovery alias.
3. **Supersession:** `ADR-0001` replaces the earlier, shorter `OFD-008`, which
   would require an accepted replacement and bidirectional lineage.

The cutover must freeze both records until the maintainer explicitly selects
one interpretation and supplies any required disposition and exception
approval. Generation must fail closed rather than publish both as independent
truth if the relationship is still unresolved.

## Later durable-decision assessment

These assessments apply the accepted significance test but remain proposals.
They do not create records or reserve authority beyond the collision-safe ID
sequence.

| Change | Evidence | Assessment | Proposed treatment |
| --- | --- | --- | --- |
| Lossless NativePath v4 and path identity | [Issue #21](https://github.com/egohygiene/optiflow/issues/21), [PR #37](https://github.com/egohygiene/optiflow/pull/37), primary implementation [`2af71b4dbe5477b6d2e47372bdb4ce5a58f3ea0e`](https://github.com/egohygiene/optiflow/commit/2af71b4dbe5477b6d2e47372bdb4ce5a58f3ea0e), `Ord` follow-up [`30a5ef42732d7d9159af17ac58f544495529df00`](https://github.com/egohygiene/optiflow/commit/30a5ef42732d7d9159af17ac58f544495529df00) | **Separate durable-decision candidate.** It changed public schemas, SQLite representation, compatibility, and native identity across every boundary; it is more than implementation detail for `OFD-004`. | If human review agrees, use `ADR-002-lossless-native-path-identity.md`; start as proposed unless explicit approval evidence is supplied. |
| Handle-bound observation evidence | [Issue #22](https://github.com/egohygiene/optiflow/issues/22), [PR #38](https://github.com/egohygiene/optiflow/pull/38), commit [`45c9eff59b696d385ad88e9c78c0c29553de36f8`](https://github.com/egohygiene/optiflow/commit/45c9eff59b696d385ad88e9c78c0c29553de36f8) | **Separate durable-decision candidate.** It defines the evidence authority, race-refusal semantics, cache identity, and future mutation preconditions. | If human review agrees, use `ADR-003-handle-bound-observation-evidence.md`; start as proposed absent approval evidence. |
| Artifact-set atomic commit and recovery | [Issue #24](https://github.com/egohygiene/optiflow/issues/24), [PR #39](https://github.com/egohygiene/optiflow/pull/39), commit [`88f4744c892a780e21f72b68a3b2a5c6adc6b8d7`](https://github.com/egohygiene/optiflow/commit/88f4744c892a780e21f72b68a3b2a5c6adc6b8d7) | **Separate durable-decision candidate.** It establishes publication visibility, durability, recovery, compatibility, and refusal semantics while strengthening `OFD-002`. | If human review agrees, use `ADR-004-artifact-set-commit-and-recovery.md`; start as proposed absent approval evidence. |
| Signed release and security authority | [Issue #27](https://github.com/egohygiene/optiflow/issues/27), [PR #57](https://github.com/egohygiene/optiflow/pull/57), policy implementation [`423ef539fd0cae12ce3eda23ebc9987b8c82a0bb`](https://github.com/egohygiene/optiflow/commit/423ef539fd0cae12ce3eda23ebc9987b8c82a0bb), released source [`f04c82a0b0c677a2939ea351c4219602cd7181af`](https://github.com/egohygiene/optiflow/commit/f04c82a0b0c677a2939ea351c4219602cd7181af), [release run](https://github.com/egohygiene/optiflow/actions/runs/34698618575), [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0) | **Separate durable-decision candidate.** Dependency admission, signing identity, provenance, immutable release evidence, publication authority, and corrective-release policy meet the security and release significance tests. [PR #58](https://github.com/egohygiene/optiflow/pull/58) and [PR #59](https://github.com/egohygiene/optiflow/pull/59) supplied release-path follow-ups before publication. | If human review agrees, use `ADR-005-signed-release-trust-boundary.md`; start as proposed absent approval evidence. |
| Lossless PNG candidate/provider validation contract | [Issue #78](https://github.com/egohygiene/optiflow/issues/78), [PR #79](https://github.com/egohygiene/optiflow/pull/79), implementation [`9be28ed1a9742860d63246412419ae6be440a06a`](https://github.com/egohygiene/optiflow/commit/9be28ed1a9742860d63246412419ae6be440a06a), merge [`d67250ca6dc71637672ad6ed5f49d46da5cbdde0`](https://github.com/egohygiene/optiflow/commit/d67250ca6dc71637672ad6ed5f49d46da5cbdde0) | **Separate durable-decision candidate.** Unlike PR #67's read-only opportunity evidence, this change creates a closed public candidate contract and selects durable provider-versus-host evidence authority, PNG preservation semantics, deterministic request identity, resource limits, and validation/refusal behavior. It changes no runtime, source-mutation, artifact-publication, or extension authority. | Preserve PR #67's earlier `ADR not required` result. If human review agrees, use `ADR-006-lossless-png-candidate-contract.md`; start as proposed absent approval evidence, and do not claim provider execution or release. |
| Production site publication hardening | [Issue #28](https://github.com/egohygiene/optiflow/issues/28), [PR #63](https://github.com/egohygiene/optiflow/pull/63) | **Implementation evidence for `OFD-008` / `ADR-0001`.** It completed DNS, TLS, canonical routes, preview/production authority, verification, and rollback without choosing a new site architecture. | Add evidence to the human-selected canonical publication record; create no separate ADR unless review identifies a new durable choice. |
| Read-only PNG profile evidence | [Issue #66](https://github.com/egohygiene/optiflow/issues/66), [PR #67](https://github.com/egohygiene/optiflow/pull/67) | **ADR not required for the #66/#67 checkpoint, preserved.** PR #67 explicitly classifies the read-only opportunity-evidence implementation as following existing boundaries. The later #78/#79 candidate contract is assessed independently and does not retroactively change that result. | Preserve PR #67 as negative decision-impact evidence for its bounded checkpoint. Evaluate #78/#79 separately as the durable-decision candidate above. |

The proposed `ADR-002` through `ADR-006` sequence is provisional. If human
review declines any candidate, do not close the numeric gap or reuse its ID
after a canonical proposed record has been created.

## Shared dependency and readiness map

The inventory can merge without activating the shared system. The canonical
cutover, continuous capture, and publication have distinct downstream gates.

| Owner | Live evidence at inventory time | Readiness for later OptiFlow work |
| --- | --- | --- |
| Hygiene | [Issue #15](https://github.com/egohygiene/hygiene/issues/15) is closed. The v1.1.0 bytes at [`f598ed659a43dd759d4ede41c27f9e5daf991aa7`](https://github.com/egohygiene/hygiene/commit/f598ed659a43dd759d4ede41c27f9e5daf991aa7) have explicit [human approval](https://github.com/egohygiene/hygiene/issues/15#issuecomment-5647398908) and were activated at [`bae230ba92fd231e8f26e24f85667c7117e4021b`](https://github.com/egohygiene/hygiene/commit/bae230ba92fd231e8f26e24f85667c7117e4021b). The active schema cannot represent `OFD-*` record IDs or lineage references to `OFD-*`. | **Policy authority ready; legacy contract incomplete.** Retain the approval/activation chain, then obtain an organization-owned legacy-ID/lineage mechanism or compatible schema revision before cutover. |
| Holon | [Issue #6](https://github.com/egohygiene/holon/issues/6) closed through [PR #57](https://github.com/egohygiene/holon/pull/57) at [`de5047ee5d1515ece0c2c38993f3cefdf51faa69`](https://github.com/egohygiene/holon/commit/de5047ee5d1515ece0c2c38993f3cefdf51faa69). Blueprint 1.0.0 pins Hygiene v1.1.0 and tests legacy `OFD-*` indexing; no Holon tag or GitHub Release was found. | **Scaffold and indexing ready for planning, not full conformance.** Select an exact reviewed revision or later release and pair it with the upstream legacy-ID/lineage contract plus validator support. |
| Aether | [Issue #49](https://github.com/egohygiene/aether/issues/49) closed through [PR #50](https://github.com/egohygiene/aether/pull/50), but the [current projection at `9e2ba7d8`](https://github.com/egohygiene/aether/blob/9e2ba7d8fb118c0976356225dcac54209fe44eee/library/organization/projections/templates/decision-impact.AGENTS.md) remains draft, pins proposed Hygiene 1.0.0 at an older commit, and has no release. | **Not ready to install.** Repin to the accepted policy, obtain human promotion, and consume a released managed module rather than copying prose. |
| EgoLint | [Issue #24](https://github.com/egohygiene/egolint/issues/24) closed through [PR #26](https://github.com/egohygiene/egolint/pull/26), but the [current catalog](https://github.com/egohygiene/egolint/blob/fc6f0c0496c8b8d4c0690fdd2fdb69a9538866fa/.config/rules/repository-intelligence.v1.toml) still marks authority proposed and the [validator](https://github.com/egohygiene/egolint/blob/fc6f0c0496c8b8d4c0690fdd2fdb69a9538866fa/src/rules/repository_intelligence.rs) enumerates only `ADR-*`. | **Not ready for blocking OptiFlow migration.** Add accepted-authority support and the approved legacy-prefix path for `OFD-*`; prove fixtures before adoption. |
| Relay | [Issue #30](https://github.com/egohygiene/relay/issues/30) delivered the renderer in [PR #36](https://github.com/egohygiene/relay/pull/36); it first shipped in [`v1.3.0`](https://github.com/egohygiene/relay/releases/tag/v1.3.0) from [`55587de4ff322931d401e964f5af0716633dd675`](https://github.com/egohygiene/relay/commit/55587de4ff322931d401e964f5af0716633dd675) and remains present in [`v1.5.0`](https://github.com/egohygiene/relay/releases/tag/v1.5.0). [Issue #5](https://github.com/egohygiene/relay/issues/5) and [issue #33](https://github.com/egohygiene/relay/issues/33) remain open; [issue #27](https://github.com/egohygiene/relay/issues/27) remains the broader experience tracker. [PR #91](https://github.com/egohygiene/relay/pull/91) is green and mergeable but unmerged at [`86490eb70ce87bcb003abc0248a4419108f53d13`](https://github.com/egohygiene/relay/commit/86490eb70ce87bcb003abc0248a4419108f53d13); it forwards supplied snapshot and comparison inputs but does not itself collect ADRs or finish representative consumer-canary and actual deployment-provenance work. | **Renderer ready; validation, collection/orchestration, and publication canary incomplete.** Relay issue #33 owns the shared collection and publication outcome, but no merged and released implementation completes it; OptiFlow must not invent a private collector. |
| Observatory | [Issue #7](https://github.com/egohygiene/observatory/issues/7) closed through [PR #8](https://github.com/egohygiene/observatory/pull/8). Its [current contract at `a4526ba6`](https://github.com/egohygiene/observatory/blob/a4526ba65a4ec1110ad9f2f47dc866a0c6d2c5a3/docs/repository-intelligence-read-model.md) normalizes a supplied, pinned Hygiene Repository Intelligence projection. | **Read-model step ready only with valid input.** Observatory does not collect ADR Markdown; the upstream validated projection is still required. |
| Pace | [Issue #5](https://github.com/egohygiene/pace/issues/5) remains open at current main [`e126190129c465b5a168947e6b70a1bc70f38097`](https://github.com/egohygiene/pace/commit/e126190129c465b5a168947e6b70a1bc70f38097); its coordination work created repository-local backfill queues including OptiFlow #60. | **Fleet rollout not ready.** Use Pace only after the scaffold, validator, guidance, and shared projection contracts are accepted and released. |

The missing shared implementation is a collector/orchestrator that turns
validated local ADR Markdown into the Hygiene projection consumed by
Observatory. Hygiene assigns semantic validation to EgoLint and reusable CI,
evidence, and static-data generation mechanics to Relay. Relay issue #33
explicitly owns collection of canonical Markdown, ADR, and Git evidence plus
EgoLint and Observatory-compatible inputs, but no merged and released
implementation yet completes the ADR-to-projection path. Relay's renderer
consumes an already supplied Observatory snapshot; it is not the collector.
If the remaining work needs a narrower checkpoint, create or link a bounded
child under issue #33. Complete that shared implementation before continuous
capture or publication, not as a prerequisite for the repository-local file
cutover. OptiFlow must not add a repository-specific replacement.

## Publication route finding

The snapshot Pages workflow pins Relay at
[`b71b090406a3a9e4cd9f107e9d14a623bbecb127`](https://github.com/egohygiene/relay/commit/b71b090406a3a9e4cd9f107e9d14a623bbecb127),
generates into `.site/producers/intelligence`, and supplies no
`observatory-snapshot`. OptiFlow then mounts the complete Relay output under
`/intelligence/`.

Consequently, the current decision route is
[`/intelligence/decisions/`](https://optiflow.egohygiene.io/intelligence/decisions/),
where the renderer honestly reports that Observatory decision history is
unavailable. The parent issue's `/decisions/` wording is shorthand, not the
implemented route; that root path returned 404 during this inventory. Route
selection, redirection, snapshot wiring, and canary verification belong to a
later publication checkpoint and are unchanged here.

## Proposed atomic cutover

Perform the following in one separately reviewed cutover only after the shared
prerequisites and human gates below are satisfied:

1. Recheck `main`, open pull requests, all dependency issues, supported
   releases, and public routes. Freeze the exact starting SHA in the migration
   PR.
2. After the selected Hygiene contract can represent the legacy IDs and
   lineage, use the exact reviewed Holon scaffold selected for the cutover to
   add the canonical `docs/decisions/README.md`, the policy reference pinned to
   an approved full Hygiene commit, and the approved alternate-prefix and
   historical-width exceptions. Do not copy or locally override the Hygiene
   policy or schema.
3. Preserve the original combined log byte-for-byte in a clearly noncanonical
   `docs/decisions/archive/DECISIONS-legacy.md` using `git mv`. Put
   source-commit and noncanonical migration notes in an adjacent archive
   `README.md`; do not modify the moved file's bytes.
4. Extract `OFD-001` through `OFD-011` into the proposed stable filenames.
   Preserve their IDs and decision substance, add the required front matter
   and missing policy sections transparently, and link the original source
   lines. When the canonical front matter is `proposed`, represent the old
   `Status: Accepted` text only as an explicitly labeled legacy claim, never as
   a second current status; the byte-preserved archive retains the exact source.
5. Upgrade the existing `ADR-0001` in place, retaining its four-digit ID and
   filename. Apply the maintainer's explicit resolution for its relationship
   with `OFD-008`.
6. Add only the later durable-decision candidates approved for creation.
   Records without policy-valid disposition evidence begin as `proposed` even
   when implementation already exists; record implementation and verification
   independently.
7. Generate the deterministic canonical index and compare its ID set against
   this map. Duplicate numeric identities, unresolved exceptions, missing
   records, non-proposed status-evidence gaps, or lineage errors fail the
   cutover.
8. Replace root `DECISIONS.md` with a concise compatibility pointer to the
   canonical index and noncanonical archive in the same commit. There must be
   no interval with two complete canonical logs.
9. Run the selected pinned shared validator locally against the represented
   Git tree, initially advisory only if rollout sequencing still requires it.
   Save a durable machine-readable report; do not install a local substitute.
10. Review the complete diff, generated index, archive bytes, policy pin,
    exception approvals, privacy result, and validation evidence before merge.

This is “atomic” at the repository review boundary: the archive, individual
records, index, policy pin, exceptions, and compatibility pointer land or
revert together. It does not claim a filesystem transaction or bypass normal
Git review.

## Validation, rollback, and preservation

### Validation for this inventory

- `git diff --check`
- frozen strict Zensical build
- complete local site composition and verifier
- manual comparison of all 12 existing IDs against their first Git commits
- manual review of issue, PR, release, workflow, and dependency links

Strict documentation validation checks local paths and anchors. External links
must also be reviewed because the local builder does not prove remote HTTP
availability.

### Validation for the later cutover

In addition to the inventory checks, require the selected pinned Holon index
check, EgoLint semantic validation, Relay orchestration report, exact
policy/schema pins, exception approval evidence, stable regeneration under a
clean checkout, and an ID-set comparison proving that no history disappeared.

### Rollback

The migration must remain one reviewable commit or a clearly identified
contiguous commit set. Rollback is a reviewed revert of that complete set,
followed by the same local and CI validation. Do not hand-edit generated
indexes, deployed HTML, or individual migrated records to simulate rollback.
If publication has already consumed the migrated projection, revert source
first and then regenerate and redeploy through the shared pipeline.

### Blame and provenance

- Move the original combined file with `git mv` so `git log --follow` retains
  discoverable blame history.
- Keep the archived bytes unchanged and mark the archive noncanonical in an
  adjacent `README.md`, never inside the preserved file.
- Link every extracted record to its immutable source lines and first commit.
- Retain the existing `ADR-0001` path rather than renaming it cosmetically.
- Never renumber an `OFD-*` record or reuse the numeric `001` slot.

## Privacy, omissions, and uncertainty

Only public repository content and public GitHub metadata were used. This map
links evidence rather than copying issue bodies, review text, workflow logs,
actor identities, email addresses, local paths, raw API responses, or release
artifacts. No private issue, discussion, or local-session content was searched
or summarized.

If private approval or rationale exists, it remains omitted until a human
chooses a durable evidence URL and a visibility-safe summary. An inaccessible
or intentionally private source must be recorded as unavailable; its contents
must not be inferred or exposed in a public ADR or projection.

Known omissions and limits:

- public Git history proves when text entered the repository, not why a human
  chose its lifecycle label;
- release inclusion proves delivery of source and artifacts, not acceptance of
  every embedded decision;
- successful current CI does not prove historical tests ran at the original
  decision point;
- the external Flow consumer was not exhaustively validated by this
  repository-local inventory;
- issues #21 through #27 cite non-repository “OptiFlow audit PDFs” as their
  source; no corresponding PDFs were found in public reachable Git history, so
  this map does not reconstruct their rationale from issue summaries; and
- shared dependency state and public routes can change after the snapshot.

## Human gates

### Before canonical cutover

1. Approve or correct the lifecycle disposition of all 12 legacy records, with
   durable evidence for every non-proposed status.
2. Approve the `OFD-*` alternate-prefix exception and the four-digit
   `ADR-0001` width exception.
3. Resolve `OFD-008` and `ADR-0001` as related, duplicate, or supersession.
4. Confirm which of the proposed `ADR-002` through `ADR-006` candidates should
   become canonical records and whether any have valid historical disposition
   evidence.
5. Select the supported Hygiene policy/schema pin and its organization-owned
   legacy-ID/lineage mechanism, plus exact reviewed Holon scaffold and
   EgoLint/Relay validation revisions that can validate the migrated corpus.

### Before continuous capture or publication

1. Select an accepted, released Aether decision-impact module pinned to the
   supported Hygiene revision.
2. Confirm Relay's shared collector/orchestration implementation, the exact
   Observatory read-model contract, and the completed publication canary.
3. Confirm Pace rollout authorization and the final public route or redirect
   behavior.

Until the canonical-cutover gates are recorded, this map is complete as an
inventory but the migration remains deliberately blocked. The second set of
gates independently blocks continuous capture and public ledger activation.
