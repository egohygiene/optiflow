use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::{
    CacheStatus, CachedAnalysis, DuplicateGroup, FileObservation, MediaDescriptor, MediaKind,
    ObservationStatus, ScanReport, ScanRun,
};

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
        let connection = Connection::open(&database_path)
            .with_context(|| format!("failed to open {}", database_path.display()))?;
        connection
            .busy_timeout(Duration::from_secs(10))
            .context("failed to configure SQLite busy timeout")?;

        // Apply migration 0001 (idempotent – uses CREATE TABLE IF NOT EXISTS).
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .context("failed to apply migration 0001")?;

        // Apply migration 0002 exactly once.
        apply_migration_0002(&connection).context("failed to apply migration 0002")?;

        // Apply migration 0003 exactly once.
        apply_migration_0003(&connection).context("failed to apply migration 0003")?;

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
            let (filesystem_id, file_id, reported_link_count, allocated_size_bytes,
                 identity_available, allocation_available) =
                identity_columns(observation);

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
                    observation.path,
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
        size_bytes: u64,
        modified_unix_ns: Option<i64>,
        required_probe_signature: Option<&str>,
    ) -> Result<Option<CachedAnalysis>> {
        let path = path.to_string_lossy();
        let row = self
            .connection
            .query_row(
                "SELECT content_type, media_kind_json, content_hash, media_json,
                        probe_signature, status_json, warnings_json
                 FROM file_cache
                 WHERE path = ?1 AND size_bytes = ?2 AND modified_unix_ns IS ?3
                   AND (?4 IS NULL OR probe_signature = ?4)",
                params![
                    path.as_ref(),
                    to_database_integer(size_bytes, "file size")?,
                    modified_unix_ns,
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
        size_bytes: u64,
        modified_unix_ns: Option<i64>,
        analysis: &CachedAnalysis,
    ) -> Result<()> {
        self.connection.execute(
            "INSERT INTO file_cache (
                path, size_bytes, modified_unix_ns, content_type, media_kind_json,
                content_hash, media_json, probe_signature, status_json, warnings_json, analyzed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
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
                analyzed_at = excluded.analyzed_at",
            params![
                path.to_string_lossy().as_ref(),
                to_database_integer(size_bytes, "file size")?,
                modified_unix_ns,
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

    pub fn cache_status(&self) -> Result<CacheStatus> {
        let cached_file_count =
            self.connection
                .query_row("SELECT COUNT(*) FROM file_cache", [], |row| {
                    row.get::<_, u64>(0)
                })?;
        let stored_run_count = self.connection.query_row(
            "SELECT COUNT(*) FROM scan_runs WHERE status = 'completed'",
            [],
            |row| row.get::<_, u64>(0),
        )?;
        let database_size_bytes = fs::metadata(&self.database_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        Ok(CacheStatus {
            state_directory: self.state_directory.to_string_lossy().into_owned(),
            database_path: self.database_path.to_string_lossy().into_owned(),
            database_size_bytes,
            cached_file_count,
            stored_run_count,
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
fn apply_migration_0002(connection: &Connection) -> Result<()> {
    let already_applied: bool = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 2",
            [],
            |row| row.get::<_, u64>(0),
        )
        .context("failed to check schema_migrations for migration 0002")?
        > 0;

    if already_applied {
        return Ok(());
    }

    // The migration SQL contains the INSERT OR IGNORE for version 2.
    connection
        .execute_batch(include_str!(
            "../migrations/0002_filesystem_identity_and_storage.sql"
        ))
        .context("failed to execute migration 0002 SQL")?;

    // Verify it was recorded.
    let recorded: bool = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 2",
            [],
            |row| row.get::<_, u64>(0),
        )
        .context("failed to verify migration 0002 was recorded")?
        > 0;

    if !recorded {
        bail!("migration 0002 executed but was not recorded in schema_migrations");
    }

    Ok(())
}

/// Apply migration 0003 exactly once (observation stability columns).
fn apply_migration_0003(connection: &Connection) -> Result<()> {
    let already_applied: bool = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 3",
            [],
            |row| row.get::<_, u64>(0),
        )
        .context("failed to check schema_migrations for migration 0003")?
        > 0;

    if already_applied {
        return Ok(());
    }

    connection
        .execute_batch(include_str!(
            "../migrations/0003_observation_stability.sql"
        ))
        .context("failed to execute migration 0003 SQL")?;

    let recorded: bool = connection
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 3",
            [],
            |row| row.get::<_, u64>(0),
        )
        .context("failed to verify migration 0003 was recorded")?
        > 0;

    if !recorded {
        bail!("migration 0003 executed but was not recorded in schema_migrations");
    }

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
                let link_count = id
                    .link_count
                    .and_then(|lc| i64::try_from(lc).ok());
                (
                    Some(id.filesystem_id.clone()),
                    Some(id.file_id.clone()),
                    link_count,
                    1_i64,
                )
            }
            None => (None, None, None, 0_i64),
        };

    let (allocated_size_bytes, allocation_available) =
        match observation
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
