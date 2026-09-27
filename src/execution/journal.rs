use std::fs::File;
use std::path::Path;

use chrono::Utc;
use rusqlite::{OptionalExtension, params};

use super::filesystem as fs;
use super::finalization::FinalizationEvent;
use super::model::*;
use super::mutation::{MutationRun, MutationStatus};
use super::recovery::RecoveryEvent;
use super::{Result, failure};
use crate::contracts::{self, Contract};
use crate::outcome::DiagnosticCode as Code;
use crate::state::StateStore;

pub(super) struct Journal {
    store: StateStore,
    _lock: File,
}

fn state_error(e: impl std::fmt::Display) -> Box<crate::outcome::Diagnostic> {
    failure(
        Code::StateTransactionFailed,
        format!("execution journal: {e}"),
    )
}

fn encode<T: serde::Serialize>(document: &T) -> Result<String> {
    contracts::validate(Contract::Execution, document).map_err(state_error)?;
    serde_json::to_string(document).map_err(state_error)
}

/// Only the process holding the OS lock may classify unfinished dry runs as
/// interrupted. Merely opening the database from another command cannot do so.
impl Journal {
    #[cfg(unix)]
    pub fn open(plan: &ExecutionPlan) -> Result<Self> {
        use rustix::fs::{FlockOperation, Mode, OFlags, flock, openat};
        use std::os::unix::fs::MetadataExt;
        let directory = fs::check_directory(&plan.body.state)?;
        fs::writable_directory(&directory)?;
        let path = fs::native_path(&plan.body.state.path)?;
        // Reject redirected SQLite files, journals and lock files before any
        // writable open. Evidence may never alias an approved source object.
        for name in [
            "state.sqlite3",
            "state.sqlite3-journal",
            "state.sqlite3-wal",
            "state.sqlite3-shm",
            "execution.lock",
        ] {
            match std::fs::symlink_metadata(path.join(name)) {
                Ok(m) if m.is_file() && !m.file_type().is_symlink() && m.nlink() == 1 => (),
                Ok(_) => {
                    return Err(failure(
                        Code::ExecutionScopeInvalid,
                        "state contains a symlink, hard-link alias or special file",
                    ));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(state_error(e)),
            }
        }
        if path.join("state.sqlite3").exists() {
            let connection = rusqlite::Connection::open_with_flags(
                path.join("state.sqlite3"),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .map_err(state_error)?;
            let newest: i64 = connection
                .query_row(
                    "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                    [],
                    |r| r.get(0),
                )
                .map_err(state_error)?;
            if newest > 9 {
                return Err(failure(
                    Code::StoredStateIncompatible,
                    "state was migrated by a newer execution implementation",
                ));
            }
        }
        let handle = openat(
            &directory,
            "execution.lock",
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(state_error)?;
        let lock = File::from(handle);
        if fs::metadata(&lock)?.nlink() != 1 {
            return Err(failure(
                Code::ExecutionScopeInvalid,
                "ambiguous execution lock",
            ));
        }
        flock(&lock, FlockOperation::NonBlockingLockExclusive).map_err(|e| {
            failure(
                Code::ExecutionSourceStale,
                format!("another execution owns the journal: {e}"),
            )
        })?;
        fs::check_directory(&plan.body.state)?;
        let store = StateStore::open_execution(&path).map_err(state_error)?;
        store
            .connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(state_error)?;
        directory.sync_all().map_err(state_error)?;
        let mut journal = Self { store, _lock: lock };
        journal.recover()?;
        journal.recover_mutations()?;
        Ok(journal)
    }
    #[cfg(not(unix))]
    pub fn open(_plan: &ExecutionPlan) -> Result<Self> {
        Err(failure(
            Code::ExecutionUnsupported,
            "execution journal locking unsupported",
        ))
    }

    fn recover(&mut self) -> Result<()> {
        let documents: Vec<String> = self
            .store
            .connection
            .prepare("SELECT run_id FROM execution_runs WHERE status = 'validating'")
            .map_err(state_error)?
            .query_map([], |r| r.get(0))
            .map_err(state_error)?
            .collect::<std::result::Result<_, _>>()
            .map_err(state_error)?;
        for run_id in documents {
            let mut run = read_run(&self.store.connection, &run_id)?.ok_or_else(|| {
                failure(
                    Code::StoredStateIncompatible,
                    "execution disappeared during recovery",
                )
            })?;
            run.status = Status::Interrupted;
            run.validation.status = Status::Interrupted;
            run.validation.diagnostics.push(*failure(
                Code::OperationInterrupted,
                "prior dry-run process stopped before a terminal record; start a fresh validation",
            ));
            for attempt in &mut run.attempts {
                if attempt.status == Status::Validating {
                    attempt.status = Status::Interrupted;
                }
            }
            run.recovery.status = "interrupted_dry_run".to_owned();
            run.completed_at = Some(Utc::now().to_rfc3339());
            self.save(&run)?;
        }
        Ok(())
    }

    fn recover_mutations(&mut self) -> Result<()> {
        let ids: Vec<String> = self
            .store
            .connection
            .prepare("SELECT run_id FROM execution_mutation_runs WHERE status = 'running'")
            .map_err(state_error)?
            .query_map([], |row| row.get(0))
            .map_err(state_error)?
            .collect::<std::result::Result<_, _>>()
            .map_err(state_error)?;
        for id in ids {
            let mut run = read_mutation(&self.store.connection, &id)?.ok_or_else(|| {
                failure(
                    Code::StoredStateIncompatible,
                    "mutation evidence disappeared",
                )
            })?;
            run.status = MutationStatus::Interrupted;
            run.completed_at = Some(Utc::now().to_rfc3339());
            run.diagnostics.push(*failure(Code::OperationInterrupted,
                "prior mutation stopped without terminal evidence; inspect the recorded source, temporary and destination paths before manual recovery"));
            self.save_mutation(&run)?;
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn begin_mutation(
        &mut self,
        plan: &ExecutionPlan,
        approval: &Approval,
        run: &MutationRun,
    ) -> Result<()> {
        let transaction = self.store.connection.transaction().map_err(state_error)?;
        for (table, key_name, key, json) in [
            (
                "execution_plans",
                "fingerprint",
                &plan.fingerprint,
                encode(plan)?,
            ),
            (
                "execution_approvals",
                "authorization_id",
                &approval.authorization_id,
                encode(approval)?,
            ),
        ] {
            let existing: Option<String> = transaction
                .query_row(
                    &format!("SELECT document_json FROM {table} WHERE {key_name} = ?1"),
                    [key],
                    |r| r.get(0),
                )
                .optional()
                .map_err(state_error)?;
            if let Some(existing) = existing {
                if existing != json {
                    return Err(failure(
                        Code::ExecutionPlanInvalid,
                        "immutable execution evidence identity collision",
                    ));
                }
            } else if table == "execution_plans" {
                transaction
                    .execute(
                        "INSERT INTO execution_plans VALUES (?1, ?2)",
                        params![key, json],
                    )
                    .map_err(state_error)?;
            } else {
                transaction
                    .execute(
                        "INSERT INTO execution_approvals VALUES (?1, ?2, ?3)",
                        params![key, plan.fingerprint, json],
                    )
                    .map_err(state_error)?;
            }
        }
        transaction
            .execute(
                "INSERT INTO execution_mutation_runs VALUES (?1, ?2, ?3, 'running', ?4)",
                params![
                    run.run_id,
                    run.plan_fingerprint,
                    run.authorization_id,
                    encode_mutation(run)?
                ],
            )
            .map_err(state_error)?;
        transaction.commit().map_err(state_error)
    }

    pub fn save_mutation(&mut self, run: &MutationRun) -> Result<()> {
        let status = serde_json::to_value(run.status).map_err(state_error)?;
        let changed = self.store.connection.execute(
            "UPDATE execution_mutation_runs SET status = ?2, document_json = ?3 WHERE run_id = ?1 AND status = 'running'",
            params![run.run_id, status.as_str(), encode_mutation(run)?]).map_err(state_error)?;
        if changed != 1 {
            return Err(failure(
                Code::ExecutionSourceStale,
                "terminal mutation evidence is immutable",
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn mutation(&self, run_id: &str) -> Result<Option<MutationRun>> {
        read_mutation(&self.store.connection, run_id)
    }

    #[cfg(target_os = "linux")]
    pub fn recovery_events(&self, run_id: &str) -> Result<Vec<RecoveryEvent>> {
        read_recovery_events(&self.store.connection, run_id)
    }

    #[cfg(target_os = "linux")]
    pub fn finalization_events(&self, run_id: &str) -> Result<Vec<FinalizationEvent>> {
        read_finalization_events(&self.store.connection, run_id)
    }

    #[cfg(target_os = "linux")]
    pub fn append_finalization(
        &mut self,
        event: &FinalizationEvent,
        max_actions: u64,
        budget: u64,
    ) -> Result<()> {
        contracts::validate(Contract::ExecutionFinalizationEvent, event).map_err(state_error)?;
        let document = serde_json::to_string(event).map_err(state_error)?;
        let (count, bytes): (i64, i64) = self.store.connection.query_row(
            "SELECT COUNT(*), COALESCE(SUM(LENGTH(document_json)), 0) FROM execution_finalization_events WHERE run_id = ?1",
            [&event.run_id], |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(state_error)?;
        if count < 0
            || count as u64 >= max_actions.saturating_mul(2)
            || bytes < 0
            || (bytes as u64).saturating_add(document.len() as u64) > budget
        {
            return Err(failure(
                Code::ExecutionBoundsExceeded,
                "finalization journal bound exceeded",
            ));
        }
        self.store.connection.execute(
            "INSERT INTO execution_finalization_events (event_id, run_id, action_id, phase, preview_fingerprint, authorization_id, document_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![event.event_id, event.run_id, event.action_id, event.phase,
                event.preview_fingerprint, event.authorization_id, document],
        ).map_err(state_error)?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn stored_authority(&self, plan: &ExecutionPlan, approval: &Approval) -> Result<()> {
        for (table, key_name, key, expected) in [
            (
                "execution_plans",
                "fingerprint",
                &plan.fingerprint,
                serde_json::to_value(plan).map_err(state_error)?,
            ),
            (
                "execution_approvals",
                "authorization_id",
                &approval.authorization_id,
                serde_json::to_value(approval).map_err(state_error)?,
            ),
        ] {
            let raw: Option<String> = self
                .store
                .connection
                .query_row(
                    &format!("SELECT document_json FROM {table} WHERE {key_name} = ?1"),
                    [key],
                    |row| row.get(0),
                )
                .optional()
                .map_err(state_error)?;
            let Some(raw) = raw else {
                return Err(failure(
                    Code::StoredStateIncompatible,
                    "execution authority snapshot is missing",
                ));
            };
            if serde_json::from_str::<serde_json::Value>(&raw).map_err(state_error)? != expected {
                return Err(failure(
                    Code::StoredStateIncompatible,
                    "execution authority snapshot differs from supplied documents",
                ));
            }
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn append_recovery(
        &mut self,
        event: &RecoveryEvent,
        max_actions: u64,
        budget: u64,
    ) -> Result<()> {
        contracts::validate(Contract::ExecutionRecoveryEvent, event).map_err(state_error)?;
        let document = serde_json::to_string(event).map_err(state_error)?;
        let (count, bytes): (i64, i64) = self.store.connection.query_row(
            "SELECT COUNT(*), COALESCE(SUM(LENGTH(document_json)), 0) FROM execution_recovery_events WHERE run_id = ?1",
            [&event.run_id], |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(state_error)?;
        if count < 0
            || count as u64 >= max_actions.saturating_mul(24).saturating_add(8)
            || bytes < 0
            || (bytes as u64).saturating_add(document.len() as u64) > budget
        {
            return Err(failure(
                Code::ExecutionBoundsExceeded,
                "recovery journal count or byte budget exceeded",
            ));
        }
        self.store.connection.execute(
            "INSERT INTO execution_recovery_events (event_id, run_id, operation_id, action_id, operation, phase, document_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![event.event_id, event.run_id, event.operation_id, event.action_id, event.operation, event.phase, document],
        ).map_err(state_error)?;
        Ok(())
    }

    pub fn begin(
        &mut self,
        plan: &ExecutionPlan,
        approval: &Approval,
        run: &ExecutionRun,
    ) -> Result<()> {
        let transaction = self.store.connection.transaction().map_err(state_error)?;
        for (table, key_name, key, json) in [
            (
                "execution_plans",
                "fingerprint",
                &plan.fingerprint,
                encode(plan)?,
            ),
            (
                "execution_approvals",
                "authorization_id",
                &approval.authorization_id,
                encode(approval)?,
            ),
        ] {
            let existing: Option<String> = transaction
                .query_row(
                    &format!("SELECT document_json FROM {table} WHERE {key_name} = ?1"),
                    [key],
                    |r| r.get(0),
                )
                .optional()
                .map_err(state_error)?;
            if let Some(existing) = existing {
                if existing != json {
                    return Err(failure(
                        Code::ExecutionPlanInvalid,
                        "immutable execution evidence identity collision",
                    ));
                }
            } else if table == "execution_plans" {
                transaction
                    .execute(
                        "INSERT INTO execution_plans VALUES (?1, ?2)",
                        params![key, json],
                    )
                    .map_err(state_error)?;
            } else {
                transaction
                    .execute(
                        "INSERT INTO execution_approvals VALUES (?1, ?2, ?3)",
                        params![key, plan.fingerprint, json],
                    )
                    .map_err(state_error)?;
            }
        }
        transaction
            .execute(
                "INSERT INTO execution_runs VALUES (?1, ?2, ?3, 'validating', ?4)",
                params![
                    run.run_id,
                    run.plan_fingerprint,
                    run.authorization_id,
                    encode(run)?
                ],
            )
            .map_err(state_error)?;
        transaction.commit().map_err(state_error)
    }

    pub fn save(&mut self, run: &ExecutionRun) -> Result<()> {
        let status = serde_json::to_value(run.status).map_err(state_error)?;
        let changed = self.store.connection.execute("UPDATE execution_runs SET status = ?2, document_json = ?3 WHERE run_id = ?1 AND status = 'validating'", params![run.run_id, status.as_str(), encode(run)?]).map_err(state_error)?;
        if changed != 1 {
            return Err(failure(
                Code::ExecutionSourceStale,
                "terminal execution records are immutable",
            ));
        }
        Ok(())
    }
}

fn encode_mutation(run: &MutationRun) -> Result<String> {
    contracts::validate(Contract::ExecutionMutation, run).map_err(state_error)?;
    serde_json::to_string(run).map_err(state_error)
}

pub fn load_mutation(state: &Path, run_id: &str) -> Result<Option<MutationRun>> {
    let connection = rusqlite::Connection::open_with_flags(
        state.join("state.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(state_error)?;
    read_mutation(&connection, run_id)
}

pub(super) fn read_mutation(
    connection: &rusqlite::Connection,
    run_id: &str,
) -> Result<Option<MutationRun>> {
    let record: Option<(String, String, String, String)> = connection.query_row(
        "SELECT plan_fingerprint, authorization_id, status, document_json FROM execution_mutation_runs WHERE run_id = ?1",
        [run_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional().map_err(state_error)?;
    record
        .map(|(plan, approval, status, document)| {
            let raw: serde_json::Value = serde_json::from_str(&document).map_err(state_error)?;
            contracts::validate(Contract::ExecutionMutation, &raw).map_err(state_error)?;
            let run: MutationRun = serde_json::from_str(&document).map_err(state_error)?;
            if run.run_id != run_id
                || run.plan_fingerprint != plan
                || run.authorization_id != approval
                || run.committed_actions
                    != run
                        .attempts
                        .iter()
                        .filter(|attempt| attempt.committed)
                        .count() as u64
                || serde_json::to_value(run.status)
                    .map_err(state_error)?
                    .as_str()
                    != Some(status.as_str())
            {
                return Err(failure(
                    Code::StoredStateIncompatible,
                    "mutation row and evidence disagree",
                ));
            }
            Ok(run)
        })
        .transpose()
}

pub(super) fn read_recovery_events(
    connection: &rusqlite::Connection,
    run_id: &str,
) -> Result<Vec<RecoveryEvent>> {
    let version: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(state_error)?;
    if version < 8 {
        return Ok(Vec::new());
    }
    if version > 9 {
        return Err(failure(
            Code::StoredStateIncompatible,
            "state was migrated by a newer recovery implementation",
        ));
    }
    let mut statement = connection.prepare(
        "SELECT event_id, operation_id, action_id, operation, phase, document_json FROM execution_recovery_events WHERE run_id = ?1 ORDER BY sequence LIMIT 24009"
    ).map_err(state_error)?;
    let rows = statement
        .query_map([run_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(state_error)?;
    let mut events = Vec::new();
    for row in rows {
        let (event_id, operation_id, action_id, operation, phase, document) =
            row.map_err(state_error)?;
        let raw: serde_json::Value = serde_json::from_str(&document).map_err(state_error)?;
        contracts::validate(Contract::ExecutionRecoveryEvent, &raw).map_err(state_error)?;
        let event: RecoveryEvent = serde_json::from_str(&document).map_err(state_error)?;
        if event.event_id != event_id
            || event.operation_id != operation_id
            || event.action_id != action_id
            || event.operation != operation
            || event.phase != phase
            || event.run_id != run_id
        {
            return Err(failure(
                Code::StoredStateIncompatible,
                "recovery row and evidence disagree",
            ));
        }
        events.push(event);
    }
    if events.len() > 24008 {
        return Err(failure(
            Code::ExecutionBoundsExceeded,
            "recovery event limit exceeded",
        ));
    }
    Ok(events)
}

pub(super) fn read_finalization_events(
    connection: &rusqlite::Connection,
    run_id: &str,
) -> Result<Vec<FinalizationEvent>> {
    let version: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .map_err(state_error)?;
    if version < 9 {
        return Ok(Vec::new());
    }
    if version > 9 {
        return Err(failure(
            Code::StoredStateIncompatible,
            "future finalization state version",
        ));
    }
    let mut query = connection.prepare("SELECT event_id, action_id, phase, preview_fingerprint, authorization_id, document_json FROM execution_finalization_events WHERE run_id = ?1 ORDER BY sequence LIMIT 2001").map_err(state_error)?;
    let rows = query
        .query_map([run_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(state_error)?;
    let mut events = Vec::new();
    for row in rows {
        let (id, action, phase, preview, authorization, json) = row.map_err(state_error)?;
        let raw: serde_json::Value = serde_json::from_str(&json).map_err(state_error)?;
        contracts::validate(Contract::ExecutionFinalizationEvent, &raw).map_err(state_error)?;
        let event: FinalizationEvent = serde_json::from_str(&json).map_err(state_error)?;
        if event.event_id != id
            || event.run_id != run_id
            || event.action_id != action
            || event.phase != phase
            || event.preview_fingerprint != preview
            || event.authorization_id != authorization
        {
            return Err(failure(
                Code::StoredStateIncompatible,
                "finalization row and document disagree",
            ));
        }
        events.push(event);
    }
    if events.len() > 2000 {
        return Err(failure(
            Code::ExecutionBoundsExceeded,
            "finalization event limit exceeded",
        ));
    }
    Ok(events)
}

pub(super) fn read_stored_plan(
    connection: &rusqlite::Connection,
    fingerprint: &str,
) -> Result<ExecutionPlan> {
    let document: String = connection
        .query_row(
            "SELECT document_json FROM execution_plans WHERE fingerprint = ?1",
            [fingerprint],
            |row| row.get(0),
        )
        .map_err(state_error)?;
    let raw: serde_json::Value = serde_json::from_str(&document).map_err(state_error)?;
    contracts::validate(Contract::Execution, &raw).map_err(state_error)?;
    let plan: ExecutionPlan = serde_json::from_str(&document).map_err(state_error)?;
    super::validation::validate_plan(&plan)?;
    if plan.fingerprint != fingerprint {
        return Err(failure(
            Code::StoredStateIncompatible,
            "stored plan identity differs",
        ));
    }
    Ok(plan)
}

pub fn load_execution(state: &Path, run_id: &str) -> Result<Option<ExecutionRun>> {
    let connection = rusqlite::Connection::open_with_flags(
        state.join("state.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(state_error)?;
    read_run(&connection, run_id)
}

fn read_run(connection: &rusqlite::Connection, run_id: &str) -> Result<Option<ExecutionRun>> {
    let record: Option<(String, String, String, String)> = connection
        .query_row("SELECT plan_fingerprint, authorization_id, status, document_json FROM execution_runs WHERE run_id = ?1", [run_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional().map_err(state_error)?;
    record
        .map(|(plan, approval, status, document)| {
            let raw: serde_json::Value = serde_json::from_str(&document).map_err(state_error)?;
            contracts::validate(Contract::Execution, &raw).map_err(state_error)?;
            let run: ExecutionRun = serde_json::from_str(&document).map_err(state_error)?;
            if run.run_id != run_id
                || run.plan_fingerprint != plan
                || run.authorization_id != approval
                || serde_json::to_value(run.status)
                    .map_err(state_error)?
                    .as_str()
                    != Some(status.as_str())
            {
                return Err(failure(
                    Code::StoredStateIncompatible,
                    "execution row and versioned evidence disagree",
                ));
            }
            Ok(run)
        })
        .transpose()
}
