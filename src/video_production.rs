//! Source-preserving stream-copy candidate coordinator for the closed subset.
//! All native calls require the selected Linux guard. No replacement/disposal.
use crate::adapters::video::{CaptureLimits, VideoProviders};
use crate::domain::{EvidenceDigest, NativePath};
use crate::video_budget::OperationBudget;
use crate::video_candidate::{
    self, CandidatePolicy, RequestedLimits, SourceDeclaration, VideoCandidateRequest,
    VideoOperation,
};
use crate::video_io::BoundedFileObservation;
use crate::video_profile::Profile;
use crate::video_sandbox::LinuxVideoSandbox;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
pub const REQUEST_SCHEMA_V2: &str = "optiflow.video-production-request.v2";
pub const EVIDENCE_SCHEMA_V2: &str = "optiflow.video-candidate-evidence.v2";

pub const EVIDENCE_SCHEMA: &str = "optiflow.video-candidate-evidence.v1";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoProductionRequest {
    pub schema: String,
    pub source: PathBuf,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub private_workdir: PathBuf,
    pub artifact_root: PathBuf,
    pub operation: VideoOperation,
    pub limits: RequestedLimits,
    pub sandbox: LinuxVideoSandbox,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum VideoProductionOutcome {
    Accepted {
        artifact_set_id: String,
        artifact_directory: PathBuf,
        source_bytes: u64,
        candidate_bytes: u64,
        encoded_bytes_saved: u64,
    },
    Unchanged {
        reason: String,
        source_bytes: u64,
        candidate_bytes: u64,
    },
}
fn digest(bytes: &[u8]) -> EvidenceDigest {
    EvidenceDigest {
        algorithm: "blake3-256".to_owned(),
        value: blake3::hash(bytes).to_hex().to_string(),
    }
}

fn check_free(root: &Path, required: u64) -> Result<()> {
    #[cfg(unix)]
    {
        let directory = crate::candidate_artifact::open_directory(root)?;
        let volume = rustix::fs::fstatvfs(&directory)?;
        ensure!(
            volume.f_frsize > 0
                && volume
                    .f_bavail
                    .checked_mul(volume.f_frsize)
                    .is_some_and(|v| v >= required),
            "video candidate free-space reserve unavailable"
        );
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (root, required);
        anyhow::bail!("video candidate capacity is unsupported")
    }
}

pub(crate) fn payload_admission(
    profile: Profile,
    limits: RequestedLimits,
    sandbox: &LinuxVideoSandbox,
) -> Result<u64> {
    sandbox.validate()?;
    ensure!(
        (1..=profile.input_bytes()).contains(&limits.input_bytes)
            && (1..=profile.candidate_bytes()).contains(&limits.candidate_bytes)
            && (1..=profile.decoded_bytes()).contains(&limits.decoded_bytes)
            && (1..=profile.temporary_bytes()).contains(&limits.temporary_bytes)
            && (1..=profile.memory_bytes()).contains(&limits.memory_bytes)
            && (1..=30_000).contains(&limits.elapsed_ms),
        "video production bounds exceed the explicitly selected profile"
    );
    // Payload-byte admission, NOT a claim to cap the Rust host's total RSS.
    let copies = if profile == Profile::TinyV1 { 1 } else { 4 };
    let metadata = if profile == Profile::TinyV1 {
        16_777_216
    } else {
        33_554_432
    };
    let host_buffers = limits
        .input_bytes
        .checked_add(
            limits
                .candidate_bytes
                .checked_mul(copies)
                .context("candidate buffer count overflow")?,
        )
        .and_then(|v| v.checked_add(limits.decoded_bytes.checked_mul(2)?))
        .and_then(|v| v.checked_add(metadata))
        .context("host payload-buffer admission overflow")?;
    ensure!(
        host_buffers
            .checked_add(sandbox.provider_subtree_memory_bytes)
            .is_some_and(|v| v <= limits.memory_bytes),
        "host payload buffers plus kernel provider quota exceed requested memory admission"
    );
    let publication = limits
        .candidate_bytes
        .checked_add(profile.publication_overhead())
        .context("video publication reservation overflow")?;
    ensure!(
        publication
            .checked_add(sandbox.provider_scratch_bytes)
            .is_some_and(|v| v <= limits.temporary_bytes),
        "aggregate provider scratch plus publication bytes exceed temporary admission"
    );
    Ok(host_buffers)
}

pub(crate) fn admission(request: &VideoProductionRequest) -> Result<u64> {
    let profile = Profile::production(&request.schema)?;
    ensure!(
        matches!(request.operation, VideoOperation::StreamCopy),
        "lossy candidate execution/quality acceptance is not available in this checkpoint"
    );
    crate::contracts::validate(profile.production_contract(), request)?;
    let host_buffers = payload_admission(profile, request.limits, &request.sandbox)?;
    for path in [
        &request.source,
        &request.ffmpeg,
        &request.ffprobe,
        &request.private_workdir,
        &request.artifact_root,
    ] {
        ensure!(
            path.is_absolute(),
            "video production requires absolute explicit locators"
        );
    }
    ensure!(
        !request.source.starts_with(&request.artifact_root)
            && !request.source.starts_with(&request.private_workdir)
            && !request.private_workdir.starts_with(&request.artifact_root)
            && !request.artifact_root.starts_with(&request.private_workdir),
        "video source and private work/publication namespaces must be disjoint"
    );
    Ok(host_buffers)
}

/// Native execution is opt-in through this request. No PATH lookup, installation,
/// model, network, collection replacement or disposal is performed.
pub fn produce(
    request: &VideoProductionRequest,
    is_cancelled: impl Fn() -> bool,
) -> Result<VideoProductionOutcome> {
    ensure!(
        cfg!(target_os = "linux"),
        "video candidate production currently requires the supported Linux guard profile"
    );
    let host_buffer_ceiling = admission(request)?;
    let profile = Profile::production(&request.schema)?;
    crate::candidate_artifact::validate_private_root(&request.artifact_root)?;
    let budget = OperationBudget::new(request.limits.elapsed_ms)?;
    let publication_reserve = request.limits.candidate_bytes + profile.publication_overhead();
    let free_reserve = request
        .sandbox
        .minimum_free_bytes
        .checked_add(publication_reserve)
        .context("free-space reservation overflow")?;
    let guard = || -> Result<()> {
        budget.check(&is_cancelled)?;
        check_free(&request.artifact_root, free_reserve)?;
        budget.check(&is_cancelled)
    };
    guard()?;
    let source =
        BoundedFileObservation::open_guarded(&request.source, request.limits.input_bytes, &guard)?;
    ensure!(
        source.snapshot().link_count == 1,
        "video candidate source must be singly linked"
    );
    let bytes = source.read_bytes_profile(profile, &guard)?;
    let source_container = crate::video_container::inspect_profile(&bytes, profile)?;
    guard()?;
    let limits = CaptureLimits {
        candidate_bytes: request.limits.candidate_bytes as usize,
        probe_bytes: 4_194_304,
        stderr_bytes: 4096,
        elapsed_ms: request.limits.elapsed_ms,
    };
    let providers = if profile == Profile::TinyV1 {
        VideoProviders::lock_enforced(
            &request.ffmpeg,
            &request.ffprobe,
            &request.private_workdir,
            limits,
            budget,
            request.sandbox.clone(),
            &is_cancelled,
        )?
    } else {
        VideoProviders::lock_enforced_profile(
            &request.ffmpeg,
            &request.ffprobe,
            &request.private_workdir,
            limits,
            budget,
            request.sandbox.clone(),
            profile,
            &is_cancelled,
        )?
    };
    let probe_capture = providers.probe_untrusted(&bytes, &is_cancelled)?;
    let mut source_probe = crate::video_probe::parse_profile(&probe_capture.bytes, profile)?;
    guard()?;
    ensure!(
        source_container
            .track_contract
            .get("default")
            .map(String::as_str)
            == Some(if source_probe.media.disposition == "default" {
                "1"
            } else {
                "0"
            }),
        "source container/native disposition differs"
    );
    ensure!(
        source_probe.media.width == source_container.width
            && source_probe.media.height == source_container.height
            && source_probe.frames.len() == source_container.packets.len(),
        "independent source structure differs from complete provider observation"
    );
    ensure!(
        source_probe.decoded_sample_bytes <= request.limits.decoded_bytes,
        "source decoded bytes exceed selected limit"
    );
    // Completeness is only for this closed EBML element allowlist, never all Matroska.
    source_probe.media.feature_observation_complete = true;
    let policy_fingerprint = if profile == Profile::TinyV1 {
        digest(&serde_json::to_vec(&(
            &request.sandbox,
            request.limits,
            "closed_ebml_stream_copy_only",
        ))?)
    } else {
        digest(&serde_json::to_vec(&(
            &request.sandbox,
            request.limits,
            profile.candidate_profile(),
            "closed_ebml_stream_copy_only",
        ))?)
    };
    let observation_fingerprint = digest(&serde_json::to_vec(&(
        source.snapshot(),
        source.digest(),
        &source_container,
    ))?);
    let declared = VideoCandidateRequest {
        schema: profile.request_schema().to_owned(),
        profile: profile.candidate_profile().to_owned(),
        source: SourceDeclaration {
            observation_id: format!("video-{}", observation_fingerprint.value),
            observation_fingerprint: observation_fingerprint.clone(),
            content_digest: source.digest().clone(),
            logical_bytes: bytes.len() as u64,
            probe_observation_fingerprint: source_probe.raw_report_digest.clone(),
            media: source_probe.media.clone(),
        },
        operation: request.operation,
        policy: CandidatePolicy {
            effective_policy_fingerprint: policy_fingerprint,
            source_preserving: true,
            metadata: "regenerate_encoder_duration_only".to_owned(),
            require_strictly_smaller: true,
        },
        producer: providers.guarded_copy_evidence(&source_probe.media.disposition)?,
        producer_build_fingerprint: providers.producer_build_fingerprint().clone(),
        probe: providers.probe_evidence()?,
        probe_build_fingerprint: providers.probe_build_fingerprint().clone(),
        limits: request.limits,
    };
    // Reuse typed source/policy/provider/limit admission; this historical pure
    // planner's review argv is not adopted as an execution receipt.
    video_candidate::prepare_candidate(&declared)?;
    let source_decode = providers.decode_untrusted(
        &bytes,
        source_probe.decoded_sample_bytes as usize,
        &is_cancelled,
    )?;
    guard()?;
    ensure!(
        source_decode.bytes.len() as u64 == source_probe.decoded_sample_bytes,
        "source decode did not produce the complete expected sample set"
    );
    let candidate =
        providers.capture_guarded_copy(&bytes, &source_probe.media.disposition, &is_cancelled)?;
    guard()?;
    let candidate_container = crate::video_container::inspect_profile(&candidate.bytes, profile)?;
    crate::video_container::compare_stream_copy(&source_container, &candidate_container)?;
    guard()?;
    let candidate_probe_capture = providers.probe_untrusted(&candidate.bytes, &is_cancelled)?;
    let candidate_probe =
        crate::video_probe::parse_profile(&candidate_probe_capture.bytes, profile)?;
    guard()?;
    ensure!(
        source_probe.frames.len() == candidate_probe.frames.len()
            && source_probe.chroma_location == candidate_probe.chroma_location
            && source_probe.decoded_sample_bytes == candidate_probe.decoded_sample_bytes
            && source_probe.media.disposition == candidate_probe.media.disposition
            && source_probe.media.width == candidate_probe.media.width
            && source_probe.media.height == candidate_probe.media.height,
        "candidate changed supported complete stream/decode properties"
    );
    let candidate_decode = providers.decode_untrusted(
        &candidate.bytes,
        candidate_probe.decoded_sample_bytes as usize,
        &is_cancelled,
    )?;
    guard()?;
    ensure!(
        source_decode.bytes == candidate_decode.bytes,
        "stream-copy candidate changed decoded YUV420 samples"
    );
    source.verify_current_guarded(&guard)?;
    providers.verify_current_with_cancel(&is_cancelled)?;
    guard()?;
    if candidate.bytes.len() >= bytes.len() {
        return Ok(VideoProductionOutcome::Unchanged {
            reason: "validated candidate is not strictly smaller; original unchanged".to_owned(),
            source_bytes: bytes.len() as u64,
            candidate_bytes: candidate.bytes.len() as u64,
        });
    }
    let id = uuid::Uuid::now_v7();
    let evidence = json!({"schema":profile.evidence_schema(),"artifact_set_id":id.to_string(),"profile":profile.evidence_profile(),
        "request":declared,"request_fingerprint":video_candidate::request_fingerprint(&declared),
        "source":{"path":NativePath::from_path(&request.source),"snapshot":source.snapshot(),"content":{"digest":source.digest(),"logical_bytes":bytes.len()},"unchanged_at_precommit":true},
        "candidate":{"content":{"digest":digest(&candidate.bytes),"logical_bytes":candidate.bytes.len()}},
        "validation":{"source_container":source_container,"candidate_container":candidate_container,
            "source_decoded_digest":digest(&source_decode.bytes),"candidate_decoded_digest":digest(&candidate_decode.bytes),"decoded_bytes":source_decode.bytes.len(),
            "elementary_packets_and_exact_timing_preserved":true,"complete_decoded_yuv420_samples_equal":true,"strictly_smaller":true},
        "resources":{"selected_policy":providers.resource_policy()?,"kernel_observations":providers.resource_receipts()?,"host_media_buffer_ceiling_bytes":host_buffer_ceiling,
            "memory_scope":"kernel provider subtree quota plus host payload-buffer admission; total host RSS not kernel limited",
            "elapsed_scope":"shared cooperative deadline plus separate at-most-2s cleanup per invocation; host syscalls/parsing not preemptible",
            "free_space_scope":"observed before and at publication; not a filesystem reservation"},
        "invocations":[probe_capture.evidence,source_decode.evidence,candidate.evidence,candidate_probe_capture.evidence,candidate_decode.evidence],
        "encoded_bytes_saved":bytes.len()-candidate.bytes.len(),"physical_space_reclaimed":false,"replacement_authority":false,"qualification":"source_only_native_host_unqualified"});
    guard()?;
    let directory = crate::video_artifact::publish(
        &request.artifact_root,
        id,
        &candidate.bytes,
        &evidence,
        || {
            budget.check(&is_cancelled)?;
            source.verify_current_guarded(&|| budget.check(&is_cancelled))?;
            providers.verify_current_with_cancel(&is_cancelled)?;
            check_free(&request.artifact_root, request.sandbox.minimum_free_bytes)?;
            budget.check(&is_cancelled)
        },
    )?;
    Ok(VideoProductionOutcome::Accepted {
        artifact_set_id: id.to_string(),
        artifact_directory: directory,
        source_bytes: bytes.len() as u64,
        candidate_bytes: candidate.bytes.len() as u64,
        encoded_bytes_saved: (bytes.len() - candidate.bytes.len()) as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lossy_execution_is_refused_before_source_or_provider_access() {
        let value:VideoProductionRequest=serde_json::from_value(serde_json::json!({
            "schema":"optiflow.video-production-request.v1","source":"/missing/source","ffmpeg":"/missing/ffmpeg","ffprobe":"/missing/ffprobe",
            "private_workdir":"/missing/work","artifact_root":"/missing/store","operation":{"kind":"h264_lossy","crf":20,"preset":"slow"},
            "limits":{"input_bytes":1024,"candidate_bytes":1024,"decoded_bytes":1024,"temporary_bytes":33554432,"memory_bytes":268435456,"elapsed_ms":30000},
            "sandbox":{"schema":"optiflow.linux-video-sandbox.v1","systemd_run":{"path":"/missing/systemd-run","digest":{"algorithm":"blake3-256","value":"a".repeat(64)}},
                "systemctl":{"path":"/missing/systemctl","digest":{"algorithm":"blake3-256","value":"a".repeat(64)}},"helper":{"path":"/missing/optiflow","digest":{"algorithm":"blake3-256","value":"a".repeat(64)}},
                "user_bus":"/run/user/1000/bus","provider_subtree_memory_bytes":67108864,"provider_scratch_bytes":1048576,"minimum_free_bytes":67108864}})).unwrap();
        assert!(admission(&value).unwrap_err().to_string().contains("lossy"));
    }
}
