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
    pub limitations: Vec<&'static str>,
}

/// Observe a small source through an explicitly selected provider pair.
///
/// The working directory must already exist, be empty, have mode 0700, and
/// have no symlink components. The caller owns it; this function does not
/// remove unexpected files. Provider limits apply to individual invocations,
/// not total wall time, binary hashing, RSS, scratch space or process trees.
/// The encoded source is retained in bounded memory for a stable stdin copy.
pub fn observe_video_source(
    source_path: &Path,
    ffmpeg: &Path,
    ffprobe: &Path,
    workdir: &Path,
    limits: CaptureLimits,
    is_cancelled: impl Fn() -> bool,
) -> Result<VideoSourceObservation> {
    ensure!(!is_cancelled(), "video observation cancelled before source read");
    let source = BoundedFileObservation::open(source_path, SOURCE_MAX)?;
    let bytes = source.read_bytes()?;
    ensure!(!is_cancelled(), "video observation cancelled after source read");
    let providers = VideoProviders::lock(ffmpeg, ffprobe, workdir, limits, &is_cancelled)?;
    let capture = providers.probe_untrusted(&bytes, &is_cancelled)?;
    ensure!(
        &capture.evidence.input_digest == source.digest(),
        "probe invocation is not bound to the observed source bytes"
    );
    let probe = parse_ffv1_observation(&capture.bytes)?;
    ensure!(
        probe.raw_report_digest == capture.evidence.stdout_digest,
        "parsed report is not bound to the captured provider output"
    );
    let producer = providers.producer_evidence(VideoOperation::StreamCopy, &probe.media.disposition)?;
    source.verify_current()?;
    providers.verify_current()?;
    source.verify_current()?;
    ensure!(!is_cancelled(), "video observation cancelled before reporting");
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
        limitations: vec![
            "ffprobe_success_is_not_complete_decode_or_preservation_validation",
            "nonseekable_probe_reports_only_provider_exposed_features",
            "producer_version_observed_but_encoding_not_executed",
            "host_hashing_and_entire_workflow_have_no_wall_time_ceiling",
            "provider_rss_scratch_process_tree_and_free_space_are_not_enforced",
            "no_candidate_acceptance_publication_replacement_or_savings_claim",
        ],
    })
}
