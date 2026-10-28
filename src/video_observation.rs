//! Source-preserving acquisition of provider-reported video observations.
//!
//! This does not turn an ffprobe report into complete preservation evidence or
//! execute the candidate planner's requested memory/temporary-space policy.

use std::path::Path;

use anyhow::{Result, ensure};
use serde::Serialize;

use crate::adapters::video::{CaptureLimits, InvocationEvidence, VideoProviders};
use crate::domain::{EvidenceDigest, MediaProviderEvidence, NativePath};
use crate::video_candidate::VideoOperation;
use crate::video_budget::OperationBudget;
use crate::video_io::BoundedFileObservation;
use crate::video_probe::{VideoProbeObservation, parse_ffv1_observation};

pub use crate::video_io::FileSnapshot;

const SOURCE_MAX: u64 = 16 * 1024 * 1024;

/// Development review output, not a published candidate or stable Flow artifact.
#[derive(Debug, Serialize)]
pub struct VideoSourceObservation {
    pub status: &'static str,
    pub source_path: NativePath,
    pub source_snapshot: FileSnapshot,
    pub source_digest: EvidenceDigest,
    pub source_reobserved: bool,
    pub probe: VideoProbeObservation,
    pub probe_invocation: InvocationEvidence,
    pub producer_for_stream_copy_recipe: MediaProviderEvidence,
    pub producer_build_fingerprint: EvidenceDigest,
    pub candidate_execution_available: bool,
    pub operation_elapsed_ms: u64,
    pub operation_elapsed_limit_ms: u64,
    pub limitations: Vec<&'static str>,
}

/// Observe a small source through an explicitly selected provider pair.
///
/// The working directory must already exist, be empty, have mode 0700, and
/// have no symlink components. The caller owns it; this function does not
/// remove unexpected files. A shared deadline is checked between host IO chunks
/// and parser calls; each child gets only the remaining time. This does not
/// preempt filesystem syscalls or enforce RSS, scratch space or escaped processes.
/// The encoded source is retained in bounded memory for a stable stdin copy.
pub fn observe_video_source(
    source_path: &Path,
    ffmpeg: &Path,
    ffprobe: &Path,
    workdir: &Path,
    limits: CaptureLimits,
    is_cancelled: impl Fn() -> bool,
) -> Result<VideoSourceObservation> {
    limits.check()?;
    ensure!(crate::subprocess::bounded_pipe_capture_supported(),
        "bounded video observation requires Linux or macOS");
    let budget = OperationBudget::new(limits.elapsed_ms)?;
    let guard = || budget.check(&is_cancelled);
    guard()?;
    let source = BoundedFileObservation::open_guarded(source_path, SOURCE_MAX, &guard)?;
    let bytes = source.read_bytes_guarded(&guard)?;
    let providers = VideoProviders::lock_with_budget(
        ffmpeg, ffprobe, workdir, limits, budget, &is_cancelled,
    )?;
    let capture = providers.probe_untrusted(&bytes, &is_cancelled)?;
    ensure!(
        &capture.evidence.input_digest == source.digest(),
        "probe invocation is not bound to the observed source bytes"
    );
    guard()?;
    let probe = parse_ffv1_observation(&capture.bytes)?;
    guard()?;
    ensure!(
        probe.raw_report_digest == capture.evidence.stdout_digest,
        "parsed report is not bound to the captured provider output"
    );
    let producer = providers.producer_evidence(VideoOperation::StreamCopy, &probe.media.disposition)?;
    source.verify_current_guarded(&guard)?;
    providers.verify_current_with_cancel(&is_cancelled)?;
    source.verify_current_guarded(&guard)?;
    guard()?;
    Ok(VideoSourceObservation {
        status: "provider_observation_only",
        source_path: NativePath::from_path(source_path),
        source_snapshot: source.snapshot().clone(),
        source_digest: source.digest().clone(),
        source_reobserved: true,
        probe,
        probe_invocation: capture.evidence,
        producer_for_stream_copy_recipe: producer,
        producer_build_fingerprint: providers.producer_build_fingerprint().clone(),
        candidate_execution_available: false,
        operation_elapsed_ms: budget.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        operation_elapsed_limit_ms: limits.elapsed_ms,
        limitations: vec![
            "ffprobe_success_is_not_complete_decode_or_preservation_validation",
            "nonseekable_probe_reports_only_provider_exposed_features",
            "producer_version_observed_but_encoding_not_executed",
            "shared_deadline_is_cooperative_host_syscalls_and_parser_are_not_preempted",
            "process_group_cleanup_does_not_contain_escaped_descendants",
            "provider_rss_scratch_process_tree_and_free_space_are_not_enforced",
            "no_candidate_acceptance_publication_replacement_or_savings_claim",
        ],
    })
}
