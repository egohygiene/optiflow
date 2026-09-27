use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::cli::ExecutionPlanArgs;
use crate::configuration::EffectivePolicyV1;
use crate::contracts::{self, Contract};
use crate::outcome::DiagnosticCode as Code;
use crate::signals::SignalState;

use super::filesystem as fs;
use super::journal::Journal;
use super::model::*;
use super::{Result, digest, failure};

const MAX_DOCUMENT_BYTES: u64 = 4 * 1024 * 1024;
const JOURNAL_BUDGET: u64 = 16 * 1024 * 1024;
const METADATA_PER_ACTION: u64 = 64 * 1024;

fn contract<T: Serialize>(value: &T) -> Result<()> {
    contracts::validate(Contract::Execution, value)
        .map_err(|e| failure(Code::ExecutionPlanInvalid, e.to_string()))
}

fn read_document<T: DeserializeOwned + Serialize>(path: &Path) -> Result<T> {
    let canonical = fs::canonical_input(path)?;
    let mut file = fs::open(&canonical, false)?;
    let before = fs::metadata(&file)?;
    if !before.is_file() || before.len() > MAX_DOCUMENT_BYTES {
        return Err(failure(
            Code::ExecutionPlanInvalid,
            "execution documents must be regular files at most 4 MiB",
        ));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| failure(Code::ExecutionPlanInvalid, e.to_string()))?;
    let signature = crate::filesystem::identity::FileStateSignature::from_file_metadata;
    let current = fs::open(&canonical, false)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES
        || signature(&before) != signature(&fs::metadata(&file)?)
        || signature(&before) != signature(&fs::metadata(&current)?)
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "execution document changed during read",
        ));
    }
    // Validate the original shape before typed decoding, including shared
    // identity/path types whose serde implementations permit unknown fields.
    let raw: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| failure(Code::ExecutionPlanInvalid, e.to_string()))?;
    contract(&raw)?;
    // Deserialize directly into strict structs: duplicate keys also fail.
    let document: T = serde_json::from_slice(&bytes)
        .map_err(|e| failure(Code::ExecutionPlanInvalid, e.to_string()))?;
    contract(&document)?;
    Ok(document)
}

pub fn load_plan(path: &Path) -> Result<ExecutionPlan> {
    let plan: ExecutionPlan = read_document(path)?;
    validate_plan(&plan)?;
    Ok(plan)
}

pub fn load_approval(path: &Path) -> Result<Approval> {
    let approval: Approval = read_document(path)?;
    if digest(&approval.body)? != approval.authorization_id {
        return Err(failure(
            Code::ExecutionApprovalMismatch,
            "approval fingerprint mismatch",
        ));
    }
    Ok(approval)
}

pub fn create_plan(
    args: &ExecutionPlanArgs,
    state: &Path,
    policy: &EffectivePolicyV1,
    signals: &SignalState,
) -> Result<ExecutionPlan> {
    if args.candidate.is_empty()
        || args.candidate.len() as u64 > args.max_actions
        || args.max_actions > 1000
        || args.reserve_bytes == 0
    {
        return Err(failure(
            Code::ExecutionBoundsExceeded,
            "invalid action count or reserve bound",
        ));
    }
    let bind_dir = |p: &PathBuf| fs::directory(&fs::canonical_input(p)?);
    let roots: Vec<_> = args.root.iter().map(bind_dir).collect::<Result<_>>()?;
    let subtrees = if args.subtree.is_empty() {
        roots.clone()
    } else {
        args.subtree.iter().map(bind_dir).collect::<Result<_>>()?
    };
    let quarantine = fs::directory(&fs::canonical_input(&args.quarantine)?)?;
    let state = fs::directory(&fs::canonical_input(state)?)?;
    let keeper = fs::snapshot(
        &fs::canonical_input(&args.keep)?,
        signals,
        args.max_in_flight_bytes,
    )?;
    let mut candidates: Vec<_> = args
        .candidate
        .iter()
        .map(|p| fs::canonical_input(p))
        .collect::<Result<_>>()?;
    candidates.sort();
    let mut actions = Vec::new();
    for (index, path) in candidates.iter().enumerate() {
        let candidate = fs::snapshot(path, signals, args.max_in_flight_bytes)?;
        let topology = if candidate.identity.filesystem_id == quarantine.identity.filesystem_id {
            Topology::SameFilesystem
        } else {
            Topology::CrossFilesystem
        };
        let action = ExactAction {
            action_id: format!("action-{:06}", index + 1),
            operation: "quarantine_exact_duplicate.v1".to_owned(),
            keeper: keeper.clone(),
            candidate,
            topology,
        };
        validate_pair(&action, signals)?;
        actions.push(action);
    }
    let body = PlanBody {
        created_at: Utc::now().to_rfc3339(),
        producer_version: env!("CARGO_PKG_VERSION").to_owned(),
        selection: "explicit_operator_paths".to_owned(),
        evidence_policy_fingerprint: policy.fingerprints.evidence_policy.value.clone(),
        roots,
        subtrees,
        quarantine,
        state,
        bounds: Bounds {
            max_actions: args.max_actions,
            max_in_flight_files: 1,
            max_in_flight_bytes: args.max_in_flight_bytes,
            free_space_reserve_bytes: args.reserve_bytes,
            journal_budget_bytes: JOURNAL_BUDGET,
        },
        actions,
    };
    let plan = ExecutionPlan {
        schema: PLAN_SCHEMA.to_owned(),
        fingerprint: digest(&body)?,
        body,
    };
    validate_plan(&plan)?;
    Ok(plan)
}

pub fn approve(plan: &ExecutionPlan, fingerprint: &str, approved_by: &str) -> Result<Approval> {
    validate_plan(plan)?;
    if fingerprint != plan.fingerprint || approved_by.trim().is_empty() {
        return Err(failure(
            Code::ExecutionApprovalMismatch,
            "approval requires the reviewed plan's exact fingerprint and a nonempty operator label",
        ));
    }
    let body = ApprovalBody {
        plan_fingerprint: plan.fingerprint.clone(),
        authority: "quarantine_exact_duplicates".to_owned(),
        approved_by: approved_by.to_owned(),
        approved_at: Utc::now().to_rfc3339(),
    };
    let approval = Approval {
        schema: APPROVAL_SCHEMA.to_owned(),
        authorization_id: digest(&body)?,
        body,
    };
    contract(&approval)?;
    Ok(approval)
}

fn overlaps(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

fn paths(bindings: &[DirectoryBinding]) -> Result<Vec<PathBuf>> {
    bindings.iter().map(|b| fs::native_path(&b.path)).collect()
}

pub(super) fn validate_plan(plan: &ExecutionPlan) -> Result<()> {
    contract(plan)?;
    let body = &plan.body;
    if digest(body)? != plan.fingerprint {
        return Err(failure(
            Code::ExecutionPlanInvalid,
            "execution plan fingerprint mismatch",
        ));
    }
    let roots = paths(&body.roots)?;
    let subtrees = paths(&body.subtrees)?;
    let quarantine = fs::native_path(&body.quarantine.path)?;
    let state = fs::native_path(&body.state.path)?;
    if roots
        .iter()
        .enumerate()
        .any(|(i, r)| roots[i + 1..].iter().any(|other| overlaps(r, other)))
        || roots
            .iter()
            .any(|r| overlaps(r, &quarantine) || overlaps(r, &state))
        || overlaps(&state, &quarantine)
        || subtrees
            .iter()
            .any(|p| !roots.iter().any(|r| p.starts_with(r)))
        || subtrees
            .iter()
            .enumerate()
            .any(|(i, p)| subtrees[i + 1..].iter().any(|other| overlaps(p, other)))
    {
        return Err(failure(
            Code::ExecutionScopeInvalid,
            "roots/subtrees must be unambiguous; state and quarantine must be separate and outside source roots",
        ));
    }
    for protected in [&body.state, &body.quarantine] {
        if body
            .roots
            .iter()
            .chain(&body.subtrees)
            .chain(
                body.actions
                    .iter()
                    .flat_map(|a| [&a.keeper.directory, &a.candidate.directory]),
            )
            .any(|source| source.identity.identity_key() == protected.identity.identity_key())
        {
            return Err(failure(
                Code::ExecutionScopeInvalid,
                "state or quarantine aliases a source directory",
            ));
        }
    }
    if body.state.identity.identity_key() == body.quarantine.identity.identity_key() {
        return Err(failure(
            Code::ExecutionScopeInvalid,
            "state and quarantine alias one directory",
        ));
    }
    if body.actions.len() as u64 > body.bounds.max_actions || body.bounds.max_in_flight_files != 1 {
        return Err(failure(
            Code::ExecutionBoundsExceeded,
            "action or in-flight-file bound exceeded",
        ));
    }
    let mut action_ids = BTreeSet::new();
    let mut candidate_ids = BTreeSet::new();
    let mut candidate_paths = BTreeSet::new();
    let mut keeper_ids = BTreeSet::new();
    for action in &body.actions {
        if !action_ids.insert(&action.action_id)
            || !candidate_ids.insert(action.candidate.identity.identity_key())
            || !candidate_paths.insert(fs::native_path(&action.candidate.path)?)
        {
            return Err(failure(
                Code::ExecutionAmbiguousIdentity,
                "duplicate action, path, or filesystem object",
            ));
        }
        keeper_ids.insert(action.keeper.identity.identity_key());
        for file in [&action.keeper, &action.candidate] {
            let path = fs::native_path(&file.path)?;
            let root = body
                .roots
                .iter()
                .find(|r| path.starts_with(r.path.to_path_buf()));
            if !subtrees.iter().any(|r| path.starts_with(r))
                || root.is_none()
                || root.is_some_and(|r| r.identity.filesystem_id != file.identity.filesystem_id)
                || path.parent() != Some(fs::native_path(&file.directory.path)?.as_path())
                || file.directory.identity.filesystem_id != file.identity.filesystem_id
            {
                return Err(failure(
                    Code::ExecutionScopeInvalid,
                    "file escapes its approved subtree, parent, or root filesystem",
                ));
            }
            if file.identity.link_count != Some(1) {
                return Err(failure(
                    Code::ExecutionAmbiguousIdentity,
                    "hard-link aliases are not supported by execution-plan v1",
                ));
            }
            if file.mode & 0o222 == 0 {
                return Err(failure(Code::ExecutionReadOnly, "read-only source file"));
            }
        }
        if action.candidate.size_bytes > body.bounds.max_in_flight_bytes {
            return Err(failure(
                Code::ExecutionBoundsExceeded,
                "candidate exceeds approved in-flight-byte bound",
            ));
        }
        if action.keeper.size_bytes != action.candidate.size_bytes
            || action.keeper.blake3 != action.candidate.blake3
        {
            return Err(failure(
                Code::ExecutionPlanInvalid,
                "planned files do not have equal complete-content evidence",
            ));
        }
        let same =
            action.candidate.identity.filesystem_id == body.quarantine.identity.filesystem_id;
        if same != (action.topology == Topology::SameFilesystem) {
            return Err(failure(
                Code::ExecutionPlanInvalid,
                "declared quarantine topology is inconsistent",
            ));
        }
    }
    if !candidate_ids.is_disjoint(&keeper_ids) {
        return Err(failure(
            Code::ExecutionAmbiguousIdentity,
            "a keeper cannot also be a quarantine candidate",
        ));
    }
    Ok(())
}

/// Create-only output, through a pinned parent directory, outside source and
/// quarantine trees. A plan or approval can never silently overwrite a file.
#[cfg(unix)]
pub fn write_document<T: Serialize>(path: &Path, document: &T, plan: &ExecutionPlan) -> Result<()> {
    use rustix::fs::{Mode, OFlags, openat};
    contract(document)?;
    let parent = std::fs::canonicalize(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
    .map_err(|e| failure(Code::ExecutionScopeInvalid, e.to_string()))?;
    let name = path
        .file_name()
        .ok_or_else(|| failure(Code::ExecutionScopeInvalid, "output requires a filename"))?;
    let output = parent.join(name);
    let protected_objects: Vec<_> = plan
        .body
        .roots
        .iter()
        .chain([&plan.body.quarantine])
        .collect();
    fs::outside_directories(&parent, &protected_objects)?;
    let mut protected = paths(&plan.body.roots)?;
    protected.push(fs::native_path(&plan.body.quarantine.path)?);
    if protected.iter().any(|root| output.starts_with(root)) {
        return Err(failure(
            Code::ExecutionScopeInvalid,
            "evidence output must be outside source roots and quarantine",
        ));
    }
    let bytes = serde_json::to_vec_pretty(document)
        .map_err(|e| failure(Code::ExecutionPlanInvalid, e.to_string()))?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(failure(
            Code::ExecutionBoundsExceeded,
            "document exceeds the 4 MiB bound",
        ));
    }
    let directory = fs::open(&parent, true)?;
    let handle = openat(
        &directory,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|e| {
        failure(
            Code::ExecutionDestinationOccupied,
            format!("cannot create immutable document: {e}"),
        )
    })?;
    let mut file = File::from(handle);
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .and_then(|_| directory.sync_all())
        .map_err(|e| {
            failure(
                Code::StateTransactionFailed,
                format!("document was not durably saved: {e}"),
            )
        })
}
#[cfg(not(unix))]
pub fn write_document<T: Serialize>(
    _path: &Path,
    _document: &T,
    _plan: &ExecutionPlan,
) -> Result<()> {
    Err(failure(
        Code::ExecutionUnsupported,
        "execution documents require supported filesystem identity",
    ))
}

pub(super) fn validate_pair(action: &ExactAction, signals: &SignalState) -> Result<()> {
    validate_pair_with_hook(action, signals, &mut || Ok(()))
}

fn validate_pair_with_hook<F: FnMut() -> Result<()>>(
    action: &ExactAction,
    signals: &SignalState,
    after_hashes: &mut F,
) -> Result<()> {
    let mut keeper = fs::open_bound(&action.keeper)?;
    let mut candidate = fs::open_bound(&action.candidate)?;
    for (file, expected) in [
        (&mut keeper, &action.keeper),
        (&mut candidate, &action.candidate),
    ] {
        if fs::hash_bound(file, expected.size_bytes, signals)? != expected.blake3 {
            return Err(failure(
                Code::ExecutionSourceStale,
                "complete content hash no longer matches the approved plan",
            ));
        }
    }
    after_hashes()?;
    // Independent byte confirmation through the same handles, after both full
    // hashes. read_exact avoids treating unequal short-read chunk sizes as data.
    let mut left = vec![0; 1024 * 1024];
    let mut right = vec![0; left.len()];
    let mut remaining = action.keeper.size_bytes;
    while remaining > 0 {
        if signals.is_cancelled() {
            return Err(fs::interrupted(signals));
        }
        let count = remaining.min(left.len() as u64) as usize;
        keeper
            .read_exact(&mut left[..count])
            .and_then(|_| candidate.read_exact(&mut right[..count]))
            .map_err(|e| failure(Code::ExecutionSourceUnavailable, e.to_string()))?;
        if left[..count] != right[..count] {
            return Err(failure(
                Code::ExecutionSourceStale,
                "direct byte comparison rejected the candidate",
            ));
        }
        remaining -= count as u64;
    }
    let mut extra = [0];
    if keeper
        .read(&mut extra)
        .map_err(|e| failure(Code::ExecutionSourceUnavailable, e.to_string()))?
        != 0
        || candidate
            .read(&mut extra)
            .map_err(|e| failure(Code::ExecutionSourceUnavailable, e.to_string()))?
            != 0
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "file length changed during byte confirmation",
        ));
    }
    fs::check_file(&keeper, &action.keeper)?;
    fs::check_file(&candidate, &action.candidate)?;
    Ok(())
}

fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| failure(Code::ExecutionBoundsExceeded, "space arithmetic overflow"))
}

fn validate_environment<F>(plan: &ExecutionPlan, probe: &mut F) -> Result<Vec<CapacityEvidence>>
where
    F: FnMut(&File) -> Result<fs::Capacity>,
{
    validate_environment_remaining(plan, probe, false, 0)
}

/// Recheck all bindings, but charge only copies that have not yet been moved.
/// A live run owns the pre-created namespace under its exclusive journal lock.
pub(super) fn validate_environment_remaining<F>(
    plan: &ExecutionPlan,
    probe: &mut F,
    namespace_exists: bool,
    first_action: usize,
) -> Result<Vec<CapacityEvidence>>
where
    F: FnMut(&File) -> Result<fs::Capacity>,
{
    let b = &plan.body;
    let protected_state: Vec<_> = b.roots.iter().chain([&b.quarantine]).collect();
    fs::outside_directories(&fs::native_path(&b.state.path)?, &protected_state)?;
    let protected_quarantine: Vec<_> = b.roots.iter().chain([&b.state]).collect();
    fs::outside_directories(&fs::native_path(&b.quarantine.path)?, &protected_quarantine)?;
    let mut measurements = BTreeMap::new();
    let directories = b
        .roots
        .iter()
        .chain(&b.subtrees)
        .chain([&b.quarantine, &b.state])
        .chain(
            b.actions
                .iter()
                .flat_map(|a| [&a.keeper.directory, &a.candidate.directory]),
        );
    for directory in directories {
        let handle = fs::check_directory(directory)?;
        fs::writable_directory(&handle)?;
        let capacity = probe(&handle)?;
        if capacity.read_only {
            return Err(failure(
                Code::ExecutionReadOnly,
                "an affected filesystem is mounted read-only",
            ));
        }
        if capacity.block_size == 0 {
            return Err(failure(
                Code::ExecutionCapacityUnavailable,
                "allocation unit unavailable",
            ));
        }
        measurements
            .entry(directory.identity.filesystem_id.clone())
            .and_modify(|v: &mut fs::Capacity| {
                v.available = v.available.min(capacity.available);
                v.block_size = v.block_size.max(capacity.block_size);
            })
            .or_insert(capacity);
    }
    let destination = fs::native_path(&b.quarantine.path)?.join(&plan.fingerprint);
    match std::fs::symlink_metadata(&destination) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Ok(m) if namespace_exists && m.is_dir() && !m.file_type().is_symlink() => {
            let _handle = fs::open(&destination, true)?;
        }
        Ok(_) => {
            return Err(failure(
                Code::ExecutionDestinationOccupied,
                "planned quarantine namespace already exists",
            ));
        }
        Err(e) => return Err(failure(Code::ExecutionSourceUnavailable, e.to_string())),
    }
    account_capacity_remaining(plan, &measurements, first_action)
}

fn account_capacity_remaining(
    plan: &ExecutionPlan,
    measurements: &BTreeMap<String, fs::Capacity>,
    first_action: usize,
) -> Result<Vec<CapacityEvidence>> {
    let b = &plan.body;
    let mut evidence: BTreeMap<_, _> = measurements
        .iter()
        .map(|(id, c)| {
            (
                id.clone(),
                CapacityEvidence {
                    filesystem_id: id.clone(),
                    available_bytes: c.available,
                    source_logical_bytes: 0,
                    retained_cross_filesystem_copy_bytes: 0,
                    journal_and_metadata_budget_bytes: 0,
                    projected_peak_additional_bytes: 0,
                    reserve_bytes: b.bounds.free_space_reserve_bytes,
                },
            )
        })
        .collect();
    let state = evidence
        .get_mut(&b.state.identity.filesystem_id)
        .expect("measured state");
    state.journal_and_metadata_budget_bytes = b.bounds.journal_budget_bytes;
    let mut seen = BTreeSet::new();
    for action in &b.actions[first_action..] {
        for source in [&action.keeper, &action.candidate] {
            if seen.insert(source.identity.identity_key()) {
                let volume = evidence
                    .get_mut(&source.identity.filesystem_id)
                    .expect("measured source");
                volume.source_logical_bytes = add(volume.source_logical_bytes, source.size_bytes)?;
            }
        }
        if action.topology == Topology::CrossFilesystem {
            let source = evidence
                .get_mut(&action.candidate.identity.filesystem_id)
                .expect("measured source");
            source.journal_and_metadata_budget_bytes = add(
                source.journal_and_metadata_budget_bytes,
                METADATA_PER_ACTION,
            )?;
        }
        let volume = evidence
            .get_mut(&b.quarantine.identity.filesystem_id)
            .expect("measured quarantine");
        volume.journal_and_metadata_budget_bytes = add(
            volume.journal_and_metadata_budget_bytes,
            METADATA_PER_ACTION,
        )?;
        if action.topology == Topology::CrossFilesystem {
            let block = measurements[&b.quarantine.identity.filesystem_id].block_size;
            let rounded = add(action.candidate.size_bytes, block - 1)? / block * block;
            volume.retained_cross_filesystem_copy_bytes =
                add(volume.retained_cross_filesystem_copy_bytes, rounded)?;
        }
    }
    for volume in evidence.values_mut() {
        volume.projected_peak_additional_bytes = add(
            volume.retained_cross_filesystem_copy_bytes,
            volume.journal_and_metadata_budget_bytes,
        )?;
        let required = add(volume.projected_peak_additional_bytes, volume.reserve_bytes)?;
        if volume.available_bytes < required {
            return Err(failure(
                Code::ExecutionCapacityInsufficient,
                format!(
                    "filesystem {} needs {required} available bytes including reserve; measured {}",
                    volume.filesystem_id, volume.available_bytes
                ),
            ));
        }
    }
    Ok(evidence.into_values().collect())
}

pub fn dry_run(
    plan: &ExecutionPlan,
    approval: &Approval,
    state: &Path,
    policy: &EffectivePolicyV1,
    signals: &SignalState,
) -> Result<ExecutionRun> {
    dry_run_with_probe(plan, approval, state, policy, signals, &mut fs::capacity)
}

fn dry_run_with_probe<F>(
    plan: &ExecutionPlan,
    approval: &Approval,
    state: &Path,
    policy: &EffectivePolicyV1,
    signals: &SignalState,
    probe: &mut F,
) -> Result<ExecutionRun>
where
    F: FnMut(&File) -> Result<fs::Capacity>,
{
    validate_plan(plan)?;
    contract(approval)?;
    if approval.body.plan_fingerprint != plan.fingerprint
        || digest(&approval.body)? != approval.authorization_id
    {
        return Err(failure(
            Code::ExecutionApprovalMismatch,
            "approval is not bound to this immutable plan",
        ));
    }
    crate::configuration::validate_fingerprints(policy)
        .map_err(|e| failure(Code::ExecutionPolicyMismatch, e.to_string()))?;
    if policy.fingerprints.evidence_policy.value != plan.body.evidence_policy_fingerprint {
        return Err(failure(
            Code::ExecutionPolicyMismatch,
            "current evidence policy differs from the approved policy",
        ));
    }
    if fs::canonical_input(state)? != fs::native_path(&plan.body.state.path)? {
        return Err(failure(
            Code::ExecutionScopeInvalid,
            "state directory differs from the approved location",
        ));
    }
    // All feasible placement, permission and capacity checks precede any state
    // creation. This also prevents migrations writing into a protected tree.
    let capacity = validate_environment(plan, probe)?;
    if signals.is_cancelled() {
        return Err(fs::interrupted(signals));
    }
    let mut journal = Journal::open(plan)?;
    let mut run = new_run(plan, approval, capacity)?;
    journal.begin(plan, approval, &run)?;
    for action in &plan.body.actions {
        let mut attempt = Attempt {
            schema: ATTEMPT_SCHEMA.to_owned(),
            action_id: action.action_id.clone(),
            status: Status::Validating,
            hash_confirmed: false,
            bytes_confirmed: false,
            identities_confirmed: false,
        };
        run.attempts.push(attempt.clone());
        journal.save(&run)?;
        match validate_pair(action, signals) {
            Ok(()) => {
                attempt.status = Status::Validated;
                attempt.hash_confirmed = true;
                attempt.bytes_confirmed = true;
                attempt.identities_confirmed = true;
            }
            Err(diagnostic) => {
                attempt.status = if signals.is_cancelled() {
                    Status::Interrupted
                } else {
                    Status::Rejected
                };
                run.validation.diagnostics.push(*diagnostic);
            }
        }
        *run.attempts.last_mut().expect("started attempt") = attempt;
        journal.save(&run)?;
        if signals.is_cancelled() {
            break;
        }
    }
    // Recheck every path and volume at the end: validating later files must not
    // conceal a replacement or disconnect affecting an earlier action.
    let final_check = (|| {
        for action in &plan.body.actions {
            fs::open_bound(&action.keeper)?;
            fs::open_bound(&action.candidate)?;
        }
        validate_environment(plan, probe)
    })();
    match final_check {
        Ok(capacity) => run.capacity = capacity,
        Err(d) => run.validation.diagnostics.push(*d),
    }
    if signals.is_cancelled()
        && !run.validation.diagnostics.iter().any(|d| {
            matches!(
                d.code,
                Code::OperationInterrupted | Code::OperationTerminated
            )
        })
    {
        run.validation.diagnostics.push(*fs::interrupted(signals));
    }
    run.status = if signals.is_cancelled() {
        Status::Interrupted
    } else if run.validation.diagnostics.is_empty() {
        Status::Validated
    } else {
        Status::Rejected
    };
    run.validation.status = run.status;
    run.completed_at = Some(Utc::now().to_rfc3339());
    journal.save(&run)?;
    Ok(run)
}

fn new_run(
    plan: &ExecutionPlan,
    approval: &Approval,
    capacity: Vec<CapacityEvidence>,
) -> Result<ExecutionRun> {
    let logical = plan
        .body
        .actions
        .iter()
        .try_fold(0, |n, a| add(n, a.candidate.size_bytes))?;
    Ok(ExecutionRun { schema: RUN_SCHEMA.to_owned(), run_id: Uuid::now_v7().to_string(), plan_fingerprint: plan.fingerprint.clone(), authorization_id: approval.authorization_id.clone(),
        started_at: Utc::now().to_rfc3339(), completed_at: None, dry_run: true, status: Status::Validating, attempts: Vec::new(),
        validation: ValidationRecord { schema: VALIDATION_SCHEMA.to_owned(), status: Status::Validating, diagnostics: Vec::new() },
        commit: CommitRecord { schema: COMMIT_SCHEMA.to_owned(), status: "not_started".to_owned(), source_mutated: false, committed_actions: 0 },
        recovery: RecoveryRecord { schema: RECOVERY_SCHEMA.to_owned(), status: "not_needed".to_owned(), guarantee: "no_source_mutation".to_owned(), resume_requires_fresh_validation: true },
        capacity, savings: SavingsEvidence { selected_logical_bytes: logical, immediate_logical_reclaimed_bytes: 0, physical_reclaimed_bytes: None,
            physical_status: "unknown".to_owned(), reason: "No mutation occurred. Quarantine retains bytes; shared extents, snapshots and future deletion savings are unknown.".to_owned() },
        limitations: vec![
            "Live apply, restore, deletion and media replacement are unsupported.".to_owned(),
            "Validation is a point-in-time observation, never a reservation or permission to bypass revalidation.".to_owned(),
            "Same-filesystem rename atomicity is not tested by a dry run; cross-filesystem actions are never atomic.".to_owned(),
            "No crash-safe mutation or metadata-preservation guarantee is earned by this dry run; #91 must prove those guarantees before mutation.".to_owned(),
            "Other processes can consume capacity or change files after validation. Quotas, remote filesystem durability and concurrent hostile state-directory writers are not guaranteed.".to_owned(),
            "Reads can update access times. Optiflow writes only its evidence/state, outside source roots and quarantine.".to_owned(),
            "Hard-link aliases, nested mount traversal and non-Unix execution are unsupported in execution-plan v1.".to_owned(),
        ] })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::signals::Interruption;
    use clap::Parser;
    use std::fs;

    fn fixture() -> (
        tempfile::TempDir,
        ExecutionPlan,
        Approval,
        EffectivePolicyV1,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        for name in ["source", "state", "quarantine"] {
            fs::create_dir(base.join(name)).unwrap();
        }
        for name in ["keep", "candidate"] {
            fs::write(base.join("source").join(name), b"same").unwrap();
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
            keep: base.join("source/keep"),
            candidate: vec![base.join("source/candidate")],
            root: vec![base.join("source")],
            subtree: vec![],
            quarantine: base.join("quarantine"),
            max_actions: 1,
            max_in_flight_bytes: 1024,
            reserve_bytes: 1024,
            output: base.join("plan.json"),
        };
        let plan = create_plan(&args, &state, &policy, &SignalState::default()).unwrap();
        let approval = approve(&plan, &plan.fingerprint, "test operator").unwrap();
        (temp, plan, approval, policy)
    }

    #[test]
    fn bytes_and_path_identity_are_rechecked_after_hashes() {
        for replace in [false, true] {
            let (_temp, plan, _, _) = fixture();
            let action = &plan.body.actions[0];
            let path = action.candidate.path.to_path_buf();
            let mut hook = || {
                if replace {
                    let replacement = path.with_extension("replacement");
                    fs::write(&replacement, b"same").unwrap();
                    fs::rename(replacement, &path).unwrap();
                } else {
                    fs::write(&path, b"DIFF").unwrap();
                }
                Ok(())
            };
            let error =
                validate_pair_with_hook(action, &SignalState::default(), &mut hook).unwrap_err();
            assert_eq!(error.code, Code::ExecutionSourceStale);
            if !replace {
                assert!(error.message.contains("direct byte comparison"));
            }
        }
    }

    #[test]
    fn bounded_hash_refuses_both_growth_and_truncation() {
        let (_temp, plan, _, _) = fixture();
        let path = plan.body.actions[0].candidate.path.to_path_buf();
        for bytes in [b"larger".as_slice(), b"x".as_slice()] {
            fs::write(&path, bytes).unwrap();
            let mut file = File::open(&path).unwrap();
            assert_eq!(
                super::fs::hash_bound(&mut file, 4, &SignalState::default())
                    .unwrap_err()
                    .code,
                Code::ExecutionSourceStale
            );
        }
    }

    #[test]
    fn readonly_unmeasurable_and_insufficient_space_fail_before_journal_creation() {
        for expected in [
            Code::ExecutionReadOnly,
            Code::ExecutionCapacityUnavailable,
            Code::ExecutionCapacityInsufficient,
        ] {
            let (_temp, plan, approval, policy) = fixture();
            let state = plan.body.state.path.to_path_buf();
            let mut probe = |_: &File| {
                if expected == Code::ExecutionCapacityUnavailable {
                    Err(failure(expected, "injected unmeasurable capacity"))
                } else {
                    Ok(super::fs::Capacity {
                        available: if expected == Code::ExecutionCapacityInsufficient {
                            0
                        } else {
                            u64::MAX
                        },
                        block_size: 4096,
                        read_only: expected == Code::ExecutionReadOnly,
                    })
                }
            };
            let error = dry_run_with_probe(
                &plan,
                &approval,
                &state,
                &policy,
                &SignalState::default(),
                &mut probe,
            )
            .unwrap_err();
            assert_eq!(error.code, expected);
            assert_eq!(fs::read_dir(state).unwrap().count(), 0);
        }
    }

    #[test]
    fn space_depletion_after_content_validation_is_durably_rejected() {
        let (_temp, plan, approval, policy) = fixture();
        let state = plan.body.state.path.to_path_buf();
        let mut calls = 0;
        let mut probe = |_: &File| {
            calls += 1;
            Ok(super::fs::Capacity {
                available: if calls > 6 { 0 } else { u64::MAX },
                block_size: 4096,
                read_only: false,
            })
        };
        let run = dry_run_with_probe(
            &plan,
            &approval,
            &state,
            &policy,
            &SignalState::default(),
            &mut probe,
        )
        .unwrap();
        assert_eq!(run.status, Status::Rejected);
        assert!(run.attempts[0].bytes_confirmed);
        assert_eq!(
            run.validation.diagnostics[0].code,
            Code::ExecutionCapacityInsufficient
        );
        assert_eq!(
            super::super::load_execution(&state, &run.run_id)
                .unwrap()
                .unwrap()
                .status,
            Status::Rejected
        );
    }

    #[test]
    fn capacity_counts_all_retained_copies_and_journal_without_subtracting_savings() {
        let (_temp, mut plan, _, _) = fixture();
        // Exercise the accounting separately from host topology using a synthetic
        // second filesystem, while the same real directory checks still run.
        let mut measurements = BTreeMap::new();
        measurements.insert(
            "source".to_owned(),
            super::fs::Capacity {
                available: 100_000_000,
                block_size: 4096,
                read_only: false,
            },
        );
        measurements.insert(
            "destination".to_owned(),
            super::fs::Capacity {
                available: 100_000_000,
                block_size: 4096,
                read_only: false,
            },
        );
        plan.body.state.identity.filesystem_id = "source".to_owned();
        plan.body.quarantine.identity.filesystem_id = "destination".to_owned();
        for a in &mut plan.body.actions {
            a.keeper.identity.filesystem_id = "source".to_owned();
            a.candidate.identity.filesystem_id = "source".to_owned();
            a.topology = Topology::CrossFilesystem;
        }
        let first = plan.body.actions[0].clone();
        let mut second = first.clone();
        second.candidate.identity.file_id = "second".to_owned();
        plan.body.actions.push(second);
        let capacity = account_capacity_remaining(&plan, &measurements, 0).unwrap();
        let destination = capacity
            .iter()
            .find(|c| c.filesystem_id == "destination")
            .unwrap();
        assert_eq!(destination.retained_cross_filesystem_copy_bytes, 8192);
        assert_eq!(
            destination.projected_peak_additional_bytes,
            8192 + 2 * METADATA_PER_ACTION
        );
        assert_eq!(
            capacity
                .iter()
                .find(|c| c.filesystem_id == "source")
                .unwrap()
                .projected_peak_additional_bytes,
            JOURNAL_BUDGET + 2 * METADATA_PER_ACTION
        );
    }

    #[test]
    fn unknown_space_arithmetic_never_wraps() {
        assert_eq!(
            add(u64::MAX, 1).unwrap_err().code,
            Code::ExecutionBoundsExceeded
        );
        let (_temp, mut plan, _, _) = fixture();
        plan.body.bounds.free_space_reserve_bytes = u64::MAX;
        let error = validate_environment(&plan, &mut |_| {
            Ok(super::fs::Capacity {
                available: u64::MAX,
                block_size: 4096,
                read_only: false,
            })
        })
        .unwrap_err();
        assert_eq!(error.code, Code::ExecutionBoundsExceeded);
    }

    #[test]
    fn cancellation_has_no_source_commit_and_cannot_create_a_journal() {
        for interruption in [Interruption::Interrupt, Interruption::Terminate] {
            let (_temp, plan, approval, policy) = fixture();
            let state = plan.body.state.path.to_path_buf();
            let error = dry_run(
                &plan,
                &approval,
                &state,
                &policy,
                &SignalState::interrupted(interruption),
            )
            .unwrap_err();
            assert_eq!(
                error.code,
                if interruption == Interruption::Interrupt {
                    Code::OperationInterrupted
                } else {
                    Code::OperationTerminated
                }
            );
            assert!(!state.join("state.sqlite3").exists());
        }
    }

    #[test]
    fn abandoned_attempts_recover_as_interrupted_and_terminal_runs_stay_immutable() {
        let (_temp, plan, approval, _) = fixture();
        let state = plan.body.state.path.to_path_buf();
        let capacity = validate_environment(&plan, &mut super::fs::capacity).unwrap();
        let mut run = new_run(&plan, &approval, capacity).unwrap();
        run.attempts.push(Attempt {
            schema: ATTEMPT_SCHEMA.to_owned(),
            action_id: plan.body.actions[0].action_id.clone(),
            status: Status::Validating,
            hash_confirmed: false,
            bytes_confirmed: false,
            identities_confirmed: false,
        });
        let mut journal = Journal::open(&plan).unwrap();
        journal.begin(&plan, &approval, &run).unwrap();
        // A concurrent opener cannot steal the active run or mark it interrupted.
        assert!(Journal::open(&plan).is_err());
        assert_eq!(
            super::super::load_execution(&state, &run.run_id)
                .unwrap()
                .unwrap()
                .status,
            Status::Validating
        );
        drop(journal); // Simulates the OS releasing the lock on process exit.
        let mut recovered = Journal::open(&plan).unwrap();
        let saved = super::super::load_execution(&state, &run.run_id)
            .unwrap()
            .unwrap();
        assert_eq!(saved.status, Status::Interrupted);
        assert_eq!(saved.attempts[0].status, Status::Interrupted);
        assert_eq!(saved.recovery.status, "interrupted_dry_run");
        assert!(!saved.commit.source_mutated);
        assert!(recovered.save(&saved).is_err());
        drop(recovered);
        let _again = Journal::open(&plan).unwrap();
        assert_eq!(
            serde_json::to_value(&saved).unwrap(),
            serde_json::to_value(
                super::super::load_execution(&state, &run.run_id)
                    .unwrap()
                    .unwrap()
            )
            .unwrap()
        );
    }
}
