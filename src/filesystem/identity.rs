use serde::{Deserialize, Serialize};

/// Stable identity key for a filesystem object on a single platform.
///
/// An object is uniquely identified only by the pair (filesystem_id, file_id).
/// Inode numbers alone are not globally unique across mounted filesystems.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FilesystemIdentity {
    /// Platform string, e.g. "linux" or "macos".
    pub platform: String,
    /// Opaque filesystem or volume identifier serialised as a string (avoids
    /// JSON integer precision issues on large device numbers).
    pub filesystem_id: String,
    /// Opaque file identifier serialised as a string (inode on Unix).
    pub file_id: String,
    /// Hard-link count reported by the filesystem, when available.
    pub link_count: Option<u64>,
}

impl FilesystemIdentity {
    /// A stable sort key composed only of the fields that establish identity.
    pub fn identity_key(&self) -> (&str, &str) {
        (self.filesystem_id.as_str(), self.file_id.as_str())
    }
}

/// How allocated-size metadata was obtained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocationSource {
    /// `st_blocks` × 512 (POSIX semantics).
    StBlocks512,
    /// Allocation information was not available on this platform.
    Unavailable,
}

/// What is known about whether extents are physically shared (copy-on-write,
/// reflink, APFS clone, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtentSharingStatus {
    /// The platform did not expose extent-sharing information; sharing is
    /// neither confirmed nor ruled out.
    Unknown,
    /// The platform indicated no extent sharing (informational only; does not
    /// prove physical independence without further verification).
    NotDetected,
}

/// Physical-storage and logical-size metadata for one filesystem object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageAllocation {
    /// Logical (apparent) size in bytes as reported by `stat`.
    pub logical_size_bytes: u64,
    /// Allocated bytes, if available.  On POSIX systems derived from
    /// `st_blocks × 512` using saturating arithmetic.
    pub allocated_size_bytes: Option<u64>,
    /// How `allocated_size_bytes` was measured.
    pub allocation_source: AllocationSource,
    /// Whether physically shared extents were detected.
    pub extent_sharing_status: ExtentSharingStatus,
}

/// Confidence level of a physical reclaimability estimate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReclaimabilityStatus {
    /// A defensible estimate could be produced.
    Estimated,
    /// Physical reclaimability could not be determined.
    Unknown,
}

/// Machine-readable codes explaining why reclaimability is unknown or limited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReclaimabilityReasonCode {
    FilesystemIdentityUnavailable,
    AllocationMetadataUnavailable,
    UnobservedHardLinks,
    ExtentSharingUnknown,
    ArithmeticOverflow,
    PlatformMetadataUnsupported,
}

/// Physical reclaimability assessment for a duplicate group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalReclaimability {
    pub status: ReclaimabilityStatus,
    pub reason_codes: Vec<ReclaimabilityReasonCode>,
}

impl PhysicalReclaimability {
    pub fn unknown(codes: Vec<ReclaimabilityReasonCode>) -> Self {
        Self {
            status: ReclaimabilityStatus::Unknown,
            reason_codes: codes,
        }
    }

    pub fn estimated(codes: Vec<ReclaimabilityReasonCode>) -> Self {
        Self {
            status: ReclaimabilityStatus::Estimated,
            reason_codes: codes,
        }
    }
}
