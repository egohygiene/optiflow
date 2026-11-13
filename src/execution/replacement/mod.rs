//! Replacement is a separate action family from byte-identical quarantine.
//! PNG review performs independent validation; approval retains the original.
use super::{
    filesystem as fs,
    model::{DirectoryBinding, FileBinding},
};
use crate::{candidate_artifact as artifact, domain::NativePath, signals::SignalState};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

pub const PLAN_SCHEMA: &str = "optiflow.replacement-plan.v1";
pub const APPROVAL_SCHEMA: &str = "optiflow.replacement-approval.v1";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub maximum_file_bytes: u64,
    pub free_space_reserve_bytes: u64,
    pub journal_budget_bytes: u64,
}
impl Limits {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=16_777_216).contains(&self.maximum_file_bytes)
                && (1..=1_099_511_627_776).contains(&self.free_space_reserve_bytes)
                && (65_536..=16_777_216).contains(&self.journal_budget_bytes),
            "replacement bounds differ from PNG profile"
        );
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanBody {
    pub operation: String,
    pub source: FileBinding,
    pub candidate: FileBinding,
    pub candidate_set: NativePath,
    pub marker_blake3: String,
    pub evidence_blake3: String,
    pub state: DirectoryBinding,
    pub retention: DirectoryBinding,
    pub limits: Limits,
    pub evidence_policy_fingerprint: String,
    pub encoded_logical_reduction_bytes: u64,
    pub original_disposition: String,
    pub qualification: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplacementPlan {
    pub schema: String,
    pub fingerprint: String,
    pub body: PlanBody,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub schema: String,
    pub authorization_id: String,
    pub plan_fingerprint: String,
    pub approved_by: String,
    pub authority: String,
    pub approved_at: String,
}
pub(super) fn pin(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn hash<T: Serialize>(value: &T) -> Result<String> {
    Ok(
        blake3::hash(&serde_json::to_vec(&serde_json::to_value(value)?)?)
            .to_hex()
            .to_string(),
    )
}
pub(super) fn diag<T>(value: super::Result<T>) -> Result<T> {
    value.map_err(|e| anyhow::anyhow!("{e:?}"))
}
impl ReplacementPlan {
    /// Self-consistency only. Applying always needs a fresh review and authority.
    pub fn validate(&self) -> Result<()> {
        self.body.limits.validate()?;
        let b = &self.body;
        for p in [
            &b.source.path,
            &b.candidate.path,
            &b.candidate_set,
            &b.state.path,
            &b.retention.path,
        ] {
            diag(fs::native_path(p))?;
        }
        ensure!(
            self.schema == PLAN_SCHEMA
                && self.fingerprint == hash(b)?
                && b.operation == "replace_validated_png.v1"
                && b.original_disposition == "retain_for_restore"
                && b.qualification == "unqualified_source_checkpoint"
                && pin(&b.marker_blake3)
                && pin(&b.evidence_blake3)
                && pin(&b.evidence_policy_fingerprint)
                && b.source.size_bytes > b.candidate.size_bytes
                && b.candidate.size_bytes > 0
                && b.source.size_bytes <= b.limits.maximum_file_bytes
                && b.encoded_logical_reduction_bytes
                    == b.source.size_bytes - b.candidate.size_bytes
                && b.source.identity.filesystem_id == b.retention.identity.filesystem_id
                && b.source.path != b.candidate.path,
            "replacement plan identity, retention or reduction differs"
        );
        Ok(())
    }
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= 4_194_304, "replacement plan exceeds 4 MiB");
        let raw: serde_json::Value = serde_json::from_slice(bytes)?;
        crate::contracts::validate(crate::contracts::Contract::ReplacementPlan, &raw)?;
        let p: Self = serde_json::from_slice(bytes)?;
        p.validate()?;
        Ok(p)
    }
}
impl Approval {
    /// Bounded closed declaration, checked against one exact reviewed PNG plan.
    /// Parsing conveys neither native qualification nor irreversible authority.
    pub fn from_json_slice(bytes: &[u8], plan: &ReplacementPlan) -> Result<Self> {
        ensure!(
            bytes.len() <= 16_384,
            "PNG replacement approval exceeds16KiB"
        );
        let raw: serde_json::Value = serde_json::from_slice(bytes)?;
        crate::contracts::validate(crate::contracts::Contract::ReplacementApproval, &raw)?;
        let value: Self = serde_json::from_slice(bytes)?;
        value.validate(plan)?;
        Ok(value)
    }
    pub fn validate(&self, plan: &ReplacementPlan) -> Result<()> {
        plan.validate()?;
        let mut copy = self.clone();
        copy.authorization_id.clear();
        ensure!(
            self.schema == APPROVAL_SCHEMA
                && self.authorization_id == hash(&copy)?
                && self.plan_fingerprint == plan.fingerprint
                && self.authority == "restorable_png_replacement_only"
                && !self.approved_by.trim().is_empty()
                && self.approved_by.len() <= 128
                && !self.approved_by.chars().any(char::is_control)
                && chrono::DateTime::parse_from_rfc3339(&self.approved_at).is_ok(),
            "replacement approval differs"
        );
        Ok(())
    }
}
pub fn approve(plan: &ReplacementPlan, approved_by: String) -> Result<Approval> {
    let mut a = Approval {
        schema: APPROVAL_SCHEMA.into(),
        authorization_id: String::new(),
        plan_fingerprint: plan.fingerprint.clone(),
        approved_by,
        authority: "restorable_png_replacement_only".into(),
        approved_at: chrono::Utc::now().to_rfc3339(),
    };
    a.authorization_id = hash(&a)?;
    a.validate(plan)?;
    Ok(a)
}
fn bytes(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let mut file = diag(fs::open(path, false))?;
    let mut data = Vec::new();
    (&mut file).take(maximum + 1).read_to_end(&mut data)?;
    ensure!(
        data.len() as u64 <= maximum,
        "replacement read exceeded bound"
    );
    Ok(data)
}
fn same_binding(a: &FileBinding, b: &FileBinding) -> Result<()> {
    ensure!(hash(a)? == hash(b)?, "replacement file binding changed");
    Ok(())
}
fn source_matches(e: &crate::png_production::SourceSnapshot, b: &FileBinding) -> bool {
    e.device_id.to_string() == b.identity.filesystem_id
        && e.inode.to_string() == b.identity.file_id
        && e.size_bytes == b.size_bytes
        && e.mode == b.mode
        && e.owner_uid == b.uid
        && e.group_gid == b.gid
        && e.modified_unix_ns == i128::from(b.modified_unix_ns)
        && e.changed_unix_ns == i128::from(b.changed_unix_ns)
        && e.link_count == 1
}
/// Read-only preparation from a CURRENT source and a committed candidate set.
/// Caller supplies exact existing private state/retention directories and policy.
pub fn review_png(
    source: &Path,
    set: &Path,
    state: &Path,
    retention: &Path,
    policy: &str,
    limits: Limits,
    signals: &SignalState,
) -> Result<ReplacementPlan> {
    limits.validate()?;
    ensure!(
        pin(policy) && !signals.is_cancelled(),
        "invalid policy pin or cancelled review"
    );
    artifact::validate_private_root(state)?;
    artifact::validate_private_root(retention)?;
    let bound_source = diag(fs::snapshot(source, signals, limits.maximum_file_bytes))?;
    ensure!(
        bound_source.identity.link_count == Some(1),
        "replacement source must be singly linked"
    );
    let state = diag(fs::directory(state))?;
    let retained = diag(fs::directory(retention))?;
    let source_parent = diag(fs::native_path(&bound_source.directory.path))?;
    diag(fs::outside_directories(
        &source_parent,
        &[&state, &retained],
    ))?;
    ensure!(
        state.identity.identity_key() != retained.identity.identity_key(),
        "state and retained originals require distinct directories"
    );
    let directory = artifact::open_directory(set)?;
    let observed = artifact::inspect(set);
    ensure!(
        observed.status == artifact::Status::Committed,
        "candidate set is not committed"
    );
    let manifest = observed.manifest.context("candidate marker absent")?;
    let marker = artifact::read_member(&directory, artifact::MARKER_NAME, 16_384)?;
    let evidence_bytes = artifact::read_member(
        &directory,
        artifact::EVIDENCE_NAME,
        artifact::MAX_EVIDENCE_BYTES,
    )?;
    let evidence: crate::png_production::CandidateEvidence =
        serde_json::from_slice(&evidence_bytes)?;
    crate::contracts::validate(crate::contracts::Contract::PngCandidateEvidence, &evidence)?;
    ensure!(
        evidence.source.path == bound_source.path
            && evidence.artifact_set_id == manifest.set_id
            && set.file_name().and_then(|s| s.to_str()) == Some(manifest.set_id.as_str())
            && evidence.effective_policy_fingerprint.algorithm == "blake3-256"
            && evidence.effective_policy_fingerprint.value == policy
            && source_matches(&evidence.source.before, &bound_source)
            && source_matches(&evidence.source.after, &bound_source),
        "candidate provenance does not bind current source/policy/set"
    );
    let candidate_path = set.join(artifact::CANDIDATE_NAME);
    let candidate = diag(fs::snapshot(
        &candidate_path,
        signals,
        limits.maximum_file_bytes,
    ))?;
    let source_bytes = bytes(source, limits.maximum_file_bytes)?;
    let candidate_bytes = artifact::read_member(
        &directory,
        artifact::CANDIDATE_NAME,
        limits.maximum_file_bytes as usize,
    )?;
    ensure!(
        blake3::hash(&source_bytes).to_hex().as_str() == bound_source.blake3
            && blake3::hash(&candidate_bytes).to_hex().as_str() == candidate.blake3,
        "replacement content changed"
    );
    let validated = crate::png_validation::validate_png_pair(
        &source_bytes,
        &candidate_bytes,
        crate::png_validation::ByteValidationLimits {
            source_bytes: limits.maximum_file_bytes as usize,
            candidate_bytes: limits.maximum_file_bytes as usize,
            decoded_bytes_per_image: 67_108_864,
            chunks_per_image: 256,
            decoder_allocation_bytes: 67_108_864,
        },
    )
    .map_err(|e| anyhow::anyhow!("independent current PNG validation refused: {e:?}"))?;
    ensure!(
        &evidence.source.content == validated.source()
            && &evidence.candidate.content == validated.candidate(),
        "candidate content identities differ"
    );
    same_binding(
        &bound_source,
        &diag(fs::snapshot(source, signals, limits.maximum_file_bytes))?,
    )?;
    same_binding(
        &candidate,
        &diag(fs::snapshot(
            &candidate_path,
            signals,
            limits.maximum_file_bytes,
        ))?,
    )?;
    ensure!(
        artifact::read_member(&directory, artifact::MARKER_NAME, 16_384)? == marker
            && artifact::read_member(
                &directory,
                artifact::EVIDENCE_NAME,
                artifact::MAX_EVIDENCE_BYTES
            )? == evidence_bytes
            && !signals.is_cancelled(),
        "candidate records changed or review cancelled"
    );
    let final_set = artifact::inspect(set);
    ensure!(
        final_set.status == artifact::Status::Committed
            && final_set.manifest.as_ref() == Some(&manifest),
        "candidate commitment changed after independent review"
    );
    let body = PlanBody {
        operation: "replace_validated_png.v1".into(),
        source: bound_source,
        candidate,
        candidate_set: NativePath::from_path(set),
        marker_blake3: blake3::hash(&marker).to_hex().to_string(),
        evidence_blake3: blake3::hash(&evidence_bytes).to_hex().to_string(),
        state,
        retention: retained,
        limits,
        evidence_policy_fingerprint: policy.into(),
        encoded_logical_reduction_bytes: validated.encoded_byte_reduction(),
        original_disposition: "retain_for_restore".into(),
        qualification: "unqualified_source_checkpoint".into(),
    };
    let plan = ReplacementPlan {
        schema: PLAN_SCHEMA.into(),
        fingerprint: hash(&body)?,
        body,
    };
    plan.validate()?;
    Ok(plan)
}
/// Fresh read-only comparison immediately before a future transaction.
pub fn recheck_png(
    plan: &ReplacementPlan,
    approval: &Approval,
    signals: &SignalState,
) -> Result<()> {
    approval.validate(plan)?;
    let b = &plan.body;
    let fresh = review_png(
        &diag(fs::native_path(&b.source.path))?,
        &diag(fs::native_path(&b.candidate_set))?,
        &diag(fs::native_path(&b.state.path))?,
        &diag(fs::native_path(&b.retention.path))?,
        &b.evidence_policy_fingerprint,
        b.limits.clone(),
        signals,
    )?;
    ensure!(
        fresh.fingerprint == plan.fingerprint,
        "current replacement review differs from approved plan"
    );
    Ok(())
}

#[cfg(test)]
mod tests;

pub mod transaction;
