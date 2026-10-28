//! Bounded read-only bindings for video sources and provider executables.
//!
//! Every path component is opened without following symlinks. Observations
//! bind a retained handle, metadata and complete content digest; they detect
//! observed changes but do not lock the filesystem against concurrent writers.

use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::domain::EvidenceDigest;

const MAX_OBSERVATION_BYTES: u64 = 128 * 1024 * 1024;
const MAX_RETAINED_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileSnapshot {
    pub device_id: u64,
    pub inode: u64,
    pub mode: u32,
    pub owner_uid: u32,
    pub group_gid: u32,
    pub link_count: u64,
    pub logical_bytes: u64,
    pub modified_unix_ns: i128,
    pub changed_unix_ns: i128,
}

#[derive(Debug)]
pub(crate) struct BoundedFileObservation {
    path: PathBuf,
    maximum: u64,
    file: File,
    snapshot: FileSnapshot,
    digest: EvidenceDigest,
}

impl BoundedFileObservation {
    /// Bind a regular, nonempty file using a bounded streaming content hash.
    pub(crate) fn open(path: &Path, maximum: u64) -> Result<Self> {
        let (file, snapshot, digest, _) = observe(path, maximum, false)?;
        Ok(Self {
            path: path.to_path_buf(),
            maximum,
            file,
            snapshot,
            digest,
        })
    }

    pub(crate) fn digest(&self) -> &EvidenceDigest {
        &self.digest
    }

    pub(crate) fn snapshot(&self) -> &FileSnapshot {
        &self.snapshot
    }

    /// Reopen through the same no-follow walk, hash every current byte and
    /// require both that path and the retained object to match the binding.
    pub(crate) fn verify_current(&self) -> Result<()> {
        ensure!(snapshot(&self.file)? == self.snapshot, "bound file changed");
        let current = Self::open(&self.path, self.maximum)?;
        ensure!(
            current.snapshot == self.snapshot && current.digest == self.digest,
            "file identity, metadata or complete content changed"
        );
        ensure!(snapshot(&self.file)? == self.snapshot, "bound file changed");
        Ok(())
    }

    /// Obtain at most 16 MiB of immutable captured bytes, freshly rebound to
    /// the original observation. Provider executables are hashed, not retained.
    pub(crate) fn read_bytes(&self) -> Result<Vec<u8>> {
        ensure!(
            self.snapshot.logical_bytes <= MAX_RETAINED_BYTES,
            "file exceeds retained-byte ceiling"
        );
        ensure!(snapshot(&self.file)? == self.snapshot, "bound file changed");
        let (_, current, digest, bytes) =
            observe(&self.path, self.maximum.min(MAX_RETAINED_BYTES), true)?;
        ensure!(
            current == self.snapshot && digest == self.digest,
            "file identity, metadata or complete content changed"
        );
        ensure!(snapshot(&self.file)? == self.snapshot, "bound file changed");
        Ok(bytes)
    }
}

fn observe(
    path: &Path,
    maximum: u64,
    retain: bool,
) -> Result<(File, FileSnapshot, EvidenceDigest, Vec<u8>)> {
    ensure!(
        maximum > 0 && maximum <= MAX_OBSERVATION_BYTES,
        "invalid observation byte ceiling"
    );
    ensure!(
        !retain || maximum <= MAX_RETAINED_BYTES,
        "invalid retained-byte ceiling"
    );
    let mut file = open_regular(path)?;
    let before = snapshot(&file)?;
    ensure!(
        before.logical_bytes > 0 && before.logical_bytes <= maximum,
        "file is empty or exceeds observation byte ceiling"
    );
    let mut hasher = blake3::Hasher::new();
    let mut total = 0u64;
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 64 * 1024];
    let read_limit = maximum.checked_add(1).context("observation limit overflow")?;
    let mut reader = (&mut file).take(read_limit);
    loop {
        let count = reader.read(&mut buffer).context("failed to read bound file")?;
        if count == 0 {
            break;
        }
        total = total.checked_add(count as u64).context("read size overflow")?;
        ensure!(total <= maximum, "file grew beyond observation byte ceiling");
        hasher.update(&buffer[..count]);
        if retain {
            bytes.extend_from_slice(&buffer[..count]);
        }
    }
    ensure!(
        total == before.logical_bytes && snapshot(&file)? == before,
        "file changed during bounded read"
    );
    // Walk the full path again rather than trusting the earlier parent handle.
    // A later path swap remains possible and must be rechecked by the caller.
    let current = open_regular(path)?;
    ensure!(snapshot(&current)? == before, "file path changed during read");
    ensure!(snapshot(&file)? == before, "bound file changed after read");
    let digest = EvidenceDigest {
        algorithm: "blake3-256".to_owned(),
        value: hasher.finalize().to_hex().to_string(),
    };
    Ok((file, before, digest, bytes))
}

fn normalized_absolute(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute()
            && path.components().all(|part| {
                matches!(part, Component::RootDir | Component::Normal(_))
            }),
        "path must be absolute and normalized"
    );
    let rebuilt: PathBuf = path.components().collect();
    ensure!(
        rebuilt.as_os_str() == path.as_os_str(),
        "path must not contain redundant separators or dot components"
    );
    Ok(())
}

#[cfg(unix)]
fn open_directory(path: &Path) -> Result<File> {
    use rustix::fs::{Mode, OFlags, open, openat};
    normalized_absolute(path)?;
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC
        | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let mut directory = File::from(open("/", flags, Mode::empty())?);
    for part in path.components() {
        if let Component::Normal(name) = part {
            directory = File::from(openat(&directory, name, flags, Mode::empty())
                .context("unsafe or unavailable directory component")?);
        }
    }
    Ok(directory)
}

#[cfg(unix)]
fn open_regular(path: &Path) -> Result<File> {
    use rustix::fs::{Mode, OFlags, openat};
    normalized_absolute(path)?;
    let directory = open_directory(path.parent().context("file path has no parent")?)?;
    let name = path.file_name().context("file path has no leaf")?;
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let file = File::from(openat(&directory, name, flags, Mode::empty())
        .context("unsafe or unavailable file leaf")?);
    ensure!(file.metadata()?.is_file(), "file leaf is not a regular file");
    Ok(file)
}

#[cfg(not(unix))]
fn open_regular(_path: &Path) -> Result<File> {
    anyhow::bail!("video file binding requires Unix no-follow handles")
}

#[cfg(unix)]
fn snapshot(file: &File) -> Result<FileSnapshot> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata().context("failed to inspect bound handle")?;
    ensure!(metadata.is_file(), "bound handle is not a regular file");
    Ok(FileSnapshot {
        device_id: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        owner_uid: metadata.uid(),
        group_gid: metadata.gid(),
        link_count: metadata.nlink(),
        logical_bytes: metadata.len(),
        modified_unix_ns: i128::from(metadata.mtime()) * 1_000_000_000
            + i128::from(metadata.mtime_nsec()),
        changed_unix_ns: i128::from(metadata.ctime()) * 1_000_000_000
            + i128::from(metadata.ctime_nsec()),
    })
}

#[cfg(not(unix))]
fn snapshot(_file: &File) -> Result<FileSnapshot> {
    anyhow::bail!("video file binding requires Unix metadata")
}

/// Check only path traversal, directory type and exact 0700 permissions.
/// This is not proof of ownership, an OS sandbox or a concurrent-path lock.
#[cfg(unix)]
pub(crate) fn validate_private_directory(path: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let directory = open_directory(path)?;
    let metadata = directory.metadata()?;
    ensure!(metadata.is_dir(), "provider work path is not a directory");
    ensure!(metadata.mode() & 0o7777 == 0o700, "provider directory must have mode 0700");
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn validate_private_directory(_path: &Path) -> Result<()> {
    anyhow::bail!("private provider directories require Unix no-follow handles")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn root() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("optiflow-video-binding-")
            .tempdir_in(std::env::temp_dir().canonicalize().unwrap())
            .unwrap()
    }

    #[test]
    fn unchanged_regular_source_round_trips_without_writes() {
        let root = root();
        let path = root.path().join("source.mkv");
        fs::write(&path, b"synthetic source bytes").unwrap();
        let observation = BoundedFileObservation::open(&path, 64).unwrap();
        let before = observation.snapshot().clone();
        assert_eq!(observation.digest().value, blake3::hash(b"synthetic source bytes").to_hex().to_string());
        assert_eq!(observation.read_bytes().unwrap(), b"synthetic source bytes");
        observation.verify_current().unwrap();
        assert_eq!(snapshot(&File::open(&path).unwrap()).unwrap(), before);
    }

    #[test]
    fn same_length_replacement_is_not_the_bound_object() {
        let root = root();
        let path = root.path().join("source.mkv");
        fs::write(&path, b"AAAA").unwrap();
        let observation = BoundedFileObservation::open(&path, 4).unwrap();
        let replacement = root.path().join("replacement.mkv");
        fs::write(&replacement, b"AAAA").unwrap();
        fs::rename(&replacement, &path).unwrap();
        assert!(observation.verify_current().is_err());
        assert!(observation.read_bytes().is_err());
    }

    #[test]
    fn changed_bytes_and_growth_are_refused() {
        let root = root();
        let path = root.path().join("source.mkv");
        fs::write(&path, b"AAAA").unwrap();
        let observation = BoundedFileObservation::open(&path, 4).unwrap();
        fs::write(&path, b"BBBB").unwrap();
        assert!(observation.verify_current().is_err());
        assert!(observation.read_bytes().is_err());
        let current = BoundedFileObservation::open(&path, 4).unwrap();
        fs::write(&path, b"BBBBB").unwrap();
        assert!(current.verify_current().is_err());
    }

    #[test]
    fn symlink_leaf_ancestor_and_later_swap_are_refused() {
        let root = root();
        let folder = root.path().join("real");
        fs::create_dir(&folder).unwrap();
        let path = folder.join("source.mkv");
        fs::write(&path, b"AAAA").unwrap();
        let alias = root.path().join("alias");
        symlink(&folder, &alias).unwrap();
        assert!(BoundedFileObservation::open(&alias.join("source.mkv"), 4).is_err());
        let leaf = root.path().join("leaf.mkv");
        symlink(&path, &leaf).unwrap();
        assert!(BoundedFileObservation::open(&leaf, 4).is_err());
        let observed = BoundedFileObservation::open(&path, 4).unwrap();
        let moved = folder.join("moved.mkv");
        fs::rename(&path, &moved).unwrap();
        symlink(&moved, &path).unwrap();
        assert!(observed.verify_current().is_err());
    }

    #[test]
    fn empty_nonregular_and_invalid_or_exceeded_limits_are_refused() {
        let root = root();
        let path = root.path().join("source.mkv");
        fs::write(&path, b"").unwrap();
        assert!(BoundedFileObservation::open(&path, 4).is_err());
        fs::write(&path, b"AAAA").unwrap();
        for maximum in [0, 3, MAX_OBSERVATION_BYTES + 1, u64::MAX] {
            assert!(BoundedFileObservation::open(&path, maximum).is_err());
        }
        assert!(BoundedFileObservation::open(root.path(), 4).is_err());
        let socket_path = root.path().join("socket");
        let _socket = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
        assert!(BoundedFileObservation::open(&socket_path, 4).is_err());
        assert!(BoundedFileObservation::open(Path::new("relative.mkv"), 4).is_err());
        assert!(BoundedFileObservation::open(&root.path().join("./source.mkv"), 4).is_err());
    }

    #[test]
    fn private_directory_validation_requires_exact_mode_and_no_symlinks() {
        let root = root();
        let path = root.path().join("work");
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        validate_private_directory(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o750)).unwrap();
        assert!(validate_private_directory(&path).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let alias = root.path().join("work-alias");
        symlink(&path, &alias).unwrap();
        assert!(validate_private_directory(&alias).is_err());
    }
}
