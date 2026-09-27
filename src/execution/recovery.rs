//! Operator-initiated v3 recovery evidence over immutable v2 mutation history.
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::journal::{read_mutation, read_recovery_events, read_stored_plan};
use super::model::{CapacityEvidence, ExecutionPlan};
use super::mutation::{MutationRun, Phase};
use super::{Result, failure};
use crate::domain::NativePath;
use crate::filesystem::identity::FilesystemIdentity;
use crate::outcome::DiagnosticCode as Code;

#[cfg(target_os = "linux")]
use chrono::Utc;
#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use uuid::Uuid;

#[cfg(target_os = "linux")]
use super::filesystem as fs;
#[cfg(target_os = "linux")]
use super::journal::Journal;
#[cfg(target_os = "linux")]
use super::model::{Approval, ExactAction, Topology};
#[cfg(target_os = "linux")]
use super::mutation::{self, MutationStatus};
#[cfg(target_os = "linux")]
use super::validation::validate_environment_remaining;
#[cfg(target_os = "linux")]
use crate::configuration::EffectivePolicyV1;
#[cfg(target_os = "linux")]
use crate::signals::SignalState;

pub const EVENT_SCHEMA: &str = "optiflow.execution-recovery-event.v3";
pub const REPORT_SCHEMA: &str = "optiflow.execution-recovery-report.v3";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryEvent {
    pub schema: String,
    pub event_id: String,
    pub operation_id: String,
    pub run_id: String,
    pub action_id: Option<String>,
    pub operation: String,
    pub phase: String,
    pub recorded_at: String,
    pub binary_version: String,
    pub configuration_fingerprint: String,
    pub policy_fingerprint: String,
    pub plan_fingerprint: String,
    pub authorization_id: String,
    pub source: Option<NativePath>,
    pub destination: Option<NativePath>,
    pub temporary: Option<NativePath>,
    pub observed_identity: Option<FilesystemIdentity>,
    pub properties_fingerprint: Option<String>,
    pub capacity: Vec<CapacityEvidence>,
    pub reason: String,
    pub recoverability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryAction {
    pub action_id: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryReport {
    pub schema: String,
    pub run_id: String,
    pub mutation: MutationRun,
    pub events: Vec<RecoveryEvent>,
    pub actions: Vec<RecoveryAction>,
    pub status: String,
    pub recovery_authority: String,
    pub physical_reclaimed_bytes: Option<u64>,
}

/// Inspection never opens writable state, takes a lock, or classifies in-flight
/// v2 rows. A pending phase is reported as ambiguous, including after a crash.
pub fn status(state: &Path, run_id: &str) -> Result<RecoveryReport> {
    let connection = rusqlite::Connection::open_with_flags(
        state.join("state.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| failure(Code::StateTransactionFailed, e.to_string()))?;
    let mutation = read_mutation(&connection, run_id)?
        .ok_or_else(|| failure(Code::StoredRunNotFound, "mutation run not found"))?;
    let plan = read_stored_plan(&connection, &mutation.plan_fingerprint)?;
    let events = read_recovery_events(&connection, run_id)?;
    report(&plan, mutation, events)
}

fn report(
    plan: &ExecutionPlan,
    mutation: MutationRun,
    events: Vec<RecoveryEvent>,
) -> Result<RecoveryReport> {
    if mutation.plan_fingerprint != plan.fingerprint
        || mutation.attempts.len() > plan.body.actions.len()
    {
        return Err(failure(
            Code::StoredStateIncompatible,
            "mutation and stored plan disagree",
        ));
    }
    let context = events.first().is_some_and(|event| {
        event.operation == "apply"
            && event.phase == "authority_bound"
            && event.action_id.is_none()
            && event.plan_fingerprint == plan.fingerprint
            && event.authorization_id == mutation.authorization_id
    });
    if events.iter().any(|event| {
        event.run_id != mutation.run_id
            || event.plan_fingerprint != plan.fingerprint
            || event.authorization_id != mutation.authorization_id
            || event.action_id.as_ref().is_some_and(|id| {
                !plan
                    .body
                    .actions
                    .iter()
                    .any(|action| &action.action_id == id)
            })
    }) {
        return Err(failure(
            Code::StoredStateIncompatible,
            "recovery event does not belong to the run and plan",
        ));
    }
    let mut actions = Vec::new();
    for (index, action) in plan.body.actions.iter().enumerate() {
        let recorded = mutation.attempts.get(index);
        if recorded.is_some_and(|a| a.action_id != action.action_id) {
            return Err(failure(
                Code::StoredStateIncompatible,
                "mutation action order differs from plan",
            ));
        }
        let mut state = match recorded {
            Some(a) if a.committed && a.phase == Phase::Committed => "quarantined",
            Some(a) if matches!(a.phase, Phase::Prepared | Phase::NamespaceDurable) => {
                "ready_to_resume"
            }
            Some(_) => "ambiguous",
            None => "not_started",
        };
        for event in events
            .iter()
            .filter(|e| e.action_id.as_deref() == Some(&action.action_id))
        {
            if event.plan_fingerprint != plan.fingerprint
                || event.authorization_id != mutation.authorization_id
            {
                return Err(failure(
                    Code::StoredStateIncompatible,
                    "recovery event authority differs",
                ));
            }
            state = match (event.operation.as_str(), event.phase.as_str()) {
                ("apply" | "resume", "committed") => "quarantined",
                ("restore", "restored") => "restored",
                ("restore", "restored_retained") => "restored_retained",
                _ => "ambiguous",
            };
        }
        if matches!(state, "quarantined" | "restored" | "restored_retained") {
            let committed = events.iter().rev().find(|e| {
                e.action_id.as_deref() == Some(&action.action_id)
                    && e.phase == "committed"
                    && matches!(e.operation.as_str(), "apply" | "resume")
            });
            if !committed.is_some_and(|e| {
                e.properties_fingerprint.is_some()
                    && (e.operation == "resume" || recorded.is_some_and(|a| a.committed))
            }) {
                state = "ambiguous";
            }
        }
        actions.push(RecoveryAction {
            action_id: action.action_id.clone(),
            state: state.to_owned(),
        });
    }
    let cleaned = events
        .last()
        .is_some_and(|e| e.operation == "cleanup" && e.phase == "cleaned");
    let status = if !context
        || actions.iter().any(|a| a.state == "ambiguous")
        || events
            .last()
            .is_some_and(|e| e.operation == "cleanup" && e.phase != "cleaned")
    {
        "attention_required"
    } else if cleaned {
        "cleaned"
    } else if actions
        .iter()
        .all(|a| matches!(a.state.as_str(), "restored" | "restored_retained"))
    {
        "restored"
    } else if actions.iter().all(|a| a.state == "quarantined") {
        "quarantined"
    } else {
        "resumable"
    };
    let report = RecoveryReport {
        schema: REPORT_SCHEMA.to_owned(),
        run_id: mutation.run_id.clone(),
        mutation,
        events,
        actions,
        status: status.to_owned(),
        recovery_authority: if context {
            "bound_v3_context"
        } else {
            "manual_inspection_only"
        }
        .to_owned(),
        physical_reclaimed_bytes: None,
    };
    crate::contracts::validate(crate::contracts::Contract::ExecutionRecoveryReport, &report)
        .map_err(|e| failure(Code::StoredStateIncompatible, e.to_string()))?;
    Ok(report)
}

#[cfg(target_os = "linux")]
#[expect(
    clippy::too_many_arguments,
    reason = "transition evidence binds every authority, path and capacity field explicitly"
)]
fn event(
    run: &MutationRun,
    plan: &ExecutionPlan,
    approval: &Approval,
    policy: &EffectivePolicyV1,
    operation_id: &str,
    operation: &str,
    phase: &str,
    action: Option<&ExactAction>,
    temporary: Option<NativePath>,
    observed_identity: Option<FilesystemIdentity>,
    capacity: Vec<CapacityEvidence>,
    reason: &str,
    recoverability: &str,
) -> RecoveryEvent {
    RecoveryEvent {
        schema: EVENT_SCHEMA.to_owned(),
        event_id: Uuid::now_v7().to_string(),
        operation_id: operation_id.to_owned(),
        run_id: run.run_id.clone(),
        action_id: action.map(|a| a.action_id.clone()),
        operation: operation.to_owned(),
        phase: phase.to_owned(),
        recorded_at: Utc::now().to_rfc3339(),
        binary_version: env!("CARGO_PKG_VERSION").to_owned(),
        configuration_fingerprint: policy.fingerprints.effective_configuration.value.clone(),
        policy_fingerprint: policy.fingerprints.evidence_policy.value.clone(),
        plan_fingerprint: plan.fingerprint.clone(),
        authorization_id: approval.authorization_id.clone(),
        source: action.map(|a| a.candidate.path.clone()),
        destination: action
            .map(|a| NativePath::from_path(&run.namespace.to_path_buf().join(&a.action_id))),
        temporary,
        observed_identity,
        properties_fingerprint: None,
        capacity,
        reason: reason.to_owned(),
        recoverability: recoverability.to_owned(),
    }
}

#[cfg(target_os = "linux")]
#[expect(
    clippy::too_many_arguments,
    reason = "durable event emission keeps the full transition context explicit"
)]
fn record(
    journal: &mut Journal,
    run: &MutationRun,
    plan: &ExecutionPlan,
    approval: &Approval,
    policy: &EffectivePolicyV1,
    operation_id: &str,
    operation: &str,
    phase: &str,
    action: Option<&ExactAction>,
    temporary: Option<NativePath>,
    observed_identity: Option<FilesystemIdentity>,
    capacity: Vec<CapacityEvidence>,
    reason: &str,
    recoverability: &str,
) -> Result<()> {
    let mut evidence = event(
        run,
        plan,
        approval,
        policy,
        operation_id,
        operation,
        phase,
        action,
        temporary,
        observed_identity,
        capacity,
        reason,
        recoverability,
    );
    if phase == "committed" {
        let action =
            action.ok_or_else(|| failure(Code::StoredStateIncompatible, "commit has no action"))?;
        let file = fs::open(&run.namespace.to_path_buf().join(&action.action_id), false)?;
        evidence.properties_fingerprint = Some(properties_fingerprint(&file)?);
    }
    journal.append_recovery(
        &evidence,
        plan.body.bounds.max_actions,
        plan.body.bounds.journal_budget_bytes,
    )
}

#[cfg(target_os = "linux")]
fn properties_fingerprint(file: &File) -> Result<String> {
    let p = mutation::properties(file)?;
    // Access time is deliberately excluded: reading for hash/byte verification
    // can advance it. Ownership, mode, mtime and every bounded xattr are bound.
    let bytes = serde_json::to_vec(&serde_json::json!({
        "uid": p.uid, "gid": p.gid, "mode": p.mode,
        "mtime": p.mtime, "xattrs": p.xattrs,
    }))
    .map_err(|e| failure(Code::StateTransactionFailed, e.to_string()))?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

#[cfg(target_os = "linux")]
pub(super) fn bind_committed(
    journal: &mut Journal,
    run: &MutationRun,
    plan: &ExecutionPlan,
    approval: &Approval,
    policy: &EffectivePolicyV1,
    action: &ExactAction,
) -> Result<()> {
    record(
        journal,
        run,
        plan,
        approval,
        policy,
        &run.run_id,
        "apply",
        "committed",
        Some(action),
        None,
        None,
        Vec::new(),
        "verified durable destination properties bound after v2 commit",
        "quarantine_retained",
    )
}

/// New applies bind the full effective configuration before the first source
/// mutation. Historical v2 rows without this event remain inspection-only.
#[cfg(target_os = "linux")]
pub(super) fn bind_apply(
    journal: &mut Journal,
    run: &MutationRun,
    plan: &ExecutionPlan,
    approval: &Approval,
    policy: &EffectivePolicyV1,
) -> Result<()> {
    record(
        journal,
        run,
        plan,
        approval,
        policy,
        &run.run_id,
        "apply",
        "authority_bound",
        None,
        None,
        None,
        Vec::new(),
        "approved exact plan and current configuration bound before mutation",
        "revalidate_before_retry",
    )
}

#[cfg(target_os = "linux")]
fn locked(
    plan: &ExecutionPlan,
    approval: &Approval,
    state: &Path,
    policy: &EffectivePolicyV1,
    run_id: &str,
) -> Result<(Journal, MutationRun, Vec<RecoveryEvent>)> {
    mutation::check_authority(plan, approval, state, policy)?;
    let journal = Journal::open(plan)?;
    journal.stored_authority(plan, approval)?;
    let run = journal
        .mutation(run_id)?
        .ok_or_else(|| failure(Code::StoredRunNotFound, "mutation run not found"))?;
    if run.plan_fingerprint != plan.fingerprint || run.authorization_id != approval.authorization_id
    {
        return Err(failure(
            Code::ExecutionApprovalMismatch,
            "run belongs to a different approval or plan",
        ));
    }
    let events = journal.recovery_events(run_id)?;
    let Some(context) = events.first() else {
        return Err(failure(
            Code::ExecutionUnsupported,
            "historical v2 mutation lacks a pre-mutation v3 configuration binding; inspect manually",
        ));
    };
    if context.operation != "apply"
        || context.phase != "authority_bound"
        || context.run_id != run.run_id
        || context.action_id.is_some()
        || context.configuration_fingerprint != policy.fingerprints.effective_configuration.value
        || context.policy_fingerprint != policy.fingerprints.evidence_policy.value
        || context.binary_version != env!("CARGO_PKG_VERSION")
        || events.iter().any(|e| {
            e.configuration_fingerprint != context.configuration_fingerprint
                || e.policy_fingerprint != context.policy_fingerprint
                || e.binary_version != context.binary_version
        })
    {
        return Err(failure(
            Code::ExecutionPolicyMismatch,
            "recovery binary, configuration, or policy differs from bound apply",
        ));
    }
    Ok((journal, run, events))
}

#[cfg(target_os = "linux")]
fn identity(file: &File) -> Result<FilesystemIdentity> {
    crate::filesystem::identity::FileStateSignature::from_file_metadata(&fs::metadata(file)?)
        .identity
        .ok_or_else(|| {
            failure(
                Code::ExecutionAmbiguousIdentity,
                "recovery object identity unavailable",
            )
        })
}

#[cfg(target_os = "linux")]
fn absent(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(failure(
            Code::ExecutionDestinationOccupied,
            "recovery destination is occupied",
        )),
        Err(e) => Err(failure(Code::ExecutionSourceUnavailable, e.to_string())),
    }
}

#[cfg(target_os = "linux")]
fn journal_error(error: std::io::Error) -> Box<crate::outcome::Diagnostic> {
    failure(
        Code::StateTransactionFailed,
        format!("recovery filesystem synchronization failed: {error}"),
    )
}

#[cfg(target_os = "linux")]
fn compare(left: &File, right: &File, size: u64, signals: &SignalState) -> Result<()> {
    use std::io::Seek;
    let mut left = left.try_clone().map_err(journal_error)?;
    let mut right = right.try_clone().map_err(journal_error)?;
    left.rewind().map_err(journal_error)?;
    right.rewind().map_err(journal_error)?;
    let mut remaining = size;
    let mut a = vec![0u8; 1024 * 1024];
    let mut b = vec![0u8; a.len()];
    while remaining > 0 {
        if signals.is_cancelled() {
            return Err(fs::interrupted(signals));
        }
        let count = remaining.min(a.len() as u64) as usize;
        left.read_exact(&mut a[..count]).map_err(journal_error)?;
        right.read_exact(&mut b[..count]).map_err(journal_error)?;
        if a[..count] != b[..count] {
            return Err(failure(
                Code::ExecutionSourceStale,
                "recovery direct byte comparison differs",
            ));
        }
        remaining -= count as u64;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn verify_content(
    action: &ExactAction,
    file: &mut File,
    signals: &SignalState,
    require_original_identity: bool,
) -> Result<FilesystemIdentity> {
    use std::os::unix::fs::MetadataExt;
    let before = identity(file)?;
    let metadata = fs::metadata(file)?;
    let p = mutation::properties(file)?;
    if !metadata.is_file()
        || before.link_count != Some(1)
        || (require_original_identity
            && before.identity_key() != action.candidate.identity.identity_key())
        || metadata.len() != action.candidate.size_bytes
        || p.mode != action.candidate.mode
        || p.uid != action.candidate.uid
        || p.gid != action.candidate.gid
        || metadata.mtime() as i128 * 1_000_000_000 + metadata.mtime_nsec() as i128
            != action.candidate.modified_unix_ns as i128
        || fs::hash_bound(file, action.candidate.size_bytes, signals)? != action.candidate.blake3
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "recovery object identity, properties or hash differs",
        ));
    }
    let mut keeper = fs::open_bound(&action.keeper)?;
    if fs::hash_bound(&mut keeper, action.keeper.size_bytes, signals)? != action.keeper.blake3 {
        return Err(failure(
            Code::ExecutionSourceStale,
            "keeper changed before recovery",
        ));
    }
    compare(file, &keeper, action.candidate.size_bytes, signals)?;
    if identity(file)?.identity_key() != before.identity_key() {
        return Err(failure(
            Code::ExecutionSourceStale,
            "recovery object changed during comparison",
        ));
    }
    fs::check_file(&keeper, &action.keeper)?;
    Ok(before)
}

#[cfg(target_os = "linux")]
fn verify_quarantined(
    plan: &ExecutionPlan,
    run: &MutationRun,
    action: &ExactAction,
    report: Option<&RecoveryReport>,
    signals: &SignalState,
) -> Result<FilesystemIdentity> {
    fs::check_directory(&plan.body.quarantine)?;
    let _namespace = fs::open(&run.namespace.to_path_buf(), true)?;
    absent(&action.candidate.path.to_path_buf())?;
    let path = run.namespace.to_path_buf().join(&action.action_id);
    let mut file = fs::open(&path, false)?;
    let identity = verify_content(
        action,
        &mut file,
        signals,
        action.topology == Topology::SameFilesystem,
    )?;
    if let Some(report) = report {
        check_properties(action, &file, report)?;
    }
    let current = fs::open(&path, false)?;
    if self::identity(&current)?.identity_key() != identity.identity_key() {
        return Err(failure(
            Code::ExecutionSourceStale,
            "quarantine object was replaced",
        ));
    }
    Ok(identity)
}

#[cfg(target_os = "linux")]
fn check_properties(action: &ExactAction, file: &File, report: &RecoveryReport) -> Result<()> {
    let expected = report
        .events
        .iter()
        .rev()
        .find(|event| {
            event.action_id.as_deref() == Some(&action.action_id)
                && event.phase == "committed"
                && matches!(event.operation.as_str(), "apply" | "resume")
        })
        .and_then(|event| event.properties_fingerprint.as_deref())
        .ok_or_else(|| {
            failure(
                Code::StoredStateIncompatible,
                "committed property evidence is missing",
            )
        })?;
    if properties_fingerprint(file)? != expected {
        return Err(failure(
            Code::ExecutionSourceStale,
            "ownership, mode, mtime or extended attributes differ from committed evidence",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn expected_namespace(
    plan: &ExecutionPlan,
    run: &MutationRun,
    report: &RecoveryReport,
) -> Result<()> {
    let namespace = run.namespace.to_path_buf();
    if namespace != fs::native_path(&plan.body.quarantine.path)?.join(&plan.fingerprint) {
        return Err(failure(
            Code::StoredStateIncompatible,
            "quarantine namespace differs from plan",
        ));
    }
    let _directory = fs::open(&namespace, true)?;
    let expected: std::collections::BTreeSet<_> = report
        .actions
        .iter()
        .filter(|a| matches!(a.state.as_str(), "quarantined" | "restored_retained"))
        .map(|a| a.action_id.as_str())
        .collect();
    for entry in std::fs::read_dir(&namespace).map_err(journal_error)? {
        let name = entry.map_err(journal_error)?.file_name();
        if !expected.contains(name.to_str().unwrap_or("")) {
            return Err(failure(
                Code::ExecutionSourceStale,
                "quarantine namespace has an unowned or ambiguous entry",
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn resume(
    plan: &ExecutionPlan,
    approval: &Approval,
    state: &Path,
    policy: &EffectivePolicyV1,
    run_id: &str,
    signals: &SignalState,
) -> Result<RecoveryReport> {
    let (mut journal, run, mut events) = locked(plan, approval, state, policy, run_id)?;
    if run.status == MutationStatus::Rejected {
        return Err(failure(
            Code::ExecutionUnsupported,
            "rejected mutation has no resumable action",
        ));
    }
    let mut current = report(plan, run.clone(), events.clone())?;
    if current.status == "attention_required"
        || current.status == "cleaned"
        || current
            .actions
            .iter()
            .any(|a| matches!(a.state.as_str(), "restored" | "restored_retained"))
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "ambiguous or restored work cannot be resumed",
        ));
    }
    expected_namespace(plan, &run, &current)?;
    for (index, action) in plan.body.actions.iter().enumerate() {
        match current.actions[index].state.as_str() {
            "quarantined" => {
                verify_quarantined(plan, &run, action, Some(&current), signals)?;
                continue;
            }
            "not_started" | "ready_to_resume" => (),
            _ => {
                return Err(failure(
                    Code::ExecutionSourceStale,
                    "action is not safely resumable",
                ));
            }
        }
        absent(&run.namespace.to_path_buf().join(&action.action_id))?;
        absent(
            &run.namespace
                .to_path_buf()
                .join(format!(".{}.part", action.action_id)),
        )?;
        if signals.is_cancelled() {
            return Err(fs::interrupted(signals));
        }
        let capacity = validate_environment_remaining(plan, &mut fs::capacity, true, index)?;
        // The v2 attempt may be 'prepared' or absent, but current source,
        // keeper and topology are checked again inside perform_one.
        let operation_id = Uuid::now_v7().to_string();
        let temporary = Some(NativePath::from_path(
            &run.namespace
                .to_path_buf()
                .join(format!(".{}.part", action.action_id)),
        ));
        record(
            &mut journal,
            &run,
            plan,
            approval,
            policy,
            &operation_id,
            "resume",
            "prepared",
            Some(action),
            temporary.clone(),
            None,
            capacity.clone(),
            "operator requested remaining approved action",
            "inspect_only",
        )?;
        mutation::perform_one(
            plan,
            action,
            index,
            &run.namespace.to_path_buf(),
            signals,
            &mut |phase| {
                let name = serde_json::to_value(phase).expect("phase serialization");
                let identity =
                    if phase == Phase::DestinationDurable || phase == Phase::SourceRemoved {
                        let destination =
                            fs::open(&run.namespace.to_path_buf().join(&action.action_id), false)?;
                        Some(identity(&destination)?)
                    } else {
                        None
                    };
                record(
                    &mut journal,
                    &run,
                    plan,
                    approval,
                    policy,
                    &operation_id,
                    "resume",
                    name.as_str().expect("phase string"),
                    Some(action),
                    temporary.clone(),
                    identity,
                    capacity.clone(),
                    "checked recovery transition",
                    "inspect_only",
                )
            },
        )?;
        let observed = verify_quarantined(plan, &run, action, None, signals)?;
        record(
            &mut journal,
            &run,
            plan,
            approval,
            policy,
            &operation_id,
            "resume",
            "committed",
            Some(action),
            temporary,
            Some(observed),
            capacity,
            "destination durable, source absent and bytes verified",
            "quarantine_retained",
        )?;
        events = journal.recovery_events(run_id)?;
        current = report(plan, run.clone(), events.clone())?;
        expected_namespace(plan, &run, &current)?;
    }
    report(plan, run, events)
}

#[cfg(not(target_os = "linux"))]
pub fn resume(
    _plan: &super::model::ExecutionPlan,
    _approval: &super::model::Approval,
    _state: &Path,
    _policy: &crate::configuration::EffectivePolicyV1,
    _run_id: &str,
    _signals: &crate::signals::SignalState,
) -> Result<RecoveryReport> {
    Err(failure(
        Code::ExecutionUnsupported,
        "quarantine recovery requires Linux",
    ))
}

#[cfg(target_os = "linux")]
fn restored_source(
    action: &ExactAction,
    run: &MutationRun,
    report: &RecoveryReport,
    signals: &SignalState,
) -> Result<FilesystemIdentity> {
    let path = action.candidate.path.to_path_buf();
    let mut file = fs::open(&path, false)?;
    let original = action.topology == Topology::SameFilesystem;
    let observed = verify_content(action, &mut file, signals, original)?;
    check_properties(action, &file, report)?;
    let reopened = fs::open(&path, false)?;
    if identity(&reopened)?.identity_key() != observed.identity_key() {
        return Err(failure(
            Code::ExecutionSourceStale,
            "restored source changed",
        ));
    }
    let destination = run.namespace.to_path_buf().join(&action.action_id);
    if original {
        absent(&destination)?;
    } else {
        let mut retained = fs::open(&destination, false)?;
        verify_content(action, &mut retained, signals, false)?;
        check_properties(action, &retained, report)?;
        compare(&file, &retained, action.candidate.size_bytes, signals)?;
    }
    Ok(observed)
}

#[cfg(target_os = "linux")]
fn restore_capacity(
    plan: &ExecutionPlan,
    action: &ExactAction,
    copy_bytes: bool,
) -> Result<Vec<CapacityEvidence>> {
    restore_capacity_with(plan, action, copy_bytes, &mut fs::capacity)
}

#[cfg(target_os = "linux")]
fn restore_capacity_with<F>(
    plan: &ExecutionPlan,
    action: &ExactAction,
    copy_bytes: bool,
    probe: &mut F,
) -> Result<Vec<CapacityEvidence>>
where
    F: FnMut(&File) -> Result<fs::Capacity>,
{
    let capacity = validate_environment_remaining(plan, probe, true, plan.body.actions.len())?;
    let parent = fs::check_directory(&action.candidate.directory)?;
    fs::writable_directory(&parent)?;
    let measured = probe(&parent)?;
    let additional = if copy_bytes {
        action.candidate.size_bytes
    } else {
        0
    };
    let required = plan
        .body
        .bounds
        .free_space_reserve_bytes
        .checked_add(plan.body.bounds.journal_budget_bytes)
        .and_then(|n| n.checked_add(additional))
        .ok_or_else(|| {
            failure(
                Code::ExecutionBoundsExceeded,
                "restore capacity bound overflow",
            )
        })?;
    if measured.read_only || measured.available < required {
        return Err(failure(
            Code::ExecutionCapacityInsufficient,
            "restore source filesystem lacks capacity or reserve",
        ));
    }
    Ok(capacity)
}

#[cfg(target_os = "linux")]
#[expect(
    clippy::too_many_arguments,
    reason = "reverse copy receives its prevalidated bounded recovery context"
)]
fn restore_cross(
    plan: &ExecutionPlan,
    run: &MutationRun,
    action: &ExactAction,
    current: &RecoveryReport,
    destination: &File,
    signals: &SignalState,
    journal: &mut Journal,
    approval: &Approval,
    policy: &EffectivePolicyV1,
    operation_id: &str,
    initial: Vec<CapacityEvidence>,
    sync_output: &mut impl FnMut(&File) -> Result<()>,
) -> Result<()> {
    use rustix::fs::{Mode, OFlags, openat, renameat_with};
    let parent = fs::check_directory(&action.candidate.directory)?;
    let source = action.candidate.path.to_path_buf();
    let source_name = source
        .file_name()
        .ok_or_else(|| failure(Code::ExecutionScopeInvalid, "source has no filename"))?;
    let temp_name = format!(".optiflow-{}-{}.restore.part", run.run_id, action.action_id);
    let temporary_path = source
        .parent()
        .expect("bound source directory")
        .join(&temp_name);
    let temporary = Some(NativePath::from_path(&temporary_path));
    absent(&temporary_path)?;
    record(
        journal,
        run,
        plan,
        approval,
        policy,
        operation_id,
        "restore",
        "restore_pending",
        Some(action),
        temporary.clone(),
        Some(identity(destination)?),
        initial,
        "exclusive reverse copy requested",
        "inspect_only",
    )?;
    let output = openat(
        &parent,
        temp_name.as_str(),
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|e| {
        failure(
            Code::ExecutionDestinationOccupied,
            format!("restore temp refused: {e}"),
        )
    })?;
    let mut output = File::from(output);
    let mut input = destination.try_clone().map_err(journal_error)?;
    use std::io::Seek;
    input.rewind().map_err(journal_error)?;
    let mut remaining = action.candidate.size_bytes;
    let mut buffer = vec![0u8; 1024 * 1024];
    while remaining > 0 {
        if signals.is_cancelled() {
            return Err(fs::interrupted(signals));
        }
        let count = remaining.min(buffer.len() as u64) as usize;
        input
            .read_exact(&mut buffer[..count])
            .map_err(journal_error)?;
        output.write_all(&buffer[..count]).map_err(journal_error)?;
        remaining -= count as u64;
    }
    let destination_properties = mutation::properties(destination)?;
    mutation::set_properties(&output, &destination_properties)?;
    sync_output(&output)?;
    verify_content(action, &mut output, signals, false)?;
    check_properties(action, &output, current)?;
    compare(&output, destination, action.candidate.size_bytes, signals)?;
    mutation::set_properties(&output, &mutation::properties(destination)?)?;
    if mutation::properties(&output)? != mutation::properties(destination)? {
        return Err(failure(
            Code::ExecutionUnsupported,
            "reverse copy properties differ",
        ));
    }
    sync_output(&output)?;
    parent.sync_all().map_err(journal_error)?;
    record(
        journal,
        run,
        plan,
        approval,
        policy,
        operation_id,
        "restore",
        "temp_synced",
        Some(action),
        temporary.clone(),
        Some(identity(&output)?),
        Vec::new(),
        "reverse copy synchronized and compared",
        "inspect_only",
    )?;
    restore_capacity(plan, action, false)?;
    absent(&source)?;
    let reopened = fs::open(&run.namespace.to_path_buf().join(&action.action_id), false)?;
    if identity(&reopened)?.identity_key() != identity(destination)?.identity_key() {
        return Err(failure(
            Code::ExecutionSourceStale,
            "quarantine object replaced before restore commit",
        ));
    }
    verify_content(action, &mut input, signals, false)?;
    compare(&output, &input, action.candidate.size_bytes, signals)?;
    record(
        journal,
        run,
        plan,
        approval,
        policy,
        operation_id,
        "restore",
        "source_pending",
        Some(action),
        temporary.clone(),
        Some(identity(&output)?),
        Vec::new(),
        "verified temp will be committed no-replace",
        "inspect_only",
    )?;
    renameat_with(
        &parent,
        temp_name.as_str(),
        &parent,
        source_name,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|e| {
        failure(
            Code::ExecutionDestinationOccupied,
            format!("restore no-replace refused: {e}"),
        )
    })?;
    parent.sync_all().map_err(journal_error)?;
    let observed = restored_source(action, run, current, signals)?;
    if observed.identity_key() != identity(&output)?.identity_key() {
        return Err(failure(
            Code::ExecutionSourceStale,
            "restored object differs from verified temp",
        ));
    }
    record(
        journal,
        run,
        plan,
        approval,
        policy,
        operation_id,
        "restore",
        "restored_retained",
        Some(action),
        temporary,
        Some(observed),
        Vec::new(),
        "source restored; verified quarantine copy deliberately retained",
        "quarantine_copy_retained",
    )
}

#[cfg(target_os = "linux")]
pub fn restore(
    plan: &ExecutionPlan,
    approval: &Approval,
    state: &Path,
    policy: &EffectivePolicyV1,
    run_id: &str,
    action_id: &str,
    signals: &SignalState,
) -> Result<RecoveryReport> {
    use rustix::fs::renameat_with;
    let (mut journal, run, events) = locked(plan, approval, state, policy, run_id)?;
    let current = report(plan, run.clone(), events)?;
    if current.status == "attention_required" || current.status == "cleaned" {
        return Err(failure(
            Code::ExecutionSourceStale,
            "ambiguous or cleaned work requires inspection",
        ));
    }
    let index = plan
        .body
        .actions
        .iter()
        .position(|a| a.action_id == action_id)
        .ok_or_else(|| failure(Code::ExecutionPlanInvalid, "action not in approved plan"))?;
    let action = &plan.body.actions[index];
    if matches!(
        current.actions[index].state.as_str(),
        "restored" | "restored_retained"
    ) {
        let observed = restored_source(action, &run, &current, signals)?;
        let last = current
            .events
            .iter()
            .rev()
            .find(|e| e.action_id.as_deref() == Some(action_id))
            .ok_or_else(|| failure(Code::StoredStateIncompatible, "restore evidence missing"))?;
        if last.observed_identity.as_ref().map(|id| id.identity_key())
            != Some(observed.identity_key())
        {
            return Err(failure(
                Code::ExecutionSourceStale,
                "restored object changed after recorded completion",
            ));
        }
        return Ok(current);
    }
    if current.actions[index].state != "quarantined" {
        return Err(failure(
            Code::ExecutionSourceStale,
            "only committed quarantine actions can be restored",
        ));
    }
    expected_namespace(plan, &run, &current)?;
    let observed = verify_quarantined(plan, &run, action, Some(&current), signals)?;
    let capacity = restore_capacity(plan, action, action.topology == Topology::CrossFilesystem)?;
    let namespace = fs::open(&run.namespace.to_path_buf(), true)?;
    let parent = fs::check_directory(&action.candidate.directory)?;
    fs::writable_directory(&parent)?;
    let source = action.candidate.path.to_path_buf();
    absent(&source)?;
    let destination = fs::open(&run.namespace.to_path_buf().join(action_id), false)?;
    if identity(&destination)?.identity_key() != observed.identity_key() {
        return Err(failure(
            Code::ExecutionSourceStale,
            "quarantine object changed before restore",
        ));
    }
    let operation_id = Uuid::now_v7().to_string();
    if action.topology == Topology::SameFilesystem {
        let source_name = source
            .file_name()
            .ok_or_else(|| failure(Code::ExecutionScopeInvalid, "source has no filename"))?;
        record(
            &mut journal,
            &run,
            plan,
            approval,
            policy,
            &operation_id,
            "restore",
            "restore_pending",
            Some(action),
            None,
            Some(observed.clone()),
            capacity,
            "verified no-replace return move requested",
            "inspect_only",
        )?;
        renameat_with(
            &namespace,
            action_id,
            &parent,
            source_name,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|e| {
            failure(
                Code::ExecutionDestinationOccupied,
                format!("restore no-replace refused: {e}"),
            )
        })?;
        parent
            .sync_all()
            .and_then(|_| namespace.sync_all())
            .map_err(journal_error)?;
        let restored = restored_source(action, &run, &current, signals)?;
        if restored.identity_key() != observed.identity_key() {
            return Err(failure(
                Code::ExecutionSourceStale,
                "restored object changed identity",
            ));
        }
        record(
            &mut journal,
            &run,
            plan,
            approval,
            policy,
            &operation_id,
            "restore",
            "restored",
            Some(action),
            None,
            Some(restored),
            Vec::new(),
            "original source identity and content restored",
            "source_restored",
        )?;
    } else {
        restore_cross(
            plan,
            &run,
            action,
            &current,
            &destination,
            signals,
            &mut journal,
            approval,
            policy,
            &operation_id,
            capacity,
            &mut |file| file.sync_all().map_err(journal_error),
        )?;
    }
    report(plan, run, journal.recovery_events(run_id)?)
}

#[cfg(not(target_os = "linux"))]
pub fn restore(
    _plan: &super::model::ExecutionPlan,
    _approval: &super::model::Approval,
    _state: &Path,
    _policy: &crate::configuration::EffectivePolicyV1,
    _run_id: &str,
    _action_id: &str,
    _signals: &crate::signals::SignalState,
) -> Result<RecoveryReport> {
    Err(failure(
        Code::ExecutionUnsupported,
        "quarantine recovery requires Linux",
    ))
}

#[cfg(target_os = "linux")]
pub fn cleanup(
    plan: &ExecutionPlan,
    approval: &Approval,
    state: &Path,
    policy: &EffectivePolicyV1,
    run_id: &str,
    signals: &SignalState,
) -> Result<RecoveryReport> {
    use rustix::fs::{AtFlags, unlinkat};
    let (mut journal, run, events) = locked(plan, approval, state, policy, run_id)?;
    let current = report(plan, run.clone(), events)?;
    if current.status == "cleaned" {
        absent(&run.namespace.to_path_buf())?;
        return Ok(current);
    }
    if current.status != "restored" || current.actions.iter().any(|a| a.state != "restored") {
        return Err(failure(
            Code::ExecutionSourceStale,
            "cleanup requires every action restored and no retained copy",
        ));
    }
    for action in &plan.body.actions {
        restored_source(action, &run, &current, signals)?;
    }
    expected_namespace(plan, &run, &current)?;
    if std::fs::read_dir(run.namespace.to_path_buf())
        .map_err(journal_error)?
        .next()
        .is_some()
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "cleanup refuses nonempty namespace",
        ));
    }
    let capacity =
        validate_environment_remaining(plan, &mut fs::capacity, true, plan.body.actions.len())?;
    let parent = fs::check_directory(&plan.body.quarantine)?;
    fs::writable_directory(&parent)?;
    let namespace = fs::open(&run.namespace.to_path_buf(), true)?;
    let original = identity(&namespace)?;
    let operation_id = Uuid::now_v7().to_string();
    record(
        &mut journal,
        &run,
        plan,
        approval,
        policy,
        &operation_id,
        "cleanup",
        "cleanup_pending",
        None,
        None,
        Some(original.clone()),
        capacity,
        "remove only the verified empty owned namespace",
        "inspect_only",
    )?;
    if identity(&fs::open(&run.namespace.to_path_buf(), true)?)?.identity_key()
        != original.identity_key()
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "namespace identity changed before cleanup",
        ));
    }
    unlinkat(&parent, plan.fingerprint.as_str(), AtFlags::REMOVEDIR).map_err(|e| {
        failure(
            Code::ExecutionSourceStale,
            format!("empty namespace removal refused: {e}"),
        )
    })?;
    parent.sync_all().map_err(journal_error)?;
    absent(&run.namespace.to_path_buf())?;
    record(
        &mut journal,
        &run,
        plan,
        approval,
        policy,
        &operation_id,
        "cleanup",
        "cleaned",
        None,
        None,
        Some(original),
        Vec::new(),
        "empty namespace removed; no quarantined content deleted",
        "empty_namespace_removed",
    )?;
    report(plan, run, journal.recovery_events(run_id)?)
}

#[cfg(not(target_os = "linux"))]
pub fn cleanup(
    _plan: &super::model::ExecutionPlan,
    _approval: &super::model::Approval,
    _state: &Path,
    _policy: &crate::configuration::EffectivePolicyV1,
    _run_id: &str,
    _signals: &crate::signals::SignalState,
) -> Result<RecoveryReport> {
    Err(failure(
        Code::ExecutionUnsupported,
        "quarantine recovery requires Linux",
    ))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::cli::{Cli, ExecutionPlanArgs};
    use clap::Parser;

    struct Interrupted {
        _temp: tempfile::TempDir,
        plan: ExecutionPlan,
        approval: Approval,
        policy: EffectivePolicyV1,
        state: std::path::PathBuf,
        run_id: String,
    }

    fn after_first_commit() -> Interrupted {
        let temp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        for name in ["source", "state", "quarantine"] {
            std::fs::create_dir(base.join(name)).unwrap();
        }
        for name in ["keeper", "candidate-a", "candidate-b"] {
            std::fs::write(base.join("source").join(name), b"duplicate").unwrap();
        }
        let state = base.join("state");
        let cli = Cli::parse_from([
            "optiflow",
            "--no-config",
            "--state-directory",
            state.to_str().unwrap(),
            "doctor",
        ]);
        let policy = crate::configuration::resolve(&cli).unwrap().policy;
        let args = ExecutionPlanArgs {
            keep: base.join("source/keeper"),
            candidate: vec![
                base.join("source/candidate-a"),
                base.join("source/candidate-b"),
            ],
            root: vec![base.join("source")],
            subtree: vec![],
            quarantine: base.join("quarantine"),
            max_actions: 2,
            max_in_flight_bytes: 1024,
            reserve_bytes: 1,
            output: base.join("plan.json"),
        };
        let plan =
            super::super::create_plan(&args, &state, &policy, &SignalState::default()).unwrap();
        let approval =
            super::super::approve(&plan, &plan.fingerprint, "synthetic operator").unwrap();
        let namespace = args.quarantine.join(&plan.fingerprint);
        let attempt = |index: usize, phase: Phase, committed: bool| {
            let action = &plan.body.actions[index];
            super::super::mutation::MutationAttempt {
                schema: "optiflow.execution-mutation-attempt.v2".to_owned(),
                action_id: action.action_id.clone(),
                source: action.candidate.path.clone(),
                temporary: NativePath::from_path(
                    &namespace.join(format!(".{}.part", action.action_id)),
                ),
                destination: NativePath::from_path(&namespace.join(&action.action_id)),
                topology: action.topology,
                phase,
                committed,
            }
        };
        let mut run = MutationRun {
            schema: super::super::mutation::MUTATION_SCHEMA.to_owned(),
            run_id: Uuid::now_v7().to_string(),
            plan_fingerprint: plan.fingerprint.clone(),
            authorization_id: approval.authorization_id.clone(),
            started_at: Utc::now().to_rfc3339(),
            completed_at: None,
            dry_run: false,
            status: MutationStatus::Running,
            namespace: NativePath::from_path(&namespace),
            attempts: vec![attempt(0, Phase::Prepared, false)],
            committed_actions: 0,
            capacity: vec![],
            savings: super::super::model::SavingsEvidence {
                selected_logical_bytes: 18,
                immediate_logical_reclaimed_bytes: 0,
                physical_reclaimed_bytes: None,
                physical_status: "unknown".to_owned(),
                reason: "synthetic interrupted run".to_owned(),
            },
            diagnostics: vec![],
            recovery_guarantee:
                "inspect_journal_and_paths_before_manual_restore; no_automatic_resume".to_owned(),
        };
        let mut journal = Journal::open(&plan).unwrap();
        journal.begin_mutation(&plan, &approval, &run).unwrap();
        bind_apply(&mut journal, &run, &plan, &approval, &policy).unwrap();
        std::fs::create_dir(&namespace).unwrap();
        std::fs::File::open(&args.quarantine)
            .unwrap()
            .sync_all()
            .unwrap();
        run.attempts[0].phase = Phase::NamespaceDurable;
        journal.save_mutation(&run).unwrap();
        run.capacity = mutation::perform_one(
            &plan,
            &plan.body.actions[0],
            0,
            &namespace,
            &SignalState::default(),
            &mut |phase| {
                run.attempts[0].phase = phase;
                journal.save_mutation(&run)
            },
        )
        .unwrap();
        run.attempts[0].phase = Phase::Committed;
        run.attempts[0].committed = true;
        run.committed_actions = 1;
        journal.save_mutation(&run).unwrap();
        bind_committed(
            &mut journal,
            &run,
            &plan,
            &approval,
            &policy,
            &plan.body.actions[0],
        )
        .unwrap();
        run.attempts.push(attempt(1, Phase::Prepared, false));
        run.status = MutationStatus::Interrupted;
        run.completed_at = Some(Utc::now().to_rfc3339());
        journal.save_mutation(&run).unwrap();
        Interrupted {
            _temp: temp,
            plan,
            approval,
            policy,
            state,
            run_id: run.run_id,
        }
    }

    #[test]
    fn resume_commits_only_remaining_action_and_repeat_is_idempotent() {
        let f = after_first_commit();
        let before = status(&f.state, &f.run_id).unwrap();
        assert_eq!(before.status, "resumable");
        assert_eq!(before.actions[0].state, "quarantined");
        assert_eq!(before.actions[1].state, "ready_to_resume");
        let first = resume(
            &f.plan,
            &f.approval,
            &f.state,
            &f.policy,
            &f.run_id,
            &SignalState::default(),
        )
        .unwrap();
        assert_eq!(first.status, "quarantined");
        assert!(!f.plan.body.actions[1].candidate.path.to_path_buf().exists());
        let repeated = resume(
            &f.plan,
            &f.approval,
            &f.state,
            &f.policy,
            &f.run_id,
            &SignalState::default(),
        )
        .unwrap();
        assert_eq!(first.events.len(), repeated.events.len());
        assert_eq!(repeated.mutation.committed_actions, 1);
    }

    #[test]
    fn stale_remaining_source_leaves_pending_transition_ambiguous() {
        let f = after_first_commit();
        std::fs::write(
            f.plan.body.actions[1].candidate.path.to_path_buf(),
            b"changed",
        )
        .unwrap();
        assert!(
            resume(
                &f.plan,
                &f.approval,
                &f.state,
                &f.policy,
                &f.run_id,
                &SignalState::default(),
            )
            .is_err()
        );
        let after = status(&f.state, &f.run_id).unwrap();
        assert_eq!(after.status, "attention_required");
        assert_eq!(after.actions[0].state, "quarantined");
        assert_eq!(after.actions[1].state, "ambiguous");
        assert!(f.plan.body.actions[1].candidate.path.to_path_buf().exists());
    }

    #[test]
    fn capacity_consumed_between_probes_refuses_restore() {
        let f = after_first_commit();
        let action = &f.plan.body.actions[0];
        let source_directory = action.candidate.directory.identity.clone();
        let mut source_probes = 0;
        let result = restore_capacity_with(&f.plan, action, false, &mut |directory| {
            let mut measured = fs::capacity(directory)?;
            if identity(directory)?.identity_key() == source_directory.identity_key() {
                source_probes += 1;
                if source_probes > 1 {
                    measured.available = 0;
                }
            }
            Ok(measured)
        });
        assert_eq!(
            result.unwrap_err().code,
            Code::ExecutionCapacityInsufficient
        );
        assert!(
            f.plan.body.actions[0]
                .candidate
                .path
                .to_path_buf()
                .parent()
                .unwrap()
                .exists()
        );
        assert!(
            f.plan
                .body
                .quarantine
                .path
                .to_path_buf()
                .join(&f.plan.fingerprint)
                .join(&action.action_id)
                .exists()
        );
    }

    #[test]
    fn failed_reverse_copy_sync_cannot_create_a_restore_commit() {
        use std::os::unix::fs::MetadataExt;
        let alternate = Path::new("/dev/shm");
        let temp = tempfile::tempdir().unwrap();
        if !alternate.is_dir()
            || std::fs::metadata(alternate).unwrap().dev()
                == std::fs::metadata(temp.path()).unwrap().dev()
        {
            return;
        }
        let quarantine = tempfile::tempdir_in(alternate).unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        for name in ["source", "state"] {
            std::fs::create_dir(base.join(name)).unwrap();
        }
        for name in ["keeper", "candidate"] {
            std::fs::write(base.join("source").join(name), b"synthetic duplicate").unwrap();
        }
        let state = base.join("state");
        let cli = Cli::parse_from([
            "optiflow",
            "--no-config",
            "--state-directory",
            state.to_str().unwrap(),
            "doctor",
        ]);
        let policy = crate::configuration::resolve(&cli).unwrap().policy;
        let args = ExecutionPlanArgs {
            keep: base.join("source/keeper"),
            candidate: vec![base.join("source/candidate")],
            root: vec![base.join("source")],
            subtree: vec![],
            quarantine: std::fs::canonicalize(quarantine.path()).unwrap(),
            max_actions: 1,
            max_in_flight_bytes: 1024,
            reserve_bytes: 1,
            output: base.join("plan.json"),
        };
        let plan =
            super::super::create_plan(&args, &state, &policy, &SignalState::default()).unwrap();
        let approval =
            super::super::approve(&plan, &plan.fingerprint, "synthetic operator").unwrap();
        let applied =
            mutation::apply_quarantine(&plan, &approval, &state, &policy, &SignalState::default())
                .unwrap();
        let (mut journal, run, events) =
            locked(&plan, &approval, &state, &policy, &applied.run_id).unwrap();
        let current = report(&plan, run.clone(), events).unwrap();
        let action = &plan.body.actions[0];
        let destination =
            fs::open(&run.namespace.to_path_buf().join(&action.action_id), false).unwrap();
        let error = restore_cross(
            &plan,
            &run,
            action,
            &current,
            &destination,
            &SignalState::default(),
            &mut journal,
            &approval,
            &policy,
            &Uuid::now_v7().to_string(),
            Vec::new(),
            &mut |_file| {
                Err(failure(
                    Code::StateTransactionFailed,
                    "injected sync failure",
                ))
            },
        )
        .unwrap_err();
        assert_eq!(error.code, Code::StateTransactionFailed);
        let recorded = status(&state, &run.run_id).unwrap();
        assert_eq!(recorded.status, "attention_required");
        assert_eq!(recorded.actions[0].state, "ambiguous");
        assert!(run.namespace.to_path_buf().join(&action.action_id).exists());
        assert!(
            base.join("source")
                .join(format!(
                    ".optiflow-{}-{}.restore.part",
                    run.run_id, action.action_id
                ))
                .exists()
        );
        assert!(!args.candidate[0].exists());
    }
}
