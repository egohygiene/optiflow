use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Schema version constants
// ---------------------------------------------------------------------------

/// Legacy v1 constants – retained for reading stored v0.1.0 artifacts.
pub const RUN_SCHEMA_VERSION_V1: &str = "optiflow.run.v1";
pub const REPORT_SCHEMA_VERSION_V1: &str = "optiflow.report.v1";
pub const PLAN_SCHEMA_VERSION_V1: &str = "optiflow.plan.v1";

/// Current v3 constants emitted for newly created runs and plans.
pub const RUN_SCHEMA_VERSION_V2: &str = "optiflow.run.v2";
pub const REPORT_SCHEMA_VERSION_V2: &str = "optiflow.report.v2";
pub const PLAN_SCHEMA_VERSION_V2: &str = "optiflow.plan.v2";

pub const RUN_SCHEMA_VERSION: &str = "optiflow.run.v3";
pub const REPORT_SCHEMA_VERSION: &str = "optiflow.report.v3";
pub const PLAN_SCHEMA_VERSION: &str = "optiflow.plan.v3";

// ---------------------------------------------------------------------------
// Scan options and run metadata
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Path representation
// ---------------------------------------------------------------------------

/// A platform-native path serialised to a lossless JSON-safe encoding.
///
/// Paths that are valid UTF-8 are stored as plain strings.  Paths that
/// contain non-UTF-8 bytes (possible on Unix) are stored as base64-encoded
/// raw bytes.  The encoding is reversible on the platform that emitted it.
///
/// Display strings derived from `SerializedPath` must never be used for
/// filesystem access or as unique keys; they are for human consumption only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "encoding", rename_all = "snake_case")]
pub enum SerializedPath {
    /// The path bytes are valid UTF-8.
    Utf8 { value: String },
    /// The path bytes are not valid UTF-8; raw bytes are base64-encoded.
    #[serde(rename = "unix_bytes")]
    UnixBytes { base64: String },
}

impl SerializedPath {
    /// Encode a `Path` without forcing it through lossy UTF-8 conversion.
    ///
    /// On Unix, the raw `OsStr` bytes are inspected; non-UTF-8 sequences are
    /// stored as base64.  On other platforms the path is converted losslessly
    /// when possible and stored as UTF-8; otherwise a best-effort lossy
    /// representation is used.
    #[cfg(unix)]
    pub fn from_path(path: &std::path::Path) -> Self {
        use std::os::unix::ffi::OsStrExt;
        let bytes = path.as_os_str().as_bytes();
        match std::str::from_utf8(bytes) {
            Ok(s) => SerializedPath::Utf8 {
                value: s.to_owned(),
            },
            Err(_) => SerializedPath::UnixBytes {
                base64: base64_encode(bytes),
            },
        }
    }

    /// Non-Unix fallback: convert to UTF-8 when possible; fall back to lossy.
    #[cfg(not(unix))]
    pub fn from_path(path: &std::path::Path) -> Self {
        SerializedPath::Utf8 {
            value: path.to_string_lossy().into_owned(),
        }
    }

    /// Human-readable display string (lossy – may not round-trip to the exact
    /// same bytes).
    pub fn display(&self) -> std::borrow::Cow<'_, str> {
        match self {
            SerializedPath::Utf8 { value } => std::borrow::Cow::Borrowed(value.as_str()),
            SerializedPath::UnixBytes { base64 } => {
                std::borrow::Cow::Owned(format!("<non-utf8:{base64}>"))
            }
        }
    }
}

/// Minimal base64 encoding (no external dependency – standard alphabet without
/// padding, using the `std` alphabet).
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity((bytes.len() * 4).div_ceil(3));
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        output.push(ALPHABET[(b0 >> 2) & 0x3f] as char);
        output.push(ALPHABET[((b0 << 4) | (b1 >> 4)) & 0x3f] as char);
        if chunk.len() > 1 {
            output.push(ALPHABET[((b1 << 2) | (b2 >> 6)) & 0x3f] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            output.push('=');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encode_produces_rfc4648_standard_output() {
        // RFC 4648 §10 test vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn serialized_path_utf8_roundtrips() {
        let path = std::path::Path::new("/tmp/example/file.txt");
        let serialized = SerializedPath::from_path(path);
        match &serialized {
            SerializedPath::Utf8 { value } => assert_eq!(value, "/tmp/example/file.txt"),
            SerializedPath::UnixBytes { .. } => panic!("expected Utf8 variant"),
        }
        assert_eq!(serialized.display(), "/tmp/example/file.txt");
    }
}

// ---------------------------------------------------------------------------
// Observation stability
// ---------------------------------------------------------------------------

/// Why an observation is considered unstable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationStability {
    /// All stability checks passed; evidence is from a single consistent state.
    Stable,
    /// The file changed (size or mtime) while being hashed.
    ChangedDuringHash,
    /// The file changed while being probed by an external tool.
    ChangedDuringProbe,
    /// The path now refers to a different filesystem object than was opened.
    ReplacedDuringScan,
    /// The path disappeared after discovery.
    DisappearedDuringScan,
    /// The path became a symbolic link after the initial metadata check.
    BecameSymlink,
    /// The file moved across a filesystem boundary during the scan.
    FilesystemBoundaryChanged,
    /// Required metadata was unavailable; stability could not be verified.
    MetadataUnavailable,
    /// The retry limit was exhausted; the observation remains unstable.
    RetryExhausted,
    /// The file could not be read.
    Unreadable,
}

impl Default for ObservationStability {
    /// Default to `Stable` so that older artifacts without the field behave as
    /// if they were stable (best-effort backward compatibility).
    fn default() -> Self {
        ObservationStability::Stable
    }
}

/// Whether the recorded evidence is current and trusted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceValidity {
    /// Evidence was collected from a single stable observation window.
    Current,
    /// Evidence was collected but the observation became unstable afterwards;
    /// the evidence must not be used for exact-duplicate decisions.
    Stale,
    /// Evidence could not be collected at all.
    Unavailable,
}

impl Default for EvidenceValidity {
    /// Default to `Current` for backward compatibility with older artifacts.
    fn default() -> Self {
        EvidenceValidity::Current
    }
}

// ---------------------------------------------------------------------------
// Media classification
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Filesystem identity (v2)
// ---------------------------------------------------------------------------

/// Stable filesystem identity for a single physical file object.
///
/// Re-exported here from `crate::filesystem::identity` so that callers that
/// only use domain types don't need to know the module layout.
pub use crate::filesystem::identity::{
    AllocationSource, ExtentSharingStatus, FilesystemIdentity, PhysicalReclaimability,
    ReclaimabilityReasonCode, ReclaimabilityStatus, StorageAllocation,
};

// ---------------------------------------------------------------------------
// File observation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileObservation {
    pub observation_id: String,
    pub run_id: String,
    pub path: String,
    pub size_bytes: u64,
    pub modified_unix_ns: Option<i64>,
    /// Device (filesystem) identifier – kept for compatibility; also encoded
    /// in `filesystem_identity` when available.
    pub device_id: Option<u64>,
    /// Inode number – kept for compatibility; also encoded in
    /// `filesystem_identity` when available.
    pub inode: Option<u64>,
    pub content_type: Option<String>,
    pub media_kind: MediaKind,
    pub content_hash: Option<String>,
    pub hash_algorithm: Option<String>,
    pub media: Option<MediaDescriptor>,
    pub status: ObservationStatus,
    pub cache_hit: bool,
    pub warnings: Vec<String>,
    // --- v2 additions ---
    /// Stable filesystem identity collected during the current scan.
    /// `None` when the platform does not expose stable identity.
    pub filesystem_identity: Option<FilesystemIdentity>,
    /// Allocated-storage metadata collected during the current scan.
    pub storage_allocation: Option<StorageAllocation>,
    // --- v3 additions ---
    /// Whether the file appeared stable throughout the observation window.
    #[serde(default)]
    pub observation_stability: ObservationStability,
    /// Whether the recorded hash and media evidence is trustworthy.
    #[serde(default)]
    pub evidence_validity: EvidenceValidity,
    /// Number of observation attempts made (1 on first success; >1 on retry).
    #[serde(default = "default_attempt_count")]
    pub attempt_count: u32,
}

fn default_attempt_count() -> u32 {
    1
}

// ---------------------------------------------------------------------------
// Hard-link groups (v2)
// ---------------------------------------------------------------------------

/// One or more observed paths that all reference the same filesystem object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardLinkGroup {
    pub group_id: String,
    pub identity: FilesystemIdentity,
    /// All observed paths that map to this identity.
    pub observed_paths: Vec<String>,
    /// Number of observed paths (convenience field).
    pub observed_path_count: u64,
    /// Hard-link count as reported by the filesystem.
    pub reported_link_count: Option<u64>,
    /// Links to the object that were *not* observed in the scan inputs.
    /// `None` when `reported_link_count` is unavailable.
    pub unobserved_link_count: Option<u64>,
    /// Logical size of the shared object.
    pub logical_size_bytes: u64,
    /// Allocated size of the shared object, when available.
    pub allocated_size_bytes: Option<u64>,
    /// Non-fatal warnings from identity or allocation collection.
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Duplicate groups (v2)
// ---------------------------------------------------------------------------

/// One path (and its aliases) within a duplicate group.
///
/// `primary_path` is the path that was selected as the representative for
/// this member; `alias_paths` are additional paths that resolve to the same
/// filesystem object (hard-link aliases).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateMember {
    /// Primary (representative) path for this filesystem object.
    pub path: String,
    pub observation_id: String,
    /// Hard-link alias paths observed for the same filesystem object.
    #[serde(default)]
    pub alias_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub group_id: String,
    pub classification: String,
    pub evidence: ExactDuplicateEvidence,
    /// Members represent *unique filesystem objects*, not merely unique paths.
    pub members: Vec<DuplicateMember>,
    /// Logical bytes that could be reclaimed if all but one member were
    /// removed – computed from unique-object sizes only, not path counts.
    pub reclaimable_bytes: u64,
    /// Physical reclaimability assessment (v2).
    #[serde(default = "default_physical_reclaimability")]
    pub physical_reclaimability: PhysicalReclaimability,
}

fn default_physical_reclaimability() -> PhysicalReclaimability {
    PhysicalReclaimability::unknown(vec![ReclaimabilityReasonCode::ExtentSharingUnknown])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExactDuplicateEvidence {
    pub algorithm: String,
    pub complete_content_hash: String,
    pub identical_size_bytes: u64,
    /// Number of *unique filesystem objects* (not paths) in the group.
    pub member_count: u64,
    /// Total number of observed paths, including hard-link aliases (v2).
    #[serde(default)]
    pub observed_path_count: u64,
}

// ---------------------------------------------------------------------------
// Storage accounting summary (v2)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSummary {
    /// Sum of logical sizes across every observed path (including aliases).
    pub path_logical_bytes: u64,
    /// Sum of logical sizes counted once per unique filesystem object.
    pub unique_object_logical_bytes: u64,
    /// Sum of allocated bytes counted once per object where available.
    pub known_allocated_bytes: u64,
    /// Number of objects for which allocation metadata was unavailable.
    pub unknown_allocation_object_count: u64,
    /// Logical bytes attributable to hard-link alias paths (paths beyond the
    /// first observed path to a given object).
    pub hard_link_alias_logical_bytes: u64,
    /// Logical bytes attributable to independent exact-duplicate objects.
    pub duplicate_logical_bytes: u64,
    /// Estimated reclaimable allocated bytes; `None` when unknown.
    pub estimated_reclaimable_allocated_bytes: Option<u64>,
    /// Physical reclaimability status across all duplicate groups.
    pub physical_reclaimability: PhysicalReclaimability,
}

// ---------------------------------------------------------------------------
// Scan summary and report
// ---------------------------------------------------------------------------

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
    // --- v2 additions ---
    /// Number of unique filesystem objects observed (paths de-duplicated by
    /// identity).
    #[serde(default)]
    pub unique_object_count: u64,
    /// Number of observed paths that are hard-link aliases.
    #[serde(default)]
    pub hard_link_alias_path_count: u64,
    // --- v3 additions ---
    /// Number of observations excluded from exact-duplicate decisions because
    /// the file appeared to change during scanning.
    #[serde(default)]
    pub unstable_observation_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub schema_version: String,
    pub generated_at: String,
    pub run: ScanRun,
    pub summary: ScanSummary,
    pub duplicate_groups: Vec<DuplicateGroup>,
    pub observations: Vec<FileObservation>,
    // --- v2 additions ---
    /// Groups of paths sharing the same filesystem identity (hard-link groups).
    #[serde(default)]
    pub hard_link_groups: Vec<HardLinkGroup>,
    /// Detailed storage accounting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageSummary>,
}

// ---------------------------------------------------------------------------
// Plan
// ---------------------------------------------------------------------------

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
    /// All observed paths belonging to the keeper filesystem object (including
    /// hard-link aliases).
    #[serde(default)]
    pub keep_alias_paths: Vec<String>,
    pub candidate_paths: Vec<String>,
    pub potential_reclaimable_bytes: u64,
    /// Physical reclaimability assessment (v2).
    #[serde(default = "default_physical_reclaimability")]
    pub physical_reclaimability: PhysicalReclaimability,
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

// ---------------------------------------------------------------------------
// Doctor and cache status
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Cache analysis (internal)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CachedAnalysis {
    pub content_type: Option<String>,
    pub media_kind: MediaKind,
    pub content_hash: Option<String>,
    pub media: Option<MediaDescriptor>,
    pub probe_signature: Option<String>,
    pub status: ObservationStatus,
    pub warnings: Vec<String>,
    /// Stability of the observation window (set by hashing stage; defaults to
    /// `Stable` for cache hits and non-duplicate files).
    pub observation_stability: ObservationStability,
    /// Whether the content evidence should be trusted for exact-duplicate grouping.
    pub evidence_validity: EvidenceValidity,
    /// Number of hash attempts made.
    pub attempt_count: u32,
}
