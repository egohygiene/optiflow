# #111 — macOS/APFS mutation port

## Current checkpoint

- Parent: [#111](https://github.com/egohygiene/optiflow/issues/111), remains open.
- Role: implementation; first checkpoint only, unvalidated draft.
- Outcome: same-volume macOS/APFS quarantine, resume, collision-safe restore,
  and verified empty-namespace cleanup using the existing execution contracts.
- Base: `c5dbcd1761e810aea228389b8ba017acc97a411e`, verified 2026-10-05.
- Base tree: `30c4b1f26f20d66e3ce43f284eee4c25d8f7abac`.
- Candidate branch: `feat/optiflow-111-macos-same-volume`.
- Candidate SHA/PR: resolve the live branch and linked parent issue; this file
  was authored before publication and cannot contain its own eventual SHA.
- Strategy: Aether worker-strategy v0.1.0 at
  [`db5f4bd339f9266758c74c3435d47c8005f89a8e`](https://github.com/egohygiene/aether/blob/db5f4bd339f9266758c74c3435d47c8005f89a8e/library/organization/specs/methodology/worker-strategy.spec.md),
  with its specfile, repository-continuity and local-validation-evidence
  dependencies at the same ref.
- Check budget: executable tests, builds, formatting/lint/schema checks and
  local/hosted CI explicitly deferred by the owner. Source review only.
- Stop: push a draft PR, preserve remaining work, report back. No merge,
  release, real-media processing or physical-drive operations authorized here.

## Observed starting state

PR #113 merged the #112 fixture tranche and the #94 implementation from stacked
PR #114. Both issues are closed. Dependabot PRs #116–#120 subsequently merged;
no Optiflow PR was open at this checkpoint's startup observation. Re-query
mutable refs/issues/PRs before resuming. The signed v0.1.1 release stays read-only.

There was no repository AGENTS.md or CONTINUITY.md at the inspected base.
AI_CONSTITUTION.md, docs/development-model.md, the execution protocols, live
issue acceptance and current user instructions govern this work. This handoff
does not grant merge or source-media mutation authority.

## Implemented source

- `src/execution/filesystem.rs`: mutation-only APFS/topology and actual-handle
  checks; same-volume plans only on macOS. Darwin fsync plus F_FULLFSYNC without
  a silent fallback; unknown/unsupported guarantees refuse.
- `src/execution/mutation.rs`: shared same-volume transaction path, pre-move
  checks, unchanged no-replace rename semantics, and bounded Mac property
  observations before/after a move. Cross-volume copying remains Linux-only.
- `src/execution/recovery.rs`: Mac resume, restore and empty cleanup using
  append-only v3 history; no ambiguous-state promotion. Linux property hashes
  remain unchanged. Mac hashes additionally bind a platform domain, BSD flags
  and birth time. Finalization stays Linux-only.
- `src/execution/journal.rs`: mutation-specific opener, Mac DELETE/EXTRA SQLite
  policy, read-back of full-sync settings, and explicit held-file/directory
  device flushes after journal transitions. Dry-run opening keeps its existing
  policy. Existing migrations and v1–v4 wire schemas are unchanged.
- `tests/execution.rs`: nine existing same-volume/refusal cases enabled for
  Mac, stronger identity/property/xattr/restore/collision/cleanup assertions,
  plus a Mac cross-volume-intent refusal case. Internal recovery coverage also
  enables same-volume resume/idempotence/ambiguity cases on Mac.
- README, roadmap and execution protocols distinguish draft implementation
  from released behavior and qualification.

Checkpoint acceptance at source level: supported-host dispatch exists;
cross-volume intent refuses before writable journal access; source paths are
never overwrite targets; recovery retains ambiguity and rechecks authority;
Linux evidence encoding remains intact; deferred checks are explicit. None of
these source observations is executable proof of acceptance.

## Boundaries and known limitations

- State, quarantine and selected source/keeper directories must be observed
  APFS on Mac. Every source/quarantine pair must share the same actual device.
- The filesystem/drive must accept the requested full-sync operation. Native
  directory F_FULLFSYNC behavior is unverified; failure is a refusal, not a
  fallback to weaker durability. Compilation and API use remain unvalidated.
- Mac extended attributes are bounded to 128 entries, 64 KiB per value and
  1 MiB total. Larger/unreadable attributes refuse. Reads may change atime;
  rename may change ctime. Neither is claimed invariant.
- Same-inode rename retains Darwin ACLs, but ACL contents are not independently
  enumerated/fingerprinted. Hostile concurrent filesystem changes are outside
  this bounded protocol's proof.
- A database flush can fail after a committed/restored event becomes visible.
  Status reports recorded state, not independent last-flush evidence. Errors
  stop the caller but are not rollback guarantees; inspect actual paths before
  recovery. No next source action follows a failed pending-transition flush.
- No cross-volume Mac copying, restore-copy, retained-copy finalization, native
  qualification, source optimization/replacement, or original deletion here.
- Logical selected bytes, retained data, and unknown physical reclamation remain
  distinct; APFS clones/snapshots do not justify physical-savings claims.
- #94's historical synthetic Linux receipt does not prove real OxiPNG, native
  Mac, MSRV or this checkpoint. Dependency merges were also not rerun here.

## Verification and deferred work

Actually performed: read current GitHub refs/issues/PRs, clone the verified base,
inspect source diffs, review safe rustix platform APIs and the SQLite durability
boundary, and reconcile the documentation. No test, compiler, build, formatting,
lint, schema, smoke or CI command was executed. Local readiness: **unknown**.
Native Mac evidence: **unavailable in this Linux workspace**. Remote CI:
**deferred**. Publication uses `[skip ci]`; no workflow dispatch, polling,
protection change or release is part of this checkpoint.

The following commands are a future execution plan, **not passed evidence**:

| Check | Command or procedure | Required result |
| --- | --- | --- |
| Same-volume behavior, Linux and native Mac | `cargo test --locked --test execution` | Quarantine/restore preserve identity and supported properties; collisions/refusals leave sources intact; no fabricated savings. |
| Recovery boundaries | `cargo test --locked --lib execution::recovery::tests` | Resume is idempotent; ambiguous transitions refuse; limits remain enforced. |
| Platform compilation and MSRV | `cargo +1.85.0 check --locked --all-targets --all-features` on supported targets | No platform-only API/type errors; dependency floor verified. |
| Linux regression suite | `cargo test --locked --all-targets` | Existing v1–v4 and cross-volume behavior remains valid. |
| Formatting/lints | `cargo fmt --all -- --check`; `cargo clippy --locked --all-targets --all-features -- -D warnings` | No formatting/lint findings. |
| Documentation/contracts | Repository CI schema/doc entrypoints | Docs, examples and emitted schema contracts remain consistent. |
| Native durability failure campaign | Inject refused/failed directory, database and file full-sync; interrupt between pending, rename and completion records | No next source move after failure; visible state is treated as recorded evidence; recovery never invents a commit. |
| APFS physical qualification | `docs/external-drive-mutation-pilot.md`, disposable media only | Actual disconnect/reconnect, property, identity and capacity observations; no qualification inferred from synthetic runs. |

The synthetic cross-volume-intent test constructs a valid approved declaration
and tests upfront refusal. It is not a real two-volume APFS pilot. Native tests
must fail on missing expected guarantees rather than silently skip them.

## Finite remaining sequence and resume

1. Review this draft and perform its focused Linux/native-Mac compilation,
   same-volume and failure checks before relying on this boundary. Resolve
   findings in this checkpoint; do not stack a consumer on an uncertain API.
2. Implement bounded Mac cross-volume copy/restore with explicit ownership,
   ACL/xattr/flags/birth-time preservation policy or refusals. Add the relevant
   synthetic and interruption coverage.
3. Implement Mac retained-copy finalization parity with the existing separate
   irreversible authorization, then finish the native/disposable-volume and
   filesystem matrix required by #111. Loop back through all deferred checks
   at this parent boundary; keep #111 open until its acceptance is proved.
4. #93 owns three-target installation/upgrade/signing, complete release
   qualification and v0.2 publication. #95 then joins the existing #94 producer
   to validated replacement. #121 remains the later low-space original-disposal
   workflow; none of those capabilities is delivered by this PR.

On resume, read this file and the changed execution modules, verify the live
base/PR head and working tree, and reconcile any concurrent work. The next
concrete job is the focused validation of this draft, or another explicitly
owner-selected bounded implementation checkpoint with these obligations kept
open. Do not close #111 from an implementation-only PR.
