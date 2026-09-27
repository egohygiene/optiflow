//! Explicit v4 irreversible finalization for verified retained cross-filesystem copies.
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::model::{Approval, ExecutionPlan};
use super::recovery::RecoveryReport;
use super::{Result, digest, failure};
use crate::configuration::EffectivePolicyV1;
use crate::contracts::{self, Contract};
use crate::filesystem::identity::FilesystemIdentity;
use crate::outcome::DiagnosticCode as Code;
use crate::signals::SignalState;

#[cfg(target_os = "linux")]
use chrono::Utc;
#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;
#[cfg(target_os = "linux")]
use uuid::Uuid;

#[cfg(target_os = "linux")]
use super::{
    filesystem as fs,
    mutation::{self, MutationStatus},
    recovery,
    validation::validate_environment_remaining,
};

pub const PREVIEW_SCHEMA: &str = "optiflow.execution-finalization-preview.v4";
pub const AUTHORIZATION_SCHEMA: &str = "optiflow.execution-finalization-authorization.v4";
pub const EVENT_SCHEMA: &str = "optiflow.execution-finalization-event.v4";
pub const STATUS_SCHEMA: &str = "optiflow.execution-finalization-status.v4";
const NOTICE: &str = "permanent_quarantine_file_removal; operator_backup_responsibility";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestEntry {
    pub action_id: String,
    pub quarantine_identity: FilesystemIdentity,
    pub survivor_identity: FilesystemIdentity,
    pub logical_bytes: u64,
    pub observed_allocated_bytes: u64,
    pub blake3: String,
    pub properties_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewBody {
    pub run_id: String,
    pub plan_fingerprint: String,
    pub execution_approval_id: String,
    pub selected_actions: Vec<String>,
    pub quarantine_manifest_digest: String,
    pub entries: Vec<ManifestEntry>,
    pub previewed_at: String,
    pub producer_version: String,
    pub configuration_fingerprint: String,
    pub policy_fingerprint: String,
    pub irreversible_notice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizationPreview {
    pub schema: String,
    pub fingerprint: String,
    pub body: PreviewBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationBody {
    pub authority: String,
    pub preview_fingerprint: String,
    pub plan_fingerprint: String,
    pub execution_run_id: String,
    pub quarantine_manifest_digest: String,
    pub selected_actions: Vec<String>,
    pub approved_by: String,
    pub approved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizationAuthorization {
    pub schema: String,
    pub authorization_id: String,
    pub body: AuthorizationBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizationEvent {
    pub schema: String,
    pub event_id: String,
    pub run_id: String,
    pub action_id: String,
    pub phase: String,
    pub preview_fingerprint: String,
    pub authorization_id: String,
    pub manifest_digest: String,
    pub recorded_at: String,
    pub binary_version: String,
    pub configuration_fingerprint: String,
    pub policy_fingerprint: String,
    pub quarantine_identity: FilesystemIdentity,
    pub survivor_identity: FilesystemIdentity,
    pub logical_bytes: u64,
    pub observed_allocated_bytes_before: u64,
    pub target_available_before: u64,
    pub target_available_after: Option<u64>,
    pub target_free_space_change_bytes: Option<i64>,
    pub shared_extent_bytes: Option<u64>,
    pub physical_reclaimed_bytes: Option<u64>,
    pub recoverability: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizationActionStatus {
    pub action_id: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizationStatus {
    pub schema: String,
    pub run_id: String,
    pub status: String,
    pub actions: Vec<FinalizationActionStatus>,
    pub events: Vec<FinalizationEvent>,
    pub logical_bytes_removed: u64,
    pub observed_allocated_bytes_removed: u64,
    pub shared_extent_bytes: Option<u64>,
    pub physical_reclaimed_bytes: Option<u64>,
    /// The v3 report is historical recovery evidence. The v4 action state is
    /// authoritative about irreversible removal.
    pub recovery_history: RecoveryReport,
}

fn contract<T: Serialize>(kind: Contract, value: &T) -> Result<()> {
    contracts::validate(kind, value).map_err(|e| failure(Code::ExecutionPlanInvalid, e.to_string()))
}

fn validate_preview(preview: &FinalizationPreview) -> Result<()> {
    contract(Contract::ExecutionFinalizationPreview, preview)?;
    if preview.schema != PREVIEW_SCHEMA
        || digest(&preview.body)? != preview.fingerprint
        || digest(&preview.body.entries)? != preview.body.quarantine_manifest_digest
        || preview.body.irreversible_notice != NOTICE
        || preview.body.selected_actions.len() != preview.body.entries.len()
        || preview
            .body
            .selected_actions
            .iter()
            .zip(&preview.body.entries)
            .any(|(id, entry)| id != &entry.action_id)
    {
        return Err(failure(
            Code::ExecutionPlanInvalid,
            "finalization preview binding differs",
        ));
    }
    Ok(())
}

pub fn load_preview(path: &Path) -> Result<FinalizationPreview> {
    let preview = super::validation::read_finalization_document(
        path,
        Contract::ExecutionFinalizationPreview,
    )?;
    validate_preview(&preview)?;
    Ok(preview)
}

pub fn load_authorization(path: &Path) -> Result<FinalizationAuthorization> {
    let authorization: FinalizationAuthorization = super::validation::read_finalization_document(
        path,
        Contract::ExecutionFinalizationAuthorization,
    )?;
    contract(Contract::ExecutionFinalizationAuthorization, &authorization)?;
    if authorization.schema != AUTHORIZATION_SCHEMA
        || digest(&authorization.body)? != authorization.authorization_id
    {
        return Err(failure(
            Code::ExecutionApprovalMismatch,
            "finalization authorization fingerprint differs",
        ));
    }
    Ok(authorization)
}

pub fn authorize(
    preview: &FinalizationPreview,
    fingerprint: &str,
    approved_by: &str,
) -> Result<FinalizationAuthorization> {
    validate_preview(preview)?;
    if fingerprint != preview.fingerprint
        || approved_by.trim().is_empty()
        || approved_by.len() > 256
    {
        return Err(failure(
            Code::ExecutionApprovalMismatch,
            "reviewed finalization fingerprint and operator label required",
        ));
    }
    let body = AuthorizationBody {
        authority: "irreversible_retained_quarantine_finalization".to_owned(),
        preview_fingerprint: preview.fingerprint.clone(),
        plan_fingerprint: preview.body.plan_fingerprint.clone(),
        execution_run_id: preview.body.run_id.clone(),
        quarantine_manifest_digest: preview.body.quarantine_manifest_digest.clone(),
        selected_actions: preview.body.selected_actions.clone(),
        approved_by: approved_by.to_owned(),
        approved_at: chrono::Utc::now().to_rfc3339(),
    };
    let authorization = FinalizationAuthorization {
        schema: AUTHORIZATION_SCHEMA.to_owned(),
        authorization_id: digest(&body)?,
        body,
    };
    contract(Contract::ExecutionFinalizationAuthorization, &authorization)?;
    Ok(authorization)
}

fn validate_authorization(
    preview: &FinalizationPreview,
    authorization: &FinalizationAuthorization,
) -> Result<()> {
    contract(Contract::ExecutionFinalizationAuthorization, authorization)?;
    if authorization.schema != AUTHORIZATION_SCHEMA
        || digest(&authorization.body)? != authorization.authorization_id
        || authorization.body.authority != "irreversible_retained_quarantine_finalization"
        || authorization.body.preview_fingerprint != preview.fingerprint
        || authorization.body.plan_fingerprint != preview.body.plan_fingerprint
        || authorization.body.execution_run_id != preview.body.run_id
        || authorization.body.quarantine_manifest_digest != preview.body.quarantine_manifest_digest
        || authorization.body.selected_actions != preview.body.selected_actions
    {
        return Err(failure(
            Code::ExecutionApprovalMismatch,
            "separate finalization authorization does not bind this preview",
        ));
    }
    Ok(())
}

fn report(recovery: RecoveryReport, events: Vec<FinalizationEvent>) -> Result<FinalizationStatus> {
    let mut actions = Vec::new();
    let mut logical = 0u64;
    let mut allocated = 0u64;
    for action in &recovery.actions {
        let action_events: Vec<_> = events
            .iter()
            .filter(|e| e.action_id == action.action_id)
            .collect();
        if action_events.iter().any(|event| {
            action.state != "restored_retained"
                || event.schema != EVENT_SCHEMA
                || event.quarantine_identity.link_count != Some(1)
                || event.survivor_identity.link_count != Some(1)
                || event.recoverability
                    != if event.phase == "removed" {
                        "irreversible_removed"
                    } else {
                        "irreversible_pending_inspection"
                    }
                || event.target_available_after.is_some() != (event.phase == "removed")
                || event.target_free_space_change_bytes
                    != event.target_available_after.and_then(|after| {
                        i64::try_from(i128::from(after) - i128::from(event.target_available_before))
                            .ok()
                    })
                || event.shared_extent_bytes.is_some()
        }) {
            return Err(failure(
                Code::StoredStateIncompatible,
                "finalization evidence contradicts retained restore",
            ));
        }
        let state = match action_events.as_slice() {
            [] => match action.state.as_str() {
                "quarantined" => "quarantined",
                "restored" => "restored",
                "restored_retained" => "retained",
                "ambiguous" => "ambiguous",
                _ => "not_selected",
            },
            [pending] if pending.phase == "removal_pending" => "pending_irreversible_inspection",
            [pending, removed]
                if pending.phase == "removal_pending"
                    && removed.phase == "removed"
                    && pending.preview_fingerprint == removed.preview_fingerprint
                    && pending.authorization_id == removed.authorization_id
                    && pending.manifest_digest == removed.manifest_digest
                    && pending.quarantine_identity == removed.quarantine_identity
                    && pending.survivor_identity == removed.survivor_identity
                    && pending.logical_bytes == removed.logical_bytes
                    && pending.observed_allocated_bytes_before
                        == removed.observed_allocated_bytes_before
                    && pending.target_available_before == removed.target_available_before
                    && pending.binary_version == removed.binary_version
                    && pending.configuration_fingerprint == removed.configuration_fingerprint
                    && pending.policy_fingerprint == removed.policy_fingerprint =>
            {
                logical = logical.checked_add(removed.logical_bytes).ok_or_else(|| {
                    failure(Code::ExecutionBoundsExceeded, "logical sum overflow")
                })?;
                allocated = allocated
                    .checked_add(removed.observed_allocated_bytes_before)
                    .ok_or_else(|| {
                        failure(Code::ExecutionBoundsExceeded, "allocation sum overflow")
                    })?;
                "finalized_irreversible"
            }
            _ => {
                return Err(failure(
                    Code::StoredStateIncompatible,
                    "invalid finalization transition order",
                ));
            }
        };
        actions.push(FinalizationActionStatus {
            action_id: action.action_id.clone(),
            state: state.to_owned(),
        });
    }
    if events.iter().any(|event| {
        event.run_id != recovery.run_id
            || !actions.iter().any(|a| a.action_id == event.action_id)
            || event.physical_reclaimed_bytes.is_some()
    }) {
        return Err(failure(
            Code::StoredStateIncompatible,
            "finalization event has unrelated authority",
        ));
    }
    let status = if actions.iter().any(|a| {
        matches!(
            a.state.as_str(),
            "pending_irreversible_inspection" | "ambiguous"
        )
    }) {
        "attention_required"
    } else if events.is_empty() {
        "not_started"
    } else if actions
        .iter()
        .all(|a| !matches!(a.state.as_str(), "retained" | "quarantined"))
    {
        "finalized_irreversible"
    } else {
        "partially_finalized"
    };
    let result = FinalizationStatus {
        schema: STATUS_SCHEMA.to_owned(),
        run_id: recovery.run_id.clone(),
        status: status.to_owned(),
        actions,
        events,
        logical_bytes_removed: logical,
        observed_allocated_bytes_removed: allocated,
        shared_extent_bytes: None,
        physical_reclaimed_bytes: None,
        recovery_history: recovery,
    };
    contract(Contract::ExecutionFinalizationStatus, &result)?;
    Ok(result)
}

/// Read-only: does not migrate state, acquire a write lock, or inspect source paths.
pub fn status(state: &Path, run_id: &str) -> Result<FinalizationStatus> {
    let recovery = super::recovery::status(state, run_id)?;
    let connection = rusqlite::Connection::open_with_flags(
        state.join("state.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| failure(Code::StateTransactionFailed, e.to_string()))?;
    let events = super::journal::read_finalization_events(&connection, run_id)?;
    report(recovery, events)
}

#[cfg(target_os = "linux")]
fn check_context(
    plan: &ExecutionPlan,
    approval: &Approval,
    state: &Path,
    policy: &EffectivePolicyV1,
    run_id: &str,
    recovery: &RecoveryReport,
) -> Result<()> {
    mutation::check_authority(plan, approval, state, policy)?;
    if recovery.run_id != run_id
        || recovery.mutation.plan_fingerprint != plan.fingerprint
        || recovery.mutation.authorization_id != approval.authorization_id
        || recovery.mutation.status != MutationStatus::Completed
        || recovery.recovery_authority != "bound_v3_context"
        || recovery.status == "attention_required"
        || recovery.status == "cleaned"
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "finalization requires an unambiguous completed approved quarantine run",
        ));
    }
    let context = recovery
        .events
        .first()
        .ok_or_else(|| failure(Code::ExecutionUnsupported, "v3 authority binding absent"))?;
    if context.configuration_fingerprint != policy.fingerprints.effective_configuration.value
        || context.policy_fingerprint != policy.fingerprints.evidence_policy.value
        || context.binary_version != env!("CARGO_PKG_VERSION")
    {
        return Err(failure(
            Code::ExecutionPolicyMismatch,
            "bound binary/configuration/policy changed",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn action<'a>(plan: &'a ExecutionPlan, id: &str) -> Result<&'a super::model::ExactAction> {
    plan.body
        .actions
        .iter()
        .find(|a| a.action_id == id)
        .ok_or_else(|| {
            failure(
                Code::ExecutionPlanInvalid,
                "action is not in the approved execution plan",
            )
        })
}

#[cfg(target_os = "linux")]
fn absent(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(failure(
            Code::ExecutionSourceStale,
            "finalized artifact path is occupied or unavailable",
        )),
    }
}

#[cfg(target_os = "linux")]
fn survivor(
    plan: &ExecutionPlan,
    recovery: &RecoveryReport,
    id: &str,
    signals: &SignalState,
) -> Result<(File, FilesystemIdentity)> {
    let selected = action(plan, id)?;
    let path = fs::native_path(&selected.candidate.path)?;
    fs::check_directory(&selected.candidate.directory)?;
    let mut source = fs::open(&path, false)?;
    let observed = recovery::verify_content(selected, &mut source, signals, false)?;
    recovery::check_properties(selected, &source, recovery)?;
    let restored = recovery
        .events
        .iter()
        .rev()
        .find(|e| {
            e.action_id.as_deref() == Some(id)
                && e.operation == "restore"
                && e.phase == "restored_retained"
        })
        .and_then(|e| e.observed_identity.as_ref())
        .ok_or_else(|| {
            failure(
                Code::StoredStateIncompatible,
                "restored survivor identity evidence missing",
            )
        })?;
    if observed.identity_key() != restored.identity_key()
        || observed.link_count != Some(1)
        || recovery::identity(&fs::open(&path, false)?)?.identity_key() != observed.identity_key()
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "restored survivor identity changed",
        ));
    }
    Ok((source, observed))
}

#[cfg(target_os = "linux")]
fn entry(
    plan: &ExecutionPlan,
    recovery: &RecoveryReport,
    id: &str,
    signals: &SignalState,
) -> Result<ManifestEntry> {
    let selected = action(plan, id)?;
    let index = plan
        .body
        .actions
        .iter()
        .position(|a| a.action_id == id)
        .expect("approved action");
    if selected.topology != super::model::Topology::CrossFilesystem
        || recovery.actions[index].state != "restored_retained"
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "finalization requires a proven retained cross-filesystem restore",
        ));
    }
    let (source, survivor_identity) = survivor(plan, recovery, id, signals)?;
    let namespace = recovery.mutation.namespace.to_path_buf();
    if namespace != fs::native_path(&plan.body.quarantine.path)?.join(&plan.fingerprint) {
        return Err(failure(
            Code::StoredStateIncompatible,
            "quarantine namespace differs from approved plan",
        ));
    }
    fs::check_directory(&plan.body.quarantine)?;
    let _namespace = fs::open(&namespace, true)?;
    let path = namespace.join(id);
    let mut quarantined = fs::open(&path, false)?;
    let quarantine_identity = recovery::verify_content(selected, &mut quarantined, signals, false)?;
    recovery::check_properties(selected, &quarantined, recovery)?;
    recovery::compare(
        &source,
        &quarantined,
        selected.candidate.size_bytes,
        signals,
    )?;
    if recovery::identity(&fs::open(&path, false)?)?.identity_key()
        != quarantine_identity.identity_key()
        || quarantine_identity.link_count != Some(1)
        || quarantine_identity.identity_key() == survivor_identity.identity_key()
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "quarantine object identity changed or aliases survivor",
        ));
    }
    let metadata = fs::metadata(&quarantined)?;
    let allocated = metadata
        .blocks()
        .checked_mul(512)
        .ok_or_else(|| failure(Code::ExecutionBoundsExceeded, "allocation overflow"))?;
    let properties_fingerprint = recovery
        .events
        .iter()
        .rev()
        .find(|e| {
            e.action_id.as_deref() == Some(id)
                && e.phase == "committed"
                && matches!(e.operation.as_str(), "apply" | "resume")
        })
        .and_then(|e| e.properties_fingerprint.clone())
        .ok_or_else(|| {
            failure(
                Code::StoredStateIncompatible,
                "commit property evidence absent",
            )
        })?;
    Ok(ManifestEntry {
        action_id: id.to_owned(),
        quarantine_identity,
        survivor_identity,
        logical_bytes: selected.candidate.size_bytes,
        observed_allocated_bytes: allocated,
        blake3: selected.candidate.blake3.clone(),
        properties_fingerprint,
    })
}

#[cfg(target_os = "linux")]
pub fn preview(
    plan: &ExecutionPlan,
    approval: &Approval,
    state: &Path,
    policy: &EffectivePolicyV1,
    run_id: &str,
    selected: &[String],
    signals: &SignalState,
) -> Result<FinalizationPreview> {
    if selected.is_empty()
        || selected.len() as u64 > plan.body.bounds.max_actions
        || selected.len() > plan.body.actions.len()
        || plan.body.bounds.max_in_flight_files != 1
    {
        return Err(failure(
            Code::ExecutionBoundsExceeded,
            "finalization action bound invalid",
        ));
    }
    let mut unique = std::collections::BTreeSet::new();
    if selected.iter().any(|id| !unique.insert(id)) {
        return Err(failure(
            Code::ExecutionPlanInvalid,
            "finalization actions must be explicit and unique",
        ));
    }
    let recovery = recovery::status(state, run_id)?;
    check_context(plan, approval, state, policy, run_id, &recovery)?;
    let prior = status(state, run_id)?;
    if selected.iter().any(|id| {
        prior
            .actions
            .iter()
            .find(|a| &a.action_id == id)
            .is_some_and(|a| {
                matches!(
                    a.state.as_str(),
                    "finalized_irreversible" | "pending_irreversible_inspection"
                )
            })
    }) || prior.status == "attention_required"
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "selected action already finalized or ambiguous",
        ));
    }
    validate_environment_remaining(plan, &mut fs::capacity, true, plan.body.actions.len())?;
    let entries = selected
        .iter()
        .map(|id| entry(plan, &recovery, id, signals))
        .collect::<Result<Vec<_>>>()?;
    let body = PreviewBody {
        run_id: run_id.to_owned(),
        plan_fingerprint: plan.fingerprint.clone(),
        execution_approval_id: approval.authorization_id.clone(),
        selected_actions: selected.to_vec(),
        quarantine_manifest_digest: digest(&entries)?,
        entries,
        previewed_at: Utc::now().to_rfc3339(),
        producer_version: env!("CARGO_PKG_VERSION").to_owned(),
        configuration_fingerprint: policy.fingerprints.effective_configuration.value.clone(),
        policy_fingerprint: policy.fingerprints.evidence_policy.value.clone(),
        irreversible_notice: NOTICE.to_owned(),
    };
    let result = FinalizationPreview {
        schema: PREVIEW_SCHEMA.to_owned(),
        fingerprint: digest(&body)?,
        body,
    };
    validate_preview(&result)?;
    Ok(result)
}

#[cfg(not(target_os = "linux"))]
pub fn preview(
    _plan: &ExecutionPlan,
    _approval: &Approval,
    _state: &Path,
    _policy: &EffectivePolicyV1,
    _run_id: &str,
    _selected: &[String],
    _signals: &SignalState,
) -> Result<FinalizationPreview> {
    Err(failure(
        Code::ExecutionUnsupported,
        "irreversible finalization requires Linux",
    ))
}

#[cfg(target_os = "linux")]
fn event(
    preview: &FinalizationPreview,
    authorization: &FinalizationAuthorization,
    item: &ManifestEntry,
    policy: &EffectivePolicyV1,
    phase: &str,
    before: u64,
    after: Option<u64>,
) -> FinalizationEvent {
    FinalizationEvent {
        schema: EVENT_SCHEMA.to_owned(),
        event_id: Uuid::now_v7().to_string(),
        run_id: preview.body.run_id.clone(),
        action_id: item.action_id.clone(),
        phase: phase.to_owned(),
        preview_fingerprint: preview.fingerprint.clone(),
        authorization_id: authorization.authorization_id.clone(),
        manifest_digest: preview.body.quarantine_manifest_digest.clone(),
        recorded_at: Utc::now().to_rfc3339(),
        binary_version: env!("CARGO_PKG_VERSION").to_owned(),
        configuration_fingerprint: policy.fingerprints.effective_configuration.value.clone(),
        policy_fingerprint: policy.fingerprints.evidence_policy.value.clone(),
        quarantine_identity: item.quarantine_identity.clone(),
        survivor_identity: item.survivor_identity.clone(),
        logical_bytes: item.logical_bytes,
        observed_allocated_bytes_before: item.observed_allocated_bytes,
        target_available_before: before,
        target_available_after: after,
        target_free_space_change_bytes: after
            .and_then(|available| i64::try_from(i128::from(available) - i128::from(before)).ok()),
        shared_extent_bytes: None,
        physical_reclaimed_bytes: None,
        recoverability: if phase == "removed" {
            "irreversible_removed"
        } else {
            "irreversible_pending_inspection"
        }
        .to_owned(),
        reason: if phase == "removed" {
            "verified quarantine entry unlinked and parent synchronized; no recovery claim"
        } else {
            "irreversible removal armed; outcome ambiguous until durable removed event"
        }
        .to_owned(),
    }
}

#[cfg(target_os = "linux")]
pub fn commit(
    plan: &ExecutionPlan,
    approval: &Approval,
    state: &Path,
    policy: &EffectivePolicyV1,
    preview: &FinalizationPreview,
    authorization: &FinalizationAuthorization,
    signals: &SignalState,
) -> Result<FinalizationStatus> {
    use rustix::fs::{AtFlags, unlinkat};
    validate_preview(preview)?;
    validate_authorization(preview, authorization)?;
    let run_id = &preview.body.run_id;
    if preview.body.plan_fingerprint != plan.fingerprint
        || preview.body.execution_approval_id != approval.authorization_id
        || preview.body.producer_version != env!("CARGO_PKG_VERSION")
        || preview.body.configuration_fingerprint
            != policy.fingerprints.effective_configuration.value
        || preview.body.policy_fingerprint != policy.fingerprints.evidence_policy.value
        || preview.body.selected_actions.len() as u64 > plan.body.bounds.max_actions
    {
        return Err(failure(
            Code::ExecutionApprovalMismatch,
            "finalization preview differs from current authority",
        ));
    }
    let (mut journal, run, recovery_events) =
        recovery::locked(plan, approval, state, policy, run_id)?;
    let recovery = recovery::report(plan, run, recovery_events)?;
    check_context(plan, approval, state, policy, run_id, &recovery)?;
    let mut current = report(recovery.clone(), journal.finalization_events(run_id)?)?;
    if current.status == "attention_required" {
        return Err(failure(
            Code::ExecutionSourceStale,
            "ambiguous finalization requires manual inspection",
        ));
    }
    let namespace_path = recovery.mutation.namespace.to_path_buf();
    if namespace_path != fs::native_path(&plan.body.quarantine.path)?.join(&plan.fingerprint) {
        return Err(failure(
            Code::StoredStateIncompatible,
            "namespace differs from plan",
        ));
    }
    fs::check_directory(&plan.body.quarantine)?;
    let namespace = fs::open(&namespace_path, true)?;
    for item in &preview.body.entries {
        let id = &item.action_id;
        let prior = current
            .actions
            .iter()
            .find(|a| &a.action_id == id)
            .ok_or_else(|| failure(Code::ExecutionPlanInvalid, "unapproved selected action"))?;
        if prior.state == "finalized_irreversible" {
            let prior_events: Vec<_> = current
                .events
                .iter()
                .filter(|e| &e.action_id == id)
                .collect();
            if prior_events.len() != 2
                || prior_events[1].preview_fingerprint != preview.fingerprint
                || prior_events[1].authorization_id != authorization.authorization_id
            {
                return Err(failure(
                    Code::ExecutionApprovalMismatch,
                    "completed removal belongs to another authorization",
                ));
            }
            absent(&namespace_path.join(id))?;
            let (_, observed) = survivor(plan, &recovery, id, signals)?;
            if observed.identity_key() != item.survivor_identity.identity_key() {
                return Err(failure(
                    Code::ExecutionSourceStale,
                    "surviving copy changed after removal",
                ));
            }
            continue;
        }
        if prior.state != "retained" {
            return Err(failure(
                Code::ExecutionSourceStale,
                "only restored retained copies may be finalized",
            ));
        }
        if signals.is_cancelled() {
            return Err(fs::interrupted(signals));
        }
        validate_environment_remaining(plan, &mut fs::capacity, true, plan.body.actions.len())?;
        let observed = entry(plan, &recovery, id, signals)?;
        if &observed != item
            || digest(&preview.body.entries)? != preview.body.quarantine_manifest_digest
        {
            return Err(failure(
                Code::ExecutionSourceStale,
                "reviewed quarantine manifest changed",
            ));
        }
        let before = fs::capacity(&namespace)?.available;
        let pending = event(
            preview,
            authorization,
            item,
            policy,
            "removal_pending",
            before,
            None,
        );
        journal.append_finalization(
            &pending,
            plan.body.bounds.max_actions,
            plan.body.bounds.journal_budget_bytes,
        )?;
        // The pending record is durable before the first irreversible call.
        // A refusal or crash from here remains inspect-only, never retried.
        validate_environment_remaining(plan, &mut fs::capacity, true, plan.body.actions.len())?;
        if entry(plan, &recovery, id, signals)? != *item || signals.is_cancelled() {
            return Err(failure(
                Code::ExecutionSourceStale,
                "pre-removal identity or content changed",
            ));
        }
        let opened = fs::open(&namespace_path.join(id), false)?;
        if recovery::identity(&opened)?.identity_key() != item.quarantine_identity.identity_key() {
            return Err(failure(
                Code::ExecutionSourceStale,
                "quarantine identity changed at removal boundary",
            ));
        }
        unlinkat(&namespace, id.as_str(), AtFlags::empty()).map_err(|e| {
            failure(
                Code::ExecutionSourceStale,
                format!("exclusive finalization unlink refused: {e}"),
            )
        })?;
        namespace.sync_all().map_err(|e| {
            failure(
                Code::StateTransactionFailed,
                format!("finalization directory sync failed: {e}"),
            )
        })?;
        absent(&namespace_path.join(id))?;
        let (_, survivor_identity) = survivor(plan, &recovery, id, signals)?;
        if survivor_identity.identity_key() != item.survivor_identity.identity_key() {
            return Err(failure(
                Code::ExecutionSourceStale,
                "survivor changed after unlink",
            ));
        }
        let after = fs::capacity(&namespace)?.available;
        let removed = event(
            preview,
            authorization,
            item,
            policy,
            "removed",
            before,
            Some(after),
        );
        journal.append_finalization(
            &removed,
            plan.body.bounds.max_actions,
            plan.body.bounds.journal_budget_bytes,
        )?;
        current = report(recovery.clone(), journal.finalization_events(run_id)?)?;
    }
    Ok(current)
}

#[cfg(not(target_os = "linux"))]
pub fn commit(
    _plan: &ExecutionPlan,
    _approval: &Approval,
    _state: &Path,
    _policy: &EffectivePolicyV1,
    _preview: &FinalizationPreview,
    _authorization: &FinalizationAuthorization,
    _signals: &SignalState,
) -> Result<FinalizationStatus> {
    Err(failure(
        Code::ExecutionUnsupported,
        "irreversible finalization requires Linux",
    ))
}
