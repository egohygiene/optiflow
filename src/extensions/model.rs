use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::NativePath;

pub const EXTENSION_SDK_CONTRACT: &str = "optiflow.extension-sdk.v1";
pub const EXTENSION_MANIFEST_SCHEMA: &str = "optiflow.extension-manifest.v1";
pub const EXTENSION_LOCK_SCHEMA: &str = "optiflow.extension-lock.v1";
pub const EXTENSION_INVOCATION_SCHEMA: &str = "optiflow.extension-invocation.v1";
pub const EXTENSION_RESULT_SCHEMA: &str = "optiflow.extension-result.v1";
pub const MAX_EXTENSION_TIMEOUT_MS: u64 = 10 * 60 * 1000;
pub const MAX_EXTENSION_STDIN_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_EXTENSION_STDOUT_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_EXTENSION_STDERR_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Digest {
    pub algorithm: String,
    pub value: String,
}

impl Digest {
    pub fn blake3(bytes: &[u8]) -> Self {
        Self {
            algorithm: "blake3-256".to_owned(),
            value: blake3::hash(bytes).to_hex().to_string(),
        }
    }
}

pub fn fingerprint<T: Serialize>(value: &T) -> Result<Digest> {
    let canonical = serde_json::to_vec(value).context("failed to canonicalize extension value")?;
    Ok(Digest::blake3(&canonical))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Publisher {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Inspector,
    Analyzer,
    PolicyContributor,
    Planner,
    Validator,
    ReportProvider,
    LifecycleObserver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionEffect {
    ArtifactRead,
    IsolatedWorkspaceWrite,
    Subprocess,
    Network,
    Gpu,
    SourceMutation,
    Destructive,
    Sign,
    Publish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Determinism {
    Deterministic,
    BestEffort,
    NonDeterministic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cacheability {
    Cacheable,
    NotCacheable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredCoverage {
    Complete,
    PartialByDesign,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageDeclaration {
    pub mode: DeclaredCoverage,
    pub dimensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceNeeds {
    pub cpu_threads: u32,
    pub memory_bytes: u64,
    pub temporary_storage_bytes: u64,
    pub gpu: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalityDeclaration {
    pub local_only: bool,
    pub network_hosts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigurationSchemaDeclaration {
    pub schema_id: String,
    pub schema_digest: Digest,
    pub document: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDeclaration {
    pub capability_id: String,
    pub capability_version: String,
    pub kind: CapabilityKind,
    pub configuration_schema: String,
    pub accepts: Vec<String>,
    pub produces: Vec<String>,
    pub preconditions: Vec<String>,
    pub postconditions: Vec<String>,
    pub determinism: Determinism,
    pub cacheability: Cacheability,
    pub coverage: CoverageDeclaration,
    pub resource_needs: ResourceNeeds,
    pub locality: LocalityDeclaration,
    pub content_changes: bool,
    pub effects: BTreeSet<ExtensionEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessLimits {
    pub timeout_ms: u64,
    pub max_stdin_bytes: u64,
    pub max_stdout_bytes: u64,
    pub max_stderr_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionModeDeclaration {
    Embedded {
        name: String,
        entrypoint: String,
    },
    Process {
        name: String,
        protocol: String,
        arguments: Vec<String>,
        limits: ProcessLimits,
    },
}

impl ExecutionModeDeclaration {
    pub fn name(&self) -> &str {
        match self {
            Self::Embedded { name, .. } | Self::Process { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacementDeclaration {
    pub replaces: Vec<String>,
    pub fallbacks_for: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObserverHookDeclaration {
    pub hook_id: String,
    pub lifecycle_points: BTreeSet<LifecyclePhase>,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionManifestV1 {
    pub schema: String,
    pub extension_id: String,
    pub version: String,
    pub publisher: Publisher,
    pub sdk_contract: String,
    pub configuration_schemas: Vec<ConfigurationSchemaDeclaration>,
    pub capabilities: Vec<CapabilityDeclaration>,
    pub execution_modes: Vec<ExecutionModeDeclaration>,
    pub requested_effects: BTreeSet<ExtensionEffect>,
    pub replacement: ReplacementDeclaration,
    pub observer_hooks: Vec<ObserverHookDeclaration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustMode {
    TrustedEmbedded,
    TrustedProcess,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedConfiguration {
    pub values: Value,
    pub fingerprint: Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedProcess {
    pub executable: NativePath,
    pub executable_digest: Digest,
    pub working_directory: NativePath,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionLockV1 {
    pub schema: String,
    pub lock_id: String,
    pub extension_id: String,
    pub extension_version: String,
    pub manifest_digest: Digest,
    pub enabled: bool,
    pub trust: TrustMode,
    pub granted_effects: BTreeSet<ExtensionEffect>,
    pub grants_digest: Digest,
    pub precedence: i32,
    pub allowed_replacements: Vec<String>,
    pub allowed_fallbacks: Vec<String>,
    pub selected_execution_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<LockedProcess>,
    pub configurations: BTreeMap<String, LockedConfiguration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GrantIdentity<'a> {
    pub enabled: bool,
    pub trust: TrustMode,
    pub granted_effects: &'a BTreeSet<ExtensionEffect>,
    pub precedence: i32,
    pub allowed_replacements: &'a [String],
    pub allowed_fallbacks: &'a [String],
    pub selected_execution_mode: &'a str,
}

impl ExtensionLockV1 {
    pub fn expected_grants_digest(&self) -> Result<Digest> {
        fingerprint(&GrantIdentity {
            enabled: self.enabled,
            trust: self.trust,
            granted_effects: &self.granted_effects,
            precedence: self.precedence,
            allowed_replacements: &self.allowed_replacements,
            allowed_fallbacks: &self.allowed_fallbacks,
            selected_execution_mode: &self.selected_execution_mode,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionIdentity {
    pub extension_id: String,
    pub version: String,
    pub manifest_digest: Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImmutableArtifactReference {
    pub artifact_id: String,
    pub schema: String,
    pub content_digest: Digest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvocationConfiguration {
    pub schema_id: String,
    pub values: Value,
    pub fingerprint: Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationEvidence {
    pub lock_id: String,
    pub lock_digest: Digest,
    pub grants_digest: Digest,
    pub granted_effects: BTreeSet<ExtensionEffect>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionInvocationV1 {
    pub schema: String,
    pub invocation_id: String,
    pub extension: ExtensionIdentity,
    pub capability_id: String,
    pub capability_kind: CapabilityKind,
    pub input_artifacts: Vec<ImmutableArtifactReference>,
    pub input_evidence: Vec<EvidenceContribution>,
    pub configuration: InvocationConfiguration,
    pub authorization: AuthorizationEvidence,
    pub limits: ProcessLimits,
    pub cancellation_id: String,
    pub checkpoint_references: Vec<ImmutableArtifactReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle_event: Option<LifecycleEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionOutcome {
    Produced,
    Reused,
    Unavailable,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionCoverageStatus {
    Complete,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionCoverage {
    pub status: ExtensionCoverageStatus,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceContribution {
    pub evidence_id: String,
    pub schema: String,
    pub value: Value,
    pub fingerprint: Digest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyContribution {
    pub policy_id: String,
    pub schema: String,
    pub value: Value,
    pub fingerprint: Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanPreconditions {
    pub observation_revalidation: bool,
    pub full_hash: bool,
    pub byte_confirmation: bool,
    pub plan_identity: bool,
    pub explicit_authorization: bool,
    pub provenance: bool,
}

impl PlanPreconditions {
    pub const fn required() -> Self {
        Self {
            observation_revalidation: true,
            full_hash: true,
            byte_confirmation: true,
            plan_identity: true,
            explicit_authorization: true,
            provenance: true,
        }
    }

    pub const fn is_complete(&self) -> bool {
        self.observation_revalidation
            && self.full_hash
            && self.byte_confirmation
            && self.plan_identity
            && self.explicit_authorization
            && self.provenance
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanOperationContribution {
    pub operation_id: String,
    pub action_kind: String,
    pub input_artifacts: Vec<ImmutableArtifactReference>,
    pub expected_output_schemas: Vec<String>,
    pub preconditions: PlanPreconditions,
    pub mutates_source: bool,
    pub requires_core_execution: bool,
    pub fingerprint: Digest,
}

#[derive(Serialize)]
struct PlanOperationIdentity<'a> {
    operation_id: &'a str,
    action_kind: &'a str,
    input_artifacts: &'a [ImmutableArtifactReference],
    expected_output_schemas: &'a [String],
    preconditions: &'a PlanPreconditions,
    mutates_source: bool,
    requires_core_execution: bool,
}

impl PlanOperationContribution {
    pub fn expected_fingerprint(&self) -> Result<Digest> {
        fingerprint(&PlanOperationIdentity {
            operation_id: &self.operation_id,
            action_kind: &self.action_kind,
            input_artifacts: &self.input_artifacts,
            expected_output_schemas: &self.expected_output_schemas,
            preconditions: &self.preconditions,
            mutates_source: self.mutates_source,
            requires_core_execution: self.requires_core_execution,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Passed,
    Warning,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationContribution {
    pub validator_id: String,
    pub validator_version: String,
    pub status: ValidationStatus,
    pub evidence: EvidenceContribution,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportContribution {
    pub report_id: String,
    pub schema: String,
    pub value: Value,
    pub fingerprint: Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionDiagnostic {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub sequence: u64,
    pub completed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    pub unit: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LifecyclePhase {
    Discover,
    Inspect,
    Plan,
    Authorize,
    Execute,
    Validate,
    CommitEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleEvent {
    pub event_id: String,
    pub invocation_id: String,
    pub sequence: u64,
    pub phase: LifecyclePhase,
    pub state: String,
    pub artifact_references: Vec<ImmutableArtifactReference>,
    pub diagnostics: Vec<ExtensionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionProvenance {
    pub extension: ExtensionIdentity,
    pub capability_id: String,
    pub capability_version: String,
    pub configuration_fingerprint: Digest,
    pub input_fingerprints: Vec<Digest>,
    pub authorization_lock_id: String,
    pub authorization_lock_digest: Digest,
    pub grants_digest: Digest,
    pub execution_mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionResultV1 {
    pub schema: String,
    pub invocation_id: String,
    pub extension: ExtensionIdentity,
    pub capability_id: String,
    pub outcome: ExtensionOutcome,
    pub coverage: ExtensionCoverage,
    pub consumed_artifacts: Vec<String>,
    pub evidence: Vec<EvidenceContribution>,
    pub policy_contributions: Vec<PolicyContribution>,
    pub plan_operations: Vec<PlanOperationContribution>,
    pub validations: Vec<ValidationContribution>,
    pub reports: Vec<ReportContribution>,
    pub diagnostics: Vec<ExtensionDiagnostic>,
    pub progress: Vec<ProgressEvent>,
    pub provenance: ExtensionProvenance,
    pub observed_effects: BTreeSet<ExtensionEffect>,
    pub checkpoint_references: Vec<ImmutableArtifactReference>,
}
