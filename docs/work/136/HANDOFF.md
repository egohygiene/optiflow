# Filesystem corpus dependency binding checkpoint

Issue: [#136](https://github.com/egohygiene/optiflow/issues/136), under #65.
Observed October 28, 2026 (America/New_York).

## Source and chosen lockfile

Base main: `1fc964fd931f0a6bb4e47a45cbf085a9d57b1c80`.
Branch: `codex/136-lock-binding`.

This checkpoint keeps the base's `Cargo.lock` unchanged. Direct SHA-256 of
those bytes is
`1c77b5517da29ef329ee2199dd4c11f644f30b9faac3630c1338c5d807a9ea3e`.
The fixture index previously recorded
`4e1f382de4f48c85705713b61d25890de6865fa517153d6afcd0b3cc1ac42306`.
Only `cargo_lock_sha256` changes in that index. The existing corpus generator,
catalog, recipes, expected outcomes, payloads and proof sources are untouched.
The issue's historical audit identified this mismatch; that audit is not a
fresh passing check of this checkpoint.

All nine open dependency PR file lists were reviewed before selecting this
baseline. #143 changes Cargo.toml/Cargo.lock (jsonschema and related crates);
#131 changes the locked toml family; #130 changes signal-hook; #127 changes
serde_json. Their lockfile patches were read. #145, #144, #142, #139 and #129
change GitHub Actions workflow pins only. None is merged or adopted here;
this is scope review, not compatibility/security qualification of those
upgrades. Any subsequently accepted lockfile change needs its own reviewed
binding update against the final combined lockfile. The draft native work in
#122 is independent and remains untouched.

## Narrow authoring path

`scripts/update-filesystem-corpus-lock.py` uses the existing generator's index
calculation without entering its runner. It proposes a dependency-only change
by default; `--write` explicitly replaces the index after refusing non-lock
differences and malformed input. Existing read-only CI gates, receipt identity,
proof selection and execution/resource/cleanup behavior are unchanged.
`docs/filesystem-corpus.md` documents the workflow. The companion is not an
automatic dependency resolver or a replacement for corpus qualification.

Focused unit cases are authored in `tests/test_filesystem_corpus_lock.py`;
they are unrun. They cover the lock-only boundary and refusal/no-write paths
without requiring Cargo or native media providers.

## Evidence and remaining qualification

Performed: live issue/PR reads, source and patch review, direct lockfile hash
inspection, Git metadata/diff inspection and repository authoring. The index
field was edited directly; neither authoring helper nor generator was run.
Independent source review found no blocker and recommended relying on the
existing generator rather than duplicating its case schema; that
simplification is included.

Not performed: tests, compilation/builds, dependency resolution/install,
format/lint, schema/drift/index checks, corpus tiers, native execution or CI.
No Actions were dispatched or polled. There is no new pass result or native
filesystem qualification. #136 stays open for its execution acceptance work.

At the deferred testing checkpoint, record source, OS/architecture, toolchain,
commands and results for the focused helper tests, unchanged index checks and
bounded PR/scheduled corpus tiers on supported hosts. A passing index check
alone does not execute proofs or qualify real-drive behavior. #137's merged
test-root correction still needs its separate supported-host results; #111
and #93 retain native transaction and release qualification.

## Video-folder coordination

New [#146](https://github.com/egohygiene/optiflow/issues/146) owns a bounded
source-preserving video candidate trial, recursive candidates and later
qualified replacement/disposal. This leaves PNG #95/#121 intact and gives the
user's video space-reclamation goal an explicit path. It adds no video codec,
provider execution, real-media operation, deletion authority or release to
this checkpoint.
