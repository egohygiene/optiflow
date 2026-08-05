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

The initial cache key is:

```text
path + size_bytes + modified_unix_ns
```

Any mismatch causes re-analysis. If a cached file previously had no full hash
but becomes a size-based duplicate candidate, OptiFlow calculates and stores its
full hash during the new scan.

The current cache recognizes unchanged paths. Content-addressed recognition
after arbitrary moves is a later enhancement because non-candidate files are
not all hashed in the MVP.

## Run lifecycle

1. Insert a `running` scan row.
2. Perform discovery and analysis.
3. Atomically write `run.json` and `report.json`.
4. Store observations and groups in one database transaction.
5. Mark the run `completed` and attach final JSON.

Interrupted rows remain visibly incomplete and are never returned as completed
reports.

