use serde::{Deserialize, Serialize};

/// Immutable snapshot of filesystem-observable state for a single path.
///
/// Used to detect whether a file changed between the start and end of an
/// observation window (before hashing, after hashing, etc.).  Two signatures
/// that compare equal do not prove the file is unchanged, but a difference
/// proves that at least one observable field changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStateSignature {
    /// Stable filesystem identity, when available.
    pub identity: Option<FilesystemIdentity>,
    /// Whether the path was a symbolic link at the time of inspection.
    pub is_symlink: bool,
    /// Logical (apparent) byte size.
    pub logical_size_bytes: u64,
    /// Modification time in nanoseconds since UNIX epoch, if available.
    pub modified_unix_ns: Option<i64>,
    /// Metadata-change time in nanoseconds since UNIX epoch, if available.
    /// On Linux/macOS this is `ctime`; `None` on platforms without it.
    pub changed_unix_ns: Option<i64>,
}

impl FileStateSignature {
    /// Capture a signature from `std::fs::Metadata` obtained via
    /// `symlink_metadata` (i.e., without following the link).
    #[cfg(unix)]
    pub fn from_symlink_metadata(metadata: &std::fs::Metadata) -> Self {
        Self::from_metadata_inner(metadata, metadata.file_type().is_symlink())
    }

    /// Capture a signature from metadata obtained from an opened file handle.
    /// Unlike path metadata, this remains bound to the opened filesystem object
    /// even if its directory entry is renamed or replaced.
    #[cfg(unix)]
    pub fn from_file_metadata(metadata: &std::fs::Metadata) -> Self {
        Self::from_metadata_inner(metadata, false)
    }

    #[cfg(unix)]
    fn from_metadata_inner(metadata: &std::fs::Metadata, is_symlink: bool) -> Self {
        use std::os::unix::fs::MetadataExt;
        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        };
        let identity = if !is_symlink {
            Some(FilesystemIdentity {
                platform: platform.to_owned(),
                filesystem_id: metadata.dev().to_string(),
                file_id: metadata.ino().to_string(),
                link_count: Some(metadata.nlink()),
            })
        } else {
            None // symlinks have their own inode; don't treat as the target
        };

        let modified_unix_ns = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| i64::try_from(d.as_nanos()).ok());

        // `ctime` in nanoseconds: seconds × 1_000_000_000 + subsecond ns.
        let changed_unix_ns = metadata
            .ctime()
            .checked_mul(1_000_000_000)
            .and_then(|sec_ns| sec_ns.checked_add(metadata.ctime_nsec()));

        Self {
            identity,
            is_symlink,
            logical_size_bytes: metadata.len(),
            modified_unix_ns,
            changed_unix_ns,
        }
    }

    /// Non-Unix stub: returns a signature with unavailable metadata.
    #[cfg(not(unix))]
    pub fn from_symlink_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            identity: None,
            is_symlink: metadata.file_type().is_symlink(),
            logical_size_bytes: metadata.len(),
            modified_unix_ns: None,
            changed_unix_ns: None,
        }
    }

    #[cfg(not(unix))]
    pub fn from_file_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            identity: None,
            is_symlink: false,
            logical_size_bytes: metadata.len(),
            modified_unix_ns: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .and_then(|duration| i64::try_from(duration.as_nanos()).ok()),
            changed_unix_ns: None,
        }
    }
}

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
