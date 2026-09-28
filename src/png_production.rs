//! Source-preserving, bounded PNG candidate production.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::adapters::oxipng::OxipngAdapter;
use crate::candidate_artifact::{self, PublishLimits};
use crate::domain::{EvidenceDigest, MediaProviderEvidence, NativePath};
use crate::png_candidate::{ContentIdentity, PROFILE, PngFacts};
use crate::png_validation::{
    ByteRefusal, ByteValidationLimits, validate_png_pair, validate_png_source,
};
use crate::subprocess::{SubprocessError, SubprocessLimits, SubprocessRunner};

const SOURCE_MAX: usize = 16 * 1024 * 1024;
const OUTPUT_MAX: usize = 16 * 1024 * 1024;
const DECODED_MAX: usize = 64 * 1024 * 1024;
const BINARY_MAX: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionLimits {
    pub source_bytes: usize,
    pub candidate_bytes: usize,
    pub decoded_bytes_per_image: usize,
    pub decoder_allocation_bytes: usize,
    pub chunks_per_image: usize,
    pub provider_binary_bytes: u64,
    pub stderr_bytes: usize,
    pub evidence_bytes: usize,
    pub elapsed_ms: u64,
}

impl Default for ProductionLimits {
    fn default() -> Self {
        Self {
            source_bytes: SOURCE_MAX,
            candidate_bytes: OUTPUT_MAX,
            decoded_bytes_per_image: DECODED_MAX,
            decoder_allocation_bytes: DECODED_MAX,
            chunks_per_image: 256,
            provider_binary_bytes: BINARY_MAX,
            stderr_bytes: 64 * 1024,
            evidence_bytes: 1024 * 1024,
            elapsed_ms: 30_000,
        }
    }
}

impl ProductionLimits {
    fn check(self) -> Result<()> {
        ensure!(
            self.source_bytes > 0 && self.source_bytes <= SOURCE_MAX,
            "invalid source limit"
        );
        ensure!(
            self.candidate_bytes > 0 && self.candidate_bytes <= OUTPUT_MAX,
            "invalid candidate limit"
        );
        ensure!(
            self.decoded_bytes_per_image > 0 && self.decoded_bytes_per_image <= DECODED_MAX,
            "invalid decoded limit"
        );
        ensure!(
            self.decoder_allocation_bytes > 0 && self.decoder_allocation_bytes <= DECODED_MAX,
            "invalid decoder allocation limit"
        );
        ensure!(
            self.chunks_per_image > 0 && self.chunks_per_image <= 256,
            "invalid chunk limit"
        );
        ensure!(
            self.provider_binary_bytes > 0 && self.provider_binary_bytes <= BINARY_MAX,
            "invalid binary limit"
        );
        ensure!(
            self.stderr_bytes > 0 && self.stderr_bytes <= 64 * 1024,
            "invalid stderr limit"
        );
        ensure!(
            self.evidence_bytes > 0 && self.evidence_bytes <= 1024 * 1024,
            "invalid evidence limit"
        );
        ensure!(
            self.elapsed_ms > 0 && self.elapsed_ms <= 30_000,
            "invalid time limit"
        );
        Ok(())
    }
    fn bytes(self) -> ByteValidationLimits {
        ByteValidationLimits {
            source_bytes: self.source_bytes,
            candidate_bytes: self.candidate_bytes,
            decoded_bytes_per_image: self.decoded_bytes_per_image,
            chunks_per_image: self.chunks_per_image,
            decoder_allocation_bytes: self.decoder_allocation_bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionRefusal {
    InvalidInput,
    UnsupportedPng,
    InvalidPng,
    ChangedSource,
    ProviderUnavailable,
    ProviderChanged,
    ProviderFailed,
    ProviderTimedOut,
    Cancelled,
    OutputBoundExceeded,
    CandidateChanged,
    CandidateNotSmaller,
    ArtifactUncommitted,
}

#[derive(Debug)]
pub struct ProductionFailure {
    pub reason: ProductionRefusal,
    pub detail: String,
    pub set_id: Option<Uuid>,
}

impl std::fmt::Display for ProductionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.reason, self.detail)
    }
}
impl std::error::Error for ProductionFailure {}
fn fail(reason: ProductionRefusal, detail: impl std::fmt::Display) -> ProductionFailure {
    ProductionFailure {
        reason,
        detail: detail.to_string(),
        set_id: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshot {
    pub device_id: u64,
    pub inode: u64,
    pub mode: u32,
    pub owner_uid: u32,
    pub group_gid: u32,
    pub link_count: u64,
    pub size_bytes: u64,
    pub modified_unix_ns: i128,
    pub changed_unix_ns: i128,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEvidence {
    pub path: NativePath,
    pub before: SourceSnapshot,
    pub after: SourceSnapshot,
    pub content: ContentIdentity,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateObservation {
    pub content: ContentIdentity,
    pub file_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionMeasurements {
    pub elapsed_ms: u64,
    pub captured_stdout_bytes: u64,
    pub captured_stderr_bytes: u64,
    pub decoded_source_bytes: u64,
    pub decoded_candidate_bytes: u64,
    pub peak_provider_memory_bytes: Option<u64>,
    pub peak_provider_temporary_bytes: Option<u64>,
    pub provider_resource_enforcement: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationEvidence {
    pub source_png: PngFacts,
    pub candidate_png: PngFacts,
    pub independent_byte_validation: bool,
    pub exact_decoded_samples: bool,
    pub ordered_non_idat_chunks_preserved: bool,
    pub strict_encoded_reduction: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanupEvidence {
    pub provider_work_directory_removed: bool,
    pub source_mutation: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateEvidence {
    pub schema: String,
    pub artifact_set_id: String,
    pub profile: String,
    pub source_observation_id: String,
    pub source: SourceEvidence,
    pub effective_policy_fingerprint: EvidenceDigest,
    pub producer: MediaProviderEvidence,
    pub argv: Vec<String>,
    pub input_binding: String,
    pub output_binding: String,
    pub provider_configuration: String,
    pub candidate: CandidateObservation,
    pub limits: ProductionLimits,
    pub measurements: ProductionMeasurements,
    pub validation: ValidationEvidence,
    pub encoded_logical_reduction_bytes: u64,
    pub physical_savings_bytes: Option<u64>,
    pub cleanup: CleanupEvidence,
}

#[derive(Debug)]
pub struct ProductionReceipt {
    pub set_id: Uuid,
    pub directory: PathBuf,
    pub evidence: CandidateEvidence,
}

/// The root is preexisting private OptiFlow-owned storage. Source bytes are
/// opened through no-follow handles and checked again before publication.
pub fn produce_png(
    source: &Path,
    executable: &Path,
    root: &Path,
    policy_digest: &EvidenceDigest,
    limits: ProductionLimits,
    is_cancelled: impl Fn() -> bool,
) -> std::result::Result<ProductionReceipt, ProductionFailure> {
    limits
        .check()
        .map_err(|e| fail(ProductionRefusal::InvalidInput, e))?;
    if policy_digest.algorithm != "blake3-256"
        || policy_digest.value.len() != 64
        || !policy_digest.value.bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err(fail(
            ProductionRefusal::InvalidInput,
            "invalid policy fingerprint",
        ));
    }
    candidate_artifact::validate_private_root(root)
        .map_err(|e| fail(ProductionRefusal::InvalidInput, e))?;
    let (source_bytes, before) = read_source(source, limits.source_bytes)
        .map_err(|e| fail(ProductionRefusal::InvalidInput, e))?;
    let source_facts =
        validate_png_source(&source_bytes, limits.bytes()).map_err(|e| byte_failure(e, true))?;
    let raw_bytes = source_facts
        .decoded_bytes
        .checked_add(u64::from(source_facts.height))
        .ok_or_else(|| fail(ProductionRefusal::InvalidInput, "raw size overflow"))?
        .max(source_bytes.len() as u64);
    let runner = SubprocessRunner::new(SubprocessLimits {
        timeout: Duration::from_millis(limits.elapsed_ms),
        poll_interval: Duration::from_millis(10),
        max_stdout_bytes: limits.candidate_bytes,
        max_stderr_bytes: limits.stderr_bytes,
        max_concurrent_children: 1,
    })
    .map_err(|e| fail(ProductionRefusal::InvalidInput, e))?;
    let adapter = OxipngAdapter::lock(executable, runner, limits.provider_binary_bytes, raw_bytes)
        .map_err(|e| provider_failure(e, ProductionRefusal::ProviderUnavailable))?;
    if is_cancelled() {
        return Err(fail(ProductionRefusal::Cancelled, "cancelled"));
    }
    let set_id = Uuid::now_v7();
    let work = ProviderWork::create(root, set_id)
        .map_err(|e| fail(ProductionRefusal::ArtifactUncommitted, e))?;
    let output = adapter
        .optimize(&source_bytes, &work.path, &is_cancelled)
        .map_err(|e| provider_failure(e, ProductionRefusal::ProviderFailed))?;
    let candidate_bytes = output.stdout;
    let stderr_len = output.stderr.len() as u64;
    work.cleanup()
        .map_err(|e| fail(ProductionRefusal::ArtifactUncommitted, e))?;
    if is_cancelled() {
        return Err(fail(ProductionRefusal::Cancelled, "cancelled"));
    }
    recheck_source(source, limits.source_bytes, &before, &source_bytes)
        .map_err(|e| fail(ProductionRefusal::ChangedSource, e))?;
    adapter
        .verify_identity()
        .map_err(|e| fail(ProductionRefusal::ProviderChanged, e))?;
    let validated = validate_png_pair(&source_bytes, &candidate_bytes, limits.bytes())
        .map_err(|e| byte_failure(e, false))?;
    let after = recheck_source(source, limits.source_bytes, &before, &source_bytes)
        .map_err(|e| fail(ProductionRefusal::ChangedSource, e))?;
    let evidence = CandidateEvidence {
        schema: candidate_artifact::EVIDENCE_SCHEMA.to_owned(), artifact_set_id: set_id.to_string(),
        profile: PROFILE.to_owned(), source_observation_id: Uuid::now_v7().to_string(),
        source: SourceEvidence {
            path: NativePath::from_path(source), before, after,
            content: validated.source().clone(),
        },
        effective_policy_fingerprint: policy_digest.clone(), producer: adapter.evidence().clone(),
        argv: adapter.invocation_arguments().iter().map(|a| a.to_string_lossy().into_owned()).collect(),
        input_binding: "complete_bounded_source_bytes_on_stdin".to_owned(),
        output_binding: "bounded_stdout_then_independent_validation".to_owned(),
        provider_configuration: "oxipng_v10.2.1_opt2_nx_keep_interlace_one_thread_no_strip".to_owned(),
        candidate: CandidateObservation {
            content: validated.candidate().clone(), file_name: candidate_artifact::CANDIDATE_NAME.to_owned(),
        }, limits,
        measurements: ProductionMeasurements {
            elapsed_ms: output.elapsed.as_millis().try_into().unwrap_or(u64::MAX),
            captured_stdout_bytes: candidate_bytes.len() as u64, captured_stderr_bytes: stderr_len,
            decoded_source_bytes: validated.source_png().decoded_bytes,
            decoded_candidate_bytes: validated.candidate_png().decoded_bytes,
            peak_provider_memory_bytes: None, peak_provider_temporary_bytes: None,
            provider_resource_enforcement: "host_bounds_stdin_stdout_stderr_elapsed_and_validator_buffers;provider_rss_and_private_workdir_usage_unmeasured".to_owned(),
        },
        validation: ValidationEvidence {
            source_png: validated.source_png().clone(), candidate_png: validated.candidate_png().clone(),
            independent_byte_validation: true, exact_decoded_samples: true,
            ordered_non_idat_chunks_preserved: true, strict_encoded_reduction: true,
        },
        encoded_logical_reduction_bytes: validated.encoded_byte_reduction(), physical_savings_bytes: None,
        cleanup: CleanupEvidence { provider_work_directory_removed: true, source_mutation: false },
    };
    // WIP #94: the full evidence schema and contract registration are still pending.
    let value = serde_json::to_value(&evidence)
        .map_err(|e| fail(ProductionRefusal::ArtifactUncommitted, e))?;
    let directory = candidate_artifact::publish(
        root,
        set_id,
        &candidate_bytes,
        &value,
        PublishLimits {
            candidate_bytes: limits.candidate_bytes,
            evidence_bytes: limits.evidence_bytes,
        },
        || {
            ensure!(!is_cancelled(), "cancelled before publication");
            recheck_source(
                source,
                limits.source_bytes,
                &evidence.source.before,
                &source_bytes,
            )?;
            adapter.verify_identity()?;
            Ok(())
        },
    )
    .map_err(|e| ProductionFailure {
        reason: ProductionRefusal::ArtifactUncommitted,
        detail: e.to_string(),
        set_id: Some(set_id),
    })?;
    if candidate_artifact::inspect(&directory).status != candidate_artifact::Status::Committed {
        return Err(ProductionFailure {
            reason: ProductionRefusal::ArtifactUncommitted,
            detail: "published candidate failed read-back inspection".to_owned(),
            set_id: Some(set_id),
        });
    }
    Ok(ProductionReceipt {
        set_id,
        directory,
        evidence,
    })
}

fn byte_failure(reason: ByteRefusal, source: bool) -> ProductionFailure {
    let kind = match reason {
        ByteRefusal::UnsupportedPng => ProductionRefusal::UnsupportedPng,
        ByteRefusal::NotSmaller => ProductionRefusal::CandidateNotSmaller,
        ByteRefusal::PreservationMismatch => ProductionRefusal::CandidateChanged,
        ByteRefusal::LimitExceeded => ProductionRefusal::OutputBoundExceeded,
        _ if source => ProductionRefusal::InvalidPng,
        _ => ProductionRefusal::CandidateChanged,
    };
    fail(
        kind,
        format!("independent PNG byte check refused: {reason:?}"),
    )
}

fn provider_failure(error: anyhow::Error, fallback: ProductionRefusal) -> ProductionFailure {
    let reason = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<SubprocessError>())
        .map_or(fallback, |cause| match cause {
            SubprocessError::Timeout { .. } => ProductionRefusal::ProviderTimedOut,
            SubprocessError::Cancelled { .. } => ProductionRefusal::Cancelled,
            SubprocessError::Truncated { .. } => ProductionRefusal::OutputBoundExceeded,
            _ => fallback,
        });
    fail(reason, error)
}

struct ProviderWork {
    path: PathBuf,
}
impl ProviderWork {
    fn create(root: &Path, id: Uuid) -> Result<Self> {
        let path = root.join(format!(".provider-work-{id}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .context("failed to create private provider work directory")?;
            Ok(Self { path })
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            anyhow::bail!("Unix private workdir required")
        }
    }
    fn cleanup(self) -> Result<()> {
        fs::remove_dir_all(&self.path).context("failed to clean provider work directory")?;
        Ok(())
    }
}
impl Drop for ProviderWork {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn read_source(path: &Path, maximum: usize) -> Result<(Vec<u8>, SourceSnapshot)> {
    #[cfg(unix)]
    {
        use rustix::fs::{Mode, OFlags, open, openat};
        normalized_absolute(path)?;
        let parent = path.parent().context("source has no parent")?;
        let name = path.file_name().context("source has no name")?;
        let dir_flags = OFlags::RDONLY
            | OFlags::DIRECTORY
            | OFlags::CLOEXEC
            | OFlags::NOFOLLOW
            | OFlags::NONBLOCK;
        let mut dir = open("/", dir_flags, Mode::empty())?;
        for part in parent.components() {
            if let Component::Normal(name) = part {
                dir =
                    openat(&dir, name, dir_flags, Mode::empty()).context("unsafe source parent")?;
            }
        }
        let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
        let mut file = File::from(openat(&dir, name, flags, Mode::empty())?);
        let before = snapshot(&file)?;
        ensure!(
            before.size_bytes > 0 && before.size_bytes <= maximum as u64,
            "source exceeds encoded bound"
        );
        let mut bytes = Vec::new();
        (&mut file)
            .take(maximum as u64 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= maximum
                && before == snapshot(&file)?
                && before.size_bytes == bytes.len() as u64,
            "source changed during read"
        );
        let reopened = File::from(openat(&dir, name, flags, Mode::empty())?);
        ensure!(
            snapshot(&reopened)? == before,
            "source path changed during read"
        );
        Ok((bytes, before))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, maximum);
        anyhow::bail!("Unix no-follow handles required")
    }
}

fn recheck_source(
    path: &Path,
    maximum: usize,
    before: &SourceSnapshot,
    bytes: &[u8],
) -> Result<SourceSnapshot> {
    let (current_bytes, current) = read_source(path, maximum)?;
    ensure!(
        &current == before && current_bytes == bytes,
        "source identity, metadata, or complete bytes changed"
    );
    Ok(current)
}

fn normalized_absolute(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute()
            && path
                .components()
                .all(|part| !matches!(part, Component::CurDir | Component::ParentDir)),
        "source path must be absolute and normalized"
    );
    let rebuilt: PathBuf = path.components().collect();
    ensure!(rebuilt == path, "source path must be normalized");
    Ok(())
}

#[cfg(unix)]
fn snapshot(file: &File) -> Result<SourceSnapshot> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    ensure!(metadata.is_file(), "source is not a regular file");
    Ok(SourceSnapshot {
        device_id: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        owner_uid: metadata.uid(),
        group_gid: metadata.gid(),
        link_count: metadata.nlink(),
        size_bytes: metadata.len(),
        modified_unix_ns: i128::from(metadata.mtime()) * 1_000_000_000
            + i128::from(metadata.mtime_nsec()),
        changed_unix_ns: i128::from(metadata.ctime()) * 1_000_000_000
            + i128::from(metadata.ctime_nsec()),
    })
}
