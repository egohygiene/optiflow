# Optiflow source sprint — 2026-11-10

All checkpoints are separate stacked **draft PRs**; merge/release is a later
review. No issue is closed merely because these source checkpoints exist.

| Owner | Source checkpoints | Draft PRs |
| --- | --- | --- |
| #111 | Bound descriptor property port; distinct strong copy/recovery receipt | #166, #167 |
| #95 | Current PNG review/approval; gated restorable transaction/recovery source and CLI | #168, #169 |
| #121 | Bounded recursive PNG inventory/batch approval; serial coordinator/discard/cleanup source and CLI | #170, #171 |
| #146 | Explicit bounded video folder v2/CLI; current staged video replacement handoff | #172, #173 |
| #133 | Pinned-record CLI; operator-locked SDK local runtime/profile handoff | #174, #175 |
| Source review | Final commitment checks, stable PNG admission, current terminal recovery reconciliation and output preflight | #176 |

The stack starts from main `bff0da6d7eff2a364a36cee9dd7e5bfd96796df6`.
Each PR is based on its predecessor branch. Review/merge in ascending order;
main was not changed by this sprint. All commits/titles carry `[skip ci]`.

51 cases were authored across these checkpoints and remain **uncompiled/unrun**.
Only source/manual review, independent authoring and Git/GitHub source transport
and metadata were used. No compiler/build/test/formatter/linter/schema validator,
dependency install/resolution, native/provider/media/model call or CI ran. No
originals were replaced/disposed and no private Akashic media was processed.

## Remaining acceptance gates

- #111: reviewed safe native descriptor ACL backend adoption, namespace/journal
  wiring for cross-volume copy/restore and disposable native APFS proof. Default
  backend and macOS cross-volume production dispatch still refuse.
- #93: native filesystem/install/crash/durability qualification and separately
  authorized release/signature assets. Source inclusion does not qualify v0.2.
- #95/#121: native replacement/recovery/disposal proof. Public mutation/run/resume
  refuse before writable state or providers; no JSON/flag bypass is present.
- #146: native usefulness across the bounded subset and explicit live video
  decode/property adapter at the replacement seam. Staged review grants no apply
  or disposal authority and does not extend arbitrary codecs/HDR/audio support.
- #133: an operator-owned local SDK wrapper, runtime/model/locality/usefulness
  qualification and final acceptance. Trusted process means no OS sandbox or
  offline-enforcement proof; model identity remains a declared pin.
- #115: blocked on the published upstream Relay naming contract (Relay #71);
  no speculative main pin or release rewrite was introduced.
- #135: roadmap closure follows fulfilled owner criteria, not draft count.

Flow #88 remains **last**, with its original 45-ticket ledger and criteria
unchanged. The deferred alignment cancellation/start-marker failure remains
unresolved with an unproven cause. No Identity adoption or ADR ratification is
claimed. Existing proposed ADRs remain proposed; release and deep testing stay
with the later explicitly authorized qualification pass.
