//! Versioned evidence only. No type in this module implements source mutation.
use serde::{Deserialize, Serialize};

use crate::domain::NativePath;
use crate::filesystem::identity::FilesystemIdentity;
use crate::outcome::Diagnostic;

pub const PLAN_SCHEMA: &str = "optiflow.execution-plan.v1";
pub const APPROVAL_SCHEMA: &str = "optiflow.execution-approval.v1";
pub const RUN_SCHEMA: &str = "optiflow.execution-run.v1";
pub const ATTEMPT_SCHEMA: &str = "optiflow.execution-attempt.v1";
pub const VALIDATION_SCHEMA: &str = "optiflow.execution-validation.v1";
pub const COMMIT_SCHEMA: &str = "optiflow.execution-commit.v1";
pub const RECOVERY_SCHEMA: &str = "optiflow.execution-recovery.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectoryBinding {
    pub path: NativePath,
    pub identity: FilesystemIdentity,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileBinding {
    pub directory: DirectoryBinding,
    pub path: NativePath,
    pub identity: FilesystemIdentity,
    pub size_bytes: u64,
    pub modified_unix_ns: i64,
    pub changed_unix_ns: i64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub blake3: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topology {
    SameFilesystem,
    CrossFilesystem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactAction {
    pub action_id: String,
    pub operation: String,
    pub keeper: FileBinding,
    pub candidate: FileBinding,
    pub topology: Topology,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    pub max_actions: u64,
    pub max_in_flight_files: u64,
    pub max_in_flight_bytes: u64,
    pub free_space_reserve_bytes: u64,
    pub journal_budget_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanBody {
    pub created_at: String,
    pub producer_version: String,
    pub selection: String,
    pub evidence_policy_fingerprint: String,
    pub roots: Vec<DirectoryBinding>,
    pub subtrees: Vec<DirectoryBinding>,
    pub quarantine: DirectoryBinding,
    pub state: DirectoryBinding,
    pub bounds: Bounds,
    pub actions: Vec<ExactAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlan {
    pub schema: String,
    pub fingerprint: String,
    pub body: PlanBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalBody {
    pub plan_fingerprint: String,
    pub authority: String,
    pub approved_by: String,
    pub approved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub schema: String,
    pub authorization_id: String,
    pub body: ApprovalBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Validating,
    Validated,
    Rejected,
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub schema: String,
    pub action_id: String,
    pub status: Status,
    pub hash_confirmed: bool,
    pub bytes_confirmed: bool,
    pub identities_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationRecord {
    pub schema: String,
    pub status: Status,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitRecord {
    pub schema: String,
    pub status: String,
    pub source_mutated: bool,
    pub committed_actions: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryRecord {
    pub schema: String,
    pub status: String,
    pub guarantee: String,
    pub resume_requires_fresh_validation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapacityEvidence {
    pub filesystem_id: String,
    pub available_bytes: u64,
    pub source_logical_bytes: u64,
    pub retained_cross_filesystem_copy_bytes: u64,
    pub journal_and_metadata_budget_bytes: u64,
    pub projected_peak_additional_bytes: u64,
    pub reserve_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavingsEvidence {
    pub selected_logical_bytes: u64,
    pub immediate_logical_reclaimed_bytes: u64,
    pub physical_reclaimed_bytes: Option<u64>,
    pub physical_status: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionRun {
    pub schema: String,
    pub run_id: String,
    pub plan_fingerprint: String,
    pub authorization_id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub dry_run: bool,
    pub status: Status,
    pub attempts: Vec<Attempt>,
    pub validation: ValidationRecord,
    pub commit: CommitRecord,
    pub recovery: RecoveryRecord,
    pub capacity: Vec<CapacityEvidence>,
    pub savings: SavingsEvidence,
    pub limitations: Vec<String>,
}
