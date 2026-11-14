//! Hermetic SDK fixture, not a model or a production explanation provider.
//! `setup DIR MODE` authors explicit local pins; `serve MODE` reads only stdin.
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Result, bail, ensure};
use optiflow::domain::NativePath;
use optiflow::explanation::runtime::{
    CONFIG_SCHEMA, PROFILE_SCHEMA, Profile, RuntimeConfiguration,
};
use optiflow::explanation::{self, Candidate, DeclaredIdentity, Note, Packet};
use optiflow::extensions::*;

const CAPABILITY: &str = "optiflow/report/evidence-explanation-runtime";
const EXTENSION: &str = "example.hermetic-explanation";

fn write_new(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    use std::fs::OpenOptions;
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(&bytes)?;
    Ok(())
}

fn setup(directory: &Path, mode: &str) -> Result<()> {
    ensure!(
        directory.is_absolute() && directory.is_dir(),
        "existing absolute directory required"
    );
    ensure!(
        [
            "valid",
            "disabled",
            "unknown-citation",
            "identity-change",
            "malformed",
            "timeout",
            "cancel",
            "unavailable",
            "oversize"
        ]
        .contains(&mode),
        "unknown fixture mode"
    );
    let template =
        "Synthetic fixture only. Cite exact packet facts; do not authorize actions.".to_owned();
    let mut profile = Profile {
        schema: PROFILE_SCHEMA.into(),
        fingerprint: Digest::blake3(b""),
        provider_id: "example.hermetic-explanation".into(),
        model_id: "fake-no-model".into(),
        runtime_revision: "fixture-v1".into(),
        template_fingerprint: Digest::blake3(template.as_bytes()),
        template,
        output_schema_digest: Digest::blake3(include_bytes!(
            "../schemas/explanation-candidate-v1.schema.json"
        )),
        backend: "local_process_sdk_v1".into(),
        local_only: true,
    };
    profile.fingerprint = fingerprint(&profile)?;
    profile.validate()?;
    let document: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../schemas/evidence-explanation-runtime-config-v1.schema.json"
    ))?;
    let effects = BTreeSet::from([ExtensionEffect::Subprocess]);
    let manifest = ExtensionManifestV1 {
        schema: EXTENSION_MANIFEST_SCHEMA.into(),
        extension_id: EXTENSION.into(),
        version: "1.0.0".into(),
        publisher: Publisher {
            id: "example.publisher".into(),
            display_name: "Hermetic test fixture".into(),
        },
        sdk_contract: EXTENSION_SDK_CONTRACT.into(),
        configuration_schemas: vec![ConfigurationSchemaDeclaration {
            schema_id: CONFIG_SCHEMA.into(),
            schema_digest: fingerprint(&document)?,
            document,
        }],
        capabilities: vec![CapabilityDeclaration {
            capability_id: CAPABILITY.into(),
            capability_version: "1.0.0".into(),
            kind: CapabilityKind::ReportProvider,
            configuration_schema: CONFIG_SCHEMA.into(),
            accepts: vec![explanation::PACKET_SCHEMA.into()],
            produces: vec![explanation::CANDIDATE_SCHEMA.into()],
            preconditions: vec![],
            postconditions: vec![],
            determinism: Determinism::Deterministic,
            cacheability: Cacheability::NotCacheable,
            coverage: CoverageDeclaration {
                mode: DeclaredCoverage::PartialByDesign,
                dimensions: vec!["synthetic-fixture-only".into()],
            },
            resource_needs: ResourceNeeds {
                cpu_threads: 1,
                memory_bytes: 16_777_216,
                temporary_storage_bytes: 0,
                gpu: false,
            },
            locality: LocalityDeclaration {
                local_only: true,
                network_hosts: vec![],
            },
            content_changes: false,
            effects: effects.clone(),
        }],
        execution_modes: vec![ExecutionModeDeclaration::Process {
            name: "json-stdio-v1".into(),
            protocol: EXTENSION_INVOCATION_SCHEMA.into(),
            arguments: vec!["serve".into(), mode.into()],
            limits: ProcessLimits {
                timeout_ms: if mode == "timeout" { 80 } else { 5_000 },
                max_stdin_bytes: 65_536,
                max_stdout_bytes: 65_536,
                max_stderr_bytes: 1_024,
            },
        }],
        requested_effects: effects.clone(),
        replacement: ReplacementDeclaration {
            replaces: vec![],
            fallbacks_for: vec![],
        },
        observer_hooks: vec![],
    };
    write_new(&directory.join("manifest.json"), &manifest)?;
    let values = serde_json::to_value(RuntimeConfiguration {
        enabled: mode != "disabled",
        profile: profile.clone(),
    })?;
    let executable = std::env::current_exe()?;
    let mut lock = ExtensionLockV1 {
        schema: EXTENSION_LOCK_SCHEMA.into(),
        lock_id: format!("hermetic-{mode}"),
        extension_id: EXTENSION.into(),
        extension_version: "1.0.0".into(),
        manifest_digest: Digest::blake3(&std::fs::read(directory.join("manifest.json"))?),
        enabled: true,
        trust: TrustMode::TrustedProcess,
        granted_effects: effects,
        grants_digest: Digest::blake3(b""),
        precedence: 1,
        allowed_replacements: vec![],
        allowed_fallbacks: vec![],
        selected_execution_mode: "json-stdio-v1".into(),
        process: Some(LockedProcess {
            executable: NativePath::from_path(&executable),
            executable_digest: Digest::blake3(&std::fs::read(&executable)?),
            working_directory: NativePath::from_path(directory),
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
    write_new(&directory.join("lock.json"), &lock)?;
    write_new(&directory.join("profile.json"), &profile)?;
    Ok(())
}

fn serve(mode: &str) -> Result<()> {
    // The SDK clears the complete inherited environment, including fixture canaries.
    ensure!(
        std::env::vars_os().next().is_none(),
        "fixture environment was not empty"
    );
    let mut bytes = Vec::new();
    std::io::stdin().take(65_537).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 65_536, "fixture input exceeded bound");
    let invocation: ExtensionInvocationV1 = serde_json::from_slice(&bytes)?;
    ensure!(
        invocation.schema == EXTENSION_INVOCATION_SCHEMA
            && invocation.extension.extension_id == EXTENSION
            && invocation.capability_id == CAPABILITY,
        "wrong fixture invocation"
    );
    ensure!(
        invocation.input_artifacts.len() == 1
            && invocation.input_evidence.len() == 1
            && invocation.checkpoint_references.is_empty(),
        "normalized packet only"
    );
    let packet: Packet = serde_json::from_value(invocation.input_evidence[0].value.clone())?;
    packet.validate()?;
    ensure!(
        invocation.input_artifacts[0].artifact_id == "normalized-records"
            && invocation.input_artifacts[0].schema == explanation::PACKET_SCHEMA,
        "raw artifact not allowed"
    );
    let config: RuntimeConfiguration =
        serde_json::from_value(invocation.configuration.values.clone())?;
    config.profile.validate()?;
    ensure!(
        config.enabled && mode != "disabled",
        "disabled fixture must never execute"
    );
    if mode == "timeout" || mode == "cancel" {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    if mode == "malformed" {
        std::io::stdout().write_all(b"{\"unexpected\":true}")?;
        return Ok(());
    }
    if mode == "oversize" {
        std::io::stdout().write_all(&vec![b'x'; 65_537])?;
        return Ok(());
    }
    let p = config.profile;
    let mut candidate = Candidate {
        schema: explanation::CANDIDATE_SCHEMA.into(),
        packet_fingerprint: packet.packet_fingerprint,
        declared_identity: DeclaredIdentity {
            provider_id: p.provider_id,
            model_id: p.model_id,
            runtime_revision: p.runtime_revision,
            profile_fingerprint: p.fingerprint,
            template_fingerprint: p.template_fingerprint,
            output_schema_digest: p.output_schema_digest,
        },
        observed_fact_ids: packet.facts.iter().map(|f| f.fact_id.clone()).collect(),
        suggestions: vec![Note {
            text: "Review the recorded duplicate candidates; no action is authorized.".into(),
            fact_ids: vec!["recorded_plan_actions".into()],
        }],
        questions: vec![Note {
            text: "Have source bytes changed since the pinned inventory?".into(),
            fact_ids: vec!["recorded_files".into()],
        }],
    };
    if mode == "unknown-citation" {
        candidate.suggestions[0].fact_ids = vec!["invented-fact".into()];
    }
    if mode == "identity-change" {
        candidate.declared_identity.model_id = "another-fake-model".into();
    }
    let value = serde_json::to_value(candidate)?;
    let unavailable = mode == "unavailable";
    let result = ExtensionResultV1 {
        schema: EXTENSION_RESULT_SCHEMA.into(),
        invocation_id: invocation.invocation_id.clone(),
        extension: invocation.extension.clone(),
        capability_id: invocation.capability_id.clone(),
        outcome: if unavailable {
            ExtensionOutcome::Unavailable
        } else {
            ExtensionOutcome::Produced
        },
        coverage: ExtensionCoverage {
            status: ExtensionCoverageStatus::Partial,
            limitations: vec![
                "synthetic fixture; no model execution or semantic qualification".into(),
            ],
        },
        consumed_artifacts: vec!["normalized-records".into()],
        evidence: vec![],
        policy_contributions: vec![],
        plan_operations: vec![],
        validations: vec![],
        reports: if unavailable {
            vec![]
        } else {
            vec![ReportContribution {
                report_id: "synthetic-candidate".into(),
                schema: explanation::CANDIDATE_SCHEMA.into(),
                fingerprint: fingerprint(&value)?,
                value,
            }]
        },
        diagnostics: vec![ExtensionDiagnostic {
            code: "fake_environment_empty".into(),
            message: "Synthetic fixture observed an empty inherited environment.".into(),
            retryable: false,
        }],
        progress: vec![],
        provenance: ExtensionProvenance {
            extension: invocation.extension,
            capability_id: invocation.capability_id,
            capability_version: "1.0.0".into(),
            configuration_fingerprint: invocation.configuration.fingerprint,
            input_fingerprints: invocation
                .input_artifacts
                .iter()
                .map(|a| a.content_digest.clone())
                .collect(),
            authorization_lock_id: invocation.authorization.lock_id,
            authorization_lock_digest: invocation.authorization.lock_digest,
            grants_digest: invocation.authorization.grants_digest,
            execution_mode: "json-stdio-v1".into(),
        },
        observed_effects: BTreeSet::new(),
        checkpoint_references: vec![],
    };
    std::io::stdout().write_all(&serde_json::to_vec(&result)?)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("hash") if args.len() == 3 => println!(
            "{}",
            serde_json::to_string(&Digest::blake3(&std::fs::read(&args[2])?))?
        ),
        Some("setup") if args.len() == 4 => setup(Path::new(&args[2]), &args[3])?,
        Some("serve") if args.len() == 3 => serve(&args[2])?,
        _ => {
            bail!("usage: fake_explanation hash FILE | setup ABSOLUTE_DIRECTORY MODE | serve MODE")
        }
    }
    Ok(())
}
