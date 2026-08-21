use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::{
    CacheStatus, CachedAnalysis, DuplicateGroup, FileObservation, MediaDescriptor, MediaKind,
    NativePath, ObservationStatus, ScanReport, ScanRun,
};
use crate::filesystem::identity::FileStateSignature;

pub struct StateStore {
    connection: Connection,
    state_directory: PathBuf,
    database_path: PathBuf,
}

impl StateStore {
    pub fn open(state_directory: &Path) -> Result<Self> {
        fs::create_dir_all(state_directory).with_context(|| {
            format!(
                "failed to create state directory {}",
                state_directory.display()
            )
        })?;
        let database_path = state_directory.join("state.sqlite3");
        let mut connection = Connection::open(&database_path)
            .with_context(|| format!("failed to open {}", database_path.display()))?;
        connection
            .busy_timeout(Duration::from_secs(10))
            .context("failed to configure SQLite busy timeout")?;

        // Apply migration 0001 (idempotent – uses CREATE TABLE IF NOT EXISTS).
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .context("failed to apply migration 0001")?;

        // Apply migration 0002 exactly once.
        apply_migration_0002(&mut connection).context("failed to apply migration 0002")?;

        // Apply migration 0003 exactly once.
        apply_migration_0003(&mut connection).context("failed to apply migration 0003")?;

        // Apply migration 0004 exactly once.
        apply_migration_0004(&mut connection).context("failed to apply migration 0004")?;

        // Apply migration 0005 exactly once.
        apply_migration_0005(&mut connection).context("failed to apply migration 0005")?;

        Ok(Self {
            connection,
            state_directory: state_directory.to_path_buf(),
            database_path,
        })
    }

    pub fn begin_scan(&self, run_id: &str, created_at: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO scan_runs (run_id, created_at, status) VALUES (?1, ?2, 'running')",
            params![run_id, created_at],
        )?;
        Ok(())
    }

    pub fn mark_scan_interrupted(&self, run_id: &str, completed_at: &str) -> Result<()> {
        self.connection.execute(
            "UPDATE scan_runs
             SET completed_at = ?2, status = 'interrupted'
             WHERE run_id = ?1 AND status = 'running'",
            params![run_id, completed_at],
        )?;
        Ok(())
    }

    pub fn mark_scan_failed(&self, run_id: &str, completed_at: &str) -> Result<()> {
        self.connection.execute(
            "UPDATE scan_runs
             SET completed_at = ?2, status = 'failed'
             WHERE run_id = ?1 AND status = 'running'",
            params![run_id, completed_at],
        )?;
        Ok(())
    }

    pub fn finalize_scan(
        &mut self,
        run: &ScanRun,
        report: &ScanReport,
        observations: &[FileObservation],
        groups: &[DuplicateGroup],
    ) -> Result<()> {
        let manifest_json = serde_json::to_string(run)?;
        let report_json = serde_json::to_string(report)?;
        let transaction = self.connection.transaction()?;

        for observation in observations {
            let (
                filesystem_id,
                file_id,
                reported_link_count,
                allocated_size_bytes,
                identity_available,
                allocation_available,
            ) = identity_columns(observation);

            transaction.execute(
                "INSERT INTO observations (
                    observation_id, run_id, path, size_bytes, content_hash, observation_json,
                    filesystem_id, file_id, reported_link_count, allocated_size_bytes,
                    identity_available, allocation_available,
                    observation_stability, evidence_validity, attempt_count
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    observation.observation_id,
                    observation.run_id,
                    observation.path.sqlite_key().as_ref(),
                    to_database_integer(observation.size_bytes, "file size")?,
                    observation.content_hash,
                    serde_json::to_string(observation)?,
                    filesystem_id,
                    file_id,
                    reported_link_count,
                    allocated_size_bytes,
                    identity_available,
                    allocation_available,
                    serde_json::to_string(&observation.observation_stability)?,
                    serde_json::to_string(&observation.evidence_validity)?,
                    i64::from(observation.attempt_count),
                ],
            )?;
        }

        for group in groups {
            transaction.execute(
                "INSERT INTO duplicate_groups (group_id, run_id, group_json)
                 VALUES (?1, ?2, ?3)",
                params![group.group_id, run.run_id, serde_json::to_string(group)?],
            )?;
        }

        transaction.execute(
            "UPDATE scan_runs
             SET completed_at = ?2, status = 'completed', manifest_json = ?3, report_json = ?4
             WHERE run_id = ?1",
            params![run.run_id, run.completed_at, manifest_json, report_json],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn lookup_cache(
        &self,
        path: &Path,
        signature: &FileStateSignature,
        required_probe_signature: Option<&str>,
    ) -> Result<Option<CachedAnalysis>> {
        let path_key = NativePath::from_path(path);
        let path_key = path_key.sqlite_key();
        let identity = signature.identity.as_ref();
        let row = self
            .connection
            .query_row(
                "SELECT content_type, media_kind_json, content_hash, media_json,
                        probe_signature, status_json, warnings_json
                 FROM file_cache
                 WHERE path = ?1 AND size_bytes = ?2 AND modified_unix_ns IS ?3
                   AND filesystem_id IS ?4 AND file_id IS ?5
                   AND changed_unix_ns IS ?6
                   AND filesystem_id IS NOT NULL AND file_id IS NOT NULL
                   AND changed_unix_ns IS NOT NULL
                   AND (?7 IS NULL OR probe_signature = ?7)",
                params![
                    path_key.as_ref(),
                    to_database_integer(signature.logical_size_bytes, "file size")?,
                    signature.modified_unix_ns,
                    identity.map(|identity| identity.filesystem_id.as_str()),
                    identity.map(|identity| identity.file_id.as_str()),
                    signature.changed_unix_ns,
                    required_probe_signature,
                ],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()?;

        row.map(
            |(content_type, media_kind, content_hash, media, probe_signature, status, warnings)| {
                Ok(CachedAnalysis {
                    content_type,
                    media_kind: serde_json::from_str::<MediaKind>(&media_kind)?,
                    content_hash,
                    media: media
                        .map(|value| serde_json::from_str::<MediaDescriptor>(&value))
                        .transpose()?,
                    probe_signature,
                    status: serde_json::from_str::<ObservationStatus>(&status)?,
                    warnings: serde_json::from_str::<Vec<String>>(&warnings)?,
                    // Cache hits are treated as stable; stability is re-checked
                    // at hash time when the entry is actually used.
                    observation_stability: crate::domain::ObservationStability::Stable,
                    evidence_validity: crate::domain::EvidenceValidity::Current,
                    attempt_count: 1,
                })
            },
        )
        .transpose()
    }

    pub fn upsert_cache(
        &self,
        path: &Path,
        signature: &FileStateSignature,
        analysis: &CachedAnalysis,
    ) -> Result<()> {
        let (Some(identity), Some(changed_unix_ns)) =
            (signature.identity.as_ref(), signature.changed_unix_ns)
        else {
            return Ok(());
        };
        self.connection.execute(
            "INSERT INTO file_cache (
                path, size_bytes, modified_unix_ns, content_type, media_kind_json,
                content_hash, media_json, probe_signature, status_json, warnings_json, analyzed_at,
                filesystem_id, file_id, changed_unix_ns
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(path) DO UPDATE SET
                size_bytes = excluded.size_bytes,
                modified_unix_ns = excluded.modified_unix_ns,
                content_type = excluded.content_type,
                media_kind_json = excluded.media_kind_json,
                content_hash = excluded.content_hash,
                media_json = excluded.media_json,
                probe_signature = excluded.probe_signature,
                status_json = excluded.status_json,
                warnings_json = excluded.warnings_json,
                analyzed_at = excluded.analyzed_at,
                filesystem_id = excluded.filesystem_id,
                file_id = excluded.file_id,
                changed_unix_ns = excluded.changed_unix_ns",
            params![
                NativePath::from_path(path).sqlite_key().as_ref(),
                to_database_integer(signature.logical_size_bytes, "file size")?,
                signature.modified_unix_ns,
                analysis.content_type,
                serde_json::to_string(&analysis.media_kind)?,
                analysis.content_hash,
                analysis
                    .media
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                analysis.probe_signature,
                serde_json::to_string(&analysis.status)?,
                serde_json::to_string(&analysis.warnings)?,
                Utc::now().to_rfc3339(),
                identity.filesystem_id,
                identity.file_id,
                changed_unix_ns,
            ],
        )?;
        Ok(())
    }

    pub fn load_report(&self, run_id: &str) -> Result<Option<ScanReport>> {
        let report_json = self
            .connection
            .query_row(
                "SELECT report_json FROM scan_runs WHERE run_id = ?1 AND status = 'completed'",
                [run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        report_json
            .map(|value| serde_json::from_str(&value).context("stored report JSON is invalid"))
            .transpose()
    }

    pub fn load_run_status(&self, run_id: &str) -> Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT status FROM scan_runs WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .optional()
            .context("failed to inspect stored scan-run status")
    }

    pub fn cache_status(&self) -> Result<CacheStatus> {
        let cached_file_count =
            self.connection
                .query_row("SELECT COUNT(*) FROM file_cache", [], |row| {
                    row.get::<_, i64>(0)
                })?;
        let stored_run_count = self.connection.query_row(
            "SELECT COUNT(*) FROM scan_runs WHERE status = 'completed'",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        let database_size_bytes = fs::metadata(&self.database_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        Ok(CacheStatus {
            state_directory: self.state_directory.to_string_lossy().into_owned(),
            database_path: self.database_path.to_string_lossy().into_owned(),
            database_size_bytes,
            cached_file_count: u64::try_from(cached_file_count)
                .context("cached file count was negative")?,
            stored_run_count: u64::try_from(stored_run_count)
                .context("stored run count was negative")?,
        })
    }
}

// ---------------------------------------------------------------------------
// Migration helpers
// ---------------------------------------------------------------------------

/// Apply migration 0002 exactly once.
///
/// Checks `schema_migrations` for version 2.  If already present, the
/// migration is skipped.  Otherwise the SQL statements are executed and the
/// version row is inserted atomically.
fn apply_migration_0002(connection: &mut Connection) -> Result<()> {
    let already_applied: bool = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 2",
            [],
            |row| row.get::<_, i64>(0),
        )
        .context("failed to check schema_migrations for migration 0002")?
        > 0;

    if already_applied {
        return Ok(());
    }

    let transaction = connection
        .transaction()
        .context("failed to begin migration 0002 transaction")?;
    transaction
        .execute_batch(include_str!(
            "../migrations/0002_filesystem_identity_and_storage.sql"
        ))
        .context("failed to execute migration 0002 SQL")?;

    // Verify it was recorded.
    let recorded: bool = transaction
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 2",
            [],
            |row| row.get::<_, i64>(0),
        )
        .context("failed to verify migration 0002 was recorded")?
        > 0;

    if !recorded {
        bail!("migration 0002 executed but was not recorded in schema_migrations");
    }

    transaction
        .commit()
        .context("failed to commit migration 0002 transaction")?;
    Ok(())
}

/// Apply migration 0003 exactly once (observation stability columns).
fn apply_migration_0003(connection: &mut Connection) -> Result<()> {
    let already_applied: bool = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 3",
            [],
            |row| row.get::<_, i64>(0),
        )
        .context("failed to check schema_migrations for migration 0003")?
        > 0;

    if already_applied {
        return Ok(());
    }

    let transaction = connection
        .transaction()
        .context("failed to begin migration 0003 transaction")?;
    transaction
        .execute_batch(include_str!("../migrations/0003_observation_stability.sql"))
        .context("failed to execute migration 0003 SQL")?;

    let recorded: bool = transaction
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 3",
            [],
            |row| row.get::<_, i64>(0),
        )
        .context("failed to verify migration 0003 was recorded")?
        > 0;

    if !recorded {
        bail!("migration 0003 executed but was not recorded in schema_migrations");
    }

    transaction
        .commit()
        .context("failed to commit migration 0003 transaction")?;
    Ok(())
}

/// Apply migration 0004 exactly once (native path encoding columns).
fn apply_migration_0004(connection: &mut Connection) -> Result<()> {
    let already_applied: bool = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 4",
            [],
            |row| row.get::<_, i64>(0),
        )
        .context("failed to check schema_migrations for migration 0004")?
        > 0;

    if already_applied {
        return Ok(());
    }

    let transaction = connection
        .transaction()
        .context("failed to begin migration 0004 transaction")?;
    transaction
        .execute_batch(include_str!("../migrations/0004_native_path.sql"))
        .context("failed to execute migration 0004 SQL")?;

    let recorded: bool = transaction
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 4",
            [],
            |row| row.get::<_, i64>(0),
        )
        .context("failed to verify migration 0004 was recorded")?
        > 0;

    if !recorded {
        bail!("migration 0004 executed but was not recorded in schema_migrations");
    }

    transaction
        .commit()
        .context("failed to commit migration 0004 transaction")?;
    Ok(())
}

/// Apply migration 0005 exactly once (handle-bound cache signatures).
fn apply_migration_0005(connection: &mut Connection) -> Result<()> {
    let already_applied: bool = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 5",
            [],
            |row| row.get::<_, i64>(0),
        )
        .context("failed to check schema_migrations for migration 0005")?
        > 0;

    if already_applied {
        return Ok(());
    }

    let transaction = connection
        .transaction()
        .context("failed to begin migration 0005 transaction")?;
    transaction
        .execute_batch(include_str!("../migrations/0005_handle_bound_cache.sql"))
        .context("failed to execute migration 0005 SQL")?;

    let recorded: bool = transaction
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 5",
            [],
            |row| row.get::<_, i64>(0),
        )
        .context("failed to verify migration 0005 was recorded")?
        > 0;

    if !recorded {
        bail!("migration 0005 executed but was not recorded in schema_migrations");
    }

    transaction
        .commit()
        .context("failed to commit migration 0005 transaction")?;
    Ok(())
}

/// Extract structured identity/allocation columns from an observation.
///
/// Returns `(filesystem_id, file_id, reported_link_count, allocated_size_bytes,
///           identity_available, allocation_available)`.
fn identity_columns(
    observation: &FileObservation,
) -> (
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    i64,
    i64,
) {
    let (filesystem_id, file_id, reported_link_count, identity_available) =
        match &observation.filesystem_identity {
            Some(id) => {
                let link_count = id.link_count.and_then(|lc| i64::try_from(lc).ok());
                (
                    Some(id.filesystem_id.clone()),
                    Some(id.file_id.clone()),
                    link_count,
                    1_i64,
                )
            }
            None => (None, None, None, 0_i64),
        };

    let (allocated_size_bytes, allocation_available) = match observation
        .storage_allocation
        .as_ref()
        .and_then(|a| a.allocated_size_bytes)
    {
        Some(bytes) => (i64::try_from(bytes).ok(), 1_i64),
        None => (None, 0_i64),
    };

    (
        filesystem_id,
        file_id,
        reported_link_count,
        allocated_size_bytes,
        identity_available,
        allocation_available,
    )
}

fn to_database_integer(value: u64, label: &str) -> Result<i64> {
    i64::try_from(value).with_context(|| format!("{label} exceeds SQLite's integer range"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_0002_rolls_back_its_version_marker_on_failure() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .expect("initial schema");
        connection
            .execute("ALTER TABLE observations ADD COLUMN filesystem_id TEXT", [])
            .expect("inject conflicting column");

        assert!(apply_migration_0002(&mut connection).is_err());
        assert_eq!(migration_count(&connection, 2), 0);
    }

    #[test]
    fn migration_0003_rolls_back_its_version_marker_on_failure() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .expect("initial schema");
        connection
            .execute(
                "ALTER TABLE observations ADD COLUMN observation_stability TEXT",
                [],
            )
            .expect("inject conflicting column");

        assert!(apply_migration_0003(&mut connection).is_err());
        assert_eq!(migration_count(&connection, 3), 0);
    }

    #[test]
    fn migration_0004_rolls_back_its_version_marker_on_failure() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .expect("initial schema");
        // Pre-inject the column that migration 0004 adds so it fails with a
        // duplicate-column error.
        connection
            .execute("ALTER TABLE observations ADD COLUMN path_encoding TEXT", [])
            .expect("inject conflicting column");

        assert!(apply_migration_0004(&mut connection).is_err());
        assert_eq!(migration_count(&connection, 4), 0);
    }

    #[test]
    fn migration_0005_rolls_back_its_version_marker_on_failure() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .expect("initial schema");
        connection
            .execute("ALTER TABLE file_cache ADD COLUMN filesystem_id TEXT", [])
            .expect("inject conflicting column");

        assert!(apply_migration_0005(&mut connection).is_err());
        assert_eq!(migration_count(&connection, 5), 0);
    }

    #[test]
    fn cache_reuse_requires_the_complete_handle_signature() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("observed.bin");
        fs::write(&path, b"stable cache bytes").expect("cache fixture");
        let metadata = fs::symlink_metadata(&path).expect("fixture metadata");
        let signature = FileStateSignature::from_symlink_metadata(&metadata);
        let store = StateStore::open(&directory.path().join("state")).expect("state store");
        let analysis = CachedAnalysis {
            content_type: Some("application/octet-stream".to_owned()),
            media_kind: MediaKind::Other,
            content_hash: None,
            media: None,
            probe_signature: None,
            status: ObservationStatus::Unsupported,
            warnings: Vec::new(),
            observation_stability: crate::domain::ObservationStability::Stable,
            evidence_validity: crate::domain::EvidenceValidity::Current,
            attempt_count: 1,
        };

        store
            .upsert_cache(&path, &signature, &analysis)
            .expect("cache insert");
        assert!(
            store
                .lookup_cache(&path, &signature, None)
                .expect("cache lookup")
                .is_some()
        );

        let mut changed = signature.clone();
        changed.changed_unix_ns = changed.changed_unix_ns.map(|value| value.saturating_add(1));
        assert!(
            store
                .lookup_cache(&path, &changed, None)
                .expect("changed-signature lookup")
                .is_none()
        );

        store
            .connection
            .execute(
                "UPDATE file_cache SET filesystem_id = NULL, file_id = NULL, changed_unix_ns = NULL",
                [],
            )
            .expect("simulate a pre-migration cache row");
        assert!(
            store
                .lookup_cache(&path, &signature, None)
                .expect("legacy-row lookup")
                .is_none()
        );
    }

    fn migration_count(connection: &Connection, version: i64) -> i64 {
        connection
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = ?1",
                [version],
                |row| row.get(0),
            )
            .expect("migration count")
    }
}
