# MP4 operational progress

Relates to #146/#135. Authored source only; no run or qualification recorded.

The SDK offers `produce_with_progress`, `folder::run_with_progress` and
`folder::resume_with_progress`, each taking a caller-owned `progress::Observer`.
Existing entry points use a silent observer. Constructing an observer starts no
threads, subscriber, exporter, network service or native work.

Events describe planning, runtime preparation, source decode, encoding, candidate
validation, artifact commitment, folder discovery/reconciliation, entry handling
and report writing. Folder entry events carry actual known entry totals and
completed counts. Encoder events have no percent or fabricated total. A phase
finishing does not mean the candidate was accepted. Whole-folder stops/failures
remain different from completion, and every event is explicitly non-authoritative.

The schema carries an ephemeral operation ID, monotonic sequence, phase,
transition, optional entry counters and local elapsed milliseconds. It contains
no filenames, source paths, raw media/metadata, prompts, tool argv, credentials
or native captures. These fields and observer settings never enter request,
plan, provider, artifact or continuation identities.

The observer admits at most the explicitly selected count (up to 16,384) and
JSONL bytes (up to 16 MiB). Dropped events and sink errors are counted; they do
not retry work, raise work authority or rewrite domain outcomes. SDK callers
can inspect `statistics()`. Callbacks are synchronous and must return promptly
without panicking. The API does not preempt caller code or blocked host I/O;
observer overhead can consume the existing operation deadline.

The CLI enables file logging only for `run`, `folder-run` and `folder-resume`:

```sh
optiflow --json mp4-delivery run --request /private/request.json --event-log /private/logs/new-run.jsonl
```

The Task alias is `mp4:run:events REQUEST=FILE EVENT_LOG=FILE`. Optional
`--maximum-events` and `--maximum-event-bytes` require `--event-log`; the CLI
defaults are 16,384 events and 8 MiB. Other command modes refuse these flags
without writing a log. Log paths must be outside selected source/tool/work/
artifact/journal namespaces, with an existing private no-follow parent and a
new leaf. No prior file is overwritten or appended. Identity/length changes or
partial writes disable successful subsequent records rather than adopting data.

Machine stdout remains one final command result. A degraded event sink adds a
bounded `mp4_progress_degraded` warning with no outcome impact. Logs can be
partial or truncated and are never evidence of validation, retained state,
native qualification, human approval or a completed workflow. Read authoritative
results and pinned artifacts separately; no progress reader grants replay.

The future GitHub Pages surface will be labeled as a demo/replay of already
calculated results. This sprint creates no page, animated UI, media run or live
service. Two observer-budget/sink/no-clobber cases are authored, uncompiled/unrun.
All compiler/build/tests/dependencies/lint/format/schema/docs/drift, product,
CLI/SDK/native/provider/media/model and CI execution remains deferred.
