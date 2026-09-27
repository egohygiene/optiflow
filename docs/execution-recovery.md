# Quarantine recovery (development source)

This #92 implementation adds operator-initiated recovery for the Linux-only
quarantine transaction. It is absent from the immutable read-only `v0.1.1`
release. Use disposable files when evaluating it. Recovery never permanently
deletes a file or claims measured physical savings.

## Inspect before an operation

Save the `run_id` returned by live `apply`. Status reads the existing journal
without taking a write lock, migrating state, classifying an abandoned run, or
touching the filesystem. It describes **recorded** evidence, not a fresh
proof that the paths still match. JSON output contains a strict
`optiflow.execution-recovery-report.v3` result and its append-only transition
events.

```bash
optiflow --no-config --state-directory /local/optiflow-state --json \
  execution status --run RUN_UUID
```

`resumable` means the v3 apply context is bound and remaining actions were
recorded as untouched or merely prepared. `quarantined`, `restored`, and
`cleaned` describe recorded terminal phases; mutating commands revalidate
current paths. `attention_required` means missing historical authority or a
pending/ambiguous transition. Do not infer that a pending rename, copy, or
unlink did or did not happen. Inspect the original, temporary, and quarantine
paths before any manual repair.

## Resume, restore, and owned cleanup

The supplied plan and approval must be the exact immutable documents for the
run, and the effective configuration, policy and binary version must match the
binding written before the original live mutation. Approval is a local audit
record, not a signature. These commands take the exclusive journal lock and
refuse changed identity, content, properties, scope, topology, authorization,
capacity, or reserve. They do not extend the approved action or in-flight byte
bounds. Only the recorded owner of a namespace may operate on it.

```bash
optiflow --no-config --state-directory /local/optiflow-state --json \
  execution resume --run RUN_UUID --plan /local/evidence/plan.json \
  --approval /local/evidence/approval.json

optiflow --no-config --state-directory /local/optiflow-state --json \
  execution restore --run RUN_UUID --action action-000001 \
  --plan /local/evidence/plan.json --approval /local/evidence/approval.json

optiflow --no-config --state-directory /local/optiflow-state --json \
  execution cleanup --run RUN_UUID --plan /local/evidence/plan.json \
  --approval /local/evidence/approval.json
```

Resume skips and rechecks already committed actions, then runs only untouched
actions in approved order. It cannot retry an action with a pending filesystem
transition or after any restore. A repeated successful resume adds no events.
Restore names one committed action; it verifies the keeper, complete content
hash, direct bytes, original ownership/mode/modification time and the bounded
extended-attribute fingerprint recorded at commit. It requires a vacant source
path. On the same filesystem an exclusive no-replace rename returns the
original object. Across filesystems it copies into an exclusive source-side
temporary file, synchronizes and verifies it, then commits it with a no-replace
rename. The verified quarantine copy remains; there is no automatic deletion
of that copy. A repeated successful restore verifies the result and appends
no events.

Cleanup is narrowly defined: after **every** action has returned by
same-filesystem rename, it checks the original sources and removes only the
empty, expected quarantine namespace, then synchronizes its parent. It refuses
unexpected entries, partial restores, or retained cross-filesystem copies.
A repeated successful cleanup only verifies absence. None of these commands
remove a quarantined data file. Logical selected bytes and physical reclaimed
bytes remain separate; the latter is always `null`.

## Interruption and version boundaries

Migration 0008 adds `execution_recovery_events`, an append-only v3 transition
journal with update/delete triggers. It does not rewrite v1 dry-run records or
v2 mutation contracts. A new live apply binds the configuration and policy
before mutation, and records a property fingerprint after each v2 commit.
Recovery adds durable pending and completed transition events. Status cannot
turn an unfinished event into a commit. An interrupted or failed step leaves
its source, destination and temporary paths untouched for inspection. An
occupied destination or stale property is a refusal, never an overwrite.

Historical #91 v2 mutation rows lack the pre-mutation v3 binding and property
fingerprint. They remain inspection-only and cannot silently gain recovery
authority. A hard exit between a filesystem change and its completion event
remains `attention_required`, even if a path happens to look correct. Manual
inspection is needed. Hostile concurrent writers, network disconnects, quotas,
remote durability and physical allocation behavior are not guaranteed by the
local protocol.
