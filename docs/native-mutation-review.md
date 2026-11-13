# Native mutation release-review packet

Relates to #93, after #111 and before #95. This source authors a review adapter;
no evidence was collected, validated by execution, signed, released or published.

The current signing/release pipeline remains pinned to its historical read-only
sources. Neither a JSON record nor this review adapter removes that gate. A
v0.2 release still needs the real native/backend/disposable-volume/install/
upgrade/signing acceptance and a separately reviewed release-policy change.

`owner_reports.py mutation-review` reuses the existing pinned-file, CPU/archive,
duplicate-JSON, bounded read, no-clobber private output and second-readback
boundaries. It accepts `optiflow.mutation-release-review-request.v1` with exact
`release_version` (`v0.2.0`), full `source_revision`, explicit
`maximum_input_bytes` (up to 1 GiB) and one to three unique target rows. Cargo must
actually declare `0.2.0`; current `0.1.1` source cannot claim that candidate.
Each row contains `target`, archive FilePin and qualification FilePin. The three
targets remain Linux x86-64, Intel macOS and Apple Silicon macOS; missing targets
are blocked findings, not supported coverage. No package version is bumped here.

The new evidence record names its exact native target/source/archive/executable,
`exact-dedup-transaction-v0.2` profile, generated-disposable-only fixture policy,
native volume observations, cases, claims and space measurements. Read-only,
synthetic or foreign-archive records refuse. Native executable CPU identity and
the exact two-file LICENSE/optiflow archive surface must match actual supplied
bytes. The whole operation reads at most 256 selected files under its byte cap.

Required cases retain actual versioned command-result captures for install,
upgrade/rollback, scan/plan/dry-run, quarantine/status/resume/restore, separately
authorized finalization and cleanup, interruption, physical reconnect, collisions,
capacity and stale-source refusal. Cases may be explicitly unavailable with
reasons. A successful read-only scan cannot stand for quarantine; a dry-run cannot
stand for committed mutation; success cannot stand for a refusal. Selected
capture identities and asserted outcomes are reviewed, not inferred from a
receipt filename or a generic `passed` flag.

Claims keep observed true, false and unknown distinct. Actual physical-removable
and disposable APFS disk-image observations are different. A detached disk image
cannot satisfy physical-disconnect coverage. APFS, portability-relevant external
format, install targets and missing physical evidence remain visible. Logical,
allocated/unknown and positive or negative observed free-space delta are separate; zero delta
does not become reclaimed space.

The portable output contains capture digests and findings, no publisher-machine
paths or raw source/command bytes. Every target keeps native qualification and
physical-reclamation verification false. Output is `blocked` while coverage/
claims remain missing, otherwise `independent_review_required`; it is always
unsigned and grants no runtime activation or publication authority. Supplied
hardware claims still need independent witness review/attestation. This format
cannot substitute for the qualified-owner report consumed by Flow.

Authored future invocation, not run:

```text
task release:report:mutation-review REQUEST=/private/review.json OUTPUT_DIRECTORY=/private/fresh-review
```

Three negative cases are authored, uncompiled/unrun. No tests/compiler/build,
dependencies/lint/format/schema/docs/drift, native/product/CLI/SDK/provider/media
or CI execution/polling occurred. No signed v0.2 assets, install/upgrade result,
native gate, finalization or physical-space claim was created. #93 remains open.
