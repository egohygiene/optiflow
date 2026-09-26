use std::fs::File;
use std::path::Path;

use chrono::Utc;
use rusqlite::{OptionalExtension, params};

use super::filesystem as fs;
use super::model::*;
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
            if newest > 6 {
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
        directory.sync_all().map_err(state_error)?;
        let mut journal = Self { store, _lock: lock };
        journal.recover()?;
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
