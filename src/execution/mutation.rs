//! Bounded Linux quarantine transactions. v1 preview evidence remains immutable.
#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
use std::path::Path;

#[cfg(target_os = "linux")]
use chrono::Utc;
use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
use uuid::Uuid;

#[cfg(target_os = "linux")]
use super::digest;
#[cfg(target_os = "linux")]
use super::filesystem as fs;
#[cfg(target_os = "linux")]
use super::journal::Journal;
#[cfg(target_os = "linux")]
use super::model::ExactAction;
use super::model::{Approval, CapacityEvidence, ExecutionPlan, SavingsEvidence, Topology};
#[cfg(target_os = "linux")]
use super::validation::{validate_environment_remaining, validate_pair, validate_plan};
use super::{Result, failure};
use crate::configuration::EffectivePolicyV1;
#[cfg(target_os = "linux")]
use crate::contracts::{self, Contract};
use crate::domain::NativePath;
use crate::outcome::{Diagnostic, DiagnosticCode as Code};
use crate::signals::SignalState;

pub const MUTATION_SCHEMA: &str = "optiflow.execution-mutation.v2";
const ATTEMPT_SCHEMA: &str = "optiflow.execution-mutation-attempt.v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationStatus {
    Running,
    Completed,
    Rejected,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Prepared,
    NamespacePending,
    NamespaceDurable,
    Preflighted,
    RenamePending,
    CopyPending,
    TempSynced,
    DestinationPending,
    DestinationDurable,
    SourceRemovalPending,
    SourceRemoved,
    Committed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationAttempt {
    pub schema: String,
    pub action_id: String,
    pub source: NativePath,
    pub temporary: NativePath,
    pub destination: NativePath,
    pub topology: Topology,
    pub phase: Phase,
    pub committed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationRun {
    pub schema: String,
    pub run_id: String,
    pub plan_fingerprint: String,
    pub authorization_id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub dry_run: bool,
    pub status: MutationStatus,
    pub namespace: NativePath,
    pub attempts: Vec<MutationAttempt>,
    pub committed_actions: u64,
    pub capacity: Vec<CapacityEvidence>,
    pub savings: SavingsEvidence,
    pub diagnostics: Vec<Diagnostic>,
    pub recovery_guarantee: String,
}

pub use super::journal::load_mutation;

#[cfg(target_os = "linux")]
fn check_authority(
    plan: &ExecutionPlan,
    approval: &Approval,
    state: &Path,
    policy: &EffectivePolicyV1,
) -> Result<()> {
    validate_plan(plan)?;
    contracts::validate(Contract::Execution, approval)
        .map_err(|e| failure(Code::ExecutionApprovalMismatch, e.to_string()))?;
    if approval.body.plan_fingerprint != plan.fingerprint
        || digest(&approval.body)? != approval.authorization_id
        || approval.body.authority != "quarantine_exact_duplicates"
    {
        return Err(failure(
            Code::ExecutionApprovalMismatch,
            "approval does not authorize this exact plan",
        ));
    }
    crate::configuration::validate_fingerprints(policy)
        .map_err(|e| failure(Code::ExecutionPolicyMismatch, e.to_string()))?;
    if policy.fingerprints.evidence_policy.value != plan.body.evidence_policy_fingerprint {
        return Err(failure(
            Code::ExecutionPolicyMismatch,
            "current evidence policy differs from approval",
        ));
    }
    if fs::canonical_input(state)? != fs::native_path(&plan.body.state.path)? {
        return Err(failure(
            Code::ExecutionScopeInvalid,
            "state directory differs from plan",
        ));
    }
    Ok(())
}

pub fn apply_quarantine(
    plan: &ExecutionPlan,
    approval: &Approval,
    state: &Path,
    policy: &EffectivePolicyV1,
    signals: &SignalState,
) -> Result<MutationRun> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (plan, approval, state, policy, signals);
        return Err(failure(
            Code::ExecutionUnsupported,
            "live quarantine requires the verified Linux filesystem implementation",
        ));
    }
    #[cfg(target_os = "linux")]
    apply_linux(plan, approval, state, policy, signals)
}

#[cfg(target_os = "linux")]
fn apply_linux(
    plan: &ExecutionPlan,
    approval: &Approval,
    state: &Path,
    policy: &EffectivePolicyV1,
    signals: &SignalState,
) -> Result<MutationRun> {
    check_authority(plan, approval, state, policy)?;
    // Permit an existing same-plan namespace only long enough to acquire the
    // lock and classify a previous abandoned run. It never authorizes reuse.
    validate_environment_remaining(plan, &mut fs::capacity, true, 0)?;
    if signals.is_cancelled() {
        return Err(fs::interrupted(signals));
    }
    let mut journal = Journal::open(plan)?;
    // Recovery may have recorded an older interrupted transaction under this
    // fingerprint. A namespace is never reused or cleaned up implicitly.
    let capacity = validate_environment_remaining(plan, &mut fs::capacity, false, 0)?;
    let namespace = fs::native_path(&plan.body.quarantine.path)?.join(&plan.fingerprint);
    let selected = plan.body.actions.iter().try_fold(0u64, |sum, a| {
        sum.checked_add(a.candidate.size_bytes)
            .ok_or_else(|| failure(Code::ExecutionBoundsExceeded, "selected byte sum overflow"))
    })?;
    let mut run = MutationRun {
        schema: MUTATION_SCHEMA.to_owned(), run_id: Uuid::now_v7().to_string(),
        plan_fingerprint: plan.fingerprint.clone(), authorization_id: approval.authorization_id.clone(),
        started_at: Utc::now().to_rfc3339(), completed_at: None, dry_run: false,
        status: MutationStatus::Running, namespace: NativePath::from_path(&namespace),
        attempts: Vec::new(), committed_actions: 0, capacity,
        savings: SavingsEvidence { selected_logical_bytes: selected, immediate_logical_reclaimed_bytes: 0,
            physical_reclaimed_bytes: None, physical_status: "unknown".to_owned(),
            reason: "Quarantine retains source bytes; allocation, snapshots and eventual finalization are unmeasured.".to_owned() },
        diagnostics: Vec::new(),
        recovery_guarantee: "inspect_journal_and_paths_before_manual_restore; no_automatic_resume".to_owned(),
    };
    journal.begin_mutation(plan, approval, &run)?;
    let result = execute(plan, signals, &mut journal, &mut run);
    match result {
        Ok(()) => run.status = MutationStatus::Completed,
        Err(error) => {
            // Once the namespace is being created, a failed call may have left
            // durable state. Never label that uncertainty as a completed action.
            run.status = if run.attempts.is_empty() {
                MutationStatus::Rejected
            } else {
                MutationStatus::Interrupted
            };
            run.diagnostics.push(*error);
        }
    }
    run.completed_at = Some(Utc::now().to_rfc3339());
    journal.save_mutation(&run)?;
    Ok(run)
}

#[cfg(target_os = "linux")]
fn execute(
    plan: &ExecutionPlan,
    signals: &SignalState,
    journal: &mut Journal,
    run: &mut MutationRun,
) -> Result<()> {
    use rustix::fs::{Mode, mkdirat};
    let quarantine = fs::check_directory(&plan.body.quarantine)?;
    fs::writable_directory(&quarantine)?;
    let namespace = run.namespace.to_path_buf();
    // The first attempt exists durably before even creating the namespace.
    let first = &plan.body.actions[0];
    run.attempts.push(attempt(first, &namespace)?);
    phase(journal, run, Phase::NamespacePending)?;
    mkdirat(
        &quarantine,
        &plan.fingerprint,
        Mode::RUSR | Mode::WUSR | Mode::XUSR,
    )
    .map_err(|e| {
        failure(
            Code::ExecutionDestinationOccupied,
            format!("cannot create quarantine namespace: {e}"),
        )
    })?;
    quarantine.sync_all().map_err(journal_error)?;
    let directory = fs::open(&namespace, true)?;
    directory.sync_all().map_err(journal_error)?;
    let namespace_identity = fs::directory(&namespace)?.identity;
    phase(journal, run, Phase::NamespaceDurable)?;
    for (index, action) in plan.body.actions.iter().enumerate() {
        if index > 0 {
            run.attempts.push(attempt(action, &namespace)?);
            journal.save_mutation(run)?;
        }
        if signals.is_cancelled() {
            return Err(fs::interrupted(signals));
        }
        if fs::directory(&namespace)?.identity != namespace_identity {
            return Err(failure(
                Code::ExecutionSourceStale,
                "quarantine namespace changed",
            ));
        }
        // Revalidate immediately before each action, regardless of previous
        // dry-run or previous actions in this batch.
        run.capacity = validate_environment_remaining(plan, &mut fs::capacity, true, index)?;
        validate_pair(action, signals)?;
        let candidate = fs::open_bound(&action.candidate)?;
        let parent = fs::check_directory(&action.candidate.directory)?;
        let name = fs::native_path(&action.candidate.path)?
            .file_name()
            .ok_or_else(|| failure(Code::ExecutionScopeInvalid, "candidate has no filename"))?
            .to_owned();
        let destination_name = action.action_id.as_str();
        phase(journal, run, Phase::Preflighted)?;
        if action.topology == Topology::SameFilesystem {
            phase(journal, run, Phase::RenamePending)?;
            fs::check_file(&candidate, &action.candidate)?;
            rustix::fs::renameat_with(
                &parent,
                &name,
                &directory,
                destination_name,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(|e| {
                failure(
                    Code::ExecutionSourceStale,
                    format!("atomic no-replace quarantine move refused: {e}"),
                )
            })?;
            parent
                .sync_all()
                .and_then(|_| directory.sync_all())
                .map_err(journal_error)?;
            verify_moved(
                action,
                &candidate,
                &run.attempts[index].destination.to_path_buf(),
                signals,
            )?;
            phase(journal, run, Phase::DestinationDurable)?;
        } else {
            copy_cross_filesystem(plan, action, &candidate, index, signals, journal, run)?;
        }
        // Only a verified durable destination and absent source become committed.
        run.attempts[index].committed = true;
        run.attempts[index].phase = Phase::Committed;
        run.committed_actions += 1;
        journal.save_mutation(run)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn attempt(action: &ExactAction, namespace: &Path) -> Result<MutationAttempt> {
    let destination = namespace.join(&action.action_id);
    Ok(MutationAttempt {
        schema: ATTEMPT_SCHEMA.to_owned(),
        action_id: action.action_id.clone(),
        source: action.candidate.path.clone(),
        temporary: NativePath::from_path(&namespace.join(format!(".{}.part", action.action_id))),
        destination: NativePath::from_path(&destination),
        topology: action.topology,
        phase: Phase::Prepared,
        committed: false,
    })
}

#[cfg(target_os = "linux")]
fn phase(journal: &mut Journal, run: &mut MutationRun, next: Phase) -> Result<()> {
    run.attempts.last_mut().expect("durable attempt").phase = next;
    journal.save_mutation(run)
}

#[cfg(target_os = "linux")]
fn journal_error(error: std::io::Error) -> Box<Diagnostic> {
    failure(
        Code::StateTransactionFailed,
        format!("quarantine synchronization failed: {error}"),
    )
}

#[cfg(target_os = "linux")]
fn verify_moved(
    action: &ExactAction,
    original: &File,
    destination: &Path,
    signals: &SignalState,
) -> Result<()> {
    let mut moved = fs::open(destination, false)?;
    let identity =
        crate::filesystem::identity::FileStateSignature::from_file_metadata(&fs::metadata(&moved)?)
            .identity
            .ok_or_else(|| {
                failure(
                    Code::ExecutionAmbiguousIdentity,
                    "moved identity unavailable",
                )
            })?;
    if identity.identity_key() != action.candidate.identity.identity_key()
        || identity.link_count != Some(1)
        || fs::hash_bound(&mut moved, action.candidate.size_bytes, signals)?
            != action.candidate.blake3
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "moved destination identity or content differs",
        ));
    }
    if crate::filesystem::identity::FileStateSignature::from_file_metadata(&fs::metadata(original)?)
        .identity
        .as_ref()
        .map(|id| id.identity_key())
        != Some(identity.identity_key())
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "original handle no longer names moved object",
        ));
    }
    match std::fs::symlink_metadata(action.candidate.path.to_path_buf()) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(failure(
            Code::ExecutionSourceStale,
            "source path still exists after quarantine move",
        )),
    }
}

#[cfg(target_os = "linux")]
fn copy_cross_filesystem(
    plan: &ExecutionPlan,
    action: &ExactAction,
    source: &File,
    index: usize,
    signals: &SignalState,
    journal: &mut Journal,
    run: &mut MutationRun,
) -> Result<()> {
    use rustix::fs::{AtFlags, Mode, OFlags, openat, renameat_with, unlinkat};
    let parent = fs::check_directory(&action.candidate.directory)?;
    let directory = fs::open(&run.namespace.to_path_buf(), true)?;
    let source_path = fs::native_path(&action.candidate.path)?;
    let source_name = source_path
        .file_name()
        .ok_or_else(|| failure(Code::ExecutionScopeInvalid, "candidate has no filename"))?;
    let destination_name = action.action_id.as_str();
    let original_properties = properties(source)?;
    let temp_name = format!(".{destination_name}.part");
    phase(journal, run, Phase::CopyPending)?;
    let temporary = openat(
        &directory,
        temp_name.as_str(),
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|e| {
        failure(
            Code::ExecutionDestinationOccupied,
            format!("cannot create exclusive temporary copy: {e}"),
        )
    })?;
    let mut temporary = File::from(temporary);
    let mut input = source.try_clone().map_err(journal_error)?;
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
        temporary
            .write_all(&buffer[..count])
            .map_err(journal_error)?;
        remaining -= count as u64;
    }
    fs::check_file(source, &action.candidate)?;
    if properties(source)? != original_properties {
        return Err(failure(
            Code::ExecutionSourceStale,
            "source properties changed during copy",
        ));
    }
    set_properties(&temporary, &original_properties)?;
    temporary.sync_all().map_err(journal_error)?;
    check_copy(
        &mut temporary,
        source,
        action,
        &original_properties,
        signals,
    )?;
    temporary.sync_all().map_err(journal_error)?;
    directory.sync_all().map_err(journal_error)?;
    phase(journal, run, Phase::TempSynced)?;
    // The destination is durable before source removal. A crash at any later
    // checkpoint retains at least the original or a verified quarantine copy.
    phase(journal, run, Phase::DestinationPending)?;
    renameat_with(
        &directory,
        temp_name.as_str(),
        &directory,
        destination_name,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|e| {
        failure(
            Code::ExecutionDestinationOccupied,
            format!("cannot commit temporary copy: {e}"),
        )
    })?;
    directory.sync_all().map_err(journal_error)?;
    let destination = run.attempts[index].destination.to_path_buf();
    let mut committed = fs::open(&destination, false)?;
    same_object(&temporary, &committed)?;
    check_copy(
        &mut committed,
        source,
        action,
        &original_properties,
        signals,
    )?;
    committed.sync_all().map_err(journal_error)?;
    phase(journal, run, Phase::DestinationDurable)?;

    // Recheck content, current identities, namespace and remaining capacity
    // immediately before the consequential source removal.
    validate_pair(action, signals)?;
    run.capacity = validate_environment_remaining(plan, &mut fs::capacity, true, index + 1)?;
    if fs::directory(&run.namespace.to_path_buf())?
        .identity
        .identity_key()
        != crate::filesystem::identity::FileStateSignature::from_file_metadata(&fs::metadata(
            &directory,
        )?)
        .identity
        .ok_or_else(|| {
            failure(
                Code::ExecutionAmbiguousIdentity,
                "quarantine directory identity unavailable",
            )
        })?
        .identity_key()
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "quarantine namespace changed before source removal",
        ));
    }
    same_object(&temporary, &committed)?;
    fs::check_file(source, &action.candidate)?;
    check_copy(
        &mut committed,
        source,
        action,
        &original_properties,
        signals,
    )?;
    committed.sync_all().map_err(journal_error)?;
    phase(journal, run, Phase::SourceRemovalPending)?;
    unlinkat(&parent, source_name, AtFlags::empty()).map_err(|e| {
        failure(
            Code::ExecutionSourceStale,
            format!("source removal refused: {e}"),
        )
    })?;
    parent.sync_all().map_err(journal_error)?;
    match std::fs::symlink_metadata(action.candidate.path.to_path_buf()) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        _ => {
            return Err(failure(
                Code::ExecutionSourceStale,
                "source path ambiguous after removal",
            ));
        }
    }
    phase(journal, run, Phase::SourceRemoved)
}

#[cfg(target_os = "linux")]
fn same_object(left: &File, right: &File) -> Result<()> {
    let signature = crate::filesystem::identity::FileStateSignature::from_file_metadata;
    let a = signature(&fs::metadata(left)?).identity.ok_or_else(|| {
        failure(
            Code::ExecutionAmbiguousIdentity,
            "copy identity unavailable",
        )
    })?;
    let b = signature(&fs::metadata(right)?).identity.ok_or_else(|| {
        failure(
            Code::ExecutionAmbiguousIdentity,
            "destination identity unavailable",
        )
    })?;
    if a.identity_key() != b.identity_key() || b.link_count != Some(1) {
        return Err(failure(
            Code::ExecutionSourceStale,
            "destination is not the verified temporary object",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[derive(PartialEq, Eq)]
struct Properties {
    uid: u32,
    gid: u32,
    mode: u32,
    atime: (i64, i64),
    mtime: (i64, i64),
    xattrs: Vec<(Vec<u8>, Vec<u8>)>,
}

#[cfg(target_os = "linux")]
fn properties(file: &File) -> Result<Properties> {
    use rustix::fs::{fgetxattr, flistxattr, ioctl_getflags};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    let flags = ioctl_getflags(file).map_err(|e| {
        failure(
            Code::ExecutionUnsupported,
            format!("cannot inspect inode flags for copy: {e}"),
        )
    })?;
    // FS_EXTENTS_FL is an allocation layout detail. All other filesystem
    // flags, including unrecognized bits, require an explicit refusal.
    if flags.bits() & !0x0008_0000 != 0 {
        return Err(failure(
            Code::ExecutionUnsupported,
            "cross-filesystem copy cannot preserve source inode flags",
        ));
    }
    let mut names_buffer = vec![0u8; 65536];
    let names = flistxattr(file, &mut names_buffer).map_err(|e| {
        failure(
            Code::ExecutionUnsupported,
            format!("cannot enumerate extended attributes: {e}"),
        )
    })?;
    let mut xattrs = Vec::new();
    let mut total = 0usize;
    for name in names_buffer[..names]
        .split(|b| *b == 0)
        .filter(|n| !n.is_empty())
    {
        if name.len() > 255 || xattrs.len() >= 128 {
            return Err(failure(
                Code::ExecutionBoundsExceeded,
                "extended-attribute bound exceeded",
            ));
        }
        let mut value_buffer = vec![0u8; 65536];
        let value =
            fgetxattr(file, std::ffi::OsStr::from_bytes(name), &mut value_buffer).map_err(|e| {
                failure(
                    Code::ExecutionUnsupported,
                    format!("cannot read extended attribute: {e}"),
                )
            })?;
        total = total
            .checked_add(value)
            .ok_or_else(|| failure(Code::ExecutionBoundsExceeded, "xattr size overflow"))?;
        if total > 1024 * 1024 {
            return Err(failure(
                Code::ExecutionBoundsExceeded,
                "extended attributes exceed 1 MiB",
            ));
        }
        xattrs.push((name.to_vec(), value_buffer[..value].to_vec()));
    }
    xattrs.sort();
    let m = fs::metadata(file)?;
    Ok(Properties {
        uid: m.uid(),
        gid: m.gid(),
        mode: m.mode(),
        atime: (m.atime(), m.atime_nsec()),
        mtime: (m.mtime(), m.mtime_nsec()),
        xattrs,
    })
}

#[cfg(target_os = "linux")]
fn set_properties(file: &File, p: &Properties) -> Result<()> {
    use rustix::fs::{Mode, Timespec, Timestamps, XattrFlags, fchmod, fchown, fsetxattr, futimens};
    use std::os::unix::ffi::OsStrExt;
    let current = fs::metadata(file)?;
    use std::os::unix::fs::MetadataExt;
    if current.uid() != p.uid || current.gid() != p.gid {
        fchown(
            file,
            Some(rustix::fs::Uid::from_raw(p.uid)),
            Some(rustix::fs::Gid::from_raw(p.gid)),
        )
        .map_err(|e| {
            failure(
                Code::ExecutionUnsupported,
                format!("cannot preserve owner: {e}"),
            )
        })?;
    }
    for (name, value) in &p.xattrs {
        fsetxattr(
            file,
            std::ffi::OsStr::from_bytes(name),
            value,
            XattrFlags::empty(),
        )
        .map_err(|e| {
            failure(
                Code::ExecutionUnsupported,
                format!("cannot preserve extended attribute: {e}"),
            )
        })?;
    }
    fchmod(file, Mode::from_bits_retain(p.mode & 0o7777)).map_err(|e| {
        failure(
            Code::ExecutionUnsupported,
            format!("cannot preserve permissions: {e}"),
        )
    })?;
    futimens(
        file,
        &Timestamps {
            last_access: Timespec {
                tv_sec: p.atime.0,
                tv_nsec: p.atime.1,
            },
            last_modification: Timespec {
                tv_sec: p.mtime.0,
                tv_nsec: p.mtime.1,
            },
        },
    )
    .map_err(|e| {
        failure(
            Code::ExecutionUnsupported,
            format!("cannot preserve timestamps: {e}"),
        )
    })
}

#[cfg(target_os = "linux")]
fn check_copy(
    copy: &mut File,
    source: &File,
    action: &ExactAction,
    expected: &Properties,
    signals: &SignalState,
) -> Result<()> {
    use std::io::Seek;
    fs::check_file(source, &action.candidate)?;
    if fs::hash_bound(copy, action.candidate.size_bytes, signals)? != action.candidate.blake3 {
        return Err(failure(
            Code::ExecutionSourceStale,
            "temporary copy hash differs",
        ));
    }
    let mut original = source.try_clone().map_err(journal_error)?;
    original.rewind().map_err(journal_error)?;
    copy.rewind().map_err(journal_error)?;
    let mut remaining = action.candidate.size_bytes;
    let mut a = vec![0u8; 1024 * 1024];
    let mut b = vec![0u8; a.len()];
    while remaining > 0 {
        if signals.is_cancelled() {
            return Err(fs::interrupted(signals));
        }
        let count = remaining.min(a.len() as u64) as usize;
        original
            .read_exact(&mut a[..count])
            .map_err(journal_error)?;
        copy.read_exact(&mut b[..count]).map_err(journal_error)?;
        if a[..count] != b[..count] {
            return Err(failure(
                Code::ExecutionSourceStale,
                "temporary copy byte comparison differs",
            ));
        }
        remaining -= count as u64;
    }
    fs::check_file(source, &action.candidate)?;
    // Reading may advance atime. Preserve the current source atime and verify
    // all other declared properties exactly before committing the destination.
    let current = properties(source)?;
    if current.uid != expected.uid
        || current.gid != expected.gid
        || current.mode != expected.mode
        || current.mtime != expected.mtime
        || current.xattrs != expected.xattrs
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "source properties changed during verification",
        ));
    }
    set_properties(copy, &current)?;
    if properties(copy)? != current {
        return Err(failure(
            Code::ExecutionUnsupported,
            "copied ownership, mode, timestamps or xattrs differ",
        ));
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::cli::{Cli, ExecutionPlanArgs};
    use clap::Parser;

    #[test]
    fn crash_after_rename_pending_recovers_interrupted_without_fabricating_commit() {
        let temp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        for name in ["source", "state", "quarantine"] {
            std::fs::create_dir(base.join(name)).unwrap();
        }
        std::fs::write(base.join("source/keeper"), b"duplicate").unwrap();
        std::fs::write(base.join("source/candidate"), b"duplicate").unwrap();
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
            quarantine: base.join("quarantine"),
            max_actions: 1,
            max_in_flight_bytes: 1024,
            reserve_bytes: 1,
            output: base.join("plan.json"),
        };
        let plan =
            super::super::create_plan(&args, &state, &policy, &SignalState::default()).unwrap();
        let approval =
            super::super::approve(&plan, &plan.fingerprint, "synthetic operator").unwrap();
        let namespace = args.quarantine.join(&plan.fingerprint);
        let mut run = MutationRun {
            schema: MUTATION_SCHEMA.to_owned(),
            run_id: Uuid::now_v7().to_string(),
            plan_fingerprint: plan.fingerprint.clone(),
            authorization_id: approval.authorization_id.clone(),
            started_at: Utc::now().to_rfc3339(),
            completed_at: None,
            dry_run: false,
            status: MutationStatus::Running,
            namespace: NativePath::from_path(&namespace),
            attempts: vec![],
            committed_actions: 0,
            capacity: vec![],
            savings: SavingsEvidence {
                selected_logical_bytes: 9,
                immediate_logical_reclaimed_bytes: 0,
                physical_reclaimed_bytes: None,
                physical_status: "unknown".to_owned(),
                reason: "synthetic".to_owned(),
            },
            diagnostics: vec![],
            recovery_guarantee:
                "inspect_journal_and_paths_before_manual_restore; no_automatic_resume".to_owned(),
        };
        let mut journal = Journal::open(&plan).unwrap();
        journal.begin_mutation(&plan, &approval, &run).unwrap();
        std::fs::create_dir(&namespace).unwrap();
        run.attempts
            .push(attempt(&plan.body.actions[0], &namespace).unwrap());
        run.attempts[0].phase = Phase::RenamePending;
        journal.save_mutation(&run).unwrap();
        std::fs::rename(&args.candidate[0], namespace.join("action-000001")).unwrap();
        drop(journal);
        let _recovery = Journal::open(&plan).unwrap();
        let saved = load_mutation(&state, &run.run_id).unwrap().unwrap();
        assert_eq!(saved.status, MutationStatus::Interrupted);
        assert_eq!(saved.committed_actions, 0);
        assert!(!saved.attempts[0].committed);
        assert_eq!(saved.attempts[0].phase, Phase::RenamePending);
        assert!(!args.candidate[0].exists());
        assert!(saved.attempts[0].destination.to_path_buf().exists());
    }
}
