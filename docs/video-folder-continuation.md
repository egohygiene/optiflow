# Reconciled serial folder continuation

The #146 source checkpoint adds `video_folder::resume(&request, cancel)` and the
existing deferred example's `resume <request.json>` action. It continues only
never-started entries. It does not retry a prior producer, adopt an unrecorded
candidate, delete scratch or authorize replacement/disposal.

New runs freeze FFmpeg/FFprobe no-follow file snapshots and complete BLAKE3 bytes
(up to 128 MiB each) into the immutable inventory. The producer retains its own
fresh exact version/native/guard checks. Discovery alone still reads no tools.
Legacy journals remain readable but cannot continue without the original tool
baseline; preserving them for explicit reconciliation is the only supported
path. A tool name/path alone cannot establish compatibility.

Resume holds the existing journal and artifact-store writer locks, checks exact
original request/settings/limits and all immutable start/result/report joins,
then freshly rediscovers and hashes the bounded source inventory and tools.
Source path/inode/metadata/content, classification/order, tool or policy changes
refuse continuation. Every recorded accepted set is rechecked through the existing
artifact/source evidence owner. Extra/missing/uncertain store members block.
Incomplete starts, failed results and uncertain publication remain blocked even
if a loose artifact appears readable later. Nothing grants replay authority.

Recorded successes/unchanged outcomes are retained verbatim and consume their
original action slots. Accepted artifacts retain their existing store/identity.
Only unresolved **never-started** work enters the same serial producer loop.
Action count and publication admission (candidate bytes plus fixed envelope)
are cumulative across all invocations; saved encoded bytes are recomputed from
verified accepted outcomes. Exhausted immutable bounds cannot be enlarged by
resume. Physical reclaimed space and replacement authority remain false.

Time uses an explicitly recorded **fresh per-invocation** cooperative deadline,
including re-observation, with the same original maximum 10 minutes; per-file
execution still gets the tighter remaining/original budget. Report `elapsed_ms`
belongs to that invocation, not lifetime elapsed time. No cumulative wall-clock
or total host-RSS reservation is claimed. At most 32 continuation report slots
are admitted, alongside the original report, under the unchanged 16 MiB journal
byte cap and at most 99 members. Interruption before a report is written leaves
existing starts/results as authority; an unresolved start still blocks.

Reports append as `report-0001.json` through `report-0032.json`; existing files
are never rewritten. Each joins the previous report digest and exact prior
result prefix. Read-only `inspect_recovery` checks all historical reports and
returns the latest observation plus unfinished/unstarted counts, without fresh
source validation or replay authority. Complete/exhausted repeated resumes return
the same report after fresh checks when no new action is possible. Unknown,
gapped or edited history is preserved and refused.

The same tiny Linux-only FFV1/Matroska video-only subset, independent container/
packet/clock/full-decode equivalence, strictly-smaller rule and kernel guard
remain authoritative. No new encoder, lossy profile, cleanup engine, cross-repo
orchestrator or native qualification is added. Broader media/platform, power-loss,
resource and replacement/disposal gates remain #146/#65/#93/#111 work.

Six synthetic source regressions are authored, uncompiled/unrun: idempotence,
report reconstruction, unresolved/failed refusal, changed source/tool/policy,
orphan-store refusal and cumulative-history accounting. No tests/builds, Cargo
resolution, dependencies, lint/format, schema/docs/corpus validators, generators,
product/provider/media/guard execution or CI ran.
