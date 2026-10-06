# #111 — macOS/APFS mutation port

## Current checkpoint

- Parent: [#111](https://github.com/egohygiene/optiflow/issues/111), remains open.
- Role: implementation proposal; fourth checkpoint, unvalidated draft.
- Outcome: deterministic failure coverage and native preservation cases for
  the safe descriptor ACL dependency extension proposal, in addition to the
  earlier same-volume implementation and recovery hardening.
  The adapter is not adopted or linked; Mac cross-volume copying and
  restoration remain unsupported.
- Base: `c5dbcd1761e810aea228389b8ba017acc97a411e`, verified 2026-10-05.
- Base tree: `30c4b1f26f20d66e3ce43f284eee4c25d8f7abac`.
- Previous remote checkpoint: `37a655efae67e60f6f0497b1565e26fff1bd1e5c`,
  tree `757d2bcb8b06c49dc62195790be4860e73ae2d2b`, reconciled 2026-10-06 UTC.
- Candidate branch: `feat/optiflow-111-macos-same-volume`.
- Candidate PR: [#122](https://github.com/egohygiene/optiflow/pull/122), draft.
- Candidate SHA: resolve the live branch and linked parent issue; this file
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
no Optiflow PR was open at the first checkpoint's startup observation. Re-query
mutable refs/issues/PRs before resuming. The signed v0.1.1 release stays read-only.

There was no repository AGENTS.md or CONTINUITY.md at the inspected base.
AI_CONSTITUTION.md, docs/development-model.md, the execution protocols, live
issue acceptance and current user instructions govern this work. This handoff
does not grant merge or source-media mutation authority.

## Implemented source

### Fourth checkpoint: adapter failure boundaries

The dependency proposal now separates its two native calls behind a private
transport boundary. The public borrowed-descriptor API and its bounds remain
unchanged. Scripted transport cases exercise the same read/replace control
flow, including refusal before a setter, native errors, capability loss,
malformed replies and independent read-back mismatch after a write. They
assert bounded requests and that failure triggers neither retry nor rollback.
They do not simulate native filesystem semantics or prove syscall safety.

Eleven new scripted cases and three additional native cases bring the proposal
to 26 authored tests (18 internal, eight native). The new native cases cover
ownership/mode/mtime/content preservation and
replacement of an ACL inherited from a destination directory. Their fixtures
must establish the required inherited state; unavailable behavior is not a
passing result. All tests remain authored and unrun. The updated
[adapter packet](acl-adapter/README.md) identifies the test inventory and
remaining native qualification.

This slice advances the dependency proposal while executable checks remain
deferred. Same-volume finalization is not an independent feature to enable:
same-volume restore returns the quarantine inode to its original path and
leaves no retained copy. Existing finalization applies only after cross-volume
restore, so Mac finalization still depends on the cross-volume work.

No application code, dependency, lockfile, public API, property fingerprint,
schema or migration changed in this checkpoint. No tests or patch-application
checks were executed, and no upstream submission or dependency adoption occurred.

### Third checkpoint: dependency adapter proposal

The [adapter review packet](acl-adapter/README.md) contains a concrete patch
against exacl 0.13.0, pinned to
`fa36df1745a41e50ab0730546c04d86a8503e46b`. Its separate macOS descriptor API
preserves ordered entries, raw UUIDs, rights, entry flags and ACL-level flags,
with bounded buffers and independent destination read-back. It avoids the
existing convenience representation's account-name resolution and omitted
ACL-level state. Native ABI and filesystem behavior remain unqualified.

The third checkpoint added seven authored unit cases and five native cases
inside the proposed dependency; the fourth extends this inventory. The patch
is an inert review artifact under `docs/work/111/acl-adapter/`;
Optiflow's build does not discover or execute it. No dependency, lockfile,
unsafe-code policy, runtime dispatch or existing evidence format changed.
No upstream submission or fork was created. The upstream descriptor API
request [exacl #298](https://github.com/byllyfish/exacl/issues/298) remains
external context, not proof of acceptance.

Adoption requires reviewing the native boundary, executing the deferred checks
and choosing an accepted upstream release or explicitly reviewed pinned fork.
The packet is not permission to install a local unsafe shim in Optiflow.

### Second checkpoint: prerequisite and recovery hardening

Source investigation found that the locked rustix descriptor-copy API requires
unsafe state management, while the reviewed exacl API is path-based. No usable
safe descriptor ACL observer/writer was identified. `unsafe_code = "forbid"`
remains intact. A new inode cannot inherit an unsupported claim that Darwin
ACLs were copied just because ordinary xattrs match. Consequently the planned
cross-volume implementation is blocked on the
[reviewable metadata adapter contract](MACOS_CROSS_VOLUME.md).

- The complete Mac topology policy is isolated before mutation-capability
  probes and writable journal access and names
  the ACL backend prerequisite. Authored pure policy tests cover same-volume,
  cross-volume and mixed action ordering.
- Authored integration coverage retains the empty-state refusal and adds a
  mixed plan with a same-volume prefix and existing unfinished journal evidence.
  Refusal must not classify that evidence, create a lock/namespace, or touch
  selected source and retained quarantine bytes.
- Recovery hardening records identity for new apply commit events, checks
  recorded quarantine identity when available, and rechecks a reverse-copy
  temporary pathname against its held descriptor plus both property
  fingerprints before source publication. Authored Linux regressions inject
  equivalent-inode replacement and late temporary/quarantine xattr changes.
  Historical v3 events lacking identity keep their existing content/property
  checks; this does not manufacture missing identity evidence.
- No new dependency, unsafe exemption, ACL fingerprint, schema or migration is
  introduced. This is preparation and hardening, not functional Mac transfer.

### First checkpoint: retained implementation

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

First-checkpoint acceptance at source level: supported-host dispatch exists;
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
boundary, inspect the cross-volume metadata APIs and recovery identity boundaries,
author and independently source-review the pinned dependency proposal and its
failure-injection/native-preservation cases, and
reconcile the documentation. No test, compiler, build, formatting,
lint, schema, smoke or CI command was executed. Local readiness: **unknown**.
Native Mac evidence: **unavailable in this Linux workspace**. Remote CI:
**deferred**. Publication uses `[skip ci]`; no workflow dispatch, polling,
protection change or release is part of this checkpoint.

The following commands are a future execution plan, **not passed evidence**:

| Check | Command or procedure | Required result |
| --- | --- | --- |
| Same-volume behavior, Linux and native Mac | `cargo test --locked --test execution` | Quarantine/restore preserve identity and supported properties; collisions/refusals leave sources intact; no fabricated savings. |
| Recovery boundaries | `cargo test --locked --lib execution::recovery::tests` | Resume is idempotent; ambiguous transitions refuse; limits remain enforced. |
| Topology prerequisite | `cargo test --locked --lib execution::filesystem::tests` | Pure Mac topology policy refuses cross-volume and mixed plans without I/O; public authority checks may read path metadata before reaching this policy. This does not qualify APFS. |
| Platform compilation and MSRV | `cargo +1.85.0 check --locked --all-targets --all-features` on supported targets | No platform-only API/type errors; dependency floor verified. |
| Linux regression suite | `cargo test --locked --all-targets` | Existing v1–v4 and cross-volume behavior remains valid. |
| Formatting/lints | `cargo fmt --all -- --check`; `cargo clippy --locked --all-targets --all-features -- -D warnings` | No formatting/lint findings. |
| Documentation/contracts | Repository CI schema/doc entrypoints | Docs, examples and emitted schema contracts remain consistent. |
| Proposed ACL dependency | [Adapter review packet](acl-adapter/README.md) in its pinned upstream checkout | Patch application, Rust 1.85 compilation, parser/native tests and upstream regressions; this proposal is not covered by Optiflow's normal test command. |
| Native durability failure campaign | Inject refused/failed directory, database and file full-sync; interrupt between pending, rename and completion records | No next source move after failure; visible state is treated as recorded evidence; recovery never invents a commit. |
| APFS physical qualification | `docs/external-drive-mutation-pilot.md`, disposable media only | Actual disconnect/reconnect, property, identity and capacity observations; no qualification inferred from synthetic runs. |

The synthetic cross-volume-intent tests construct valid approved declarations
and test upfront refusal, including existing-state preservation. They are not
a real two-volume APFS pilot. Native tests
must fail on missing expected guarantees rather than silently skip them.
The existing Linux cross-volume fixture uses `/dev/shm` when it is a different
device and otherwise skips; its new recovery cases retain that limitation.
Qualification must record that coverage as unavailable when the fixture skips.

## Finite remaining sequence and resume

1. Review this draft and perform its focused Linux/native-Mac compilation,
   same-volume and failure checks before relying on this boundary. Resolve
   findings in this checkpoint; do not stack a consumer on an uncertain API.
2. Review and qualify the [concrete dependency patch](acl-adapter/README.md),
   select its accepted upstream release or explicitly reviewed pinned fork,
   and adopt the safe descriptor ACL backend specified in
   [MACOS_CROSS_VOLUME.md](MACOS_CROSS_VOLUME.md). Then implement bounded Mac
   cross-volume copy/restore. Preserve ownership, ACLs, xattrs, flags and birth
   time with explicit refusals and versioned property evidence. Add the
   relevant synthetic and interruption coverage. The current dependencies
   do not yet include the proposed backend; do not simply widen Linux platform cfgs.
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
concrete job is dependency proposal review/qualification and adoption, then
cross-volume evidence and copy/restore integration;
focused validation remains owed before relying on this draft. Owner-directed
implementation-first continuation does not erase those obligations. Do not close
#111 from an implementation-only PR.
