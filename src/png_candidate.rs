//! Contract-only preparation for future static PNG candidate evaluation.
//!
//! This module performs no IO, decoding, optimization, or authorization. It
//! checks the consistency of supplied declarations, including separately
//! supplied host evidence. Even consistent evidence requires actual runtime
//! byte validation before any future candidate can be accepted.

use std::collections::BTreeSet;
use std::path::Component;

use serde::{Deserialize, Serialize};

use crate::contracts::{self, Contract};
use crate::domain::{EvidenceDigest, MediaProviderEvidence, NativePath};
use crate::filesystem::identity::FilesystemIdentity;

pub const SCHEMA: &str = "optiflow.png-candidate-contract.v1";
pub const PROFILE: &str = "optiflow.png-idat-preserve.v1";

/// A review packet, not an executable request or an accepted artifact set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateContract {
    pub schema: String,
    pub request: CandidateRequest,
    pub provider_result: ProviderResult,
    pub host_evidence: Option<HostEvidence>,
}

/// Explicit future evaluation intent. It grants no filesystem permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateRequest {
    pub profile: String,
    pub source_observation_id: String,
    pub source: FileEvidence,
    pub effective_policy_fingerprint: EvidenceDigest,
    pub producer: MediaProviderEvidence,
    pub validator: MediaProviderEvidence,
    pub limits: CandidateLimits,
}

/// Full content identity is distinct from the observation fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentIdentity {
    pub digest: EvidenceDigest,
    pub logical_bytes: u64,
}

/// Claimed regular-file observation. A future host must acquire this itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEvidence {
    pub path: NativePath,
    pub filesystem_identity: FilesystemIdentity,
    pub observation_fingerprint: EvidenceDigest,
    pub content: ContentIdentity,
    pub regular_file: bool,
    pub symlink: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateLimits {
    pub input_bytes: u64,
    pub candidate_bytes: u64,
    pub decoded_bytes: u64,
    pub temporary_bytes: u64,
    pub memory_bytes: u64,
    pub elapsed_ms: u64,
}

/// Untrusted provider claims cannot contain host evidence or validation flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResult {
    pub request_fingerprint: EvidenceDigest,
    pub producer: MediaProviderEvidence,
    pub status: ProviderStatus,
    pub candidate: Option<ContentIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStatus {
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
}

/// Separate host observations, never copied from provider stdout. These remain
/// declarations in this contract-only API: deserialization does not attest them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostEvidence {
    pub request_fingerprint: EvidenceDigest,
    pub source_before: FileEvidence,
    pub source_after: FileEvidence,
    pub candidate: FileEvidence,
    pub producer_before: MediaProviderEvidence,
    pub producer_after: MediaProviderEvidence,
    pub validator_before: MediaProviderEvidence,
    pub validator_after: MediaProviderEvidence,
    pub source_png: PngFacts,
    pub candidate_png: PngFacts,
    pub checks: Vec<ValidationCheck>,
    pub resource_usage: ResourceUsage,
}

/// Canonical sample and ordered non-IDAT chunk digests are defined in the spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PngFacts {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub color_type: u8,
    pub frames: u32,
    pub complete_decode: bool,
    pub animation_chunks: bool,
    pub unknown_unsafe_to_copy_chunks: bool,
    pub decoded_bytes: u64,
    pub ihdr_digest: EvidenceDigest,
    pub decoded_samples_digest: EvidenceDigest,
    pub non_idat_chunks_digest: EvidenceDigest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    SourceStable,
    CompleteDecode,
    Dimensions,
    FrameCount,
    DecodedSamples,
    AlphaPreserved,
    ColorPreserved,
    MetadataPreserved,
}

pub const REQUIRED_CHECKS: [CheckKind; 8] = [
    CheckKind::SourceStable,
    CheckKind::CompleteDecode,
    CheckKind::Dimensions,
    CheckKind::FrameCount,
    CheckKind::DecodedSamples,
    CheckKind::AlphaPreserved,
    CheckKind::ColorPreserved,
    CheckKind::MetadataPreserved,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Passed,
    Failed,
    Unsupported,
    NotRun,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationCheck {
    pub kind: CheckKind,
    pub status: CheckStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceUsage {
    pub peak_temporary_bytes: u64,
    pub peak_memory_bytes: u64,
    pub elapsed_ms: u64,
}

/// This success type deliberately contains no accepted artifact or permission.
#[derive(Debug, PartialEq, Eq)]
pub struct ConsistentForReview {
    pub reported_encoded_byte_reduction: u64,
}

/// Stable, content-free reasons; errors never reproduce private paths/outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractRefusal {
    InvalidShape,
    InvalidPath,
    RequestMismatch,
    ProviderUnsuccessful,
    MissingHostEvidence,
    ProviderIdentityMismatch,
    SourceChanged,
    CandidateMismatch,
    SourceAlias,
    UnsupportedPng,
    PreservationMismatch,
    ValidationIncomplete,
    LimitExceeded,
    NotSmaller,
}

/// Hash the validated typed request with a domain separator and compact JSON.
/// This is the v1 Rust serialization contract, not arbitrary input JSON bytes.
pub fn request_fingerprint(request: &CandidateRequest) -> EvidenceDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"optiflow.png-candidate-request.v1\0");
    // All request fields serialize infallibly (no floats or non-string map keys).
    let bytes = serde_json::to_vec(request).expect("typed request serialization");
    hasher.update(&bytes);
    EvidenceDigest {
        algorithm: "blake3-256".to_owned(),
        value: hasher.finalize().to_hex().to_string(),
    }
}

/// Validate raw shape before deserialization so nested legacy domain types
/// cannot silently discard an unknown wire field.
pub fn parse_contract(value: &serde_json::Value) -> Result<CandidateContract, ContractRefusal> {
    contracts::validate(Contract::PngCandidateContract, value)
        .map_err(|_| ContractRefusal::InvalidShape)?;
    serde_json::from_value(value.clone()).map_err(|_| ContractRefusal::InvalidShape)
}

/// Check supplied evidence consistency only. No bytes are read and no tool is
/// launched. A future coordinator must perform all declared host observations,
/// complete decode and preservation checks; this result cannot replace them.
pub fn check_candidate_contract(
    packet: &CandidateContract,
) -> Result<ConsistentForReview, ContractRefusal> {
    contracts::validate(Contract::PngCandidateContract, packet)
        .map_err(|_| ContractRefusal::InvalidShape)?;
    let request = &packet.request;
    let result = &packet.provider_result;
    let fingerprint = request_fingerprint(request);
    if result.request_fingerprint != fingerprint {
        return Err(ContractRefusal::RequestMismatch);
    }
    if result.status != ProviderStatus::Succeeded {
        return Err(ContractRefusal::ProviderUnsuccessful);
    }
    let host = packet
        .host_evidence
        .as_ref()
        .ok_or(ContractRefusal::MissingHostEvidence)?;
    if host.request_fingerprint != fingerprint {
        return Err(ContractRefusal::RequestMismatch);
    }
    for path in [
        &request.source.path,
        &request.producer.executable,
        &request.validator.executable,
        &host.candidate.path,
    ] {
        if !canonical_absolute_path(path) {
            return Err(ContractRefusal::InvalidPath);
        }
    }
    if result.producer != request.producer
        || host.producer_before != request.producer
        || host.producer_after != request.producer
        || host.validator_before != request.validator
        || host.validator_after != request.validator
    {
        return Err(ContractRefusal::ProviderIdentityMismatch);
    }
    if host.source_before != request.source || host.source_after != request.source {
        return Err(ContractRefusal::SourceChanged);
    }
    if result.candidate.as_ref() != Some(&host.candidate.content) {
        return Err(ContractRefusal::CandidateMismatch);
    }
    if host.candidate.path.to_path_buf() == request.source.path.to_path_buf()
        || host.candidate.filesystem_identity.platform
            != request.source.filesystem_identity.platform
        || host.candidate.filesystem_identity.identity_key()
            == request.source.filesystem_identity.identity_key()
    {
        return Err(ContractRefusal::SourceAlias);
    }
    let required: BTreeSet<_> = REQUIRED_CHECKS.into_iter().collect();
    let supplied: BTreeSet<_> = host.checks.iter().map(|check| check.kind).collect();
    if host.checks.len() != required.len()
        || supplied != required
        || host
            .checks
            .iter()
            .any(|check| check.status != CheckStatus::Passed)
    {
        return Err(ContractRefusal::ValidationIncomplete);
    }
    if !supported_png(&host.source_png) || !supported_png(&host.candidate_png) {
        return Err(ContractRefusal::UnsupportedPng);
    }
    if host.source_png != host.candidate_png {
        return Err(ContractRefusal::PreservationMismatch);
    }
    let limits = &request.limits;
    let source_bytes = request.source.content.logical_bytes;
    let candidate_bytes = host.candidate.content.logical_bytes;
    if source_bytes > limits.input_bytes
        || candidate_bytes > limits.candidate_bytes
        || host.source_png.decoded_bytes > limits.decoded_bytes
        || candidate_bytes > limits.temporary_bytes
        || host.resource_usage.peak_temporary_bytes < candidate_bytes
        || host.resource_usage.peak_temporary_bytes > limits.temporary_bytes
        || host.resource_usage.peak_memory_bytes > limits.memory_bytes
        || host.resource_usage.elapsed_ms > limits.elapsed_ms
    {
        return Err(ContractRefusal::LimitExceeded);
    }
    if candidate_bytes >= source_bytes {
        return Err(ContractRefusal::NotSmaller);
    }
    if host.candidate.content.digest == request.source.content.digest {
        return Err(ContractRefusal::CandidateMismatch);
    }
    Ok(ConsistentForReview {
        reported_encoded_byte_reduction: source_bytes - candidate_bytes,
    })
}

fn canonical_absolute_path(path: &NativePath) -> bool {
    let decoded = path.to_path_buf();
    if NativePath::from_path(&decoded) != *path || !decoded.is_absolute() {
        return false;
    }
    if decoded
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return false;
    }
    // Rebuild to reject dot segments/repeated separators that Path normalizes.
    let rebuilt: std::path::PathBuf = decoded.components().collect();
    if rebuilt.as_os_str() != decoded.as_os_str() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        !decoded.as_os_str().as_bytes().contains(&0)
    }
    #[cfg(not(unix))]
    {
        false // Contract execution is scoped to the supported Unix platforms.
    }
}

fn supported_png(facts: &PngFacts) -> bool {
    // Deliberately bounded first profile: 8-bit RGB/RGBA, static and fully decoded.
    let channels = match facts.color_type {
        2 => 3_u64,
        6 => 4_u64,
        _ => return false,
    };
    let expected_bytes = u64::from(facts.width)
        .checked_mul(u64::from(facts.height))
        .and_then(|pixels| pixels.checked_mul(channels));
    facts.complete_decode
        && !facts.animation_chunks
        && !facts.unknown_unsafe_to_copy_chunks
        && facts.frames == 1
        && facts.bit_depth == 8
        && expected_bytes == Some(facts.decoded_bytes)
}
