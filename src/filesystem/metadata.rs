use std::path::Path;

pub use crate::filesystem::identity::{
    AllocationSource, ExtentSharingStatus, FilesystemIdentity, StorageAllocation,
};

/// Platform-neutral filesystem metadata for a single path.
///
/// All fields that require platform support are wrapped in `Option` so that
/// callers can distinguish "unavailable" from zero/false.
#[derive(Debug, Clone)]
pub struct RawFilesystemMetadata {
    /// Stable filesystem identity, when available.
    pub identity: Option<FilesystemIdentity>,
    /// Logical size in bytes (always present).
    pub logical_size_bytes: u64,
    /// Allocated storage, if the platform provided it.
    pub allocated_size_bytes: Option<u64>,
    /// How `allocated_size_bytes` was obtained.
    pub allocation_source: AllocationSource,
    /// Non-fatal limitation notes from metadata collection.
    pub warnings: Vec<String>,
}

impl RawFilesystemMetadata {
    pub fn into_storage_allocation(self) -> StorageAllocation {
        StorageAllocation {
            logical_size_bytes: self.logical_size_bytes,
            allocated_size_bytes: self.allocated_size_bytes,
            allocation_source: self.allocation_source,
            extent_sharing_status: ExtentSharingStatus::Unknown,
        }
    }
}

/// Collect filesystem metadata for `path`.
///
/// This function never fails; missing or unsupported fields are returned as
/// `None` with a warning appended to `RawFilesystemMetadata::warnings`.
pub fn collect(path: &Path, logical_size_bytes: u64) -> RawFilesystemMetadata {
    platform::collect(path, logical_size_bytes)
}

#[cfg(unix)]
mod platform {
    use std::path::Path;

    use super::{RawFilesystemMetadata};
    use crate::filesystem::unix;

    pub(super) fn collect(path: &Path, logical_size_bytes: u64) -> RawFilesystemMetadata {
        unix::collect(path, logical_size_bytes)
    }
}

#[cfg(not(unix))]
mod platform {
    use std::path::Path;

    use super::{AllocationSource, RawFilesystemMetadata};

    pub(super) fn collect(path: &Path, logical_size_bytes: u64) -> RawFilesystemMetadata {
        RawFilesystemMetadata {
            identity: None,
            logical_size_bytes,
            allocated_size_bytes: None,
            allocation_source: AllocationSource::Unavailable,
            warnings: vec![
                "filesystem identity and allocation metadata are unavailable on this platform"
                    .to_owned(),
            ],
        }
    }
}
