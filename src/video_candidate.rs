//! Pure preparation for a bounded video candidate, using supplied declarations.
//!
//! No source/provider is opened, no subprocess is launched and no candidate is
//! accepted. Preparing a recipe does not authenticate the declared observations
//! or authorize its execution. Runtime observation and validation remain future
//! work; the ordinary scan descriptor is insufficient for this subset.

use serde::{Deserialize, Serialize};

use crate::contracts::{self, Contract};
use crate::domain::{EvidenceDigest, MediaProviderEvidence, NativePath};

pub const SCHEMA: &str = "optiflow.video-candidate-request.v1";
pub const PROFILE: &str = "optiflow.matroska-ffv1-sdr-candidate.v1";
pub const PROVIDER_VERSION: &str = "8.0.1";
const INPUT_BINDING: &str = "fresh_host_observed_read_only_source_descriptor_on_stdin";
const OUTPUT_BINDING: &str = "bounded_stdout_to_untrusted_host_owned_staging";
const ENVIRONMENT: &str = "cleared_environment_private_working_directory";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoCandidateRequest {
    pub schema: String,
    pub profile: String,
    pub source: SourceDeclaration,
    pub operation: VideoOperation,
    pub policy: CandidatePolicy,
    pub producer: MediaProviderEvidence,
    /// Claimed BLAKE3 of complete `-version` stdout bytes, including configuration
    /// and library lines, with no trimming. This planner captures no output.
    pub producer_build_fingerprint: EvidenceDigest,
    pub probe: MediaProviderEvidence,
    /// The probe's separately claimed complete `-version` stdout digest.
    pub probe_build_fingerprint: EvidenceDigest,
    pub limits: RequestedLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDeclaration {
    pub observation_id: String,
    pub observation_fingerprint: EvidenceDigest,
    pub content_digest: EvidenceDigest,
    pub logical_bytes: u64,
    pub probe_observation_fingerprint: EvidenceDigest,
    pub media: VideoDeclaration,
}

/// These are complete declared observations, not facts inferred from omission.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoDeclaration {
    pub container: String,
    pub codec: String,
    pub stream_index: u32,
    pub total_streams: u32,
    pub disposition: String,
    pub width: u32,
    pub height: u32,
    pub pixel_format: String,
    pub field_order: String,
    pub color_range: String,
    pub color_space: String,
    pub color_transfer: String,
    pub color_primaries: String,
    pub sample_aspect_ratio: Rational,
    pub frame_rate: Rational,
    pub time_base: Rational,
    pub frame_count: u64,
    pub start_pts: i64,
    pub duration_ticks: u64,
    pub constant_frame_rate: bool,
    pub rotation_degrees: i32,
    pub feature_observation_complete: bool,
    pub chapters: u32,
    pub side_data_entries: u32,
    pub descriptive_metadata_entries: u32,
    pub unknown_metadata_entries: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rational {
    pub numerator: u32,
    pub denominator: u32,
}

/// Lossy settings have no default. Stream copy makes no container-byte or
/// guaranteed-savings claim and still needs payload/timing validation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum VideoOperation {
    StreamCopy,
    H264Lossy { crf: u8, preset: H264Preset },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum H264Preset {
    Fast,
    Medium,
    Slow,
}

impl H264Preset {
    fn argument(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Medium => "medium",
            Self::Slow => "slow",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidatePolicy {
    pub effective_policy_fingerprint: EvidenceDigest,
    pub source_preserving: bool,
    pub metadata: String,
    pub require_strictly_smaller: bool,
}

/// Requested host limits only. A recipe does not enforce resource ceilings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestedLimits {
    pub input_bytes: u64,
    pub candidate_bytes: u64,
    /// Cumulative decoded sample bytes per video, applied independently to
    /// source and candidate later; excludes allocator/provider overhead.
    pub decoded_bytes: u64,
    pub temporary_bytes: u64,
    pub memory_bytes: u64,
    pub elapsed_ms: u64,
}

/// A review artifact, deliberately not a SubprocessCommand or accepted media.
#[derive(Debug, Clone, Serialize)]
pub struct PreparedForReview {
    pub status: &'static str,
    pub request_fingerprint: EvidenceDigest,
    pub preparation_fingerprint: EvidenceDigest,
    pub arguments: Vec<String>,
    pub input_binding: &'static str,
    pub output_binding: &'static str,
    pub required_environment: &'static str,
    pub preservation_claim: &'static str,
    pub requested_limits: RequestedLimits,
    pub outstanding_validation: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanRefusal {
    InvalidShape,
    UnsupportedSource,
    IncompleteObservation,
    InvalidTiming,
    ProviderMismatch,
    LimitExceeded,
}

impl std::fmt::Display for PlanRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "video candidate preparation refused: {self:?}")
    }
}

impl std::error::Error for PlanRefusal {}

/// Validate nested shape before deserialization of legacy shared domain types.
pub fn parse_request(value: &serde_json::Value) -> Result<VideoCandidateRequest, PlanRefusal> {
    contracts::validate(Contract::VideoCandidateRequest, value)
        .map_err(|_| PlanRefusal::InvalidShape)?;
    serde_json::from_value(value.clone()).map_err(|_| PlanRefusal::InvalidShape)
}

/// Fingerprint the typed request with a separate domain; not arbitrary JSON.
pub fn request_fingerprint(request: &VideoCandidateRequest) -> EvidenceDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"optiflow.video-candidate-request.v1\0");
    hasher.update(&serde_json::to_vec(request).expect("typed request serialization"));
    EvidenceDigest {
        algorithm: "blake3-256".to_owned(),
        value: hasher.finalize().to_hex().to_string(),
    }
}

pub fn prepare_candidate(request: &VideoCandidateRequest) -> Result<PreparedForReview, PlanRefusal> {
    contracts::validate(Contract::VideoCandidateRequest, request)
        .map_err(|_| PlanRefusal::InvalidShape)?;
    check_provider(&request.producer, "ffmpeg")?;
    check_provider(&request.probe, "ffprobe")?;
    let media = &request.source.media;
    if !media.feature_observation_complete {
        return Err(PlanRefusal::IncompleteObservation);
    }
    if media.container != "matroska"
        || media.codec != "ffv1"
        || media.stream_index != 0
        || media.total_streams != 1
        || !matches!(media.disposition.as_str(), "none" | "default")
        || media.width % 2 != 0
        || media.height % 2 != 0
        || media.pixel_format != "yuv420p"
        || media.field_order != "progressive"
        || media.color_range != "tv"
        || media.color_space != "bt709"
        || media.color_transfer != "bt709"
        || media.color_primaries != "bt709"
        || media.sample_aspect_ratio != (Rational { numerator: 1, denominator: 1 })
        || media.rotation_degrees != 0
        || media.chapters != 0
        || media.side_data_entries != 0
        || media.descriptive_metadata_entries != 0
        || media.unknown_metadata_entries != 0
    {
        return Err(PlanRefusal::UnsupportedSource);
    }
    // Deliberately small CFR subset: exact integer ticks avoid rounding or
    // duration tolerances being invented by this preparation checkpoint.
    if !media.constant_frame_rate
        || media.frame_rate != (Rational { numerator: 25, denominator: 1 })
        || media.time_base != (Rational { numerator: 1, denominator: 1000 })
        || media.start_pts != 0
        || media.frame_count.checked_mul(40) != Some(media.duration_ticks)
    {
        return Err(PlanRefusal::InvalidTiming);
    }
    let frame_bytes = u64::from(media.width)
        .checked_mul(u64::from(media.height))
        .and_then(|pixels| pixels.checked_mul(3))
        .map(|bytes| bytes / 2)
        .ok_or(PlanRefusal::LimitExceeded)?;
    let decoded_bytes = frame_bytes.checked_mul(media.frame_count)
        .ok_or(PlanRefusal::LimitExceeded)?;
    if request.source.logical_bytes > request.limits.input_bytes
        || decoded_bytes > request.limits.decoded_bytes
        || request.limits.temporary_bytes < request.limits.candidate_bytes
        || request.limits.memory_bytes < frame_bytes
    {
        return Err(PlanRefusal::LimitExceeded);
    }

    let (arguments, preservation_claim) = invocation_recipe(request.operation, &media.disposition)?;
    let mut outstanding_validation = vec![
        "fresh_source_identity_content_and_complete_feature_observation",
        "exact_provider_build_identity_and_required_capabilities",
        "host_enforcement_of_requested_resource_limits",
        "complete_source_and_candidate_decode",
        "stream_timing_dimensions_color_and_metadata_policy_comparison",
        "strictly_smaller_candidate_and_source_stability_before_publication",
        "versioned_video_artifact_publication_and_recovery",
    ];
    outstanding_validation.push(match request.operation {
        VideoOperation::StreamCopy => "elementary_payload_and_timestamp_preservation",
        VideoOperation::H264Lossy { .. } => "declared_lossy_evidence_and_operator_quality_review",
    });
    let request_fingerprint = request_fingerprint(request);
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"optiflow.video-candidate-preparation.v1\0");
    hasher.update(&serde_json::to_vec(&(
        &request_fingerprint, &arguments, INPUT_BINDING, OUTPUT_BINDING,
        ENVIRONMENT, preservation_claim, &outstanding_validation,
    )).expect("typed preparation serialization"));
    let preparation_fingerprint = EvidenceDigest {
        algorithm: "blake3-256".to_owned(),
        value: hasher.finalize().to_hex().to_string(),
    };
    Ok(PreparedForReview {
        status: "prepared_for_review_only",
        request_fingerprint,
        preparation_fingerprint,
        arguments,
        input_binding: INPUT_BINDING,
        output_binding: OUTPUT_BINDING,
        required_environment: ENVIRONMENT,
        preservation_claim,
        requested_limits: request.limits,
        outstanding_validation,
    })
}

/// Shared fixed recipe for review and internal untrusted capture. This does not
/// establish source eligibility or consume the full requested resource policy.
pub(crate) fn invocation_recipe(
    operation: VideoOperation,
    disposition: &str,
) -> Result<(Vec<String>, &'static str), PlanRefusal> {
    if !matches!(disposition, "none" | "default") {
        return Err(PlanRefusal::UnsupportedSource);
    }
    if matches!(operation, VideoOperation::H264Lossy { crf, .. } if !(1..=51).contains(&crf)) {
        return Err(PlanRefusal::InvalidShape);
    }
    // Only fixed tokens and bounded enum/integer policy values enter argv.
    // Source paths never enter the provider command.
    let mut arguments: Vec<String> = [
        "-hide_banner", "-nostdin", "-loglevel", "error", "-xerror",
        "-copyts", "-protocol_whitelist", "pipe", "-threads", "1",
        "-f", "matroska", "-i", "pipe:0", "-map", "0:0",
        "-map_metadata", "0", "-map_metadata:s:v:0", "0:s:0",
        "-map_chapters", "-1",
    ].into_iter().map(str::to_owned).collect();
    arguments.extend([
        "-disposition:v:0".to_owned(),
        if disposition == "default" { "default" } else { "0" }.to_owned(),
        "-avoid_negative_ts".to_owned(), "disabled".to_owned(),
    ]);
    let preservation_claim = match operation {
        VideoOperation::StreamCopy => {
            arguments.extend(["-c:v", "copy"].into_iter().map(str::to_owned));
            "stream_copy_intent_only_not_verified_packet_or_timing_preservation"
        }
        VideoOperation::H264Lossy { crf, preset } => {
            arguments.extend([
                "-c:v".to_owned(), "libx264".to_owned(),
                "-crf".to_owned(), crf.to_string(),
                "-preset".to_owned(), preset.argument().to_owned(),
                "-threads:v".to_owned(), "1".to_owned(),
                "-pix_fmt".to_owned(), "+yuv420p".to_owned(),
                "-fps_mode:v".to_owned(), "passthrough".to_owned(),
                "-color_range".to_owned(), "tv".to_owned(),
                "-colorspace".to_owned(), "bt709".to_owned(),
                "-color_trc".to_owned(), "bt709".to_owned(),
                "-color_primaries".to_owned(), "bt709".to_owned(),
            ]);
            "explicit_lossy_preview_no_exact_equivalence_or_quality_acceptance"
        }
    };
    arguments.extend(["-f", "matroska", "pipe:1"].into_iter().map(str::to_owned));
    Ok((arguments, preservation_claim))
}

fn check_provider(provider: &MediaProviderEvidence, name: &str) -> Result<(), PlanRefusal> {
    let NativePath::Utf8 { value } = &provider.executable else {
        return Err(PlanRefusal::ProviderMismatch);
    };
    if provider.name != name
        || provider.version != PROVIDER_VERSION
        || !value.starts_with('/')
        || value.contains('\0')
        || value.split('/').skip(1).any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(PlanRefusal::ProviderMismatch);
    }
    Ok(())
}
