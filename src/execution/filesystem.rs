use std::fs::{self, File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use crate::domain::NativePath;
use crate::filesystem::identity::FileStateSignature;
use crate::outcome::DiagnosticCode as Code;
use crate::signals::SignalState;

use super::model::{DirectoryBinding, ExecutionPlan, FileBinding};
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

#[cfg(any(target_os = "macos", test))]
const MACOS_CROSS_VOLUME_METADATA_UNAVAILABLE: &str = "macOS cross-volume quarantine and restore require a reviewed descriptor-bound ACL observation and preservation backend; same-volume APFS renames remain the supported mutation profile";

/// Check the complete topology intent before opening or synchronizing any
/// filesystem object. Passing this policy check is not filesystem qualification:
/// the caller must still establish APFS identity and durable synchronization.
///
/// Extended attributes do not attest Darwin ACLs. A successful metadata-copy
/// call also cannot replace independent destination ACL observation before
/// removing the source, so cross-volume intent remains unsupported.
#[cfg(any(target_os = "macos", test))]
fn check_macos_mutation_topologies(
    topologies: impl IntoIterator<Item = super::model::Topology>,
) -> Result<()> {
    if topologies
        .into_iter()
        .any(|topology| topology != super::model::Topology::SameFilesystem)
    {
        return Err(failure(
            Code::ExecutionUnsupported,
            MACOS_CROSS_VOLUME_METADATA_UNAVAILABLE,
        ));
    }
    Ok(())
}

/// The mutation platform gate is deliberately separate from read-only preview.
/// Linux keeps its existing contract. macOS v1 supports only same-volume APFS
/// renames and requires the filesystem's durable synchronization operation.
#[cfg(target_os = "linux")]
pub fn check_mutation_platform(_plan: &ExecutionPlan) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn check_mutation_platform(plan: &ExecutionPlan) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    // Reject unsupported intent before journal creation or any source change.
    check_macos_mutation_topologies(plan.body.actions.iter().map(|action| action.topology))?;
    let quarantine = check_directory(&plan.body.quarantine)?;
    require_apfs(&quarantine)?;
    sync_directory(&quarantine)?;
    let quarantine_device = metadata(&quarantine)?.dev();
    let state = check_directory(&plan.body.state)?;
    require_apfs(&state)?;
    sync_directory(&state)?;
    // Do not require the candidate pathname to exist here: recovery operates
    // after that object has moved. The retained directory binding stays exact.
    for action in &plan.body.actions {
        let parent = check_directory(&action.candidate.directory)?;
        require_apfs(&parent)?;
        if metadata(&parent)?.dev() != quarantine_device {
            return Err(failure(
                Code::ExecutionUnsupported,
                "macOS source and quarantine must be on the same actual APFS volume",
            ));
        }
        sync_directory(&parent)?;
        let keeper_parent = check_directory(&action.keeper.directory)?;
        require_apfs(&keeper_parent)?;
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn check_mutation_platform(_plan: &ExecutionPlan) -> Result<()> {
    Err(failure(Code::ExecutionUnsupported, "mutation filesystem evidence requires Linux or macOS APFS"))
}

#[cfg(target_os = "macos")]
fn require_apfs(file: &File) -> Result<()> {
    let filesystem = rustix::fs::fstatfs(file).map_err(|error| failure(
        Code::ExecutionUnsupported, format!("cannot identify mutation filesystem: {error}"),
    ))?;
    let name: Vec<u8> = filesystem.f_fstypename.iter()
        .take_while(|byte| **byte != 0).map(|byte| *byte as u8).collect();
    if name != b"apfs" {
        return Err(failure(Code::ExecutionUnsupported, "macOS mutation requires an observed APFS filesystem"));
    }
    Ok(())
}

/// Recheck the actual opened objects, not merely their declared parent paths,
/// immediately before a same-volume move or restore.
#[cfg(target_os = "macos")]
pub fn check_rename_platform(source: &File, source_parent: &File, destination_parent: &File) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    require_apfs(source)?;
    require_apfs(source_parent)?;
    require_apfs(destination_parent)?;
    let device = metadata(source)?.dev();
    if metadata(source_parent)?.dev() != device || metadata(destination_parent)?.dev() != device {
        return Err(failure(Code::ExecutionUnsupported, "opened macOS rename objects are not on the same APFS volume"));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn check_rename_platform(_source: &File, _source_parent: &File, _destination_parent: &File) -> Result<()> {
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn check_rename_platform(_source: &File, _source_parent: &File, _destination_parent: &File) -> Result<()> {
    Err(failure(Code::ExecutionUnsupported, "mutation rename platform is unsupported"))
}

/// Synchronize without silently falling back from Darwin F_FULLFSYNC. A
/// filesystem that cannot provide this operation is unsupported for mutation.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn sync_file(file: &File) -> Result<()> {
    #[cfg(target_os = "linux")]
    file.sync_all().map_err(|error| failure(
        Code::StateTransactionFailed, format!("filesystem synchronization failed: {error}"),
    ))?;
    #[cfg(target_os = "macos")]
    {
        rustix::fs::fsync(file).map_err(|error| failure(
            Code::StateTransactionFailed, format!("macOS filesystem synchronization failed: {error}"),
        ))?;
        rustix::fs::fcntl_fullfsync(file).map_err(|error| failure(
            Code::ExecutionUnsupported, format!("macOS durable F_FULLFSYNC is unavailable: {error}"),
        ))?;
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn sync_file(_file: &File) -> Result<()> {
    Err(failure(Code::ExecutionUnsupported, "durable mutation synchronization is unsupported"))
}

pub fn sync_directory(file: &File) -> Result<()> {
    if !metadata(file)?.is_dir() {
        return Err(failure(Code::ExecutionScopeInvalid, "directory synchronization requires a directory handle"));
    }
    sync_file(file)
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
        "execution interrupted; inspect durable evidence and filesystem paths before retry",
    )
}

#[cfg(test)]
mod tests {
    use super::{Code, check_macos_mutation_topologies};
    use crate::execution::model::Topology::{CrossFilesystem, SameFilesystem};

    #[test]
    fn macos_topology_policy_admits_only_same_volume_intent() {
        // This is a pure policy check, not an APFS or native durability proof.
        assert!(check_macos_mutation_topologies([SameFilesystem]).is_ok());
        assert!(check_macos_mutation_topologies([SameFilesystem, SameFilesystem]).is_ok());
    }

    #[test]
    fn macos_topology_policy_identifies_the_missing_metadata_backend() {
        let error = check_macos_mutation_topologies([CrossFilesystem, CrossFilesystem])
            .expect_err("cross-volume intent must remain unsupported");
        assert_eq!(error.code, Code::ExecutionUnsupported);
        assert!(error.message.contains("descriptor-bound ACL observation and preservation"));
    }

    #[test]
    fn macos_topology_policy_refuses_mixed_plans_in_either_order() {
        for topologies in [
            [SameFilesystem, CrossFilesystem],
            [CrossFilesystem, SameFilesystem],
        ] {
            let error = check_macos_mutation_topologies(topologies)
                .expect_err("a supported prefix cannot authorize a mixed plan");
            assert_eq!(error.code, Code::ExecutionUnsupported);
            assert!(error.message.contains("descriptor-bound ACL observation and preservation"));
        }
    }
}
