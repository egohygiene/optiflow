//! Durable, source-preserving publication of one independently validated PNG candidate.
//!
//! This is a separate protocol from `artifact-set.v1`, whose closed scan/plan
//! member vocabulary cannot describe binary candidate media. A complete
//! staging directory is *not* committed: only a no-replace directory rename
//! followed by a synchronized parent makes a candidate visible to readers.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::contracts::{self, Contract};

pub const SCHEMA: &str = "optiflow.png-candidate-artifact-set.v1";
pub const EVIDENCE_SCHEMA: &str = "optiflow.png-candidate-evidence.v1";
pub const MARKER_NAME: &str = "candidate-artifact-set.json";
pub const CANDIDATE_NAME: &str = "candidate.png";
pub const EVIDENCE_NAME: &str = "evidence.json";
pub const MAX_CANDIDATE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_EVIDENCE_BYTES: usize = 1024 * 1024;
const MAX_MARKER_BYTES: usize = 16 * 1024;
const STAGING_PREFIX: &str = ".candidate-staging-";

#[derive(Debug, Clone, Copy)]
pub struct PublishLimits {
    pub candidate_bytes: usize,
    pub evidence_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Digest {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub kind: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub digest: Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub set_id: String,
    pub created_at: String,
    pub state: String,
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Committed,
    Incomplete,
    Incompatible,
}

#[derive(Debug, Clone)]
pub struct Inspection {
    pub status: Status,
    pub manifest: Option<Manifest>,
    pub detail: String,
}

/// Refuse a non-private or symlinked store before launching a provider.
pub fn validate_private_root(root: &Path) -> Result<()> {
    let file = open_directory(root)?;
    require_private_root(&file)
}

impl Inspection {
    fn committed(manifest: Manifest) -> Self {
        Self {
            status: Status::Committed,
            manifest: Some(manifest),
            detail: "candidate and evidence match their committed marker".to_owned(),
        }
    }

    fn incomplete(detail: impl Into<String>) -> Self {
        Self {
            status: Status::Incomplete,
            manifest: None,
            detail: detail.into(),
        }
    }

    fn incompatible(detail: impl Into<String>) -> Self {
        Self {
            status: Status::Incompatible,
            manifest: None,
            detail: detail.into(),
        }
    }
}

/// Publish only host-validated bytes and provenance into an existing private
/// root. The coordinator's final source/provider check runs after every member
/// is durable but before publication. A failed check leaves inspectable
/// staging; it cannot produce a committed candidate.
pub fn publish(
    root: &Path,
    set_id: Uuid,
    candidate: &[u8],
    evidence: &Value,
    limits: PublishLimits,
    precommit: impl FnOnce() -> Result<()>,
) -> Result<PathBuf> {
    ensure!(
        limits.candidate_bytes > 0
            && limits.candidate_bytes <= MAX_CANDIDATE_BYTES
            && limits.evidence_bytes > 0
            && limits.evidence_bytes <= MAX_EVIDENCE_BYTES,
        "candidate publication limits exceed the protocol ceilings"
    );
    ensure!(
        !candidate.is_empty() && candidate.len() <= limits.candidate_bytes,
        "candidate exceeds its publication byte bound"
    );
    let candidate_member = member("candidate_png", CANDIDATE_NAME, candidate);
    validate_evidence(evidence, set_id, &candidate_member)?;
    let mut evidence_bytes = serde_json::to_vec_pretty(evidence)?;
    evidence_bytes.push(b'\n');
    ensure!(
        evidence_bytes.len() <= limits.evidence_bytes,
        "candidate evidence exceeds its publication byte bound"
    );

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (root, set_id, candidate, evidence_bytes, precommit);
        bail!("atomic no-replace candidate publication is unsupported on this platform");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use rustix::fs::{Mode, RenameFlags, mkdirat, renameat_with};

        let root_file = open_directory(root)?;
        require_private_root(&root_file)?;
        ensure!(
            fs::symlink_metadata(root.join(set_id.to_string()))
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "refusing to replace an existing candidate namespace"
        );
        let staging_name = staging_name(set_id);
        mkdirat(
            &root_file,
            staging_name.as_str(),
            Mode::RUSR | Mode::WUSR | Mode::XUSR,
        )
        .context("refusing to reuse an existing candidate staging namespace")?;
        root_file
            .sync_all()
            .context("failed to sync staging reservation")?;
        let staging = root.join(&staging_name);
        let staging_file = open_directory(&staging)?;

        write_new(&staging_file, CANDIDATE_NAME, candidate)?;
        write_new(&staging_file, EVIDENCE_NAME, &evidence_bytes)?;
        let manifest = Manifest {
            schema: SCHEMA.to_owned(),
            set_id: set_id.to_string(),
            created_at: Utc::now().to_rfc3339(),
            state: "committed".to_owned(),
            members: vec![
                candidate_member,
                member("evidence", EVIDENCE_NAME, &evidence_bytes),
            ],
        };
        validate_manifest(&manifest, set_id)?;
        let mut marker_bytes = serde_json::to_vec_pretty(&manifest)?;
        marker_bytes.push(b'\n');
        ensure!(
            marker_bytes.len() <= MAX_MARKER_BYTES,
            "candidate marker is too large"
        );
        write_new(&staging_file, MARKER_NAME, &marker_bytes)?;
        staging_file
            .sync_all()
            .context("failed to sync candidate staging directory")?;

        precommit().context("candidate prepublication source/provider check refused")?;
        let final_name = set_id.to_string();
        renameat_with(
            &root_file,
            staging_name.as_str(),
            &root_file,
            final_name.as_str(),
            RenameFlags::NOREPLACE,
        )
        .context("atomic no-replace candidate publication refused")?;
        root_file
            .sync_all()
            .context("candidate became visible but parent durability is uncertain")?;
        let published = root.join(final_name);
        let inspection = inspect(&published);
        ensure!(
            inspection.status == Status::Committed,
            "published candidate did not pass its committed reader: {}",
            inspection.detail
        );
        Ok(published)
    }
}

/// Read-only classification. Staging is always incomplete, even if its marker
/// and payloads are fully written. Readers accept only a UUID-named final set.
pub fn inspect(directory: &Path) -> Inspection {
    let Some(name) = directory.file_name().and_then(|name| name.to_str()) else {
        return Inspection::incompatible("candidate directory has no canonical UUID name");
    };
    if name.starts_with(STAGING_PREFIX) {
        return Inspection::incomplete("candidate staging is not published");
    }
    let Ok(set_id) = Uuid::parse_str(name) else {
        return Inspection::incompatible("candidate directory name is not a UUID");
    };
    if name != set_id.to_string() {
        return Inspection::incompatible("candidate directory UUID is not canonical");
    }
    let root = match directory.parent() {
        Some(root) => root,
        None => return Inspection::incompatible("candidate directory has no parent"),
    };
    let root_file = match open_directory(root) {
        Ok(file) => file,
        Err(error) => return Inspection::incompatible(format!("unsafe candidate root: {error}")),
    };
    if let Err(error) = require_private_root(&root_file) {
        return Inspection::incompatible(format!("unsafe candidate root: {error}"));
    }
    if let Err(error) = require_absent_provider_work(root, set_id) {
        return Inspection::incompatible(error.to_string());
    }
    let directory_file = match open_directory(directory) {
        Ok(file) => file,
        Err(error) => {
            return Inspection::incomplete(format!("candidate set is unavailable: {error}"));
        }
    };
    let marker_bytes = match read_member(&directory_file, MARKER_NAME, MAX_MARKER_BYTES) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Inspection::incomplete(format!("candidate marker is unavailable: {error}"));
        }
    };
    let marker_value: Value = match serde_json::from_slice(&marker_bytes) {
        Ok(value) => value,
        Err(error) => {
            return Inspection::incompatible(format!("candidate marker JSON is invalid: {error}"));
        }
    };
    let manifest: Manifest = match serde_json::from_value(marker_value) {
        Ok(value) => value,
        Err(error) => {
            return Inspection::incompatible(format!(
                "candidate marker shape is incompatible: {error}"
            ));
        }
    };
    if let Err(error) = validate_manifest(&manifest, set_id) {
        return Inspection::incompatible(format!("candidate marker is incompatible: {error}"));
    }
    let children = match fs::read_dir(directory) {
        Ok(entries) => entries
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<std::io::Result<Vec<_>>>(),
        Err(error) => Err(error),
    };
    match children {
        Ok(entries)
            if entries.iter().any(|name| {
                ![CANDIDATE_NAME, EVIDENCE_NAME, MARKER_NAME]
                    .iter()
                    .any(|expected| name == *expected)
            }) =>
        {
            return Inspection::incompatible("candidate set has unexpected members");
        }
        Ok(entries) if entries.len() != 3 => {
            return Inspection::incomplete("candidate set is missing a declared member");
        }
        Ok(_) => {}
        Err(error) => {
            return Inspection::incomplete(format!("candidate set cannot be listed: {error}"));
        }
    }
    for (member, max) in [
        (&manifest.members[0], MAX_CANDIDATE_BYTES),
        (&manifest.members[1], MAX_EVIDENCE_BYTES),
    ] {
        let bytes = match read_member(&directory_file, &member.file_name, max) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Inspection::incomplete(format!("candidate member is unavailable: {error}"));
            }
        };
        if bytes.len() as u64 != member.size_bytes || digest(&bytes) != member.digest.value {
            return Inspection::incomplete("candidate member bytes disagree with committed marker");
        }
        if member.kind == "evidence" {
            let evidence: Value = match serde_json::from_slice(&bytes) {
                Ok(value) => value,
                Err(error) => {
                    return Inspection::incompatible(format!(
                        "candidate evidence JSON is invalid: {error}"
                    ));
                }
            };
            if let Err(error) = validate_evidence(&evidence, set_id, &manifest.members[0]) {
                return Inspection::incompatible(format!(
                    "candidate evidence is incompatible: {error}"
                ));
            }
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let current = match open_directory(directory) {
            Ok(file) => file,
            Err(error) => {
                return Inspection::incomplete(format!(
                    "candidate directory path changed during inspection: {error}"
                ));
            }
        };
        let bound = match directory_file.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                return Inspection::incomplete(format!(
                    "candidate directory identity is unavailable: {error}"
                ));
            }
        };
        let current = match current.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                return Inspection::incomplete(format!(
                    "candidate directory identity changed: {error}"
                ));
            }
        };
        if bound.dev() != current.dev() || bound.ino() != current.ino() {
            return Inspection::incomplete("candidate directory path changed during inspection");
        }
    }
    Inspection::committed(manifest)
}

/// Inspect a final set or safely discard only the exact abandoned private
/// staging namespace. A conflicting final+staging pair is left untouched for
/// human inspection; no incomplete or ambiguous work is promoted.
pub fn recover(root: &Path, set_id: Uuid) -> Inspection {
    let root_file = match open_directory(root) {
        Ok(file) => file,
        Err(error) => return Inspection::incompatible(format!("unsafe candidate root: {error}")),
    };
    if let Err(error) = require_private_root(&root_file) {
        return Inspection::incompatible(format!("unsafe candidate root: {error}"));
    }
    if let Err(error) = require_absent_provider_work(root, set_id) {
        return Inspection::incompatible(error.to_string());
    }
    let final_path = root.join(set_id.to_string());
    let staging_name = staging_name(set_id);
    let staging = root.join(&staging_name);
    let final_exists = fs::symlink_metadata(&final_path).is_ok();
    let staging_exists = fs::symlink_metadata(&staging).is_ok();
    if final_exists && staging_exists {
        return Inspection::incompatible("candidate final and staging namespaces both exist");
    }
    if final_exists {
        return inspect(&final_path);
    }
    if !staging_exists {
        return Inspection::incomplete("candidate has no published set");
    }
    match discard_staging(&root_file, &staging, &staging_name) {
        Ok(()) => Inspection::incomplete("abandoned candidate staging discarded; no set published"),
        Err(error) => {
            Inspection::incompatible(format!("candidate staging requires inspection: {error}"))
        }
    }
}

fn member(kind: &str, file_name: &str, bytes: &[u8]) -> Member {
    Member {
        kind: kind.to_owned(),
        file_name: file_name.to_owned(),
        size_bytes: bytes.len() as u64,
        digest: Digest {
            algorithm: "blake3-256".to_owned(),
            value: digest(bytes),
        },
    }
}

fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

// A killed provider may leave this namespace behind. It is outside the
// three-member set and must never be inferred committed or deleted by set
// recovery, even if a seemingly valid final marker also exists.
fn require_absent_provider_work(root: &Path, set_id: Uuid) -> Result<()> {
    let provider_work = root.join(format!(".provider-work-{set_id}"));
    match fs::symlink_metadata(provider_work) {
        Ok(_) => bail!("provider work directory remains; inspect it separately before recovery"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => bail!("provider work directory state is unknown: {error}"),
    }
}

fn validate_evidence(evidence: &Value, set_id: Uuid, candidate: &Member) -> Result<()> {
    contracts::validate(Contract::PngCandidateEvidence, evidence)
        .context("candidate production evidence does not satisfy its v1 contract")?;
    ensure!(
        evidence.get("schema").and_then(Value::as_str) == Some(EVIDENCE_SCHEMA),
        "candidate evidence schema is unsupported"
    );
    ensure!(
        evidence.get("artifact_set_id").and_then(Value::as_str)
            == Some(set_id.to_string().as_str()),
        "candidate evidence set identity disagrees with marker"
    );
    let content = evidence
        .pointer("/candidate/content")
        .context("candidate evidence omits observed candidate content")?;
    ensure!(
        content.get("logical_bytes").and_then(Value::as_u64) == Some(candidate.size_bytes)
            && content.pointer("/digest/algorithm").and_then(Value::as_str) == Some("blake3-256")
            && content.pointer("/digest/value").and_then(Value::as_str)
                == Some(candidate.digest.value.as_str()),
        "candidate evidence content identity disagrees with committed media bytes"
    );
    let source = evidence
        .pointer("/source/content/logical_bytes")
        .and_then(Value::as_u64)
        .context("candidate evidence omits source logical size")?;
    let reduction = source
        .checked_sub(candidate.size_bytes)
        .context("candidate is not strictly smaller than the source")?;
    ensure!(
        reduction > 0
            && evidence
                .get("encoded_logical_reduction_bytes")
                .and_then(Value::as_u64)
                == Some(reduction),
        "claimed logical reduction disagrees with source and candidate bytes"
    );
    ensure!(
        evidence.pointer("/source/before") == evidence.pointer("/source/after")
            && evidence
                .pointer("/source/before/size_bytes")
                .and_then(Value::as_u64)
                == Some(source),
        "source metadata or size changed across candidate production"
    );
    ensure!(
        evidence
            .pointer("/measurements/captured_stdout_bytes")
            .and_then(Value::as_u64)
            == Some(candidate.size_bytes)
            && evidence.pointer("/measurements/decoded_source_bytes")
                == evidence.pointer("/validation/source_png/decoded_bytes")
            && evidence.pointer("/measurements/decoded_candidate_bytes")
                == evidence.pointer("/validation/candidate_png/decoded_bytes"),
        "measured output or decoded size contradicts the independent validation"
    );
    for field in [
        "width",
        "height",
        "bit_depth",
        "color_type",
        "frames",
        "ihdr_digest",
        "decoded_samples_digest",
        "non_idat_chunks_digest",
    ] {
        ensure!(
            evidence.pointer(&format!("/validation/source_png/{field}"))
                == evidence.pointer(&format!("/validation/candidate_png/{field}")),
            "source and candidate validation facts differ for {field}"
        );
    }
    let limits = evidence
        .pointer("/limits")
        .context("candidate evidence omits limits")?;
    for (measure, limit) in [
        ("/measurements/captured_stdout_bytes", "candidate_bytes"),
        ("/measurements/captured_stderr_bytes", "stderr_bytes"),
        (
            "/measurements/decoded_source_bytes",
            "decoded_bytes_per_image",
        ),
        (
            "/measurements/decoded_candidate_bytes",
            "decoded_bytes_per_image",
        ),
    ] {
        ensure!(
            evidence.pointer(measure).and_then(Value::as_u64)
                <= limits.get(limit).and_then(Value::as_u64),
            "measured value exceeds the declared {limit} bound"
        );
    }
    ensure!(
        source
            <= limits
                .get("source_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0)
            && candidate.size_bytes
                <= limits
                    .get("candidate_bytes")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
        "source or candidate exceeds its declared byte bound"
    );
    let raw_cap = evidence
        .pointer("/argv/8")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok())
        .context("provider raw-byte argument is not a bounded integer")?;
    let decoded = evidence
        .pointer("/validation/source_png/decoded_bytes")
        .and_then(Value::as_u64)
        .context("candidate evidence omits decoded source size")?;
    let height = evidence
        .pointer("/validation/source_png/height")
        .and_then(Value::as_u64)
        .context("candidate evidence omits source height")?;
    let expected_cap = decoded
        .checked_add(height)
        .context("decoded raw-byte cap overflows")?
        .max(source);
    ensure!(
        raw_cap == expected_cap,
        "provider raw-byte argument disagrees with bounded source bytes"
    );
    Ok(())
}

fn validate_manifest(manifest: &Manifest, set_id: Uuid) -> Result<()> {
    let schema: Value = serde_json::from_str(include_str!(
        "../schemas/png-candidate-artifact-set-v1.schema.json"
    ))?;
    let validator = jsonschema::validator_for(&schema)?;
    if let Err(error) = validator.validate(&serde_json::to_value(manifest)?) {
        bail!(
            "schema validation failed at {}: {error}",
            error.instance_path()
        );
    }
    ensure!(
        manifest.set_id == set_id.to_string(),
        "candidate set identity mismatch"
    );
    chrono::DateTime::parse_from_rfc3339(&manifest.created_at)
        .context("candidate marker timestamp is invalid")?;
    ensure!(
        manifest.members.len() == 2
            && manifest.members[0].kind == "candidate_png"
            && manifest.members[0].file_name == CANDIDATE_NAME
            && manifest.members[1].kind == "evidence"
            && manifest.members[1].file_name == EVIDENCE_NAME,
        "candidate marker member names are incompatible"
    );
    Ok(())
}

fn staging_name(set_id: Uuid) -> String {
    format!("{STAGING_PREFIX}{set_id}")
}

#[cfg(unix)]
fn open_directory(path: &Path) -> Result<File> {
    use rustix::fs::{Mode, OFlags, open, openat};

    ensure!(path.is_absolute(), "candidate paths must be absolute");
    let components: Vec<_> = path.components().collect();
    ensure!(
        components
            .iter()
            .all(|part| !matches!(part, Component::ParentDir | Component::CurDir)),
        "candidate paths must be normalized"
    );
    let rebuilt: PathBuf = components.iter().copied().collect();
    ensure!(rebuilt == path, "candidate paths must be normalized");
    let flags =
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let mut handle = open("/", flags, Mode::empty())?;
    for part in components {
        if let Component::Normal(name) = part {
            handle = openat(&handle, name, flags, Mode::empty())
                .with_context(|| format!("unsafe directory component in {}", path.display()))?;
        }
    }
    Ok(File::from(handle))
}

#[cfg(not(unix))]
fn open_directory(_path: &Path) -> Result<File> {
    bail!("candidate publication needs Unix no-follow directory handles")
}

#[cfg(unix)]
fn require_private_root(file: &File) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = file.metadata()?;
    ensure!(metadata.is_dir(), "candidate root is not a directory");
    ensure!(
        metadata.permissions().mode() & 0o077 == 0,
        "candidate root must exclude group/other access"
    );
    Ok(())
}

#[cfg(not(unix))]
fn require_private_root(_file: &File) -> Result<()> {
    bail!("candidate publication needs a private Unix directory")
}

#[cfg(unix)]
fn write_new(directory: &File, name: &str, bytes: &[u8]) -> Result<()> {
    use rustix::fs::{Mode, OFlags, openat};
    let mut file = File::from(openat(
        directory,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )?);
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn read_member(directory: &File, name: &str, maximum: usize) -> Result<Vec<u8>> {
    use rustix::fs::{Mode, OFlags, openat};
    use std::os::unix::fs::MetadataExt;

    let mut file = File::from(openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )?);
    let before = file.metadata()?;
    ensure!(
        before.is_file() && before.nlink() == 1,
        "candidate member is not a singly linked regular file"
    );
    ensure!(
        before.len() <= maximum as u64,
        "candidate member exceeds byte ceiling"
    );
    let mut bytes = Vec::new();
    (&mut file)
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= maximum,
        "candidate member grew beyond byte ceiling"
    );
    let after = file.metadata()?;
    ensure!(
        before.dev() == after.dev()
            && before.ino() == after.ino()
            && before.len() == after.len()
            && before.mtime() == after.mtime()
            && before.mtime_nsec() == after.mtime_nsec()
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec()
            && bytes.len() as u64 == after.len(),
        "candidate member changed during inspection"
    );
    // Verify that the directory entry still names the file whose bytes were
    // read. A completed read from a handle alone does not prove path binding.
    let current = File::from(openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )?);
    let current = current.metadata()?;
    ensure!(
        current.is_file()
            && current.dev() == after.dev()
            && current.ino() == after.ino()
            && current.len() == after.len()
            && current.mtime() == after.mtime()
            && current.mtime_nsec() == after.mtime_nsec()
            && current.ctime() == after.ctime()
            && current.ctime_nsec() == after.ctime_nsec(),
        "candidate member path changed during inspection"
    );
    Ok(bytes)
}

#[cfg(not(unix))]
fn read_member(_directory: &File, _name: &str, _maximum: usize) -> Result<Vec<u8>> {
    bail!("candidate reader needs Unix no-follow file handles")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn discard_staging(root: &File, staging: &Path, staging_name: &str) -> Result<()> {
    use rustix::fs::{AtFlags, unlinkat};

    let directory = open_directory(staging)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(staging)? {
        let entry = entry?;
        let name = entry.file_name();
        ensure!(
            [CANDIDATE_NAME, EVIDENCE_NAME, MARKER_NAME]
                .iter()
                .any(|expected| name == *expected),
            "staging contains an unexpected entry"
        );
        let metadata = fs::symlink_metadata(entry.path())?;
        ensure!(
            metadata.file_type().is_file(),
            "staging member is not regular"
        );
        entries.push(name);
    }
    for name in entries {
        unlinkat(&directory, &name, AtFlags::empty())?;
    }
    directory.sync_all()?;
    unlinkat(root, staging_name, AtFlags::REMOVEDIR)?;
    root.sync_all()?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn discard_staging(_root: &File, _staging: &Path, _staging_name: &str) -> Result<()> {
    bail!("candidate staging recovery is unsupported on this platform")
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use anyhow::bail;
    use serde_json::json;
    use tempfile::{TempDir, tempdir};

    use super::*;

    fn evidence(id: Uuid, candidate: &[u8]) -> Value {
        let digest_value =
            |byte: char| json!({ "algorithm": "blake3-256", "value": byte.to_string().repeat(64) });
        let snapshot = json!({
            "device_id": 1, "inode": 2, "mode": 33152, "owner_uid": 1000,
            "group_gid": 1000, "link_count": 1, "size_bytes": candidate.len() + 1,
            "modified_unix_ns": 1, "changed_unix_ns": 1
        });
        let png = json!({
            "width": 1, "height": 1, "bit_depth": 8, "color_type": 2,
            "frames": 1, "complete_decode": true, "animation_chunks": false,
            "unknown_unsafe_to_copy_chunks": false, "decoded_bytes": 3,
            "ihdr_digest": digest_value('a'),
            "decoded_samples_digest": digest_value('b'),
            "non_idat_chunks_digest": digest_value('c')
        });
        json!({
            "schema": EVIDENCE_SCHEMA,
            "artifact_set_id": id.to_string(),
            "profile": "optiflow.png-idat-preserve.v1",
            "source_observation_id": Uuid::now_v7().to_string(),
            "source": {
                "path": { "encoding": "utf8", "value": "/synthetic/source.png" },
                "before": snapshot, "after": snapshot,
                "content": { "digest": digest_value('d'), "logical_bytes": candidate.len() + 1 }
            },
            "effective_policy_fingerprint": digest_value('e'),
            "producer": {
                "name": "oxipng", "version": "10.2.1",
                "executable": { "encoding": "utf8", "value": "/synthetic/oxipng" },
                "binary_digest": digest_value('f'),
                "invocation_fingerprint": digest_value('a')
            },
            "argv": ["--opt", "2", "--nx", "--interlace", "keep", "--threads", "1", "--max-raw-size", (candidate.len() + 1).max(4).to_string(), "--quiet", "--stdout", "-"],
            "input_binding": "complete_bounded_source_bytes_on_stdin",
            "output_binding": "bounded_stdout_then_independent_validation",
            "provider_configuration": "oxipng_v10.2.1_opt2_nx_keep_interlace_one_thread_no_strip",
            "candidate": {
                "file_name": CANDIDATE_NAME,
                "content": {
                    "digest": { "algorithm": "blake3-256", "value": digest(candidate) },
                    "logical_bytes": candidate.len()
                }
            },
            "limits": {
                "source_bytes": 1024, "candidate_bytes": 1024,
                "decoded_bytes_per_image": 1024, "decoder_allocation_bytes": 1024,
                "chunks_per_image": 2, "provider_binary_bytes": 1024,
                "stderr_bytes": 1024, "evidence_bytes": 8192, "elapsed_ms": 1000
            },
            "measurements": {
                "elapsed_ms": 1, "captured_stdout_bytes": candidate.len(),
                "captured_stderr_bytes": 0, "decoded_source_bytes": 3,
                "decoded_candidate_bytes": 3, "peak_provider_memory_bytes": null,
                "peak_provider_temporary_bytes": null,
                "provider_resource_enforcement": "host_bounds_stdin_stdout_stderr_elapsed_and_validator_buffers;provider_rss_and_private_workdir_usage_unmeasured"
            },
            "validation": {
                "source_png": png, "candidate_png": png,
                "independent_byte_validation": true, "exact_decoded_samples": true,
                "ordered_non_idat_chunks_preserved": true, "strict_encoded_reduction": true
            },
            "encoded_logical_reduction_bytes": 1,
            "physical_savings_bytes": null,
            "cleanup": { "provider_work_directory_removed": true, "source_mutation": false }
        })
    }

    fn limits() -> PublishLimits {
        PublishLimits {
            candidate_bytes: 1024,
            evidence_bytes: 8192,
        }
    }

    fn private_root() -> TempDir {
        let root = tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        root
    }

    #[test]
    fn committed_candidate_requires_exact_unchanged_members() {
        let root = private_root();
        let id = Uuid::now_v7();
        let candidate = b"\x89PNG\r\n\x1a\nsynthetic";
        let directory = publish(
            root.path(),
            id,
            candidate,
            &evidence(id, candidate),
            limits(),
            || Ok(()),
        )
        .unwrap();
        let inspection = inspect(&directory);
        assert_eq!(inspection.status, Status::Committed);
        assert_eq!(inspection.manifest.unwrap().members.len(), 2);
        assert_eq!(recover(root.path(), id).status, Status::Committed);
        assert!(
            publish(
                root.path(),
                id,
                candidate,
                &evidence(id, candidate),
                limits(),
                || Ok(())
            )
            .is_err()
        );
        assert!(!root.path().join(staging_name(id)).exists());

        fs::write(directory.join(CANDIDATE_NAME), b"tampered").unwrap();
        assert_eq!(inspect(&directory).status, Status::Incomplete);
        assert_eq!(recover(root.path(), id).status, Status::Incomplete);
    }

    #[test]
    fn precommit_refusal_stays_unpublished_and_recovery_discards_only_own_stage() {
        let root = private_root();
        let id = Uuid::now_v7();
        let candidate = b"\x89PNG\r\n\x1a\nsynthetic";
        assert!(
            publish(
                root.path(),
                id,
                candidate,
                &evidence(id, candidate),
                limits(),
                || {
                    assert_eq!(
                        inspect(&root.path().join(staging_name(id))).status,
                        Status::Incomplete
                    );
                    bail!("source changed before commit")
                }
            )
            .is_err()
        );
        assert!(!root.path().join(id.to_string()).exists());
        assert!(root.path().join(staging_name(id)).exists());
        assert_eq!(recover(root.path(), id).status, Status::Incomplete);
        assert!(!root.path().join(staging_name(id)).exists());
        assert!(!root.path().join(id.to_string()).exists());
    }

    #[test]
    fn unknown_staging_entry_is_ambiguous_and_never_deleted() {
        let root = private_root();
        let id = Uuid::now_v7();
        let candidate = b"\x89PNG\r\n\x1a\nsynthetic";
        let _ = publish(
            root.path(),
            id,
            candidate,
            &evidence(id, candidate),
            limits(),
            || bail!("deliberate interruption"),
        );
        let extra = root.path().join(staging_name(id)).join("unrecognized");
        fs::write(&extra, b"leave me alone").unwrap();
        assert_eq!(recover(root.path(), id).status, Status::Incompatible);
        assert_eq!(fs::read(extra).unwrap(), b"leave me alone");
    }

    #[test]
    fn evidence_and_byte_bounds_refuse_without_staging() {
        let root = private_root();
        let id = Uuid::now_v7();
        let mut wrong = evidence(id, b"x");
        wrong["artifact_set_id"] = json!(Uuid::now_v7().to_string());
        assert!(publish(root.path(), id, b"x", &wrong, limits(), || Ok(())).is_err());
        assert!(
            publish(
                root.path(),
                id,
                &[0; 1025],
                &evidence(id, &[0; 1025]),
                limits(),
                || Ok(())
            )
            .is_err()
        );
        assert!(!root.path().join(staging_name(id)).exists());
    }

    #[test]
    fn malformed_or_contradictory_evidence_never_creates_staging() {
        let root = private_root();
        let id = Uuid::now_v7();
        let candidate = b"synthetic candidate";
        let baseline = evidence(id, candidate);
        validate_evidence(
            &baseline,
            id,
            &member("candidate_png", CANDIDATE_NAME, candidate),
        )
        .expect("closed test evidence satisfies the production contract");
        let mut invalid = Vec::new();
        let mut missing = baseline.clone();
        missing.as_object_mut().unwrap().remove("source");
        invalid.push(missing);
        let mut extra = baseline.clone();
        extra["provider_untrusted_claim"] = json!(true);
        invalid.push(extra);
        let mut false_validation = baseline.clone();
        false_validation["validation"]["exact_decoded_samples"] = json!(false);
        invalid.push(false_validation);
        let mut fabricated_savings = baseline.clone();
        fabricated_savings["physical_savings_bytes"] = json!(1);
        invalid.push(fabricated_savings);
        let mut fabricated_peak = baseline.clone();
        fabricated_peak["measurements"]["peak_provider_memory_bytes"] = json!(1);
        invalid.push(fabricated_peak);
        let mut wrong_reduction = baseline.clone();
        wrong_reduction["encoded_logical_reduction_bytes"] = json!(2);
        invalid.push(wrong_reduction);
        let mut changed_source = baseline.clone();
        changed_source["source"]["after"]["inode"] = json!(3);
        invalid.push(changed_source);
        let mut spoofed_argv = baseline.clone();
        spoofed_argv["argv"][2] = json!("--strip");
        invalid.push(spoofed_argv);

        for document in invalid {
            assert!(
                publish(root.path(), id, candidate, &document, limits(), || Ok(())).is_err(),
                "invalid evidence must be rejected"
            );
            assert!(!root.path().join(staging_name(id)).exists());
        }
    }

    #[test]
    fn symlinked_root_and_member_are_never_read_as_committed() {
        use std::os::unix::fs::symlink;

        let root = private_root();
        let link = root.path().join("linked");
        symlink(root.path(), &link).unwrap();
        let id = Uuid::now_v7();
        assert!(publish(&link, id, b"x", &evidence(id, b"x"), limits(), || Ok(())).is_err());

        let directory = publish(root.path(), id, b"x", &evidence(id, b"x"), limits(), || {
            Ok(())
        })
        .unwrap();
        fs::remove_file(directory.join(CANDIDATE_NAME)).unwrap();
        symlink(
            root.path().join("outside.png"),
            directory.join(CANDIDATE_NAME),
        )
        .unwrap();
        assert_eq!(inspect(&directory).status, Status::Incomplete);
    }

    #[test]
    fn internally_consistent_member_digest_cannot_override_evidence_content_binding() {
        let root = private_root();
        let id = Uuid::now_v7();
        let candidate = b"\x89PNG\r\n\x1a\nsynthetic";
        let directory = publish(
            root.path(),
            id,
            candidate,
            &evidence(id, candidate),
            limits(),
            || Ok(()),
        )
        .unwrap();
        let evidence_path = directory.join(EVIDENCE_NAME);
        let mut evidence: Value =
            serde_json::from_slice(&fs::read(&evidence_path).unwrap()).unwrap();
        evidence["candidate"]["content"]["digest"]["value"] = json!("0".repeat(64));
        let mut bytes = serde_json::to_vec_pretty(&evidence).unwrap();
        bytes.push(b'\n');
        fs::write(&evidence_path, &bytes).unwrap();

        let marker_path = directory.join(MARKER_NAME);
        let mut marker: Manifest =
            serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
        marker.members[1] = member("evidence", EVIDENCE_NAME, &bytes);
        let mut marker_bytes = serde_json::to_vec_pretty(&marker).unwrap();
        marker_bytes.push(b'\n');
        fs::write(marker_path, marker_bytes).unwrap();
        assert_eq!(inspect(&directory).status, Status::Incompatible);
    }

    #[test]
    fn missing_declared_member_is_incomplete() {
        let root = private_root();
        let id = Uuid::now_v7();
        let candidate = b"\x89PNG\r\n\x1a\nsynthetic";
        let directory = publish(
            root.path(),
            id,
            candidate,
            &evidence(id, candidate),
            limits(),
            || Ok(()),
        )
        .unwrap();
        fs::remove_file(directory.join(EVIDENCE_NAME)).unwrap();
        assert_eq!(inspect(&directory).status, Status::Incomplete);
    }

    #[test]
    fn recovery_preserves_abandoned_provider_work_for_manual_inspection() {
        let root = private_root();
        let id = Uuid::now_v7();
        let work = root.path().join(format!(".provider-work-{id}"));
        fs::create_dir(&work).unwrap();
        fs::write(work.join("unknown"), b"opaque provider bytes").unwrap();
        let inspection = recover(root.path(), id);
        assert_eq!(inspection.status, Status::Incompatible);
        assert!(inspection.detail.contains("provider work directory"));
        assert_eq!(
            fs::read(work.join("unknown")).unwrap(),
            b"opaque provider bytes"
        );
    }

    #[test]
    fn provider_work_conflicting_with_final_marker_never_appears_committed() {
        let root = private_root();
        let id = Uuid::now_v7();
        let candidate = b"synthetic candidate";
        let directory = publish(
            root.path(),
            id,
            candidate,
            &evidence(id, candidate),
            limits(),
            || Ok(()),
        )
        .unwrap();
        let work = root.path().join(format!(".provider-work-{id}"));
        fs::create_dir(&work).unwrap();
        assert_eq!(inspect(&directory).status, Status::Incompatible);
        assert_eq!(recover(root.path(), id).status, Status::Incompatible);
        assert!(work.exists());
        assert!(directory.exists());
    }
}
