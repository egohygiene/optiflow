//! Transactional publication for related JSON artifacts.
//!
//! Scan artifacts are prepared in a sibling directory and exposed together by
//! one directory rename. Plan outputs retain their public file path and use a
//! pending marker that recovery can promote only after verifying the plan.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contracts::{self, Contract};
use crate::domain::NativePath;

pub const ARTIFACT_SET_SCHEMA: &str = "optiflow.artifact-set.v1";
pub const SCAN_MARKER_FILE_NAME: &str = "artifact-set.json";
const STAGING_INFIX: &str = ".artifact-set-staging-";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactSetKind {
    Scan,
    Plan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactSetCommitState {
    Committed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDigest {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSetMember {
    pub kind: String,
    pub schema: String,
    pub path: NativePath,
    pub size_bytes: u64,
    pub digest: ArtifactDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSetManifest {
    pub schema: String,
    pub set_id: String,
    pub set_kind: ArtifactSetKind,
    pub run_id: String,
    pub created_at: String,
    pub state: ArtifactSetCommitState,
    pub source_set_id: Option<String>,
    pub members: Vec<ArtifactSetMember>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactSetStatus {
    Committed,
    Incomplete,
    Incompatible,
}

#[derive(Debug, Clone)]
pub struct ArtifactSetInspection {
    pub status: ArtifactSetStatus,
    pub manifest: Option<ArtifactSetManifest>,
    pub detail: String,
}

impl ArtifactSetInspection {
    fn committed(manifest: ArtifactSetManifest) -> Self {
        Self {
            status: ArtifactSetStatus::Committed,
            manifest: Some(manifest),
            detail: "artifact set is committed and all members match the marker".to_owned(),
        }
    }

    fn incomplete(detail: impl Into<String>) -> Self {
        Self {
            status: ArtifactSetStatus::Incomplete,
            manifest: None,
            detail: detail.into(),
        }
    }

    fn incompatible(detail: impl Into<String>) -> Self {
        Self {
            status: ArtifactSetStatus::Incompatible,
            manifest: None,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArtifactPayload {
    pub kind: String,
    pub schema: String,
    pub file_name: OsString,
    bytes: Vec<u8>,
}

impl ArtifactPayload {
    pub fn json<T: Serialize>(
        kind: impl Into<String>,
        schema: impl Into<String>,
        file_name: impl Into<OsString>,
        value: &T,
    ) -> Result<Self> {
        let mut bytes =
            serde_json::to_vec_pretty(value).context("failed to serialize artifact-set member")?;
        bytes.push(b'\n');
        Ok(Self {
            kind: kind.into(),
            schema: schema.into(),
            file_name: file_name.into(),
            bytes,
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum CommitFault {
    None,
    #[cfg(test)]
    CrashAfterMember(usize),
    #[cfg(test)]
    DiskFullAfterBytes(usize),
    #[cfg(test)]
    CrashAfterPublish,
}

#[derive(Debug)]
struct FaultController {
    #[cfg(test)]
    mode: CommitFault,
    bytes_written: usize,
    members_written: usize,
}

impl FaultController {
    fn new(mode: CommitFault) -> Self {
        #[cfg(not(test))]
        let _ = mode;
        Self {
            #[cfg(test)]
            mode,
            bytes_written: 0,
            members_written: 0,
        }
    }

    fn member_committed(&mut self) -> Result<()> {
        self.members_written = self.members_written.saturating_add(1);
        #[cfg(test)]
        if matches!(
            self.mode,
            CommitFault::CrashAfterMember(count) if count == self.members_written
        ) {
            bail!("simulated crash after staged artifact member");
        }
        Ok(())
    }

    fn published(&self) -> Result<()> {
        #[cfg(test)]
        if matches!(self.mode, CommitFault::CrashAfterPublish) {
            bail!("simulated crash after publication rename");
        }
        Ok(())
    }
}

pub fn commit_scan_set(
    final_directory: &Path,
    run_id: &str,
    set_id: &str,
    payloads: Vec<ArtifactPayload>,
) -> Result<ArtifactSetManifest> {
    commit_scan_set_with_fault(final_directory, run_id, set_id, payloads, CommitFault::None)
}

fn commit_scan_set_with_fault(
    final_directory: &Path,
    run_id: &str,
    set_id: &str,
    payloads: Vec<ArtifactPayload>,
    fault: CommitFault,
) -> Result<ArtifactSetManifest> {
    require_payload_kinds(&payloads, &["effective_policy", "report", "run"])?;
    let parent = final_directory
        .parent()
        .context("scan artifact-set directory has no parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create artifact-set parent {}", parent.display()))?;
    if final_directory.exists() {
        bail!(
            "refusing to replace existing artifact set {}",
            final_directory.display()
        );
    }

    let final_name = final_directory
        .file_name()
        .context("scan artifact-set directory has no file name")?;
    Uuid::parse_str(set_id).context("artifact-set identifier is not a UUID")?;
    let staging_directory = parent.join(staging_name(final_name, set_id));
    fs::create_dir(&staging_directory).with_context(|| {
        format!(
            "failed to create artifact-set staging directory {}",
            staging_directory.display()
        )
    })?;

    let mut controller = FaultController::new(fault);
    let mut members = Vec::with_capacity(payloads.len());
    for payload in payloads {
        validate_member_name(Path::new(&payload.file_name))?;
        let staged_path = staging_directory.join(&payload.file_name);
        write_new_file(&staged_path, &payload.bytes, &mut controller)?;
        members.push(member_from_payload(&payload)?);
        controller.member_committed()?;
    }
    members.sort_by(|left, right| left.kind.cmp(&right.kind));

    let manifest = ArtifactSetManifest {
        schema: ARTIFACT_SET_SCHEMA.to_owned(),
        set_id: set_id.to_owned(),
        set_kind: ArtifactSetKind::Scan,
        run_id: run_id.to_owned(),
        created_at: Utc::now().to_rfc3339(),
        state: ArtifactSetCommitState::Committed,
        source_set_id: None,
        members,
    };
    contracts::validate(Contract::ArtifactSet, &manifest)
        .context("generated scan artifact-set marker is invalid")?;
    let marker_bytes = json_bytes(&manifest)?;
    write_new_file(
        &staging_directory.join(SCAN_MARKER_FILE_NAME),
        &marker_bytes,
        &mut controller,
    )?;
    sync_directory(&staging_directory)?;

    fs::rename(&staging_directory, final_directory).with_context(|| {
        format!(
            "failed to publish artifact set {}",
            final_directory.display()
        )
    })?;
    controller.published()?;
    sync_directory(parent)?;
    Ok(manifest)
}

pub fn commit_plan_set(
    output_path: &Path,
    run_id: &str,
    source_set_id: Option<&str>,
    payload: ArtifactPayload,
) -> Result<ArtifactSetManifest> {
    commit_plan_set_with_fault(
        output_path,
        run_id,
        source_set_id,
        payload,
        CommitFault::None,
    )
}

fn commit_plan_set_with_fault(
    output_path: &Path,
    run_id: &str,
    source_set_id: Option<&str>,
    payload: ArtifactPayload,
    fault: CommitFault,
) -> Result<ArtifactSetManifest> {
    if payload.kind != "plan" {
        bail!("plan artifact set requires exactly one plan payload");
    }
    let parent = artifact_parent(output_path)?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create plan output directory {}",
            parent.display()
        )
    })?;

    let recovered = recover_plan_set(output_path)?;
    if recovered.status == ArtifactSetStatus::Committed {
        bail!(
            "refusing to replace committed plan artifact set {}",
            output_path.display()
        );
    }
    if output_path.exists() || plan_pending_marker_path(output_path).exists() {
        bail!(
            "refusing to replace incomplete or unrecognized plan output {}",
            output_path.display()
        );
    }

    let output_name = output_path
        .file_name()
        .context("plan output path has no file name")?;
    if payload.file_name != output_name {
        bail!("plan payload name does not match its output destination");
    }
    validate_member_name(Path::new(&payload.file_name))?;

    let set_id = Uuid::now_v7().to_string();
    let staging_path = parent.join(staging_name(output_name, &set_id));
    let pending_marker = plan_pending_marker_path(output_path);
    let marker_path = plan_marker_path(output_path);
    let mut controller = FaultController::new(fault);
    write_new_file(&staging_path, &payload.bytes, &mut controller)?;
    controller.member_committed()?;

    let manifest = ArtifactSetManifest {
        schema: ARTIFACT_SET_SCHEMA.to_owned(),
        set_id,
        set_kind: ArtifactSetKind::Plan,
        run_id: run_id.to_owned(),
        created_at: Utc::now().to_rfc3339(),
        state: ArtifactSetCommitState::Committed,
        source_set_id: source_set_id.map(str::to_owned),
        members: vec![member_from_payload(&payload)?],
    };
    contracts::validate(Contract::ArtifactSet, &manifest)
        .context("generated plan artifact-set marker is invalid")?;
    write_new_file(&pending_marker, &json_bytes(&manifest)?, &mut controller)?;
    sync_directory(parent)?;

    fs::rename(&staging_path, output_path)
        .with_context(|| format!("failed to publish plan {}", output_path.display()))?;
    sync_directory(parent)?;
    controller.published()?;
    fs::rename(&pending_marker, &marker_path).with_context(|| {
        format!(
            "failed to publish plan artifact-set marker {}",
            marker_path.display()
        )
    })?;
    sync_directory(parent)?;
    Ok(manifest)
}

pub fn inspect_scan_set(directory: &Path) -> ArtifactSetInspection {
    inspect_marker(
        &directory.join(SCAN_MARKER_FILE_NAME),
        directory,
        ArtifactSetKind::Scan,
    )
}

pub fn inspect_plan_set(output_path: &Path) -> ArtifactSetInspection {
    let parent = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let inspection = inspect_marker(
        &plan_marker_path(output_path),
        parent,
        ArtifactSetKind::Plan,
    );
    if inspection.status == ArtifactSetStatus::Incomplete
        && plan_pending_marker_path(output_path).exists()
    {
        return ArtifactSetInspection::incomplete(
            "plan member was published but its pending commit marker requires recovery",
        );
    }
    inspection
}

pub fn recover_plan_set(output_path: &Path) -> Result<ArtifactSetInspection> {
    let parent = artifact_parent(output_path)?;
    if !parent.exists() {
        return Ok(ArtifactSetInspection::incomplete(
            "plan artifact-set directory does not exist",
        ));
    }
    remove_staging_entries(parent, output_path.file_name())?;

    let marker_path = plan_marker_path(output_path);
    if marker_path.exists() {
        return Ok(inspect_plan_set(output_path));
    }
    let pending_marker = plan_pending_marker_path(output_path);
    if !pending_marker.exists() {
        return Ok(inspect_plan_set(output_path));
    }
    if !output_path.exists() {
        fs::remove_file(&pending_marker).with_context(|| {
            format!(
                "failed to discard orphaned plan marker {}",
                pending_marker.display()
            )
        })?;
        sync_directory(parent)?;
        return Ok(ArtifactSetInspection::incomplete(
            "orphaned plan staging state was discarded",
        ));
    }

    let pending = inspect_marker(&pending_marker, parent, ArtifactSetKind::Plan);
    if pending.status != ArtifactSetStatus::Committed {
        return Ok(pending);
    }
    fs::rename(&pending_marker, &marker_path)
        .with_context(|| format!("failed to recover plan marker {}", marker_path.display()))?;
    sync_directory(parent)?;
    Ok(inspect_plan_set(output_path))
}

pub fn recover_scan_staging(runs_directory: &Path) -> Result<u64> {
    if !runs_directory.exists() {
        return Ok(0);
    }
    let mut removed = 0_u64;
    for entry in fs::read_dir(runs_directory).with_context(|| {
        format!(
            "failed to inspect scan artifact-set directory {}",
            runs_directory.display()
        )
    })? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && is_staging_name(&entry.file_name()) {
            fs::remove_dir_all(entry.path()).with_context(|| {
                format!(
                    "failed to remove stale staging set {}",
                    entry.path().display()
                )
            })?;
            removed = removed.saturating_add(1);
        }
    }
    if removed > 0 {
        sync_directory(runs_directory)?;
    }
    Ok(removed)
}

pub fn plan_marker_path(output_path: &Path) -> PathBuf {
    output_path.with_file_name(appended_name(
        output_path
            .file_name()
            .unwrap_or_else(|| OsStr::new("plan")),
        ".artifact-set.json",
        false,
    ))
}

fn plan_pending_marker_path(output_path: &Path) -> PathBuf {
    output_path.with_file_name(appended_name(
        output_path
            .file_name()
            .unwrap_or_else(|| OsStr::new("plan")),
        ".artifact-set.pending.json",
        true,
    ))
}

fn inspect_marker(
    marker_path: &Path,
    member_directory: &Path,
    expected_kind: ArtifactSetKind,
) -> ArtifactSetInspection {
    let bytes = match fs::read(marker_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return ArtifactSetInspection::incomplete(format!(
                "artifact-set marker is missing: {}",
                marker_path.display()
            ));
        }
        Err(error) => {
            return ArtifactSetInspection::incomplete(format!(
                "artifact-set marker could not be read: {error}"
            ));
        }
    };
    let document: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(document) => document,
        Err(error) => {
            return ArtifactSetInspection::incompatible(format!(
                "artifact-set marker is not valid JSON: {error}"
            ));
        }
    };
    if document.get("schema").and_then(serde_json::Value::as_str) != Some(ARTIFACT_SET_SCHEMA) {
        return ArtifactSetInspection::incompatible(
            "artifact-set marker declares an unsupported schema",
        );
    }
    let manifest: ArtifactSetManifest = match serde_json::from_value(document) {
        Ok(manifest) => manifest,
        Err(error) => {
            return ArtifactSetInspection::incompatible(format!(
                "artifact-set marker is structurally incompatible: {error}"
            ));
        }
    };
    if let Err(error) = contracts::validate(Contract::ArtifactSet, &manifest) {
        return ArtifactSetInspection::incompatible(format!(
            "artifact-set marker failed contract validation: {error}"
        ));
    }
    if manifest.set_kind != expected_kind {
        return ArtifactSetInspection::incompatible(
            "artifact-set marker kind does not match its reader",
        );
    }
    let expected_kinds: &[&str] = match expected_kind {
        ArtifactSetKind::Scan => &["effective_policy", "report", "run"],
        ArtifactSetKind::Plan => &["plan"],
    };
    if let Err(error) = require_member_kinds(&manifest.members, expected_kinds) {
        return ArtifactSetInspection::incompatible(error.to_string());
    }
    let member_paths: BTreeSet<NativePath> = manifest
        .members
        .iter()
        .map(|member| member.path.clone())
        .collect();
    if member_paths.len() != manifest.members.len() {
        return ArtifactSetInspection::incompatible(
            "artifact-set marker contains duplicate member paths",
        );
    }
    if let Err(error) = verify_members(member_directory, &manifest.members) {
        return ArtifactSetInspection::incomplete(error.to_string());
    }
    if let Err(error) = verify_document_bindings(member_directory, &manifest) {
        return ArtifactSetInspection::incompatible(error.to_string());
    }
    ArtifactSetInspection::committed(manifest)
}

fn verify_members(directory: &Path, members: &[ArtifactSetMember]) -> Result<()> {
    for member in members {
        let relative_path = member.path.to_path_buf();
        validate_member_name(&relative_path)?;
        let bytes = fs::read(directory.join(&relative_path)).with_context(|| {
            format!(
                "artifact-set member is missing or unreadable: {}",
                member.path.display()
            )
        })?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != member.size_bytes {
            bail!("artifact-set member size does not match marker");
        }
        if digest(&bytes) != member.digest.value {
            bail!("artifact-set member digest does not match marker");
        }
    }
    Ok(())
}

fn verify_document_bindings(directory: &Path, manifest: &ArtifactSetManifest) -> Result<()> {
    for member in &manifest.members {
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join(member.path.to_path_buf()))?)
                .with_context(|| {
                    format!("artifact-set member {} is not valid JSON", member.kind)
                })?;
        let declared_schema = document
            .get(if member.kind == "effective_policy" {
                "schema"
            } else {
                "schema_version"
            })
            .and_then(serde_json::Value::as_str);
        if declared_schema != Some(member.schema.as_str()) {
            bail!("artifact-set member schema does not match its marker");
        }

        match (manifest.set_kind, member.kind.as_str()) {
            (ArtifactSetKind::Scan, "run") => {
                if document
                    .get("artifact_set_id")
                    .and_then(serde_json::Value::as_str)
                    != Some(manifest.set_id.as_str())
                {
                    bail!("run artifact does not bind to its artifact-set marker");
                }
            }
            (ArtifactSetKind::Scan, "report") => {
                if document
                    .pointer("/run/artifact_set_id")
                    .and_then(serde_json::Value::as_str)
                    != Some(manifest.set_id.as_str())
                {
                    bail!("report artifact does not bind to its artifact-set marker");
                }
            }
            (ArtifactSetKind::Plan, "plan") => {
                let source = document
                    .get("source_artifact_set_id")
                    .and_then(serde_json::Value::as_str);
                if source != manifest.source_set_id.as_deref() {
                    bail!("plan source set does not match its artifact-set marker");
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn require_payload_kinds(payloads: &[ArtifactPayload], expected: &[&str]) -> Result<()> {
    let kinds: Vec<&str> = payloads
        .iter()
        .map(|payload| payload.kind.as_str())
        .collect();
    require_kinds(kinds, expected)
}

fn require_member_kinds(members: &[ArtifactSetMember], expected: &[&str]) -> Result<()> {
    let kinds: Vec<&str> = members.iter().map(|member| member.kind.as_str()).collect();
    require_kinds(kinds, expected)
}

fn require_kinds(actual: Vec<&str>, expected: &[&str]) -> Result<()> {
    let actual_count = actual.len();
    let actual: BTreeSet<&str> = actual.into_iter().collect();
    let expected: BTreeSet<&str> = expected.iter().copied().collect();
    if actual_count != actual.len() || actual != expected {
        bail!("artifact set has missing, duplicate, or unexpected member kinds");
    }
    Ok(())
}

fn member_from_payload(payload: &ArtifactPayload) -> Result<ArtifactSetMember> {
    Ok(ArtifactSetMember {
        kind: payload.kind.clone(),
        schema: payload.schema.clone(),
        path: NativePath::from_path(Path::new(&payload.file_name)),
        size_bytes: u64::try_from(payload.bytes.len())
            .context("artifact-set member exceeds the supported size range")?,
        digest: ArtifactDigest {
            algorithm: "blake3-256".to_owned(),
            value: digest(&payload.bytes),
        },
    })
}

fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_new_file(path: &Path, bytes: &[u8], controller: &mut FaultController) -> Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("failed to create staged artifact {}", path.display()))?;
    let mut writer = BufWriter::new(file);

    #[cfg(test)]
    if let CommitFault::DiskFullAfterBytes(limit) = controller.mode {
        let available = limit.saturating_sub(controller.bytes_written);
        if available < bytes.len() {
            writer.write_all(&bytes[..available])?;
            writer.flush()?;
            writer.get_ref().sync_all()?;
            controller.bytes_written = controller.bytes_written.saturating_add(available);
            return Err(io::Error::other("simulated disk full while staging artifact").into());
        }
    }

    writer.write_all(bytes)?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    controller.bytes_written = controller.bytes_written.saturating_add(bytes.len());
    Ok(())
}

fn validate_member_name(path: &Path) -> Result<()> {
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        bail!("artifact-set member path must be one relative file name");
    }
    Ok(())
}

fn artifact_parent(path: &Path) -> Result<&Path> {
    path.parent()
        .map(|parent| {
            if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            }
        })
        .context("artifact path has no parent directory")
}

fn staging_name(file_name: &OsStr, set_id: &str) -> OsString {
    appended_name(file_name, &format!("{STAGING_INFIX}{set_id}"), true)
}

fn appended_name(file_name: &OsStr, suffix: &str, hidden: bool) -> OsString {
    let mut name = OsString::new();
    if hidden {
        name.push(".");
    }
    name.push(file_name);
    name.push(suffix);
    name
}

fn is_staging_name(file_name: &OsStr) -> bool {
    file_name.to_string_lossy().contains(STAGING_INFIX)
}

fn remove_staging_entries(directory: &Path, file_name: Option<&OsStr>) -> Result<()> {
    let Some(file_name) = file_name else {
        return Ok(());
    };
    let prefix = appended_name(file_name, STAGING_INFIX, true);
    let prefix = prefix.to_string_lossy();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry
                .file_name()
                .to_string_lossy()
                .starts_with(prefix.as_ref())
        {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .with_context(|| format!("failed to open directory for sync: {}", path.display()))?
        .sync_all()
        .with_context(|| format!("failed to sync directory: {}", path.display()))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN_ID: &str = "018f47a2-4f17-7b00-8000-000000000000";
    const SOURCE_SET_ID: &str = "018f47a2-4f17-7b00-8000-000000000001";

    fn scan_payloads(set_id: &str) -> Vec<ArtifactPayload> {
        vec![
            ArtifactPayload::json(
                "effective_policy",
                "policy.v1",
                "effective-policy.json",
                &serde_json::json!({ "schema": "policy.v1" }),
            )
            .unwrap(),
            ArtifactPayload::json(
                "run",
                "run.v1",
                "run.json",
                &serde_json::json!({
                    "schema_version": "run.v1",
                    "artifact_set_id": set_id,
                }),
            )
            .unwrap(),
            ArtifactPayload::json(
                "report",
                "report.v1",
                "report.json",
                &serde_json::json!({
                    "schema_version": "report.v1",
                    "run": { "artifact_set_id": set_id },
                }),
            )
            .unwrap(),
        ]
    }

    #[test]
    fn scan_set_is_visible_only_after_complete_publication() {
        let directory = tempfile::tempdir().unwrap();
        let final_directory = directory.path().join(RUN_ID);

        let set_id = Uuid::now_v7().to_string();
        let manifest =
            commit_scan_set(&final_directory, RUN_ID, &set_id, scan_payloads(&set_id)).unwrap();
        let inspected = inspect_scan_set(&final_directory);

        assert_eq!(inspected.status, ArtifactSetStatus::Committed);
        assert_eq!(inspected.manifest.unwrap().set_id, manifest.set_id);
    }

    #[test]
    fn crash_before_directory_rename_leaves_only_recoverable_staging() {
        let directory = tempfile::tempdir().unwrap();
        let final_directory = directory.path().join(RUN_ID);

        assert!(
            commit_scan_set_with_fault(
                &final_directory,
                RUN_ID,
                SOURCE_SET_ID,
                scan_payloads(SOURCE_SET_ID),
                CommitFault::CrashAfterMember(1),
            )
            .is_err()
        );
        assert!(!final_directory.exists());
        assert_eq!(
            inspect_scan_set(&final_directory).status,
            ArtifactSetStatus::Incomplete
        );
        assert_eq!(recover_scan_staging(directory.path()).unwrap(), 1);
    }

    #[test]
    fn disk_full_during_staging_exposes_no_final_set() {
        let directory = tempfile::tempdir().unwrap();
        let final_directory = directory.path().join(RUN_ID);

        assert!(
            commit_scan_set_with_fault(
                &final_directory,
                RUN_ID,
                SOURCE_SET_ID,
                scan_payloads(SOURCE_SET_ID),
                CommitFault::DiskFullAfterBytes(4),
            )
            .is_err()
        );
        assert!(!final_directory.exists());
        assert_eq!(recover_scan_staging(directory.path()).unwrap(), 1);
    }

    #[test]
    fn crash_after_directory_rename_is_a_complete_readable_set() {
        let directory = tempfile::tempdir().unwrap();
        let final_directory = directory.path().join(RUN_ID);

        assert!(
            commit_scan_set_with_fault(
                &final_directory,
                RUN_ID,
                SOURCE_SET_ID,
                scan_payloads(SOURCE_SET_ID),
                CommitFault::CrashAfterPublish,
            )
            .is_err()
        );
        assert_eq!(
            inspect_scan_set(&final_directory).status,
            ArtifactSetStatus::Committed
        );
    }

    #[test]
    fn readers_distinguish_incomplete_and_incompatible_sets() {
        let directory = tempfile::tempdir().unwrap();
        let final_directory = directory.path().join(RUN_ID);
        commit_scan_set(
            &final_directory,
            RUN_ID,
            SOURCE_SET_ID,
            scan_payloads(SOURCE_SET_ID),
        )
        .unwrap();

        fs::remove_file(final_directory.join("report.json")).unwrap();
        assert_eq!(
            inspect_scan_set(&final_directory).status,
            ArtifactSetStatus::Incomplete
        );

        fs::write(
            final_directory.join(SCAN_MARKER_FILE_NAME),
            br#"{"schema":"optiflow.artifact-set.v99"}"#,
        )
        .unwrap();
        assert_eq!(
            inspect_scan_set(&final_directory).status,
            ArtifactSetStatus::Incompatible
        );
    }

    #[test]
    fn pending_plan_marker_is_completed_by_recovery() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("plan.json");
        let payload = ArtifactPayload::json(
            "plan",
            "plan.v1",
            "plan.json",
            &serde_json::json!({
                "schema_version": "plan.v1",
                "source_artifact_set_id": SOURCE_SET_ID,
            }),
        )
        .unwrap();

        assert!(
            commit_plan_set_with_fault(
                &output,
                RUN_ID,
                Some(SOURCE_SET_ID),
                payload,
                CommitFault::CrashAfterPublish,
            )
            .is_err()
        );
        assert_eq!(
            inspect_plan_set(&output).status,
            ArtifactSetStatus::Incomplete
        );
        assert_eq!(
            recover_plan_set(&output).unwrap().status,
            ArtifactSetStatus::Committed
        );
    }
}
