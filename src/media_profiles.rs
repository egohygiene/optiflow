//! Read-only media-profile analysis.
//!
//! Profiles consume already accepted handle-bound observations. They may emit
//! review evidence, but they cannot create outputs or grant mutation authority.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::configuration::PolicyDigest;
use crate::domain::{
    EvidenceDigest, EvidenceValidity, FileObservation, MEDIA_PROFILE_EVIDENCE_SCHEMA, MediaKind,
    MediaOptimizationOpportunity, MediaProfileCoverage, MediaProfileCoverageStatus,
    MediaProfileDefinition, MediaProfileEntry, MediaProfileEntryStatus, MediaProfileEvidence,
    MediaProfileGuarantees, MediaProfileLimitation, MediaProfileSource, MediaProviderEvidence,
    NormalizedPngEvidence, ObservationStability, ObservationStatus, OutputValidationRequirement,
    SavingsClaim,
};

pub const LOSSLESS_PNG_PROFILE_ID: &str = "optiflow.builtin.lossless-png-review";
pub const LOSSLESS_PNG_PROFILE_VERSION: &str = "1.0.0";

const LOSSLESS_PNG_INTENT: &str = "lossless_recompression_review";
const LOSSLESS_PNG_OPERATION: &str = "evaluate_lossless_png_recompression";

#[derive(Serialize)]
struct ProfileConfiguration<'a> {
    id: &'a str,
    version: &'a str,
    content_type: &'a str,
    intent: &'a str,
    proposed_operation: &'a str,
    required_output_validations: &'a [OutputValidationRequirement],
    source_mutation: bool,
    outputs_produced: bool,
    savings_claim: SavingsClaim,
}

#[derive(Serialize)]
struct SourceIdentity<'a> {
    profile_configuration_fingerprint: &'a EvidenceDigest,
    evidence_policy_fingerprint: &'a EvidenceDigest,
    path: &'a crate::domain::NativePath,
    size_bytes: u64,
    modified_unix_ns: Option<i64>,
    filesystem_identity: &'a Option<crate::domain::FilesystemIdentity>,
    content_type: &'a Option<String>,
    media_kind: &'a MediaKind,
    media: &'a Option<crate::domain::MediaDescriptor>,
    observation_stability: &'a ObservationStability,
    evidence_validity: &'a EvidenceValidity,
    provider: Option<&'a MediaProviderEvidence>,
}

#[derive(Serialize)]
struct AnalysisIdentity<'a> {
    profile_configuration_fingerprint: &'a EvidenceDigest,
    evidence_policy_fingerprint: &'a EvidenceDigest,
    entry_ids: Vec<&'a str>,
}

/// Build the first built-in media profile from accepted scan observations.
pub fn lossless_png_evidence(
    source_run_id: &str,
    observations: &[FileObservation],
    probe_media: bool,
    provider: Option<&MediaProviderEvidence>,
    evidence_policy_fingerprint: &PolicyDigest,
) -> Result<MediaProfileEvidence> {
    let required_output_validations = required_output_validations();
    let configuration_fingerprint = digest(&ProfileConfiguration {
        id: LOSSLESS_PNG_PROFILE_ID,
        version: LOSSLESS_PNG_PROFILE_VERSION,
        content_type: "image/png",
        intent: LOSSLESS_PNG_INTENT,
        proposed_operation: LOSSLESS_PNG_OPERATION,
        required_output_validations: &required_output_validations,
        source_mutation: false,
        outputs_produced: false,
        savings_claim: SavingsClaim::NotEstimated,
    })?;
    let evidence_policy_fingerprint = EvidenceDigest {
        algorithm: evidence_policy_fingerprint.algorithm.clone(),
        value: evidence_policy_fingerprint.value.clone(),
    };
    let profile = MediaProfileDefinition {
        id: LOSSLESS_PNG_PROFILE_ID.to_owned(),
        version: LOSSLESS_PNG_PROFILE_VERSION.to_owned(),
        media_kind: MediaKind::Image,
        intent: LOSSLESS_PNG_INTENT.to_owned(),
        configuration_fingerprint: configuration_fingerprint.clone(),
        guarantees: MediaProfileGuarantees {
            source_mutation: false,
            outputs_produced: false,
            savings_estimate: SavingsClaim::NotEstimated,
        },
    };

    let mut png_observations: Vec<&FileObservation> = observations
        .iter()
        .filter(|observation| {
            observation.content_type.as_deref() == Some("image/png")
                && observation.media_kind == MediaKind::Image
        })
        .collect();
    png_observations.sort_by(|left, right| left.path.cmp(&right.path));

    let mut entries = Vec::with_capacity(png_observations.len());
    for observation in png_observations {
        entries.push(build_entry(
            observation,
            probe_media,
            provider,
            &configuration_fingerprint,
            &evidence_policy_fingerprint,
            &required_output_validations,
        )?);
    }

    let complete_evidence_count = u64::try_from(
        entries
            .iter()
            .filter(|entry| entry.status == MediaProfileEntryStatus::Opportunity)
            .count(),
    )
    .unwrap_or(u64::MAX);
    let candidate_media_count = u64::try_from(entries.len()).unwrap_or(u64::MAX);
    let limited_evidence_count = candidate_media_count.saturating_sub(complete_evidence_count);
    let limitations = entries
        .iter()
        .flat_map(|entry| entry.limitations.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let status = if candidate_media_count == 0 {
        MediaProfileCoverageStatus::NotApplicable
    } else if !probe_media {
        MediaProfileCoverageStatus::NotRequested
    } else if complete_evidence_count == candidate_media_count {
        MediaProfileCoverageStatus::Complete
    } else if complete_evidence_count == 0 {
        MediaProfileCoverageStatus::Unavailable
    } else {
        MediaProfileCoverageStatus::Partial
    };
    let coverage = MediaProfileCoverage {
        status,
        candidate_media_count,
        complete_evidence_count,
        limited_evidence_count,
        opportunity_count: complete_evidence_count,
        limitations,
    };
    let analysis_id = prefixed_id(
        "mpa",
        &AnalysisIdentity {
            profile_configuration_fingerprint: &configuration_fingerprint,
            evidence_policy_fingerprint: &evidence_policy_fingerprint,
            entry_ids: entries
                .iter()
                .map(|entry| entry.entry_id.as_str())
                .collect(),
        },
    )?;

    Ok(MediaProfileEvidence {
        schema: MEDIA_PROFILE_EVIDENCE_SCHEMA.to_owned(),
        analysis_id,
        source_run_id: source_run_id.to_owned(),
        profile,
        evidence_policy_fingerprint,
        coverage,
        entries,
    })
}

fn build_entry(
    observation: &FileObservation,
    probe_media: bool,
    provider: Option<&MediaProviderEvidence>,
    profile_configuration_fingerprint: &EvidenceDigest,
    evidence_policy_fingerprint: &EvidenceDigest,
    required_output_validations: &[OutputValidationRequirement],
) -> Result<MediaProfileEntry> {
    let fingerprint = digest(&SourceIdentity {
        profile_configuration_fingerprint,
        evidence_policy_fingerprint,
        path: &observation.path,
        size_bytes: observation.size_bytes,
        modified_unix_ns: observation.modified_unix_ns,
        filesystem_identity: &observation.filesystem_identity,
        content_type: &observation.content_type,
        media_kind: &observation.media_kind,
        media: &observation.media,
        observation_stability: &observation.observation_stability,
        evidence_validity: &observation.evidence_validity,
        provider,
    })?;
    let entry_id = prefixed_id("mpe", &fingerprint)?;
    let source = MediaProfileSource {
        observation_id: observation.observation_id.clone(),
        path: observation.path.clone(),
        size_bytes: observation.size_bytes,
        modified_unix_ns: observation.modified_unix_ns,
        fingerprint: fingerprint.clone(),
    };
    let mut limitations = Vec::new();

    let current_source = observation.evidence_validity == EvidenceValidity::Current
        && observation.observation_stability == ObservationStability::Stable
        && observation.status == ObservationStatus::Analyzed;
    if !current_source {
        limitations.push(MediaProfileLimitation::SourceEvidenceNotCurrent);
    }
    if !probe_media {
        limitations.push(MediaProfileLimitation::MediaProbeDisabled);
    } else if provider.is_none() {
        limitations.push(MediaProfileLimitation::ProviderUnavailable);
    } else if observation.media.is_none() {
        limitations.push(MediaProfileLimitation::ProviderResultUnavailable);
    }

    let normalized = normalized_png_evidence(observation);
    if limitations.is_empty() && !valid_png_evidence(observation) {
        limitations.push(MediaProfileLimitation::ProviderEvidenceInvalid);
    }

    let status = if limitations.is_empty() {
        MediaProfileEntryStatus::Opportunity
    } else if limitations.contains(&MediaProfileLimitation::SourceEvidenceNotCurrent) {
        MediaProfileEntryStatus::Excluded
    } else {
        MediaProfileEntryStatus::InsufficientEvidence
    };
    let opportunity = (status == MediaProfileEntryStatus::Opportunity).then(|| {
        MediaOptimizationOpportunity {
            opportunity_id: format!("opo-{}", &fingerprint.value[..32]),
            classification: "review_candidate".to_owned(),
            proposed_operation: LOSSLESS_PNG_OPERATION.to_owned(),
            source_logical_bytes: observation.size_bytes,
            estimated_output_bytes: None,
            estimated_logical_savings_bytes: None,
            savings_claim: SavingsClaim::NotEstimated,
            reason: "content and stream evidence identify a current PNG input for later lossless-recompression evaluation; no candidate output or savings estimate exists"
                .to_owned(),
            required_output_validations: required_output_validations.to_vec(),
        }
    });

    Ok(MediaProfileEntry {
        entry_id,
        source,
        provider: provider.cloned(),
        status,
        limitations,
        observations: normalized,
        opportunity,
    })
}

fn normalized_png_evidence(observation: &FileObservation) -> NormalizedPngEvidence {
    let stream = observation
        .media
        .as_ref()
        .and_then(|media| media.streams.first());
    NormalizedPngEvidence {
        content_type: observation
            .content_type
            .clone()
            .unwrap_or_else(|| "image/png".to_owned()),
        format_name: observation
            .media
            .as_ref()
            .and_then(|media| media.format_name.clone()),
        codec_name: stream.and_then(|stream| stream.codec_name.clone()),
        width: stream.and_then(|stream| stream.width),
        height: stream.and_then(|stream| stream.height),
        stream_count: observation
            .media
            .as_ref()
            .map(|media| u64::try_from(media.streams.len()).unwrap_or(u64::MAX))
            .unwrap_or(0),
    }
}

fn valid_png_evidence(observation: &FileObservation) -> bool {
    let Some(media) = observation.media.as_ref() else {
        return false;
    };
    let Some(format_name) = media.format_name.as_deref() else {
        return false;
    };
    if !format_name.split(',').any(|name| name == "png_pipe") || media.streams.len() != 1 {
        return false;
    }
    let stream = &media.streams[0];
    stream.codec_type.as_deref() == Some("video")
        && stream.codec_name.as_deref() == Some("png")
        && stream.width.is_some_and(|width| width > 0)
        && stream.height.is_some_and(|height| height > 0)
        && stream.sample_rate.is_none()
        && stream.channels.is_none()
}

fn required_output_validations() -> Vec<OutputValidationRequirement> {
    vec![
        OutputValidationRequirement::SourceObservationRevalidated,
        OutputValidationRequirement::OutputExists,
        OutputValidationRequirement::OutputIsRegularFile,
        OutputValidationRequirement::OutputContentTypePng,
        OutputValidationRequirement::CompleteDecode,
        OutputValidationRequirement::DimensionsPreserved,
        OutputValidationRequirement::FrameCountPreserved,
        OutputValidationRequirement::AlphaPreserved,
        OutputValidationRequirement::ColorProfilePolicySatisfied,
        OutputValidationRequirement::MetadataPolicySatisfied,
        OutputValidationRequirement::DecodedPixelEquivalence,
        OutputValidationRequirement::OutputSmallerThanSource,
    ]
}

fn digest<T: Serialize>(value: &T) -> Result<EvidenceDigest> {
    let bytes =
        serde_json::to_vec(value).context("failed to canonicalize media-profile evidence")?;
    Ok(EvidenceDigest {
        algorithm: "blake3-256".to_owned(),
        value: blake3::hash(&bytes).to_hex().to_string(),
    })
}

fn prefixed_id<T: Serialize>(prefix: &str, value: &T) -> Result<String> {
    let digest = digest(value)?;
    Ok(format!("{prefix}-{}", &digest.value[..32]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{self, Contract};
    use crate::domain::{
        FileObservation, MediaDescriptor, MediaStream, NativePath, ObservationStatus,
    };

    const RUN_ID: &str = "018f47a2-4f17-7b00-8000-000000000000";

    fn policy_digest() -> PolicyDigest {
        PolicyDigest {
            algorithm: "blake3-256".to_owned(),
            value: "1".repeat(64),
        }
    }

    fn provider() -> MediaProviderEvidence {
        MediaProviderEvidence {
            name: "ffprobe".to_owned(),
            version: "ffprobe version fixture".to_owned(),
            executable: NativePath::Utf8 {
                value: "/fixture/ffprobe".to_owned(),
            },
            binary_digest: EvidenceDigest {
                algorithm: "blake3-256".to_owned(),
                value: "2".repeat(64),
            },
            invocation_fingerprint: EvidenceDigest {
                algorithm: "blake3-256".to_owned(),
                value: "3".repeat(64),
            },
        }
    }

    fn observation(id: &str, path: &str) -> FileObservation {
        FileObservation {
            observation_id: id.to_owned(),
            run_id: RUN_ID.to_owned(),
            path: NativePath::Utf8 {
                value: path.to_owned(),
            },
            size_bytes: 128,
            modified_unix_ns: Some(42),
            device_id: None,
            inode: None,
            content_type: Some("image/png".to_owned()),
            media_kind: MediaKind::Image,
            content_hash: None,
            hash_algorithm: None,
            media: Some(MediaDescriptor {
                format_name: Some("png_pipe".to_owned()),
                duration_seconds: None,
                bit_rate: None,
                streams: vec![MediaStream {
                    index: 0,
                    codec_type: Some("video".to_owned()),
                    codec_name: Some("png".to_owned()),
                    width: Some(16),
                    height: Some(12),
                    sample_rate: None,
                    channels: None,
                }],
            }),
            status: ObservationStatus::Analyzed,
            cache_hit: false,
            warnings: Vec::new(),
            filesystem_identity: None,
            storage_allocation: None,
            observation_stability: ObservationStability::Stable,
            evidence_validity: EvidenceValidity::Current,
            attempt_count: 1,
        }
    }

    #[test]
    fn valid_png_emits_schema_valid_review_opportunity_without_savings_claim() {
        let evidence = lossless_png_evidence(
            RUN_ID,
            &[observation(
                "018f47a2-4f17-7b00-8000-000000000001",
                "/media/image.png",
            )],
            true,
            Some(&provider()),
            &policy_digest(),
        )
        .expect("profile evidence");

        contracts::validate(Contract::MediaProfileEvidence, &evidence)
            .expect("schema-valid evidence");
        assert_eq!(
            evidence.coverage.status,
            MediaProfileCoverageStatus::Complete
        );
        assert_eq!(evidence.coverage.opportunity_count, 1);
        let opportunity = evidence.entries[0]
            .opportunity
            .as_ref()
            .expect("review opportunity");
        assert_eq!(opportunity.estimated_output_bytes, None);
        assert_eq!(opportunity.estimated_logical_savings_bytes, None);
        assert_eq!(opportunity.savings_claim, SavingsClaim::NotEstimated);
    }

    #[test]
    fn identifiers_and_order_are_stable_across_reruns() {
        let first = lossless_png_evidence(
            RUN_ID,
            &[
                observation("018f47a2-4f17-7b00-8000-000000000002", "/media/z.png"),
                observation("018f47a2-4f17-7b00-8000-000000000001", "/media/a.png"),
            ],
            true,
            Some(&provider()),
            &policy_digest(),
        )
        .expect("first analysis");
        let second = lossless_png_evidence(
            "018f47a2-4f17-7b00-8000-000000000099",
            &[
                observation("018f47a2-4f17-7b00-8000-000000000091", "/media/a.png"),
                observation("018f47a2-4f17-7b00-8000-000000000092", "/media/z.png"),
            ],
            true,
            Some(&provider()),
            &policy_digest(),
        )
        .expect("second analysis");

        assert_eq!(first.analysis_id, second.analysis_id);
        assert_eq!(
            first
                .entries
                .iter()
                .map(|entry| (&entry.entry_id, &entry.source.path))
                .collect::<Vec<_>>(),
            second
                .entries
                .iter()
                .map(|entry| (&entry.entry_id, &entry.source.path))
                .collect::<Vec<_>>()
        );
        assert!(matches!(
            first.entries[0].source.path,
            NativePath::Utf8 { ref value } if value == "/media/a.png"
        ));
    }

    #[test]
    fn disabled_absent_invalid_and_stale_evidence_never_emit_opportunities() {
        let current = observation("018f47a2-4f17-7b00-8000-000000000001", "/media/current.png");
        let disabled = lossless_png_evidence(
            RUN_ID,
            std::slice::from_ref(&current),
            false,
            None,
            &policy_digest(),
        )
        .expect("disabled profile");
        assert_eq!(
            disabled.coverage.status,
            MediaProfileCoverageStatus::NotRequested
        );
        assert_eq!(disabled.coverage.opportunity_count, 0);
        contracts::validate(Contract::MediaProfileEvidence, &disabled)
            .expect("schema-valid disabled evidence");

        let unavailable = lossless_png_evidence(
            RUN_ID,
            std::slice::from_ref(&current),
            true,
            None,
            &policy_digest(),
        )
        .expect("unavailable provider");
        assert_eq!(
            unavailable.coverage.limitations,
            vec![MediaProfileLimitation::ProviderUnavailable]
        );

        let mut failed = current.clone();
        failed.media = None;
        let failed =
            lossless_png_evidence(RUN_ID, &[failed], true, Some(&provider()), &policy_digest())
                .expect("failed provider");
        assert_eq!(
            failed.coverage.limitations,
            vec![MediaProfileLimitation::ProviderResultUnavailable]
        );

        let mut invalid = current.clone();
        invalid.media.as_mut().expect("descriptor").streams[0].codec_name = Some("jpeg".to_owned());
        let invalid = lossless_png_evidence(
            RUN_ID,
            &[invalid],
            true,
            Some(&provider()),
            &policy_digest(),
        )
        .expect("invalid provider evidence");
        assert_eq!(
            invalid.coverage.limitations,
            vec![MediaProfileLimitation::ProviderEvidenceInvalid]
        );

        let mut stale = current;
        stale.evidence_validity = EvidenceValidity::Stale;
        stale.observation_stability = ObservationStability::ChangedDuringProbe;
        let stale =
            lossless_png_evidence(RUN_ID, &[stale], true, Some(&provider()), &policy_digest())
                .expect("stale evidence");
        assert_eq!(stale.entries[0].status, MediaProfileEntryStatus::Excluded);
        assert_eq!(stale.coverage.opportunity_count, 0);
        contracts::validate(Contract::MediaProfileEvidence, &stale)
            .expect("schema-valid stale evidence");

        let mut stale_without_probe = observation(
            "018f47a2-4f17-7b00-8000-000000000010",
            "/media/stale-without-probe.png",
        );
        stale_without_probe.evidence_validity = EvidenceValidity::Stale;
        stale_without_probe.observation_stability = ObservationStability::ChangedDuringProbe;
        let stale_without_probe = lossless_png_evidence(
            RUN_ID,
            &[stale_without_probe],
            false,
            None,
            &policy_digest(),
        )
        .expect("stale disabled evidence");
        assert_eq!(
            stale_without_probe.coverage.status,
            MediaProfileCoverageStatus::NotRequested
        );
        contracts::validate(Contract::MediaProfileEvidence, &stale_without_probe)
            .expect("schema-valid stale disabled evidence");
    }

    #[test]
    fn mixed_evidence_is_partial_and_non_png_content_is_not_claimed() {
        let valid = observation("018f47a2-4f17-7b00-8000-000000000001", "/media/valid.png");
        let mut failed = observation("018f47a2-4f17-7b00-8000-000000000002", "/media/failed.png");
        failed.media = None;
        let mut misleading = observation(
            "018f47a2-4f17-7b00-8000-000000000003",
            "/media/misleading.png",
        );
        misleading.content_type = Some("text/plain".to_owned());
        misleading.media_kind = MediaKind::Other;

        let evidence = lossless_png_evidence(
            RUN_ID,
            &[valid, failed, misleading],
            true,
            Some(&provider()),
            &policy_digest(),
        )
        .expect("partial analysis");

        assert_eq!(
            evidence.coverage.status,
            MediaProfileCoverageStatus::Partial
        );
        assert_eq!(evidence.coverage.candidate_media_count, 2);
        assert_eq!(evidence.coverage.opportunity_count, 1);
        assert_eq!(evidence.entries.len(), 2);
    }
}
