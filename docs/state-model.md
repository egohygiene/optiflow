# State Model

## Location

The primary state database is local to the machine, not placed automatically on
a scanned removable or network volume.

- macOS: `~/Library/Application Support/optiflow/state.sqlite3`
- Linux: `$XDG_STATE_HOME/optiflow/state.sqlite3`
- Linux fallback: `~/.local/state/optiflow/state.sqlite3`

## SQLite policy

The database uses the rollback journal and `FULL` synchronization. This avoids
assuming SQLite WAL shared-memory behavior is safe on arbitrary scanned
filesystems. A busy timeout handles short contention without hiding persistent
ownership conflicts.

## Tables

| Table | Purpose |
| --- | --- |
| `schema_migrations` | Applied database schema versions |
| `scan_runs` | Run lifecycle and final manifest/report JSON |
| `observations` | Queryable path, size, hash, and complete observation JSON |
| `duplicate_groups` | Derived group JSON for each completed run |
| `file_cache` | Reusable unchanged-path analysis |

## Cache identity and invalidation

The cache key is:

```text
native path + filesystem id + file id + size_bytes + modified_unix_ns + changed_unix_ns
```

Any mismatch causes re-analysis. Missing filesystem identity or metadata-change
time disables cache lookup and insertion for that observation. Cache rows from
older migrations have null handle-signature fields and are therefore not
reused. A cache hit is accepted only inside a fresh, stable
[handle-bound observation window](observation-protocol.md).

If a cached file previously had no full hash but becomes a size-based duplicate
candidate, OptiFlow calculates and stores its full hash during the new scan.

The cache recognizes unchanged filesystem objects at unchanged native paths.
Content-addressed recognition
after arbitrary moves is a later enhancement because non-candidate files are
not all hashed in the MVP.

## Run lifecycle

1. Insert a `running` scan row.
2. Perform discovery and analysis.
3. Validate and stage `effective-policy.json`, `run.json`, `report.json`, and
   their digest-bearing commit marker.
4. Flush the members and marker, then publish the directory with one rename.
5. Store observations and groups in one database transaction.
6. Mark the run `completed` and attach final JSON.

Handled interruptions mark rows `interrupted`; internal failures mark rows
`failed`; a hard process failure may leave recoverable `running` state. None is
returned as a completed report. A still-`running` row with a committed v5
artifact set is recovered idempotently when state is next opened. Stale staging
directories are removed; incomplete or incompatible final sets are never
promoted.

The effective-policy sidecar is referenced through the existing immutable run
artifact directory, so migrations `0001`–`0003` and v1-v4 run/report schemas are
unchanged. Historical runs without a sidecar have unknown policy evidence; the
current resolver never fabricates a snapshot for them.

See the [artifact-set commit protocol](artifact-set-protocol.md) for marker,
reader-state, plan-handshake, crash-recovery, and durability semantics.
