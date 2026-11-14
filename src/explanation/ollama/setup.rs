//! Reviewable local setup documents. No service query, model access/download,
//! inference, installation or provider registration is performed here.
use super::*;
use crate::signals::SignalState;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub enabled: bool,
    pub executable: PathBuf,
    pub working_directory: PathBuf,
    pub deployment: Deployment,
    pub template: String,
}
impl Request {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= 32_768, "local setup request exceeds 32 KiB");
        let raw: serde_json::Value = serde_json::from_slice(bytes)?;
        contracts::validate(Contract::ExplanationLocalSetupRequest, &raw)?;
        Ok(serde_json::from_slice(bytes)?)
    }
}
pub struct Prepared {
    pub profile: Profile,
    pub configuration: Configuration,
    pub manifest: ExtensionManifestV1,
    pub lock: ExtensionLockV1,
    request: Request,
}
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    ensure!(bytes.len() <= 131_072, "local setup member exceeds 128 KiB");
    Ok(bytes)
}
pub fn prepare(request: &Request, signals: &SignalState) -> Result<Prepared> {
    ensure!(
        request.schema == "optiflow.local-explanation-setup-request.v1" && !signals.is_cancelled(),
        "invalid/cancelled local setup request"
    );
    crate::candidate_artifact::validate_private_root(&request.working_directory)?;
    let guard = || {
        ensure!(!signals.is_cancelled(), "local setup cancelled");
        Ok(())
    };
    let executable = crate::video_io::BoundedFileObservation::open_guarded(
        &request.executable,
        134_217_728,
        &guard,
    )?;
    ensure!(
        executable.snapshot().mode & 0o111 != 0
            && executable.snapshot().mode & 0o6000 == 0
            && executable.snapshot().link_count == 1,
        "explicit executable has unsupported mode/link identity"
    );
    let d = &request.deployment;
    let mut profile = Profile {
        schema: runtime::PROFILE_SCHEMA.into(),
        fingerprint: Digest::blake3(b""),
        provider_id: "optiflow.local-ollama-sdk-v1".into(),
        model_id: format!("{}/sha256:{}", d.model_name, d.model_sha256),
        runtime_revision: d.profile_runtime_revision(),
        template: request.template.clone(),
        template_fingerprint: Digest::blake3(request.template.as_bytes()),
        output_schema_digest: Digest::blake3(include_bytes!(
            "../../../schemas/explanation-candidate-v1.schema.json"
        )),
        backend: "local_process_sdk_v1".into(),
        local_only: true,
    };
    profile.fingerprint = fingerprint(&profile)?;
    let configuration = Configuration {
        enabled: request.enabled,
        profile: profile.clone(),
        deployment: d.clone(),
    };
    configuration.validate()?;
    let config_document = contracts::schema(Contract::ExplanationOllamaConfig)?;
    let effects = BTreeSet::from([ExtensionEffect::Subprocess, ExtensionEffect::Network]);
    let manifest = ExtensionManifestV1 {
        schema: EXTENSION_MANIFEST_SCHEMA.into(),
        extension_id: "optiflow.local-evidence-explanation".into(),
        version: "1.0.0".into(),
        publisher: Publisher {
            id: "egohygiene".into(),
            display_name: "Ego Hygiene".into(),
        },
        sdk_contract: EXTENSION_SDK_CONTRACT.into(),
        configuration_schemas: vec![ConfigurationSchemaDeclaration {
            schema_id: CONFIG_SCHEMA.into(),
            schema_digest: fingerprint(&config_document)?,
            document: config_document,
        }],
        capabilities: vec![CapabilityDeclaration {
            capability_id: "optiflow/report/evidence-explanation-runtime".into(),
            capability_version: "1.0.0".into(),
            kind: CapabilityKind::ReportProvider,
            configuration_schema: CONFIG_SCHEMA.into(),
            accepts: vec![PACKET_SCHEMA.into()],
            produces: vec![CANDIDATE_SCHEMA.into()],
            preconditions: vec!["exact_normalized_packet_and_selected_local_model".into()],
            postconditions: vec!["host_validated_cited_candidate_only".into()],
            determinism: Determinism::NonDeterministic,
            cacheability: Cacheability::NotCacheable,
            coverage: CoverageDeclaration {
                mode: DeclaredCoverage::PartialByDesign,
                dimensions: vec!["normalized_record_facts_only".into()],
            },
            resource_needs: ResourceNeeds {
                cpu_threads: 1,
                memory_bytes: 33_554_432,
                temporary_storage_bytes: 0,
                gpu: false,
            },
            locality: LocalityDeclaration {
                local_only: false,
                network_hosts: vec![d.endpoint.to_string()],
            },
            content_changes: false,
            effects: effects.clone(),
        }],
        execution_modes: vec![ExecutionModeDeclaration::Process {
            name: "ollama-sdk-v1".into(),
            protocol: EXTENSION_INVOCATION_SCHEMA.into(),
            arguments: vec![MARKER.into()],
            limits: ProcessLimits {
                timeout_ms: 30_000,
                max_stdin_bytes: 131_072,
                max_stdout_bytes: 131_072,
                max_stderr_bytes: 4096,
            },
        }],
        requested_effects: effects.clone(),
        replacement: ReplacementDeclaration {
            replaces: vec![],
            fallbacks_for: vec![],
        },
        observer_hooks: vec![],
    };
    contracts::validate(Contract::ExtensionManifest, &manifest)?;
    let values = serde_json::to_value(&configuration)?;
    let mut lock = ExtensionLockV1 {
        schema: EXTENSION_LOCK_SCHEMA.into(),
        lock_id: "optiflow-local-ollama-v1".into(),
        extension_id: manifest.extension_id.clone(),
        extension_version: manifest.version.clone(),
        manifest_digest: Digest::blake3(&encode(&manifest)?),
        enabled: request.enabled,
        trust: TrustMode::TrustedProcess,
        granted_effects: effects,
        grants_digest: Digest::blake3(b""),
        precedence: 0,
        allowed_replacements: vec![],
        allowed_fallbacks: vec![],
        selected_execution_mode: "ollama-sdk-v1".into(),
        process: Some(LockedProcess {
            executable: crate::domain::NativePath::from_path(&request.executable),
            executable_digest: Digest {
                algorithm: executable.digest().algorithm.clone(),
                value: executable.digest().value.clone(),
            },
            working_directory: crate::domain::NativePath::from_path(&request.working_directory),
        }),
        configurations: BTreeMap::from([(
            CONFIG_SCHEMA.into(),
            LockedConfiguration {
                fingerprint: fingerprint(&values)?,
                values,
            },
        )]),
    };
    lock.grants_digest = lock.expected_grants_digest()?;
    contracts::validate(Contract::ExtensionLock, &lock)?;
    executable.verify_current_guarded(&guard)?;
    Ok(Prepared {
        profile,
        configuration,
        manifest,
        lock,
        request: request.clone(),
    })
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub name: String,
    pub byte_count: u64,
    pub blake3: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupReport {
    pub schema: String,
    pub enabled: bool,
    pub profile_fingerprint: Digest,
    pub members: Vec<Member>,
    pub process_invocation_observed: bool,
    pub model_execution_qualified: bool,
    pub offline_enforcement_verified: bool,
}
const FILES: [&str; 5] = [
    "profile.json",
    "configuration.json",
    "manifest.json",
    "lock.json",
    "request.json",
];
/// New files in an existing empty private directory, followed by a last marker.
/// Failed publication is incomplete and retained; no automatic cleanup/adoption.
pub fn write(request: &Request, directory: &Path, signals: &SignalState) -> Result<SetupReport> {
    let root = crate::candidate_artifact::open_directory(directory)?;
    crate::candidate_artifact::require_private_root(&root)?;
    ensure!(
        std::fs::read_dir(directory)?.next().is_none(),
        "local setup output directory must be empty"
    );
    let prepared = prepare(request, signals)?;
    let data = [
        encode(&prepared.profile)?,
        encode(&prepared.configuration)?,
        encode(&prepared.manifest)?,
        encode(&prepared.lock)?,
        encode(&prepared.request)?,
    ];
    let mut members = Vec::new();
    for (name, bytes) in FILES.into_iter().zip(data) {
        ensure!(
            !signals.is_cancelled(),
            "setup publication cancelled; retain incomplete directory"
        );
        crate::candidate_artifact::write_new(&root, name, &bytes)?;
        members.push(Member {
            name: name.into(),
            byte_count: bytes.len() as u64,
            blake3: blake3::hash(&bytes).to_hex().to_string(),
        });
    }
    let report = SetupReport {
        schema: "optiflow.local-explanation-setup-report.v1".into(),
        enabled: request.enabled,
        profile_fingerprint: prepared.profile.fingerprint,
        members,
        process_invocation_observed: false,
        model_execution_qualified: false,
        offline_enforcement_verified: false,
    };
    contracts::validate(Contract::ExplanationLocalSetupReport, &report)?;
    let _catalog = ExtensionCatalog::load(
        &[directory.join("manifest.json")],
        &[directory.join("lock.json")],
    )?;
    crate::candidate_artifact::write_new(&root, "setup.json", &encode(&report)?)?;
    root.sync_all()?;
    inspect(directory, signals)
}
pub fn inspect(directory: &Path, signals: &SignalState) -> Result<SetupReport> {
    let root = crate::candidate_artifact::open_directory(directory)?;
    crate::candidate_artifact::require_private_root(&root)?;
    let raw = crate::candidate_artifact::read_member(&root, "setup.json", 16_384)?;
    let value: serde_json::Value = serde_json::from_slice(&raw)?;
    contracts::validate(Contract::ExplanationLocalSetupReport, &value)?;
    let report: SetupReport = serde_json::from_slice(&raw)?;
    ensure!(
        report.schema == "optiflow.local-explanation-setup-report.v1"
            && report.members.len() == FILES.len()
            && !report.process_invocation_observed
            && !report.model_execution_qualified
            && !report.offline_enforcement_verified,
        "local setup marker identity differs"
    );
    for (expected, m) in FILES.into_iter().zip(&report.members) {
        ensure!(
            !signals.is_cancelled() && m.name == expected,
            "setup member order/cancellation differs"
        );
        let bytes = crate::candidate_artifact::read_member(&root, &m.name, 131_072)?;
        ensure!(
            bytes.len() as u64 == m.byte_count
                && blake3::hash(&bytes).to_hex().as_str() == m.blake3,
            "local setup member changed"
        );
    }
    let request = Request::from_json_slice(&crate::candidate_artifact::read_member(
        &root,
        "request.json",
        32_768,
    )?)?;
    let fresh = prepare(&request, signals)?;
    ensure!(
        fresh.profile.fingerprint == report.profile_fingerprint
            && request.enabled == report.enabled,
        "local setup profile basis changed"
    );
    let expected = [
        encode(&fresh.profile)?,
        encode(&fresh.configuration)?,
        encode(&fresh.manifest)?,
        encode(&fresh.lock)?,
        encode(&fresh.request)?,
    ];
    for (bytes, member) in expected.iter().zip(&report.members) {
        ensure!(
            bytes.len() as u64 == member.byte_count
                && blake3::hash(bytes).to_hex().as_str() == member.blake3,
            "local setup executable/configuration basis changed"
        );
    }
    ensure!(
        std::fs::read_dir(directory)?
            .take(7)
            .collect::<std::io::Result<Vec<_>>>()?
            .len()
            == 6,
        "local setup contains unknown members"
    );
    Ok(report)
}
