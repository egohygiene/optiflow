use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::OutputFormat;
use crate::domain::{ScanOptions, SerializedPath};

pub const EFFECTIVE_POLICY_SCHEMA: &str = "optiflow.effective-policy.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicySourceKind {
    CompiledDefault,
    UserConfiguration,
    ProjectConfiguration,
    ExplicitConfiguration,
    Environment,
    CommandLine,
    LockedInvariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationSourceStatus {
    Loaded,
    Absent,
    Suppressed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationSourceRecord {
    pub source: PolicySourceKind,
    pub status: ConfigurationSourceStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<SerializedPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyProvenance {
    pub source: PolicySourceKind,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<SerializedPath>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShadowedValue {
    pub setting: String,
    pub value: Value,
    pub provenance: PolicyProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactGroupingProfile {
    SizeAndBlake3_256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StabilityRequirement {
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnstableObservationPolicy {
    Exclude,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePolicy {
    pub follow_symlinks: bool,
    pub include_hidden: bool,
    pub cross_filesystems: bool,
    pub probe_media: bool,
    pub exact_grouping_profile: ExactGroupingProfile,
    pub observation_stability: StabilityRequirement,
    pub unstable_observations: UnstableObservationPolicy,
    pub maximum_observation_attempts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationalPolicy {
    pub state_directory: SerializedPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationPolicy {
    pub output_format: OutputFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyInvariants {
    pub source_mutation: bool,
    pub apply_enabled: bool,
    pub shell_execution: bool,
    pub artifact_validation_required: bool,
    pub atomic_artifact_commit_required: bool,
    pub current_stability_required_for_exact_groups: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDigest {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyFingerprints {
    pub effective_configuration: PolicyDigest,
    pub evidence_policy: PolicyDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectivePolicyV1 {
    pub schema: String,
    pub evidence_policy: EvidencePolicy,
    pub operational_policy: OperationalPolicy,
    pub presentation_policy: PresentationPolicy,
    pub safety_invariants: SafetyInvariants,
    pub fingerprints: PolicyFingerprints,
    pub provenance: BTreeMap<String, PolicyProvenance>,
}

#[derive(Debug, Clone)]
pub struct RuntimePolicy {
    pub state_directory: PathBuf,
    pub scan: ScanOptions,
    pub output_format: OutputFormat,
}

#[derive(Serialize)]
struct EffectiveConfigurationIdentity<'a> {
    evidence_policy: &'a EvidencePolicy,
    operational_policy: &'a OperationalPolicy,
    presentation_policy: &'a PresentationPolicy,
    safety_invariants: &'a SafetyInvariants,
}

#[derive(Serialize)]
struct EvidencePolicyIdentity<'a> {
    evidence_policy: &'a EvidencePolicy,
    source_mutation: bool,
    apply_enabled: bool,
    artifact_validation_required: bool,
    current_stability_required_for_exact_groups: bool,
}

pub fn build_effective_policy(
    evidence_policy: EvidencePolicy,
    state_directory: &std::path::Path,
    output_format: OutputFormat,
    provenance: BTreeMap<String, PolicyProvenance>,
) -> Result<EffectivePolicyV1> {
    let operational_policy = OperationalPolicy {
        state_directory: SerializedPath::from_path(state_directory),
    };
    let presentation_policy = PresentationPolicy { output_format };
    let safety_invariants = locked_safety_invariants();
    let fingerprints = calculate_fingerprints(
        &evidence_policy,
        &operational_policy,
        &presentation_policy,
        &safety_invariants,
    )?;
    Ok(EffectivePolicyV1 {
        schema: EFFECTIVE_POLICY_SCHEMA.to_owned(),
        evidence_policy,
        operational_policy,
        presentation_policy,
        safety_invariants,
        fingerprints,
        provenance,
    })
}

pub fn validate_fingerprints(policy: &EffectivePolicyV1) -> Result<()> {
    if policy.schema != EFFECTIVE_POLICY_SCHEMA {
        bail!("unsupported effective-policy schema");
    }
    let expected = calculate_fingerprints(
        &policy.evidence_policy,
        &policy.operational_policy,
        &policy.presentation_policy,
        &policy.safety_invariants,
    )?;
    if policy.fingerprints != expected {
        bail!("effective-policy fingerprint mismatch");
    }
    Ok(())
}

fn calculate_fingerprints(
    evidence_policy: &EvidencePolicy,
    operational_policy: &OperationalPolicy,
    presentation_policy: &PresentationPolicy,
    safety_invariants: &SafetyInvariants,
) -> Result<PolicyFingerprints> {
    let effective = EffectiveConfigurationIdentity {
        evidence_policy,
        operational_policy,
        presentation_policy,
        safety_invariants,
    };
    let evidence = EvidencePolicyIdentity {
        evidence_policy,
        source_mutation: safety_invariants.source_mutation,
        apply_enabled: safety_invariants.apply_enabled,
        artifact_validation_required: safety_invariants.artifact_validation_required,
        current_stability_required_for_exact_groups: safety_invariants
            .current_stability_required_for_exact_groups,
    };
    Ok(PolicyFingerprints {
        effective_configuration: digest(&effective)?,
        evidence_policy: digest(&evidence)?,
    })
}

fn digest<T: Serialize>(value: &T) -> Result<PolicyDigest> {
    let canonical = serde_json::to_vec(value).context("failed to canonicalize policy")?;
    Ok(PolicyDigest {
        algorithm: "blake3-256".to_owned(),
        value: blake3::hash(&canonical).to_hex().to_string(),
    })
}

fn locked_safety_invariants() -> SafetyInvariants {
    SafetyInvariants {
        source_mutation: false,
        apply_enabled: false,
        shell_execution: false,
        artifact_validation_required: true,
        atomic_artifact_commit_required: true,
        current_stability_required_for_exact_groups: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn evidence() -> EvidencePolicy {
        EvidencePolicy {
            follow_symlinks: false,
            include_hidden: false,
            cross_filesystems: false,
            probe_media: true,
            exact_grouping_profile: ExactGroupingProfile::SizeAndBlake3_256,
            observation_stability: StabilityRequirement::Required,
            unstable_observations: UnstableObservationPolicy::Exclude,
            maximum_observation_attempts: 2,
        }
    }

    #[test]
    fn presentation_changes_do_not_change_evidence_fingerprint() {
        let human = build_effective_policy(
            evidence(),
            PathBuf::from("/tmp/state").as_path(),
            OutputFormat::Human,
            BTreeMap::new(),
        )
        .expect("human policy");
        let json = build_effective_policy(
            evidence(),
            PathBuf::from("/tmp/state").as_path(),
            OutputFormat::Json,
            BTreeMap::new(),
        )
        .expect("JSON policy");

        assert_eq!(
            human.fingerprints.evidence_policy,
            json.fingerprints.evidence_policy
        );
        assert_ne!(
            human.fingerprints.effective_configuration,
            json.fingerprints.effective_configuration
        );
    }

    #[test]
    fn provenance_does_not_change_semantic_fingerprints() {
        let mut first_provenance = BTreeMap::new();
        first_provenance.insert(
            "presentation_policy.output_format".to_owned(),
            PolicyProvenance {
                source: PolicySourceKind::CompiledDefault,
                detail: "default".to_owned(),
                path: None,
            },
        );
        let mut second_provenance = first_provenance.clone();
        second_provenance
            .get_mut("presentation_policy.output_format")
            .expect("provenance")
            .source = PolicySourceKind::CommandLine;

        let first = build_effective_policy(
            evidence(),
            Path::new("/tmp/state"),
            OutputFormat::Human,
            first_provenance,
        )
        .expect("first policy");
        let second = build_effective_policy(
            evidence(),
            Path::new("/tmp/state"),
            OutputFormat::Human,
            second_provenance,
        )
        .expect("second policy");
        assert_eq!(first.fingerprints, second.fingerprints);
    }

    #[test]
    fn evidence_changes_change_the_evidence_fingerprint() {
        let first = build_effective_policy(
            evidence(),
            Path::new("/tmp/state"),
            OutputFormat::Human,
            BTreeMap::new(),
        )
        .expect("first policy");
        let mut changed = evidence();
        changed.include_hidden = true;
        let second = build_effective_policy(
            changed,
            Path::new("/tmp/state"),
            OutputFormat::Human,
            BTreeMap::new(),
        )
        .expect("second policy");
        assert_ne!(
            first.fingerprints.evidence_policy,
            second.fingerprints.evidence_policy
        );
    }

    #[test]
    fn fingerprint_mismatch_fails_closed() {
        let mut policy = build_effective_policy(
            evidence(),
            Path::new("/tmp/state"),
            OutputFormat::Human,
            BTreeMap::new(),
        )
        .expect("policy");
        policy.fingerprints.evidence_policy.value = "0".repeat(64);
        assert!(validate_fingerprints(&policy).is_err());
    }
}
