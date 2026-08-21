use std::path::Path;

use crate::filesystem::identity::{AllocationSource, FilesystemIdentity};
use crate::filesystem::metadata::RawFilesystemMetadata;

/// Collect Unix filesystem metadata via `std::os::unix::fs::MetadataExt`.
///
/// Uses only safe standard-library APIs.  `unsafe_code` is forbidden at the
/// crate level so no unsafe blocks appear here.
pub fn collect(path: &Path, logical_size_bytes: u64) -> RawFilesystemMetadata {
    let mut warnings = Vec::new();

    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(error) => {
            warnings.push(format!(
                "could not read filesystem metadata for {}: {error}",
                path.display()
            ));
            return RawFilesystemMetadata {
                identity: None,
                logical_size_bytes,
                allocated_size_bytes: None,
                allocation_source: AllocationSource::Unavailable,
                warnings,
            };
        }
    };

    collect_from_metadata(&metadata, logical_size_bytes, warnings)
}

/// Convert metadata obtained from an opened handle into the portable storage
/// representation. This is the preferred observation path because it cannot be
/// redirected by a later directory-entry replacement.
pub fn collect_from_metadata(
    metadata: &std::fs::Metadata,
    logical_size_bytes: u64,
    warnings: Vec<String>,
) -> RawFilesystemMetadata {
    use std::os::unix::fs::MetadataExt;

    let dev = metadata.dev();
    let ino = metadata.ino();
    let nlink = metadata.nlink();

    // `st_blocks` counts 512-byte units regardless of the system's
    // preferred block size.  Use saturating multiplication to avoid
    // overflow on pathologically large values.
    let allocated_size_bytes = metadata.blocks().saturating_mul(512);

    let identity = FilesystemIdentity {
        platform: std::env::consts::OS.to_owned(),
        filesystem_id: dev.to_string(),
        file_id: ino.to_string(),
        link_count: Some(nlink),
    };

    RawFilesystemMetadata {
        identity: Some(identity),
        logical_size_bytes,
        allocated_size_bytes: Some(allocated_size_bytes),
        allocation_source: AllocationSource::StBlocks512,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn collect_returns_identity_for_regular_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.txt");
        fs::write(&path, b"hello world").expect("write");

        let meta = collect(&path, 11);
        assert!(
            meta.identity.is_some(),
            "expected identity on Unix, got None"
        );
        let id = meta.identity.unwrap();
        assert_eq!(id.platform, std::env::consts::OS);
        assert!(
            id.link_count.is_some_and(|n| n >= 1),
            "link_count should be >= 1"
        );
        assert!(meta.allocated_size_bytes.is_some());
        assert_eq!(meta.allocation_source, AllocationSource::StBlocks512);
    }

    #[test]
    fn collect_detects_hard_link() {
        let dir = tempdir().expect("tempdir");
        let original = dir.path().join("original.txt");
        let link = dir.path().join("link.txt");
        fs::write(&original, b"content").expect("write");
        fs::hard_link(&original, &link).expect("hard_link");

        let meta_original = collect(&original, 7);
        let meta_link = collect(&link, 7);

        let id_orig = meta_original
            .identity
            .as_ref()
            .expect("identity for original");
        let id_link = meta_link.identity.as_ref().expect("identity for link");

        assert_eq!(id_orig.filesystem_id, id_link.filesystem_id, "same device");
        assert_eq!(id_orig.file_id, id_link.file_id, "same inode");
        assert_eq!(
            id_orig.link_count,
            Some(2),
            "link count should be 2 after hard-linking"
        );
    }

    #[test]
    fn collect_returns_warning_for_missing_file() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("no_such_file.txt");

        let meta = collect(&missing, 0);
        assert!(meta.identity.is_none());
        assert!(!meta.warnings.is_empty());
    }
}
