use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Schema version constants
// ---------------------------------------------------------------------------

/// Legacy v1 constants – retained for reading stored v0.1.0 artifacts.
pub const RUN_SCHEMA_VERSION_V1: &str = "optiflow.run.v1";
pub const REPORT_SCHEMA_VERSION_V1: &str = "optiflow.report.v1";
pub const PLAN_SCHEMA_VERSION_V1: &str = "optiflow.plan.v1";

pub const RUN_SCHEMA_VERSION_V2: &str = "optiflow.run.v2";
pub const REPORT_SCHEMA_VERSION_V2: &str = "optiflow.report.v2";
pub const PLAN_SCHEMA_VERSION_V2: &str = "optiflow.plan.v2";

/// v3 constants – retained for reading stored artifacts that predate v4.
pub const RUN_SCHEMA_VERSION_V3: &str = "optiflow.run.v3";
pub const REPORT_SCHEMA_VERSION_V3: &str = "optiflow.report.v3";
pub const PLAN_SCHEMA_VERSION_V3: &str = "optiflow.plan.v3";

/// v4 constants – lossless `NativePath` artifacts that predate artifact sets.
pub const RUN_SCHEMA_VERSION_V4: &str = "optiflow.run.v4";
pub const REPORT_SCHEMA_VERSION_V4: &str = "optiflow.report.v4";
pub const PLAN_SCHEMA_VERSION_V4: &str = "optiflow.plan.v4";

/// Current v5 constants bind newly published documents to an artifact set.
pub const RUN_SCHEMA_VERSION: &str = "optiflow.run.v5";
pub const REPORT_SCHEMA_VERSION: &str = "optiflow.report.v5";
pub const PLAN_SCHEMA_VERSION: &str = "optiflow.plan.v5";

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_set_id: Option<String>,
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
// Path representation (v4 – NativePath)
// ---------------------------------------------------------------------------

/// A lossless, versioned, platform-native path representation.
///
/// `NativePath` serialises a filesystem path into a JSON-safe tagged union
/// without discarding any bytes.
///
/// * Paths that are valid UTF-8 are stored as `{"encoding":"utf8","value":"…"}`.
/// * Paths that contain non-UTF-8 bytes (possible on Unix) are stored as
///   `{"encoding":"unix_bytes","base64":"…"}` where the value is the
///   RFC 4648 standard base64 encoding of the raw path bytes.
///
/// The encoding is fully reversible on the platform that produced it.
/// `NativePath` values are suitable for use in JSON artifacts, SQLite rows,
/// plan files, and CLI output without any loss of identity.
///
/// **Display strings** derived from `NativePath::display()` are for human
/// consumption only and must never be used for filesystem access or as
/// unique keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "encoding", rename_all = "snake_case")]
pub enum NativePath {
    /// The path bytes are valid UTF-8.
    Utf8 { value: String },
    /// The path bytes are not valid UTF-8; raw bytes are base64-encoded.
    #[serde(rename = "unix_bytes")]
    UnixBytes { base64: String },
}

/// `SerializedPath` is a backward-compatible alias for `NativePath` retained
/// so that existing call-sites outside this module continue to compile while
/// the codebase migrates to the `NativePath` name.
pub type SerializedPath = NativePath;

impl NativePath {
    /// Encode a `Path` without forcing it through lossy UTF-8 conversion.
    ///
    /// On Unix the raw `OsStr` bytes are inspected directly; sequences that
    /// are not valid UTF-8 are stored as RFC 4648 base64.  On non-Unix
    /// platforms the path is converted to UTF-8 losslessly when possible and
    /// falls back to a best-effort lossy representation.
    #[cfg(unix)]
    pub fn from_path(path: &std::path::Path) -> Self {
        use std::os::unix::ffi::OsStrExt;
        let bytes = path.as_os_str().as_bytes();
        match std::str::from_utf8(bytes) {
            Ok(s) => NativePath::Utf8 {
                value: s.to_owned(),
            },
            Err(_) => NativePath::UnixBytes {
                base64: base64_encode(bytes),
            },
        }
    }

    /// Non-Unix fallback: convert to UTF-8 when possible; fall back to lossy.
    #[cfg(not(unix))]
    pub fn from_path(path: &std::path::Path) -> Self {
        NativePath::Utf8 {
            value: path.to_string_lossy().into_owned(),
        }
    }

    /// Decode back to a `PathBuf`, preserving the original bytes on Unix.
    ///
    /// On Unix `UnixBytes` paths are reconstructed from the raw byte sequence.
    /// On non-Unix platforms `UnixBytes` paths fall back to lossy UTF-8
    /// (the bytes cannot be represented as an `OsString` on those platforms).
    #[cfg(unix)]
    pub fn to_path_buf(&self) -> std::path::PathBuf {
        use std::os::unix::ffi::OsStrExt;
        match self {
            NativePath::Utf8 { value } => std::path::PathBuf::from(value),
            NativePath::UnixBytes { base64 } => {
                let bytes = base64_decode(base64);
                let os_str = std::ffi::OsStr::from_bytes(&bytes);
                std::path::PathBuf::from(os_str)
            }
        }
    }

    /// Non-Unix fallback: reconstruct from UTF-8; `UnixBytes` paths are
    /// decoded from base64 and then converted lossily.
    #[cfg(not(unix))]
    pub fn to_path_buf(&self) -> std::path::PathBuf {
        match self {
            NativePath::Utf8 { value } => std::path::PathBuf::from(value),
            NativePath::UnixBytes { base64 } => {
                let bytes = base64_decode(base64);
                // Best-effort: replace non-UTF-8 bytes with U+FFFD.
                let s = String::from_utf8_lossy(&bytes).into_owned();
                std::path::PathBuf::from(s)
            }
        }
    }

    /// Human-readable display string (lossy – may not round-trip to the exact
    /// same bytes on the filesystem).
    ///
    /// ASCII control characters in UTF-8 paths are escaped with Rust's
    /// `char::escape_default` so that terminal control sequences cannot be
    /// injected through path display.
    pub fn display(&self) -> std::borrow::Cow<'_, str> {
        match self {
            NativePath::Utf8 { value }
                if value.chars().all(|character| !character.is_control()) =>
            {
                std::borrow::Cow::Borrowed(value.as_str())
            }
            NativePath::Utf8 { value } => {
                std::borrow::Cow::Owned(value.chars().flat_map(char::escape_default).collect())
            }
            NativePath::UnixBytes { base64 } => {
                std::borrow::Cow::Owned(format!("<non-utf8:{base64}>"))
            }
        }
    }

    /// A stable string key suitable for use as a SQLite column value or as a
    /// `HashMap`/`BTreeMap` key.
    ///
    /// * UTF-8 paths return the string as-is.
    /// * Non-UTF-8 paths return `"\x00unix_bytes:" + base64`.  The leading
    ///   `NUL` byte is not valid in any filesystem path, so there is no risk
    ///   of collision with real UTF-8 paths.
    pub fn sqlite_key(&self) -> std::borrow::Cow<'_, str> {
        match self {
            NativePath::Utf8 { value } => std::borrow::Cow::Borrowed(value.as_str()),
            NativePath::UnixBytes { base64 } => {
                std::borrow::Cow::Owned(format!("\x00unix_bytes:{base64}"))
            }
        }
    }
}

// `NativePath` can be compared and sorted using a stable ordering that
// does not require heap allocation:
//
//  * `UnixBytes` paths sort before `Utf8` paths because the sqlite_key for
//    non-UTF-8 paths starts with a NUL byte (`\x00`), which is less than
//    any byte that can appear in a valid filesystem path.
//  * Within the same variant the inner string is compared directly.
//
// This ordering is consistent with the `sqlite_key()` byte ordering and
// produces a stable, deterministic sequence for duplicate-detection and
// plan generation without allocating on each comparison.
impl PartialOrd for NativePath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NativePath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (NativePath::UnixBytes { base64: a }, NativePath::UnixBytes { base64: b }) => a.cmp(b),
            (NativePath::Utf8 { value: a }, NativePath::Utf8 { value: b }) => a.cmp(b),
            // UnixBytes sorts before Utf8 (NUL-prefixed key < any real path byte).
            (NativePath::UnixBytes { .. }, NativePath::Utf8 { .. }) => std::cmp::Ordering::Less,
            (NativePath::Utf8 { .. }, NativePath::UnixBytes { .. }) => std::cmp::Ordering::Greater,
        }
    }
}

/// Minimal base64 encoding (no external dependency – standard alphabet with
/// padding, RFC 4648 §4).
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity((bytes.len() * 4).div_ceil(3));
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 {
            chunk[1] as usize
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            chunk[2] as usize
        } else {
            0
        };
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

/// Minimal base64 decoding (inverse of `base64_encode`).
///
/// Accepts standard RFC 4648 base64 (with or without padding).  Invalid
/// characters are silently skipped, which matches the lenient behaviour
/// common in base64 decoders and is safe because the input always originates
/// from our own encoder.
fn base64_decode(encoded: &str) -> Vec<u8> {
    const DECODE: [u8; 128] = {
        let mut table = [0xff_u8; 128];
        let mut i = 0u8;
        while i < 64 {
            let ch =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"[i as usize];
            table[ch as usize] = i;
            i += 1;
        }
        table
    };

    let mut output = Vec::with_capacity(encoded.len() * 3 / 4);
    let bytes: Vec<u8> = encoded
        .bytes()
        .filter(|&b| b != b'=' && (b as usize) < 128 && DECODE[b as usize] != 0xff)
        .collect();

    for chunk in bytes.chunks(4) {
        let v0 = DECODE[chunk[0] as usize] as u32;
        let v1 = if chunk.len() > 1 {
            DECODE[chunk[1] as usize] as u32
        } else {
            0
        };
        let v2 = if chunk.len() > 2 {
            DECODE[chunk[2] as usize] as u32
        } else {
            0
        };
        let v3 = if chunk.len() > 3 {
            DECODE[chunk[3] as usize] as u32
        } else {
            0
        };
        let combined = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
        output.push(((combined >> 16) & 0xff) as u8);
        if chunk.len() > 2 {
            output.push(((combined >> 8) & 0xff) as u8);
        }
        if chunk.len() > 3 {
            output.push((combined & 0xff) as u8);
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
    fn base64_decode_inverts_encode() {
        // RFC 4648 §10 test vectors (roundtrip).
        for original in [
            b"".as_ref(),
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"fooba",
            b"foobar",
        ] {
            let encoded = base64_encode(original);
            let decoded = base64_decode(&encoded);
            assert_eq!(
                decoded.as_slice(),
                original,
                "roundtrip failed for {:?}",
                original
            );
        }
    }

    #[test]
    fn base64_decode_handles_arbitrary_bytes() {
        // Ensure every byte value round-trips through encode/decode.
        let all_bytes: Vec<u8> = (0u8..=255).collect();
        let encoded = base64_encode(&all_bytes);
        let decoded = base64_decode(&encoded);
        assert_eq!(decoded, all_bytes);
    }

    #[test]
    fn native_path_utf8_roundtrips() {
        let path = std::path::Path::new("/tmp/example/file.txt");
        let native = NativePath::from_path(path);
        match &native {
            NativePath::Utf8 { value } => assert_eq!(value, "/tmp/example/file.txt"),
            NativePath::UnixBytes { .. } => panic!("expected Utf8 variant"),
        }
        assert_eq!(native.display(), "/tmp/example/file.txt");
        assert_eq!(native.to_path_buf(), path);
    }

    #[test]
    fn native_path_display_escapes_terminal_control_characters() {
        let native = NativePath::Utf8 {
            value: "/tmp/line\n\u{1b}[31m.txt".to_owned(),
        };

        assert_eq!(native.display(), "/tmp/line\\n\\u{1b}[31m.txt");
    }

    #[cfg(unix)]
    #[test]
    fn native_path_non_utf8_unix_roundtrips() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        // A byte sequence that is not valid UTF-8.
        let raw: &[u8] = b"/tmp/\xff\xfe/file.bin";
        let os_str = OsStr::from_bytes(raw);
        let path = std::path::Path::new(os_str);

        let native = NativePath::from_path(path);
        // Must be encoded as unix_bytes, not utf8.
        match &native {
            NativePath::UnixBytes { .. } => {}
            NativePath::Utf8 { .. } => panic!("expected UnixBytes variant for non-UTF-8 path"),
        }

        // sqlite_key must differ from the lossy display form.
        let key = native.sqlite_key();
        assert!(key.starts_with('\x00'), "sqlite_key must have NUL sentinel");

        // to_path_buf() must round-trip to the exact original bytes.
        let reconstructed = native.to_path_buf();
        assert_eq!(
            reconstructed.as_os_str().as_bytes(),
            raw,
            "to_path_buf must exactly reproduce original bytes"
        );
    }

    #[cfg(unix)]
    #[test]
    fn native_path_non_utf8_serialisation_roundtrips() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let raw: &[u8] = b"/data/\x80\x81\x82/media.mkv";
        let os_str = OsStr::from_bytes(raw);
        let path = std::path::Path::new(os_str);

        let native = NativePath::from_path(path);

        // Serialise to JSON and deserialise back – must be bit-identical.
        let json = serde_json::to_string(&native).expect("serialise");
        let deserialised: NativePath = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(native, deserialised);

        // The reconstructed path must match the original bytes.
        let reconstructed = deserialised.to_path_buf();
        assert_eq!(reconstructed.as_os_str().as_bytes(), raw);
    }

    #[test]
    fn native_path_sqlite_key_is_stable_for_utf8_paths() {
        let native = NativePath::Utf8 {
            value: "/home/user/file.txt".to_owned(),
        };
        assert_eq!(native.sqlite_key(), "/home/user/file.txt");
    }

    #[test]
    fn native_path_ordering_is_consistent() {
        let a = NativePath::Utf8 {
            value: "/a".to_owned(),
        };
        let b = NativePath::Utf8 {
            value: "/b".to_owned(),
        };
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a.cmp(&a), std::cmp::Ordering::Equal);

        // UnixBytes sorts before Utf8 because the sqlite_key starts with NUL.
        let non_utf8 = NativePath::UnixBytes {
            base64: "Zm9v".to_owned(),
        };
        assert!(non_utf8 < a, "UnixBytes must sort before Utf8");
    }

    #[test]
    fn checked_in_schemas_match_current_contract_versions() {
        let cases = [
            (
                include_str!("../schemas/run.schema.json"),
                RUN_SCHEMA_VERSION,
            ),
            (
                include_str!("../schemas/report.schema.json"),
                REPORT_SCHEMA_VERSION,
            ),
            (
                include_str!("../schemas/plan.schema.json"),
                PLAN_SCHEMA_VERSION,
            ),
        ];

        for (schema, expected_version) in cases {
            let document: serde_json::Value =
                serde_json::from_str(schema).expect("checked-in schema must be valid JSON");
            let declared_version = document
                .pointer("/properties/schema_version/const")
                .and_then(serde_json::Value::as_str);
            assert_eq!(declared_version, Some(expected_version));
        }
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
    // --- v4: lossless NativePath (was String) ---
    pub path: NativePath,
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
    // --- v4: lossless NativePath (was Vec<String>) ---
    pub observed_paths: Vec<NativePath>,
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
    // --- v4: lossless NativePath (was String) ---
    pub path: NativePath,
    pub observation_id: String,
    /// Hard-link alias paths observed for the same filesystem object.
    #[serde(default)]
    // --- v4: lossless NativePath (was Vec<String>) ---
    pub alias_paths: Vec<NativePath>,
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
    pub source_artifact_set_id: Option<String>,
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
    // --- v4: lossless NativePath (was String) ---
    pub keep_path: NativePath,
    /// All observed paths belonging to the keeper filesystem object (including
    /// hard-link aliases).
    #[serde(default)]
    // --- v4: lossless NativePath (was Vec<String>) ---
    pub keep_alias_paths: Vec<NativePath>,
    // --- v4: lossless NativePath (was Vec<String>) ---
    pub candidate_paths: Vec<NativePath>,
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
    // --- v4: lossless NativePath (was String) ---
    pub path: NativePath,
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
