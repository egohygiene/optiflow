//! Minimal typed embedded providers for the four primary extension roles.

use std::collections::BTreeSet;

use anyhow::Result;
use optiflow::extensions::{
    Analyzer, Cacheability, CapabilityDeclaration, CapabilityKind, ConfigurationSchemaDeclaration,
    CoverageDeclaration, DeclaredCoverage, Determinism, Digest, EXTENSION_MANIFEST_SCHEMA,
    EXTENSION_RESULT_SCHEMA, EXTENSION_SDK_CONTRACT, EvidenceContribution,
    ExecutionModeDeclaration, ExtensionContext, ExtensionCoverage, ExtensionCoverageStatus,
    ExtensionManifestV1, ExtensionOutcome, ExtensionProvenance, ExtensionResultV1, Inspector,
    LocalityDeclaration, PlanOperationContribution, PlanPreconditions, Planner, Publisher,
    ReplacementDeclaration, ResourceNeeds, ValidationContribution, ValidationStatus, Validator,
    fingerprint,
};
use serde_json::json;

const CAPABILITY_VERSION: &str = "1.0.0";
const CONFIGURATION_SCHEMA: &str = "optiflow.example.embedded-roles.config.v1";
const INPUT_SCHEMA: &str = "optiflow.artifact-reference.v1";
const INSPECTION_SCHEMA: &str = "optiflow.example.inspection.v1";
const ANALYSIS_SCHEMA: &str = "optiflow.example.analysis.v1";
const PLAN_OUTPUT_SCHEMA: &str = "optiflow.example.planned-artifact.v1";
const VALIDATION_SCHEMA: &str = "optiflow.example.validation.v1";

fn main() -> Result<()> {
    let manifest = reference_manifest();
    if std::env::args().nth(1).as_deref() == Some("--print-manifest") {
        println!("{}", serde_json::to_string_pretty(&manifest)?);
        return Ok(());
    }
    optiflow::contracts::validate(optiflow::contracts::Contract::ExtensionManifest, &manifest)?;
    let _typed_providers = (
        ReferenceInspector,
        ReferenceAnalyzer,
        ReferencePlanner,
        ReferenceValidator,
    );
    println!("embedded inspector, analyzer, planner, and validator manifest is valid");
    Ok(())
}

struct ReferenceInspector;

impl Inspector for ReferenceInspector {
    fn inspect(
        &self,
        context: &ExtensionContext<'_>,
    ) -> Result<ExtensionResultV1, optiflow::extensions::ExtensionFailure> {
        let mut result = base_result(context);
        result.evidence.push(evidence(
            "inspection",
            INSPECTION_SCHEMA,
            json!({ "inspected": result.consumed_artifacts.len() }),
        ));
        Ok(result)
    }
}

struct ReferenceAnalyzer;

impl Analyzer for ReferenceAnalyzer {
    fn analyze(
        &self,
        context: &ExtensionContext<'_>,
    ) -> Result<ExtensionResultV1, optiflow::extensions::ExtensionFailure> {
        let mut result = base_result(context);
        result.evidence.push(evidence(
            "analysis",
            ANALYSIS_SCHEMA,
            json!({ "relationship": "example-only" }),
        ));
        Ok(result)
    }
}

struct ReferencePlanner;

impl Planner for ReferencePlanner {
    fn plan(
        &self,
        context: &ExtensionContext<'_>,
    ) -> Result<ExtensionResultV1, optiflow::extensions::ExtensionFailure> {
        let mut result = base_result(context);
        let mut operation = PlanOperationContribution {
            operation_id: "derive-example-artifact".to_owned(),
            action_kind: "derive-new-artifact".to_owned(),
            input_artifacts: context.invocation.input_artifacts.clone(),
            expected_output_schemas: vec![PLAN_OUTPUT_SCHEMA.to_owned()],
            preconditions: PlanPreconditions::required(),
            mutates_source: false,
            requires_core_execution: true,
            fingerprint: Digest::blake3(b"pending"),
        };
        operation.fingerprint = operation.expected_fingerprint().map_err(|error| {
            optiflow::extensions::ExtensionFailure::new(
                "fingerprint_failed",
                error.to_string(),
                false,
            )
        })?;
        result.plan_operations.push(operation);
        Ok(result)
    }
}

struct ReferenceValidator;

impl Validator for ReferenceValidator {
    fn validate(
        &self,
        context: &ExtensionContext<'_>,
    ) -> Result<ExtensionResultV1, optiflow::extensions::ExtensionFailure> {
        let mut result = base_result(context);
        result.validations.push(ValidationContribution {
            validator_id: context.invocation.capability_id.clone(),
            validator_version: CAPABILITY_VERSION.to_owned(),
            status: ValidationStatus::Passed,
            evidence: evidence(
                "validation",
                VALIDATION_SCHEMA,
                json!({ "validated": true }),
            ),
        });
        Ok(result)
    }
}

fn evidence(id: &str, schema: &str, value: serde_json::Value) -> EvidenceContribution {
    EvidenceContribution {
        evidence_id: id.to_owned(),
        schema: schema.to_owned(),
        fingerprint: fingerprint(&value).expect("example evidence is serializable"),
        value,
    }
}

fn base_result(context: &ExtensionContext<'_>) -> ExtensionResultV1 {
    let invocation = context.invocation;
    ExtensionResultV1 {
        schema: EXTENSION_RESULT_SCHEMA.to_owned(),
        invocation_id: invocation.invocation_id.clone(),
        extension: invocation.extension.clone(),
        capability_id: invocation.capability_id.clone(),
        outcome: ExtensionOutcome::Produced,
        coverage: ExtensionCoverage {
            status: ExtensionCoverageStatus::Complete,
            limitations: Vec::new(),
        },
        consumed_artifacts: invocation
            .input_artifacts
            .iter()
            .map(|artifact| artifact.artifact_id.clone())
            .collect(),
        evidence: Vec::new(),
        policy_contributions: Vec::new(),
        plan_operations: Vec::new(),
        validations: Vec::new(),
        reports: Vec::new(),
        diagnostics: Vec::new(),
        progress: Vec::new(),
        provenance: ExtensionProvenance {
            extension: invocation.extension.clone(),
            capability_id: invocation.capability_id.clone(),
            capability_version: CAPABILITY_VERSION.to_owned(),
            configuration_fingerprint: invocation.configuration.fingerprint.clone(),
            input_fingerprints: invocation
                .input_artifacts
                .iter()
                .map(|artifact| artifact.content_digest.clone())
                .collect(),
            authorization_lock_id: invocation.authorization.lock_id.clone(),
            authorization_lock_digest: invocation.authorization.lock_digest.clone(),
            grants_digest: invocation.authorization.grants_digest.clone(),
            execution_mode: "embedded-v1".to_owned(),
        },
        observed_effects: BTreeSet::new(),
        checkpoint_references: invocation.checkpoint_references.clone(),
    }
}

fn reference_manifest() -> ExtensionManifestV1 {
    let configuration_document = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false
    });
    ExtensionManifestV1 {
        schema: EXTENSION_MANIFEST_SCHEMA.to_owned(),
        extension_id: "example.embedded-roles".to_owned(),
        version: CAPABILITY_VERSION.to_owned(),
        publisher: Publisher {
            id: "example.publisher".to_owned(),
            display_name: "Example Publisher".to_owned(),
        },
        sdk_contract: EXTENSION_SDK_CONTRACT.to_owned(),
        configuration_schemas: vec![ConfigurationSchemaDeclaration {
            schema_id: CONFIGURATION_SCHEMA.to_owned(),
            schema_digest: fingerprint(&configuration_document)
                .expect("static configuration schema is serializable"),
            document: configuration_document,
        }],
        capabilities: vec![
            capability(
                "optiflow/inspect/example",
                CapabilityKind::Inspector,
                INSPECTION_SCHEMA,
                false,
            ),
            capability(
                "optiflow/analyze/example",
                CapabilityKind::Analyzer,
                ANALYSIS_SCHEMA,
                false,
            ),
            capability(
                "optiflow/plan/example",
                CapabilityKind::Planner,
                PLAN_OUTPUT_SCHEMA,
                true,
            ),
            capability(
                "optiflow/validate/example",
                CapabilityKind::Validator,
                VALIDATION_SCHEMA,
                false,
            ),
        ],
        execution_modes: vec![ExecutionModeDeclaration::Embedded {
            name: "embedded-v1".to_owned(),
            entrypoint: "embedded_roles".to_owned(),
        }],
        requested_effects: BTreeSet::new(),
        replacement: ReplacementDeclaration {
            replaces: Vec::new(),
            fallbacks_for: Vec::new(),
        },
        observer_hooks: Vec::new(),
    }
}

fn capability(
    capability_id: &str,
    kind: CapabilityKind,
    output_schema: &str,
    content_changes: bool,
) -> CapabilityDeclaration {
    CapabilityDeclaration {
        capability_id: capability_id.to_owned(),
        capability_version: CAPABILITY_VERSION.to_owned(),
        kind,
        configuration_schema: CONFIGURATION_SCHEMA.to_owned(),
        accepts: vec![INPUT_SCHEMA.to_owned()],
        produces: vec![output_schema.to_owned()],
        preconditions: vec!["immutable-input-digests".to_owned()],
        postconditions: vec!["fingerprinted-contribution".to_owned()],
        determinism: Determinism::Deterministic,
        cacheability: Cacheability::Cacheable,
        coverage: CoverageDeclaration {
            mode: DeclaredCoverage::Complete,
            dimensions: Vec::new(),
        },
        resource_needs: ResourceNeeds {
            cpu_threads: 1,
            memory_bytes: 16 * 1024 * 1024,
            temporary_storage_bytes: 0,
            gpu: false,
        },
        locality: LocalityDeclaration {
            local_only: true,
            network_hosts: Vec::new(),
        },
        content_changes,
        effects: BTreeSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::Arc;

    use optiflow::extensions::{
        AuthorizationEvidence, EXTENSION_INVOCATION_SCHEMA, EXTENSION_LOCK_SCHEMA,
        ExtensionCatalog, ExtensionIdentity, ExtensionInvocationV1, ExtensionLockV1,
        ExtensionRegistry, ImmutableArtifactReference, InvocationConfiguration,
        LockedConfiguration, ProcessLimits, TrustMode,
    };

    use super::*;

    fn invocation(
        extension: &optiflow::extensions::LoadedExtension,
        capability_id: &str,
    ) -> ExtensionInvocationV1 {
        let capability = extension.capability(capability_id).unwrap();
        let locked_configuration = extension
            .lock()
            .configurations
            .get(CONFIGURATION_SCHEMA)
            .unwrap();
        ExtensionInvocationV1 {
            schema: EXTENSION_INVOCATION_SCHEMA.to_owned(),
            invocation_id: format!("invoke-{capability_id}"),
            extension: ExtensionIdentity {
                extension_id: extension.manifest().extension_id.clone(),
                version: extension.manifest().version.clone(),
                manifest_digest: extension.manifest_digest().clone(),
            },
            capability_id: capability_id.to_owned(),
            capability_kind: capability.kind,
            input_artifacts: vec![ImmutableArtifactReference {
                artifact_id: "example-input".to_owned(),
                schema: INPUT_SCHEMA.to_owned(),
                content_digest: Digest::blake3(b"example immutable input"),
            }],
            input_evidence: Vec::new(),
            configuration: InvocationConfiguration {
                schema_id: CONFIGURATION_SCHEMA.to_owned(),
                values: locked_configuration.values.clone(),
                fingerprint: locked_configuration.fingerprint.clone(),
            },
            authorization: AuthorizationEvidence {
                lock_id: extension.lock().lock_id.clone(),
                lock_digest: extension.lock_digest().clone(),
                grants_digest: extension.lock().grants_digest.clone(),
                granted_effects: extension.lock().granted_effects.clone(),
            },
            limits: ProcessLimits {
                timeout_ms: 1000,
                max_stdin_bytes: 4096,
                max_stdout_bytes: 4096,
                max_stderr_bytes: 4096,
            },
            cancellation_id: "example-cancellation".to_owned(),
            checkpoint_references: Vec::new(),
            lifecycle_event: None,
        }
    }

    #[test]
    fn manifest_and_all_four_typed_roles_pass_the_host_boundary() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = reference_manifest();
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        let manifest_path = directory.path().join("manifest.json");
        fs::write(&manifest_path, &manifest_bytes).unwrap();
        let values = json!({});
        let mut lock = ExtensionLockV1 {
            schema: EXTENSION_LOCK_SCHEMA.to_owned(),
            lock_id: "example.embedded-roles.local".to_owned(),
            extension_id: manifest.extension_id.clone(),
            extension_version: manifest.version.clone(),
            manifest_digest: Digest::blake3(&manifest_bytes),
            enabled: true,
            trust: TrustMode::TrustedEmbedded,
            granted_effects: BTreeSet::new(),
            grants_digest: Digest::blake3(b"pending"),
            precedence: 100,
            allowed_replacements: Vec::new(),
            allowed_fallbacks: Vec::new(),
            selected_execution_mode: "embedded-v1".to_owned(),
            process: None,
            configurations: BTreeMap::from([(
                CONFIGURATION_SCHEMA.to_owned(),
                LockedConfiguration {
                    fingerprint: fingerprint(&values).unwrap(),
                    values,
                },
            )]),
        };
        lock.grants_digest = lock.expected_grants_digest().unwrap();
        let lock_path = directory.path().join("lock.json");
        fs::write(&lock_path, serde_json::to_vec_pretty(&lock).unwrap()).unwrap();
        let catalog = ExtensionCatalog::load(&[manifest_path], &[lock_path]).unwrap();
        let extension = catalog.extensions()[0].clone();
        let mut registry = ExtensionRegistry::new(extension.clone()).unwrap();
        registry
            .register_inspector("optiflow/inspect/example", Arc::new(ReferenceInspector))
            .unwrap();
        registry
            .register_analyzer("optiflow/analyze/example", Arc::new(ReferenceAnalyzer))
            .unwrap();
        registry
            .register_planner("optiflow/plan/example", Arc::new(ReferencePlanner))
            .unwrap();
        registry
            .register_validator("optiflow/validate/example", Arc::new(ReferenceValidator))
            .unwrap();

        let cancelled = || false;
        let progress = |_: &optiflow::extensions::ProgressEvent| {};
        for capability_id in [
            "optiflow/inspect/example",
            "optiflow/analyze/example",
            "optiflow/plan/example",
            "optiflow/validate/example",
        ] {
            let invocation = invocation(&extension, capability_id);
            let context = ExtensionContext::new(&invocation, &cancelled, &progress);
            let result = registry.invoke(&context).unwrap();
            assert_eq!(result.consumed_artifacts, vec!["example-input"]);
        }
    }
}
