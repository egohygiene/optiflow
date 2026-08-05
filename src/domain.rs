use serde::{Deserialize, Serialize};

pub const RUN_SCHEMA_VERSION: &str = "optiflow.run.v1";
pub const REPORT_SCHEMA_VERSION: &str = "optiflow.report.v1";
pub const PLAN_SCHEMA_VERSION: &str = "optiflow.plan.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanOptions {
    pub follow_symlinks: bool,
    pub include_hidden: bool,
    pub cross_filesystems: bool,
    pub probe_media: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanRun {
    pub schema_version: String,
    pub run_id: String,
    pub created_at: String,
    pub completed_at: String,
    pub inputs: Vec<String>,
    pub options: ScanOptions,
    pub artifact_directory: String,
    pub discovered_files: u64,
    pub analyzed_files: u64,
    pub cache_hits: u64,
    pub total_bytes: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Image,
    Video,
    Audio,
    Other,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObservationStatus {
    Analyzed,
    Unsupported,
    Unreadable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDescriptor {
    pub format_name: Option<String>,
    pub duration_seconds: Option<f64>,
    pub bit_rate: Option<u64>,
    pub streams: Vec<MediaStream>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaStream {
    pub index: u32,
    pub codec_type: Option<String>,
    pub codec_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileObservation {
    pub observation_id: String,
    pub run_id: String,
    pub path: String,
    pub size_bytes: u64,
    pub modified_unix_ns: Option<i64>,
    pub device_id: Option<u64>,
    pub inode: Option<u64>,
    pub content_type: Option<String>,
    pub media_kind: MediaKind,
    pub content_hash: Option<String>,
    pub hash_algorithm: Option<String>,
    pub media: Option<MediaDescriptor>,
    pub status: ObservationStatus,
    pub cache_hit: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateMember {
    pub path: String,
    pub observation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub group_id: String,
    pub classification: String,
    pub evidence: ExactDuplicateEvidence,
    pub members: Vec<DuplicateMember>,
    pub reclaimable_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExactDuplicateEvidence {
    pub algorithm: String,
    pub complete_content_hash: String,
    pub identical_size_bytes: u64,
    pub member_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub file_count: u64,
    pub total_bytes: u64,
    pub media_files: u64,
    pub unsupported_files: u64,
    pub unreadable_files: u64,
    pub exact_duplicate_groups: u64,
    pub exact_duplicate_files: u64,
    pub reclaimable_bytes: u64,
    pub cache_hits: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub schema_version: String,
    pub generated_at: String,
    pub run: ScanRun,
    pub summary: ScanSummary,
    pub duplicate_groups: Vec<DuplicateGroup>,
    pub observations: Vec<FileObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub schema_version: String,
    pub plan_id: String,
    pub source_run_id: String,
    pub created_at: String,
    pub mode: String,
    pub safety: PlanSafety,
    pub summary: PlanSummary,
    pub actions: Vec<PlanAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSafety {
    pub mutates_files: bool,
    pub requires_explicit_apply: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSummary {
    pub action_count: u64,
    pub candidate_file_count: u64,
    pub potential_reclaimable_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanAction {
    pub action_id: String,
    pub classification: String,
    pub proposed_operation: String,
    pub keep_path: String,
    pub candidate_paths: Vec<String>,
    pub potential_reclaimable_bytes: u64,
    pub reason: String,
    pub evidence: ExactDuplicateEvidence,
    pub preconditions: Vec<FilePrecondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePrecondition {
    pub path: String,
    pub expected_size_bytes: u64,
    pub expected_modified_unix_ns: Option<i64>,
    pub expected_complete_content_hash: String,
    pub required_apply_behavior: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStatus {
    pub name: String,
    pub required_for: String,
    pub available: bool,
    pub executable: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub optiflow_version: String,
    pub platform: String,
    pub state_directory: String,
    pub state_ready: bool,
    pub tools: Vec<ToolStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatus {
    pub state_directory: String,
    pub database_path: String,
    pub database_size_bytes: u64,
    pub cached_file_count: u64,
    pub stored_run_count: u64,
}

#[derive(Debug, Clone)]
pub struct CachedAnalysis {
    pub content_type: Option<String>,
    pub media_kind: MediaKind,
    pub content_hash: Option<String>,
    pub media: Option<MediaDescriptor>,
    pub probe_signature: Option<String>,
    pub status: ObservationStatus,
    pub warnings: Vec<String>,
}
