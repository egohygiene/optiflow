//! Bounded read-only bindings for video sources and provider executables.
//!
//! Every path component is opened without following symlinks. Observations
//! bind a retained handle, metadata and complete content digest; they detect
//! observed changes but do not lock the filesystem against concurrent writers.
//! Guarded variants check a caller's cancellation/deadline between bounded
//! operations. They cannot interrupt an individual blocked filesystem syscall.

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
        Self::open_guarded(path, maximum, &|| Ok(()))
    }

    pub(crate) fn open_guarded(
        path: &Path,
        maximum: u64,
        guard: &impl Fn() -> Result<()>,
    ) -> Result<Self> {
        let (file, snapshot, digest, _) = observe(path, maximum, false, guard)?;
        guard()?;
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
        self.verify_current_guarded(&|| Ok(()))
    }

    pub(crate) fn verify_current_guarded(&self, guard: &impl Fn() -> Result<()>) -> Result<()> {
        ensure!(snapshot_guarded(&self.file, guard)? == self.snapshot, "bound file changed");
        let current = Self::open_guarded(&self.path, self.maximum, guard)?;
        ensure!(
            current.snapshot == self.snapshot && current.digest == self.digest,
            "file identity, metadata or complete content changed"
        );
        ensure!(snapshot_guarded(&self.file, guard)? == self.snapshot, "bound file changed");
        guard()?;
        Ok(())
    }

    /// Obtain at most 16 MiB of immutable captured bytes, freshly rebound to
    /// the original observation. Provider executables are hashed, not retained.
    pub(crate) fn read_bytes(&self) -> Result<Vec<u8>> {
        self.read_bytes_guarded(&|| Ok(()))
    }

    pub(crate) fn read_bytes_guarded(&self, guard: &impl Fn() -> Result<()>) -> Result<Vec<u8>> {
        guard()?;
        ensure!(
            self.snapshot.logical_bytes <= MAX_RETAINED_BYTES,
            "file exceeds retained-byte ceiling"
        );
        ensure!(snapshot_guarded(&self.file, guard)? == self.snapshot, "bound file changed");
        let (_, current, digest, bytes) =
            observe(&self.path, self.maximum.min(MAX_RETAINED_BYTES), true, guard)?;
        ensure!(
            current == self.snapshot && digest == self.digest,
            "file identity, metadata or complete content changed"
        );
        ensure!(snapshot_guarded(&self.file, guard)? == self.snapshot, "bound file changed");
        guard()?;
        Ok(bytes)
    }
}

fn observe(
    path: &Path,
    maximum: u64,
    retain: bool,
    guard: &impl Fn() -> Result<()>,
) -> Result<(File, FileSnapshot, EvidenceDigest, Vec<u8>)> {
    guard()?;
    ensure!(
        maximum > 0 && maximum <= MAX_OBSERVATION_BYTES,
        "invalid observation byte ceiling"
    );
    ensure!(
        !retain || maximum <= MAX_RETAINED_BYTES,
        "invalid retained-byte ceiling"
    );
    let mut file = open_regular_guarded(path, guard)?;
    let before = snapshot_guarded(&file, guard)?;
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
        guard()?;
        let count = reader.read(&mut buffer).context("failed to read bound file")?;
        guard()?;
        if count == 0 {
            break;
        }
        total = total.checked_add(count as u64).context("read size overflow")?;
        ensure!(total <= maximum, "file grew beyond observation byte ceiling");
        hasher.update(&buffer[..count]);
        if retain {
            bytes.extend_from_slice(&buffer[..count]);
        }
        guard()?;
    }
    ensure!(
        total == before.logical_bytes && snapshot_guarded(&file, guard)? == before,
        "file changed during bounded read"
    );
    // Walk the full path again rather than trusting the earlier parent handle.
    // A later path swap remains possible and must be rechecked by the caller.
    let current = open_regular_guarded(path, guard)?;
    ensure!(snapshot_guarded(&current, guard)? == before, "file path changed during read");
    ensure!(snapshot_guarded(&file, guard)? == before, "bound file changed after read");
    let digest = EvidenceDigest {
        algorithm: "blake3-256".to_owned(),
        value: hasher.finalize().to_hex().to_string(),
    };
    guard()?;
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
fn open_directory_guarded(path: &Path, guard: &impl Fn() -> Result<()>) -> Result<File> {
    use rustix::fs::{Mode, OFlags, open, openat};
    guard()?;
    normalized_absolute(path)?;
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC
        | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    guard()?;
    let mut directory = File::from(open("/", flags, Mode::empty())?);
    guard()?;
    for part in path.components() {
        if let Component::Normal(name) = part {
            guard()?;
            directory = File::from(openat(&directory, name, flags, Mode::empty())
                .context("unsafe or unavailable directory component")?);
            guard()?;
        }
    }
    guard()?;
    Ok(directory)
}

#[cfg(unix)]
fn open_regular_guarded(path: &Path, guard: &impl Fn() -> Result<()>) -> Result<File> {
    use rustix::fs::{Mode, OFlags, openat};
    guard()?;
    normalized_absolute(path)?;
    let directory = open_directory_guarded(path.parent().context("file path has no parent")?, guard)?;
    let name = path.file_name().context("file path has no leaf")?;
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    guard()?;
    let file = File::from(openat(&directory, name, flags, Mode::empty())
        .context("unsafe or unavailable file leaf")?);
    guard()?;
    let metadata = file.metadata()?;
    guard()?;
    ensure!(metadata.is_file(), "file leaf is not a regular file");
    guard()?;
    Ok(file)
}

#[cfg(not(unix))]
fn open_regular_guarded(_path: &Path, guard: &impl Fn() -> Result<()>) -> Result<File> {
    guard()?;
    anyhow::bail!("video file binding requires Unix no-follow handles")
}

fn snapshot_guarded(file: &File, guard: &impl Fn() -> Result<()>) -> Result<FileSnapshot> {
    guard()?;
    let observation = snapshot(file)?;
    guard()?;
    Ok(observation)
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
pub(crate) fn validate_private_directory(path: &Path) -> Result<()> {
    validate_private_directory_guarded(path, &|| Ok(()))
}

#[cfg(unix)]
pub(crate) fn validate_private_directory_guarded(
    path: &Path,
    guard: &impl Fn() -> Result<()>,
) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let directory = open_directory_guarded(path, guard)?;
    guard()?;
    let metadata = directory.metadata()?;
    guard()?;
    ensure!(metadata.is_dir(), "provider work path is not a directory");
    ensure!(metadata.mode() & 0o7777 == 0o700, "provider directory must have mode 0700");
    guard()
}

#[cfg(not(unix))]
pub(crate) fn validate_private_directory_guarded(
    _path: &Path,
    guard: &impl Fn() -> Result<()>,
) -> Result<()> {
    guard()?;
    anyhow::bail!("private provider directories require Unix no-follow handles")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::cell::Cell;
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

    #[test]
    fn cancellation_precedes_path_access_and_cannot_return_captured_bytes() {
        let cancelled = || anyhow::bail!("synthetic cancellation");
        let missing = Path::new("/optiflow-deliberately-unavailable/source.mkv");
        let error = BoundedFileObservation::open_guarded(missing, 64, &cancelled).unwrap_err();
        assert_eq!(error.to_string(), "synthetic cancellation");
        assert_eq!(
            validate_private_directory_guarded(missing, &cancelled).unwrap_err().to_string(),
            "synthetic cancellation"
        );

        let root = root();
        let path = root.path().join("source.mkv");
        fs::write(&path, b"synthetic source bytes").unwrap();
        let observed = BoundedFileObservation::open(&path, 64).unwrap();
        assert_eq!(observed.verify_current_guarded(&cancelled).unwrap_err().to_string(), "synthetic cancellation");
        assert_eq!(observed.read_bytes_guarded(&cancelled).unwrap_err().to_string(), "synthetic cancellation");
        assert_eq!(fs::read(&path).unwrap(), b"synthetic source bytes");
    }

    #[test]
    fn guard_refusals_at_every_read_checkpoint_propagate_without_partial_results() {
        let root = root();
        let path = root.path().join("source.mkv");
        let content = vec![0x5a; 128 * 1024 + 1];
        fs::write(&path, &content).unwrap();
        let observed = BoundedFileObservation::open(&path, content.len() as u64).unwrap();
        let checkpoints = Cell::new(0usize);
        let captured = observed.read_bytes_guarded(&|| {
            checkpoints.set(checkpoints.get() + 1);
            Ok(())
        }).unwrap();
        assert_eq!(captured, content);
        assert!(checkpoints.get() > 0);

        // Discover the checkpoints instead of assuming a fixed path depth or
        // chunk count. Refusal includes the checks after the final read and
        // re-observation, where late success would otherwise leak a result.
        for refuse_at in 1..=checkpoints.get() {
            let current = Cell::new(0usize);
            let result = observed.read_bytes_guarded(&|| {
                current.set(current.get() + 1);
                ensure!(current.get() != refuse_at, "synthetic deadline");
                Ok(())
            });
            assert_eq!(result.unwrap_err().to_string(), "synthetic deadline");
            assert_eq!(current.get(), refuse_at);
        }
        assert_eq!(fs::read(&path).unwrap(), content);
    }
}
