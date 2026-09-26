use std::fs::{self, File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use crate::domain::NativePath;
use crate::filesystem::identity::FileStateSignature;
use crate::outcome::DiagnosticCode as Code;
use crate::signals::SignalState;

use super::model::{DirectoryBinding, FileBinding};
use super::{Result, failure};

pub fn native_path(path: &NativePath) -> Result<PathBuf> {
    let decoded = path.to_path_buf();
    if NativePath::from_path(&decoded) != *path
        || !decoded.is_absolute()
        || decoded
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(failure(
            Code::ExecutionScopeInvalid,
            "paths must be canonical absolute native paths",
        ));
    }
    Ok(decoded)
}

/// Walk from / with no-follow directory handles. No component can redirect the
/// open through a symlink; nonblocking opens cannot hang on a substituted FIFO.
#[cfg(unix)]
pub fn open(path: &Path, directory: bool) -> Result<File> {
    use rustix::fs::{Mode, OFlags, open, openat};
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(failure(
            Code::ExecutionScopeInvalid,
            "an absolute normalized path is required",
        ));
    }
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let mut handle = open("/", flags | OFlags::DIRECTORY, Mode::empty())
        .map_err(|e| failure(Code::ExecutionSourceUnavailable, e.to_string()))?;
    let parts: Vec<_> = path
        .components()
        .filter_map(|c| {
            if let Component::Normal(v) = c {
                Some(v)
            } else {
                None
            }
        })
        .collect();
    for (i, part) in parts.iter().enumerate() {
        let next_flags = if i + 1 < parts.len() || directory {
            flags | OFlags::DIRECTORY
        } else {
            flags
        };
        handle = openat(&handle, *part, next_flags, Mode::empty()).map_err(|e| {
            failure(
                Code::ExecutionSourceUnavailable,
                format!("cannot safely open {}: {e}", path.display()),
            )
        })?;
    }
    Ok(File::from(handle))
}

#[cfg(not(unix))]
pub fn open(_path: &Path, _directory: bool) -> Result<File> {
    Err(failure(
        Code::ExecutionUnsupported,
        "execution validation requires Linux or macOS filesystem evidence",
    ))
}

pub fn canonical_input(path: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| failure(Code::ExecutionSourceUnavailable, e.to_string()))?;
    if metadata.file_type().is_symlink() {
        return Err(failure(
            Code::ExecutionAmbiguousIdentity,
            "selected paths cannot be symbolic links",
        ));
    }
    fs::canonicalize(path).map_err(|e| failure(Code::ExecutionSourceUnavailable, e.to_string()))
}

pub fn metadata(file: &File) -> Result<Metadata> {
    file.metadata()
        .map_err(|e| failure(Code::ExecutionSourceUnavailable, e.to_string()))
}

#[cfg(unix)]
fn ownership(m: &Metadata) -> (u32, u32, u32) {
    use std::os::unix::fs::MetadataExt;
    (m.mode(), m.uid(), m.gid())
}
#[cfg(not(unix))]
fn ownership(_m: &Metadata) -> (u32, u32, u32) {
    (0, 0, 0)
}

pub fn directory(path: &Path) -> Result<DirectoryBinding> {
    let file = open(path, true)?;
    let m = metadata(&file)?;
    let identity = FileStateSignature::from_file_metadata(&m)
        .identity
        .ok_or_else(|| {
            failure(
                Code::ExecutionAmbiguousIdentity,
                "directory identity is unavailable",
            )
        })?;
    let (mode, uid, gid) = ownership(&m);
    // Directory link counts change when unrelated child directories are made.
    let mut identity = identity;
    identity.link_count = None;
    Ok(DirectoryBinding {
        path: NativePath::from_path(path),
        identity,
        mode,
        uid,
        gid,
    })
}

/// Compare ancestor objects as well as path spellings, including case aliases
/// on case-insensitive filesystems. All directories must already exist.
pub fn outside_directories(path: &Path, protected: &[&DirectoryBinding]) -> Result<()> {
    for ancestor in path.ancestors() {
        let actual = directory(ancestor)?;
        if protected
            .iter()
            .any(|b| b.identity.identity_key() == actual.identity.identity_key())
        {
            return Err(failure(
                Code::ExecutionScopeInvalid,
                "writable evidence location is inside a protected directory object",
            ));
        }
    }
    Ok(())
}

pub fn check_directory(expected: &DirectoryBinding) -> Result<File> {
    let path = native_path(&expected.path)?;
    if directory(&path)? != *expected {
        return Err(failure(
            Code::ExecutionSourceStale,
            format!(
                "directory identity or permissions changed: {}",
                path.display()
            ),
        ));
    }
    let file = open(&path, true)?;
    let m = metadata(&file)?;
    let sig = FileStateSignature::from_file_metadata(&m);
    if sig.identity.as_ref().map(|id| id.identity_key()) != Some(expected.identity.identity_key()) {
        return Err(failure(
            Code::ExecutionSourceStale,
            "directory changed while being opened",
        ));
    }
    Ok(file)
}

#[cfg(unix)]
pub fn writable_directory(file: &File) -> Result<()> {
    use rustix::fs::{Access, AtFlags, accessat};
    if ownership(&metadata(file)?).0 & 0o222 == 0 {
        return Err(failure(
            Code::ExecutionReadOnly,
            "directory has no writable mode bits",
        ));
    }
    if ownership(&metadata(file)?).0 & 0o1000 != 0 {
        return Err(failure(
            Code::ExecutionUnsupported,
            "sticky directories require an action-specific deletion-authority check; execution-plan v1 refuses them",
        ));
    }
    check_mutability_flags(file)?;
    accessat(
        file,
        ".",
        Access::READ_OK | Access::WRITE_OK | Access::EXEC_OK,
        AtFlags::EACCESS,
    )
    .map_err(|e| {
        failure(
            Code::ExecutionReadOnly,
            format!("directory access is unavailable: {e}"),
        )
    })
}
#[cfg(not(unix))]
pub fn writable_directory(_file: &File) -> Result<()> {
    Err(failure(
        Code::ExecutionUnsupported,
        "permission evidence unsupported",
    ))
}

pub struct Capacity {
    pub available: u64,
    pub block_size: u64,
    pub read_only: bool,
}

#[cfg(target_os = "linux")]
fn check_mutability_flags(file: &File) -> Result<()> {
    use rustix::fs::{IFlags, ioctl_getflags};
    let flags = ioctl_getflags(file).map_err(|e| {
        failure(
            Code::ExecutionUnsupported,
            format!("filesystem inode mutability flags cannot be checked: {e}"),
        )
    })?;
    if flags.intersects(IFlags::IMMUTABLE | IFlags::APPEND) {
        return Err(failure(
            Code::ExecutionReadOnly,
            "immutable or append-only inode",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn check_mutability_flags(file: &File) -> Result<()> {
    let stat = rustix::fs::fstat(file)
        .map_err(|e| failure(Code::ExecutionSourceUnavailable, e.to_string()))?;
    // Darwin UF_IMMUTABLE, UF_APPEND, SF_IMMUTABLE and SF_APPEND.
    if stat.st_flags & 0x0006_0006 != 0 {
        return Err(failure(
            Code::ExecutionReadOnly,
            "immutable or append-only inode",
        ));
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn check_mutability_flags(_file: &File) -> Result<()> {
    Err(failure(
        Code::ExecutionUnsupported,
        "inode mutability evidence requires Linux or macOS",
    ))
}

#[cfg(unix)]
pub fn capacity(file: &File) -> Result<Capacity> {
    use rustix::fs::{StatVfsMountFlags, fstatvfs};
    let v = fstatvfs(file).map_err(|e| {
        failure(
            Code::ExecutionCapacityUnavailable,
            format!("cannot measure filesystem capacity: {e}"),
        )
    })?;
    let block_size = v.f_frsize;
    if block_size == 0 || v.f_blocks == 0 || v.f_bavail > v.f_blocks || v.f_bfree > v.f_blocks {
        return Err(failure(
            Code::ExecutionCapacityUnavailable,
            "filesystem returned ambiguous space measurements",
        ));
    }
    if v.f_namemax < 64 {
        return Err(failure(
            Code::ExecutionUnsupported,
            "filesystem cannot represent the approved quarantine namespace",
        ));
    }
    let available = v.f_bavail.checked_mul(block_size).ok_or_else(|| {
        failure(
            Code::ExecutionCapacityUnavailable,
            "available-space arithmetic overflow",
        )
    })?;
    Ok(Capacity {
        available,
        block_size,
        read_only: v.f_flag.contains(StatVfsMountFlags::RDONLY),
    })
}
#[cfg(not(unix))]
pub fn capacity(_file: &File) -> Result<Capacity> {
    Err(failure(
        Code::ExecutionCapacityUnavailable,
        "capacity measurement unsupported",
    ))
}

pub fn snapshot(path: &Path, signals: &SignalState, max_bytes: u64) -> Result<FileBinding> {
    let parent = path
        .parent()
        .ok_or_else(|| failure(Code::ExecutionScopeInvalid, "file has no parent"))?;
    let directory = directory(parent)?;
    let mut file = open(path, false)?;
    let m = metadata(&file)?;
    if m.len() > max_bytes {
        return Err(failure(
            Code::ExecutionBoundsExceeded,
            "selected file exceeds the in-flight byte limit",
        ));
    }
    let sig = FileStateSignature::from_file_metadata(&m);
    let identity = sig.identity.clone().ok_or_else(|| {
        failure(
            Code::ExecutionAmbiguousIdentity,
            "file identity unavailable",
        )
    })?;
    if !m.is_file() || identity.link_count != Some(1) {
        return Err(failure(
            Code::ExecutionAmbiguousIdentity,
            "only singly linked regular files are supported by execution-plan v1",
        ));
    }
    let (mode, uid, gid) = ownership(&m);
    if mode & 0o222 == 0 {
        return Err(failure(
            Code::ExecutionReadOnly,
            "read-only source files are not eligible",
        ));
    }
    let hash = hash_bound(&mut file, m.len(), signals)?;
    let binding = FileBinding {
        directory,
        path: NativePath::from_path(path),
        identity,
        size_bytes: m.len(),
        modified_unix_ns: sig
            .modified_unix_ns
            .ok_or_else(|| failure(Code::ExecutionSourceStale, "modification time unavailable"))?,
        changed_unix_ns: sig
            .changed_unix_ns
            .ok_or_else(|| failure(Code::ExecutionSourceStale, "change time unavailable"))?,
        mode,
        uid,
        gid,
        blake3: hash,
    };
    check_file(&file, &binding)?;
    Ok(binding)
}

fn matches(m: &Metadata, b: &FileBinding) -> bool {
    let sig = FileStateSignature::from_file_metadata(m);
    m.is_file()
        && sig.identity.as_ref() == Some(&b.identity)
        && m.len() == b.size_bytes
        && sig.modified_unix_ns == Some(b.modified_unix_ns)
        && sig.changed_unix_ns == Some(b.changed_unix_ns)
        && ownership(m) == (b.mode, b.uid, b.gid)
}

pub fn check_file(file: &File, binding: &FileBinding) -> Result<()> {
    check_mutability_flags(file)?;
    check_directory(&binding.directory)?;
    let path = native_path(&binding.path)?;
    let current = open(&path, false)?;
    if !matches(&metadata(file)?, binding) || !matches(&metadata(&current)?, binding) {
        return Err(failure(
            Code::ExecutionSourceStale,
            format!("file replaced or metadata changed: {}", path.display()),
        ));
    }
    Ok(())
}

pub fn open_bound(binding: &FileBinding) -> Result<File> {
    let file = open(&native_path(&binding.path)?, false)?;
    check_file(&file, binding)?;
    Ok(file)
}

/// Read exactly the bound size plus one EOF probe. A continuously growing
/// source cannot turn a bounded dry run into an unbounded read.
pub fn hash_bound(file: &mut File, size: u64, signals: &SignalState) -> Result<String> {
    file.rewind()
        .map_err(|e| failure(Code::ExecutionSourceUnavailable, e.to_string()))?;
    let mut remaining = size;
    let mut buffer = vec![0; 1024 * 1024];
    let mut hasher = blake3::Hasher::new();
    while remaining > 0 {
        if signals.is_cancelled() {
            return Err(interrupted(signals));
        }
        let count = remaining.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..count]).map_err(|e| {
            failure(
                Code::ExecutionSourceStale,
                format!("bounded hash read failed: {e}"),
            )
        })?;
        hasher.update(&buffer[..count]);
        remaining -= count as u64;
    }
    if signals.is_cancelled() {
        return Err(interrupted(signals));
    }
    if file
        .read(&mut [0])
        .map_err(|e| failure(Code::ExecutionSourceUnavailable, e.to_string()))?
        != 0
    {
        return Err(failure(
            Code::ExecutionSourceStale,
            "source grew beyond its bound during hashing",
        ));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|e| failure(Code::ExecutionSourceUnavailable, e.to_string()))?;
    Ok(hasher.finalize().to_hex().to_string())
}

pub fn interrupted(signals: &SignalState) -> Box<crate::outcome::Diagnostic> {
    let code = if signals.current() == Some(crate::signals::Interruption::Terminate) {
        Code::OperationTerminated
    } else {
        Code::OperationInterrupted
    };
    failure(
        code,
        "execution validation interrupted; no source mutation occurred",
    )
}
