use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use optiflow::domain::NativePath;
use optiflow::extensions::{
    Analyzer, AuthorizationEvidence, Cacheability, CapabilityDeclaration, CapabilityKind,
    ConfigurationSchemaDeclaration, CoverageDeclaration, DeclaredCoverage, Determinism, Digest,
    EXTENSION_INVOCATION_SCHEMA, EXTENSION_LOCK_SCHEMA, EXTENSION_MANIFEST_SCHEMA,
    EXTENSION_RESULT_SCHEMA, EXTENSION_SDK_CONTRACT, EvidenceContribution,
    ExecutionModeDeclaration, ExtensionCatalog, ExtensionContext, ExtensionCoverage,
    ExtensionCoverageStatus, ExtensionEffect, ExtensionFailure, ExtensionIdentity,
    ExtensionInvocationV1, ExtensionLockV1, ExtensionManifestV1, ExtensionOutcome,
    ExtensionProvenance, ExtensionRegistry, ExtensionResolutionStatus, ExtensionResultV1,
    Inspector, InvocationConfiguration, LifecycleEvent, LifecycleObserver, LifecyclePhase,
    LocalityDeclaration, LockedConfiguration, LockedProcess, ObserverHookDeclaration,
    PlanOperationContribution, PlanPreconditions, PolicyContribution, PolicyContributor,
    ProcessExtensionClient, ProcessExtensionError, ProcessLimits, ProgressEvent, Publisher,
    RegistryError, ReplacementDeclaration, ResourceNeeds, TrustMode, fingerprint,
};
use optiflow::subprocess::SubprocessError;
use proptest::prelude::*;
use serde_json::json;
use tempfile::TempDir;

struct Fixture {
    _directory: TempDir,
    extension: optiflow::extensions::LoadedExtension,
    invocation: ExtensionInvocationV1,
}

fn not_cancelled() -> bool {
    false
}

fn ignore_progress(_event: &ProgressEvent) {}

fn limits(timeout_ms: u64) -> ProcessLimits {
    ProcessLimits {
        timeout_ms,
        max_stdin_bytes: 1024 * 1024,
        max_stdout_bytes: 1024 * 1024,
        max_stderr_bytes: 64 * 1024,
    }
}

fn configuration_schema() -> ConfigurationSchemaDeclaration {
    let document = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": { "threshold": { "type": "integer", "minimum": 0 } },
        "additionalProperties": false
    });
    ConfigurationSchemaDeclaration {
        schema_id: "optiflow.test.config.v1".to_owned(),
        schema_digest: fingerprint(&document).unwrap(),
        document,
    }
}

fn capability(kind: CapabilityKind, effects: BTreeSet<ExtensionEffect>) -> CapabilityDeclaration {
    CapabilityDeclaration {
        capability_id: "optiflow/test-capability".to_owned(),
        capability_version: "1.0.0".to_owned(),
        kind,
        configuration_schema: "optiflow.test.config.v1".to_owned(),
        accepts: vec!["optiflow.test.input.v1".to_owned()],
        produces: vec!["optiflow.test.evidence.v1".to_owned()],
        preconditions: vec!["immutable-input-digests".to_owned()],
        postconditions: vec!["fingerprinted-evidence".to_owned()],
        determinism: Determinism::Deterministic,
        cacheability: Cacheability::Cacheable,
        coverage: CoverageDeclaration {
            mode: DeclaredCoverage::Complete,
            dimensions: Vec::new(),
        },
        resource_needs: ResourceNeeds {
            cpu_threads: 1,
            memory_bytes: 1024 * 1024,
            temporary_storage_bytes: 0,
            gpu: false,
        },
        locality: LocalityDeclaration {
            local_only: true,
            network_hosts: Vec::new(),
        },
        content_changes: kind == CapabilityKind::Planner,
        effects,
    }
}

fn manifest(
    extension_id: &str,
    kind: CapabilityKind,
    sdk_contract: &str,
    execution_modes: Vec<ExecutionModeDeclaration>,
    requested_effects: BTreeSet<ExtensionEffect>,
) -> ExtensionManifestV1 {
    ExtensionManifestV1 {
        schema: EXTENSION_MANIFEST_SCHEMA.to_owned(),
        extension_id: extension_id.to_owned(),
        version: "1.0.0".to_owned(),
        publisher: Publisher {
            id: "example.publisher".to_owned(),
            display_name: "Example Publisher".to_owned(),
        },
        sdk_contract: sdk_contract.to_owned(),
        configuration_schemas: vec![configuration_schema()],
        capabilities: vec![capability(kind, requested_effects.clone())],
        execution_modes,
        requested_effects,
        replacement: ReplacementDeclaration {
            replaces: Vec::new(),
            fallbacks_for: Vec::new(),
        },
        observer_hooks: if kind == CapabilityKind::LifecycleObserver {
            vec![ObserverHookDeclaration {
                hook_id: "test-observer".to_owned(),
                lifecycle_points: BTreeSet::from([LifecyclePhase::Inspect]),
                read_only: true,
            }]
        } else {
            Vec::new()
        },
    }
}

fn write_fixture(
    directory: TempDir,
    manifest: ExtensionManifestV1,
    trust: TrustMode,
    precedence: i32,
    process: Option<LockedProcess>,
) -> Fixture {
    write_fixture_with_fallbacks(directory, manifest, trust, precedence, process, Vec::new())
}

fn write_fixture_with_fallbacks(
    directory: TempDir,
    manifest: ExtensionManifestV1,
    trust: TrustMode,
    precedence: i32,
    process: Option<LockedProcess>,
    allowed_fallbacks: Vec<String>,
) -> Fixture {
    write_fixture_with_resolution_policy(
        directory,
        manifest,
        trust,
        precedence,
        process,
        Vec::new(),
        allowed_fallbacks,
    )
}

fn write_fixture_with_resolution_policy(
    directory: TempDir,
    manifest: ExtensionManifestV1,
    trust: TrustMode,
    precedence: i32,
    process: Option<LockedProcess>,
    allowed_replacements: Vec<String>,
    allowed_fallbacks: Vec<String>,
) -> Fixture {
    let manifest_path = directory.path().join("extension-manifest.json");
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    fs::write(&manifest_path, &manifest_bytes).unwrap();
    let values = json!({ "threshold": 1 });
    let mut lock = ExtensionLockV1 {
        schema: EXTENSION_LOCK_SCHEMA.to_owned(),
        lock_id: format!("{}-lock", manifest.extension_id),
        extension_id: manifest.extension_id.clone(),
        extension_version: manifest.version.clone(),
        manifest_digest: Digest::blake3(&manifest_bytes),
        enabled: true,
        trust,
        granted_effects: manifest.requested_effects.clone(),
        grants_digest: Digest::blake3(b"pending"),
        precedence,
        allowed_replacements,
        allowed_fallbacks,
        selected_execution_mode: manifest.execution_modes[0].name().to_owned(),
        process,
        configurations: BTreeMap::from([(
            "optiflow.test.config.v1".to_owned(),
            LockedConfiguration {
                fingerprint: fingerprint(&values).unwrap(),
                values,
            },
        )]),
    };
    lock.grants_digest = lock.expected_grants_digest().unwrap();
    let lock_path = directory.path().join("extension-lock.json");
    fs::write(&lock_path, serde_json::to_vec_pretty(&lock).unwrap()).unwrap();
    let catalog = ExtensionCatalog::load(&[manifest_path], &[lock_path]).unwrap();
    let extension = catalog.extensions()[0].clone();
    let invocation = invocation(&extension);
    Fixture {
        _directory: directory,
        extension,
        invocation,
    }
}

fn disable_fixture(fixture: &Fixture) {
    let mut lock = fixture.extension.lock().clone();
    lock.enabled = false;
    lock.trust = TrustMode::Disabled;
    lock.grants_digest = lock.expected_grants_digest().unwrap();
    fs::write(
        fixture.extension.lock_path(),
        serde_json::to_vec_pretty(&lock).unwrap(),
    )
    .unwrap();
}

fn embedded_fixture(extension_id: &str, kind: CapabilityKind, precedence: i32) -> Fixture {
    let effects = BTreeSet::from([ExtensionEffect::ArtifactRead]);
    let manifest = manifest(
        extension_id,
        kind,
        EXTENSION_SDK_CONTRACT,
        vec![ExecutionModeDeclaration::Embedded {
            name: "embedded-v1".to_owned(),
            entrypoint: "example::Provider".to_owned(),
        }],
        effects,
    );
    write_fixture(
        tempfile::tempdir().unwrap(),
        manifest,
        TrustMode::TrustedEmbedded,
        precedence,
        None,
    )
}

fn invocation(extension: &optiflow::extensions::LoadedExtension) -> ExtensionInvocationV1 {
    let capability = &extension.manifest().capabilities[0];
    let configuration = extension
        .lock()
        .configurations
        .get(&capability.configuration_schema)
        .unwrap();
    let limits = match extension.selected_execution_mode().unwrap() {
        ExecutionModeDeclaration::Process { limits, .. } => *limits,
        ExecutionModeDeclaration::Embedded { .. } => limits(1000),
    };
    ExtensionInvocationV1 {
        schema: EXTENSION_INVOCATION_SCHEMA.to_owned(),
        invocation_id: "invocation-1".to_owned(),
        extension: ExtensionIdentity {
            extension_id: extension.manifest().extension_id.clone(),
            version: extension.manifest().version.clone(),
            manifest_digest: extension.manifest_digest().clone(),
        },
        capability_id: capability.capability_id.clone(),
        capability_kind: capability.kind,
        input_artifacts: Vec::new(),
        input_evidence: Vec::new(),
        configuration: InvocationConfiguration {
            schema_id: capability.configuration_schema.clone(),
            values: configuration.values.clone(),
            fingerprint: configuration.fingerprint.clone(),
        },
        authorization: AuthorizationEvidence {
            lock_id: extension.lock().lock_id.clone(),
            lock_digest: extension.lock_digest().clone(),
            grants_digest: extension.lock().grants_digest.clone(),
            granted_effects: extension.lock().granted_effects.clone(),
        },
        limits,
        cancellation_id: "cancel-1".to_owned(),
        checkpoint_references: Vec::new(),
        lifecycle_event: None,
    }
}

fn empty_result(
    extension: &optiflow::extensions::LoadedExtension,
    invocation: &ExtensionInvocationV1,
) -> ExtensionResultV1 {
    let capability = extension.capability(&invocation.capability_id).unwrap();
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
        consumed_artifacts: Vec::new(),
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
            capability_version: capability.capability_version.clone(),
            configuration_fingerprint: invocation.configuration.fingerprint.clone(),
            input_fingerprints: invocation
                .input_artifacts
                .iter()
                .map(|artifact| artifact.content_digest.clone())
                .collect(),
            authorization_lock_id: extension.lock().lock_id.clone(),
            authorization_lock_digest: extension.lock_digest().clone(),
            grants_digest: extension.lock().grants_digest.clone(),
            execution_mode: extension.lock().selected_execution_mode.clone(),
        },
        observed_effects: BTreeSet::new(),
        checkpoint_references: invocation.checkpoint_references.clone(),
    }
}

#[derive(Clone)]
struct FixedInspector(ExtensionResultV1);

impl Inspector for FixedInspector {
    fn inspect(
        &self,
        _context: &ExtensionContext<'_>,
    ) -> Result<ExtensionResultV1, ExtensionFailure> {
        Ok(self.0.clone())
    }
}

#[derive(Clone)]
struct FixedPlanner(ExtensionResultV1);

impl optiflow::extensions::Planner for FixedPlanner {
    fn plan(&self, _context: &ExtensionContext<'_>) -> Result<ExtensionResultV1, ExtensionFailure> {
        Ok(self.0.clone())
    }
}

fn planner_result_with_missing_precondition(
    missing_precondition: usize,
) -> (Fixture, ExtensionResultV1) {
    let mut fixture = embedded_fixture("example.planner", CapabilityKind::Planner, 1);
    let input = optiflow::extensions::ImmutableArtifactReference {
        artifact_id: "source-evidence".to_owned(),
        schema: "optiflow.test.input.v1".to_owned(),
        content_digest: Digest::blake3(b"immutable-source-evidence"),
    };
    fixture.invocation.input_artifacts.push(input.clone());
    let mut result = empty_result(&fixture.extension, &fixture.invocation);
    result.consumed_artifacts.push(input.artifact_id.clone());
    let mut preconditions = PlanPreconditions::required();
    match missing_precondition {
        0 => preconditions.observation_revalidation = false,
        1 => preconditions.full_hash = false,
        2 => preconditions.byte_confirmation = false,
        3 => preconditions.plan_identity = false,
        4 => preconditions.explicit_authorization = false,
        5 => preconditions.provenance = false,
        _ => unreachable!("the property strategy emits indices from 0 through 5"),
    }
    let mut operation = PlanOperationContribution {
        operation_id: "unsafe".to_owned(),
        action_kind: "derive-new-artifact".to_owned(),
        input_artifacts: vec![input],
        expected_output_schemas: vec!["optiflow.test.evidence.v1".to_owned()],
        preconditions,
        mutates_source: false,
        requires_core_execution: true,
        fingerprint: Digest::blake3(b"pending"),
    };
    operation.fingerprint = operation.expected_fingerprint().unwrap();
    result.plan_operations.push(operation);
    (fixture, result)
}

#[test]
fn rejects_malformed_evidence_before_it_crosses_the_registry_boundary() {
    let fixture = embedded_fixture("example.malformed", CapabilityKind::Inspector, 1);
    let mut result = empty_result(&fixture.extension, &fixture.invocation);
    result.evidence.push(EvidenceContribution {
        evidence_id: "bad-evidence".to_owned(),
        schema: "optiflow.test.evidence.v1".to_owned(),
        value: json!({ "observed": true }),
        fingerprint: Digest::blake3(b"not-the-evidence"),
    });
    let mut registry = ExtensionRegistry::new(fixture.extension.clone()).unwrap();
    registry
        .register_inspector(
            fixture.invocation.capability_id.clone(),
            Arc::new(FixedInspector(result)),
        )
        .unwrap();
    let context = ExtensionContext::new(&fixture.invocation, &not_cancelled, &ignore_progress);

    assert!(matches!(
        registry.invoke(&context),
        Err(RegistryError::ResultRejected { .. })
    ));
}

#[test]
fn rejects_malformed_inline_input_evidence_before_handler_execution() {
    let mut fixture = embedded_fixture("example.bad-input", CapabilityKind::Inspector, 1);
    let value = json!({ "trusted": false });
    fixture
        .invocation
        .input_artifacts
        .push(optiflow::extensions::ImmutableArtifactReference {
            artifact_id: "input-evidence".to_owned(),
            schema: "optiflow.test.input.v1".to_owned(),
            content_digest: fingerprint(&value).unwrap(),
        });
    fixture
        .invocation
        .input_evidence
        .push(EvidenceContribution {
            evidence_id: "input-evidence".to_owned(),
            schema: "optiflow.test.input.v1".to_owned(),
            value,
            fingerprint: Digest::blake3(b"mismatch"),
        });
    let result = empty_result(&fixture.extension, &fixture.invocation);
    let mut registry = ExtensionRegistry::new(fixture.extension.clone()).unwrap();
    registry
        .register_inspector(
            fixture.invocation.capability_id.clone(),
            Arc::new(FixedInspector(result)),
        )
        .unwrap();
    let context = ExtensionContext::new(&fixture.invocation, &not_cancelled, &ignore_progress);

    assert!(matches!(
        registry.invoke(&context),
        Err(RegistryError::InvocationRejected { .. })
    ));
}

#[test]
fn rejects_an_invocation_not_bound_to_the_exact_lock_bytes() {
    let mut fixture = embedded_fixture("example.wrong-lock", CapabilityKind::Inspector, 1);
    fixture.invocation.authorization.lock_digest = Digest::blake3(b"different-lock");
    let result = empty_result(&fixture.extension, &fixture.invocation);
    let mut registry = ExtensionRegistry::new(fixture.extension.clone()).unwrap();
    registry
        .register_inspector(
            fixture.invocation.capability_id.clone(),
            Arc::new(FixedInspector(result)),
        )
        .unwrap();
    let context = ExtensionContext::new(&fixture.invocation, &not_cancelled, &ignore_progress);

    assert!(matches!(
        registry.invoke(&context),
        Err(RegistryError::InvocationRejected { .. })
    ));
}

#[test]
fn complete_coverage_cannot_skip_an_invoked_artifact() {
    let mut fixture = embedded_fixture("example.skipped-input", CapabilityKind::Inspector, 1);
    fixture
        .invocation
        .input_artifacts
        .push(optiflow::extensions::ImmutableArtifactReference {
            artifact_id: "input-1".to_owned(),
            schema: "optiflow.test.input.v1".to_owned(),
            content_digest: Digest::blake3(b"immutable-input"),
        });
    let result = empty_result(&fixture.extension, &fixture.invocation);
    let mut registry = ExtensionRegistry::new(fixture.extension.clone()).unwrap();
    registry
        .register_inspector(
            fixture.invocation.capability_id.clone(),
            Arc::new(FixedInspector(result)),
        )
        .unwrap();
    let context = ExtensionContext::new(&fixture.invocation, &not_cancelled, &ignore_progress);

    assert!(matches!(
        registry.invoke(&context),
        Err(RegistryError::ResultRejected { .. })
    ));
}

#[test]
fn planner_cannot_publish_untyped_analysis_evidence() {
    let fixture = embedded_fixture("example.planner-evidence", CapabilityKind::Planner, 1);
    let mut result = empty_result(&fixture.extension, &fixture.invocation);
    let value = json!({ "bypasses": "analyzer-role" });
    result.evidence.push(EvidenceContribution {
        evidence_id: "planner-evidence".to_owned(),
        schema: "optiflow.test.evidence.v1".to_owned(),
        fingerprint: fingerprint(&value).unwrap(),
        value,
    });
    let mut registry = ExtensionRegistry::new(fixture.extension.clone()).unwrap();
    registry
        .register_planner(
            fixture.invocation.capability_id.clone(),
            Arc::new(FixedPlanner(result)),
        )
        .unwrap();
    let context = ExtensionContext::new(&fixture.invocation, &not_cancelled, &ignore_progress);

    assert!(matches!(
        registry.invoke(&context),
        Err(RegistryError::ResultRejected { .. })
    ));
}

#[test]
fn preserves_honest_partial_coverage() {
    let fixture = embedded_fixture("example.partial", CapabilityKind::Inspector, 1);
    let mut result = empty_result(&fixture.extension, &fixture.invocation);
    result.coverage = ExtensionCoverage {
        status: ExtensionCoverageStatus::Partial,
        limitations: vec!["one artifact schema was unsupported".to_owned()],
    };
    let mut registry = ExtensionRegistry::new(fixture.extension.clone()).unwrap();
    registry
        .register_inspector(
            fixture.invocation.capability_id.clone(),
            Arc::new(FixedInspector(result)),
        )
        .unwrap();
    let context = ExtensionContext::new(&fixture.invocation, &not_cancelled, &ignore_progress);

    let accepted = registry.invoke(&context).unwrap();
    assert_eq!(accepted.coverage.status, ExtensionCoverageStatus::Partial);
    assert_eq!(accepted.coverage.limitations.len(), 1);
}

#[test]
fn accepts_fingerprinted_policy_only_from_the_typed_policy_role() {
    struct FixedPolicy(ExtensionResultV1);
    impl PolicyContributor for FixedPolicy {
        fn contribute_policy(
            &self,
            _context: &ExtensionContext<'_>,
        ) -> Result<ExtensionResultV1, ExtensionFailure> {
            Ok(self.0.clone())
        }
    }

    let fixture = embedded_fixture("example.policy", CapabilityKind::PolicyContributor, 1);
    let mut result = empty_result(&fixture.extension, &fixture.invocation);
    let value = json!({ "normalization": "preserve-original" });
    result.policy_contributions.push(PolicyContribution {
        policy_id: "normalization-policy".to_owned(),
        schema: "optiflow.test.evidence.v1".to_owned(),
        fingerprint: fingerprint(&value).unwrap(),
        value,
    });
    let mut registry = ExtensionRegistry::new(fixture.extension.clone()).unwrap();
    registry
        .register_policy_contributor(
            fixture.invocation.capability_id.clone(),
            Arc::new(FixedPolicy(result)),
        )
        .unwrap();
    let context = ExtensionContext::new(&fixture.invocation, &not_cancelled, &ignore_progress);

    assert_eq!(
        registry
            .invoke(&context)
            .unwrap()
            .policy_contributions
            .len(),
        1
    );
}

#[test]
fn rejects_plans_that_bypass_core_preconditions() {
    let (fixture, result) = planner_result_with_missing_precondition(0);
    let mut registry = ExtensionRegistry::new(fixture.extension.clone()).unwrap();
    registry
        .register_planner(
            fixture.invocation.capability_id.clone(),
            Arc::new(FixedPlanner(result)),
        )
        .unwrap();
    let context = ExtensionContext::new(&fixture.invocation, &not_cancelled, &ignore_progress);

    assert!(matches!(
        registry.invoke(&context),
        Err(RegistryError::ResultRejected { .. })
    ));
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 32,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn no_missing_core_plan_precondition_crosses_the_boundary(missing in 0_usize..6) {
        let (fixture, result) = planner_result_with_missing_precondition(missing);
        let mut registry = ExtensionRegistry::new(fixture.extension.clone()).unwrap();
        registry
            .register_planner(
                fixture.invocation.capability_id.clone(),
                Arc::new(FixedPlanner(result)),
            )
            .unwrap();
        let context = ExtensionContext::new(&fixture.invocation, &not_cancelled, &ignore_progress);

        let rejected = matches!(
            registry.invoke(&context),
            Err(RegistryError::ResultRejected { .. })
        );
        prop_assert!(rejected);
    }
}

#[test]
fn fails_closed_on_precedence_conflicts() {
    let first = embedded_fixture("example.first", CapabilityKind::Inspector, 10);
    let second = embedded_fixture("example.second", CapabilityKind::Inspector, 10);
    let catalog = ExtensionCatalog::load(
        &[
            first.extension.manifest_path().to_path_buf(),
            second.extension.manifest_path().to_path_buf(),
        ],
        &[
            first.extension.lock_path().to_path_buf(),
            second.extension.lock_path().to_path_buf(),
        ],
    )
    .unwrap();

    let resolution = catalog.resolve("optiflow/test-capability");
    assert_eq!(resolution.status, ExtensionResolutionStatus::Conflict);
    assert!(resolution.selected_extension_id.is_none());
    assert!(!catalog.doctor().complete);
}

#[test]
fn fallback_requires_provider_declaration_and_operator_lock() {
    let primary = embedded_fixture("example.primary", CapabilityKind::Inspector, 20);
    disable_fixture(&primary);
    let effects = BTreeSet::from([ExtensionEffect::ArtifactRead]);
    let mut fallback_manifest = manifest(
        "example.fallback",
        CapabilityKind::Inspector,
        EXTENSION_SDK_CONTRACT,
        vec![ExecutionModeDeclaration::Embedded {
            name: "embedded-v1".to_owned(),
            entrypoint: "example::Provider".to_owned(),
        }],
        effects,
    );
    fallback_manifest.replacement.fallbacks_for = vec!["example.primary".to_owned()];
    let fallback = write_fixture_with_fallbacks(
        tempfile::tempdir().unwrap(),
        fallback_manifest,
        TrustMode::TrustedEmbedded,
        10,
        None,
        vec!["example.primary".to_owned()],
    );
    let catalog = ExtensionCatalog::load(
        &[
            primary.extension.manifest_path().to_path_buf(),
            fallback.extension.manifest_path().to_path_buf(),
        ],
        &[
            primary.extension.lock_path().to_path_buf(),
            fallback.extension.lock_path().to_path_buf(),
        ],
    )
    .unwrap();

    let resolution = catalog.resolve("optiflow/test-capability");
    assert_eq!(resolution.status, ExtensionResolutionStatus::Selected);
    assert_eq!(
        resolution.selected_extension_id.as_deref(),
        Some("example.fallback")
    );
    assert_eq!(resolution.fallback_extension_ids, vec!["example.fallback"]);
}

#[test]
fn never_silently_selects_an_unauthorized_fallback() {
    let primary = embedded_fixture("example.preferred", CapabilityKind::Inspector, 20);
    disable_fixture(&primary);
    let secondary = embedded_fixture("example.secondary", CapabilityKind::Inspector, 10);
    let catalog = ExtensionCatalog::load(
        &[
            primary.extension.manifest_path().to_path_buf(),
            secondary.extension.manifest_path().to_path_buf(),
        ],
        &[
            primary.extension.lock_path().to_path_buf(),
            secondary.extension.lock_path().to_path_buf(),
        ],
    )
    .unwrap();

    let resolution = catalog.resolve("optiflow/test-capability");
    assert_eq!(resolution.status, ExtensionResolutionStatus::Unavailable);
    assert!(resolution.selected_extension_id.is_none());
}

#[test]
fn replacement_intent_is_advisory_and_precedence_remains_operator_owned() {
    let primary = embedded_fixture("example.original", CapabilityKind::Inspector, 20);
    let effects = BTreeSet::from([ExtensionEffect::ArtifactRead]);
    let mut replacement_manifest = manifest(
        "example.replacement",
        CapabilityKind::Inspector,
        EXTENSION_SDK_CONTRACT,
        vec![ExecutionModeDeclaration::Embedded {
            name: "embedded-v1".to_owned(),
            entrypoint: "example::Provider".to_owned(),
        }],
        effects,
    );
    replacement_manifest.replacement.replaces = vec!["example.original".to_owned()];
    let replacement = write_fixture_with_resolution_policy(
        tempfile::tempdir().unwrap(),
        replacement_manifest,
        TrustMode::TrustedEmbedded,
        10,
        None,
        vec!["example.original".to_owned()],
        Vec::new(),
    );
    let catalog = ExtensionCatalog::load(
        &[
            primary.extension.manifest_path().to_path_buf(),
            replacement.extension.manifest_path().to_path_buf(),
        ],
        &[
            primary.extension.lock_path().to_path_buf(),
            replacement.extension.lock_path().to_path_buf(),
        ],
    )
    .unwrap();

    let resolution = catalog.resolve("optiflow/test-capability");
    assert_eq!(resolution.status, ExtensionResolutionStatus::Selected);
    assert_eq!(
        resolution.selected_extension_id.as_deref(),
        Some("example.original")
    );
}

#[test]
fn lock_cannot_authorize_undeclared_replacement() {
    let manifest = manifest(
        "example.undeclared-replacement",
        CapabilityKind::Inspector,
        EXTENSION_SDK_CONTRACT,
        vec![ExecutionModeDeclaration::Embedded {
            name: "embedded-v1".to_owned(),
            entrypoint: "example::Provider".to_owned(),
        }],
        BTreeSet::from([ExtensionEffect::ArtifactRead]),
    );
    let fixture = write_fixture_with_resolution_policy(
        tempfile::tempdir().unwrap(),
        manifest,
        TrustMode::TrustedEmbedded,
        10,
        None,
        vec!["example.original".to_owned()],
        Vec::new(),
    );

    assert!(!fixture.extension.availability_reasons().is_empty());
    assert!(
        fixture
            .extension
            .availability_reasons()
            .iter()
            .any(|reason| reason.contains("undeclared replacement"))
    );
}

#[test]
fn reports_incompatible_sdk_versions_as_unavailable() {
    let effects = BTreeSet::from([ExtensionEffect::ArtifactRead]);
    let manifest = manifest(
        "example.incompatible",
        CapabilityKind::Inspector,
        "optiflow.extension-sdk.v2",
        vec![ExecutionModeDeclaration::Embedded {
            name: "embedded-v1".to_owned(),
            entrypoint: "example::Provider".to_owned(),
        }],
        effects,
    );
    let fixture = write_fixture(
        tempfile::tempdir().unwrap(),
        manifest,
        TrustMode::TrustedEmbedded,
        1,
        None,
    );

    assert!(!fixture.extension.availability_reasons().is_empty());
    assert!(fixture.extension.availability_reasons()[0].contains("incompatible"));
}

#[test]
fn embedded_cancellation_prevents_handler_execution() {
    struct CountingAnalyzer(Arc<AtomicUsize>);
    impl Analyzer for CountingAnalyzer {
        fn analyze(
            &self,
            _context: &ExtensionContext<'_>,
        ) -> Result<ExtensionResultV1, ExtensionFailure> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Err(ExtensionFailure::new("unexpected", "should not run", false))
        }
    }

    let fixture = embedded_fixture("example.cancel", CapabilityKind::Analyzer, 1);
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ExtensionRegistry::new(fixture.extension.clone()).unwrap();
    registry
        .register_analyzer(
            fixture.invocation.capability_id.clone(),
            Arc::new(CountingAnalyzer(Arc::clone(&calls))),
        )
        .unwrap();
    let cancelled = || true;
    let context = ExtensionContext::new(&fixture.invocation, &cancelled, &ignore_progress);

    assert!(matches!(
        registry.invoke(&context),
        Err(RegistryError::Cancelled { .. })
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn embedded_panic_does_not_allow_a_result_or_poison_the_registry() {
    struct RecoveringInspector {
        calls: AtomicUsize,
        result: ExtensionResultV1,
    }
    impl Inspector for RecoveringInspector {
        fn inspect(
            &self,
            _context: &ExtensionContext<'_>,
        ) -> Result<ExtensionResultV1, ExtensionFailure> {
            if self.calls.fetch_add(1, Ordering::Relaxed) == 0 {
                panic!("adversarial embedded crash");
            }
            Ok(self.result.clone())
        }
    }

    let fixture = embedded_fixture("example.panic", CapabilityKind::Inspector, 1);
    let result = empty_result(&fixture.extension, &fixture.invocation);
    let mut registry = ExtensionRegistry::new(fixture.extension.clone()).unwrap();
    registry
        .register_inspector(
            fixture.invocation.capability_id.clone(),
            Arc::new(RecoveringInspector {
                calls: AtomicUsize::new(0),
                result,
            }),
        )
        .unwrap();
    let context = ExtensionContext::new(&fixture.invocation, &not_cancelled, &ignore_progress);

    assert!(matches!(
        registry.invoke(&context),
        Err(RegistryError::HandlerFailed { .. })
    ));
    registry.invoke(&context).unwrap();
}

#[test]
fn lifecycle_observers_are_read_only_and_phase_scoped() {
    struct CountingObserver(Arc<AtomicUsize>);
    impl LifecycleObserver for CountingObserver {
        fn observe(
            &self,
            _context: &ExtensionContext<'_>,
            _event: &LifecycleEvent,
        ) -> Result<(), ExtensionFailure> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    let mut fixture = embedded_fixture("example.observer", CapabilityKind::LifecycleObserver, 1);
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ExtensionRegistry::new(fixture.extension.clone()).unwrap();
    registry
        .register_lifecycle_observer(
            fixture.invocation.capability_id.clone(),
            Arc::new(CountingObserver(Arc::clone(&calls))),
        )
        .unwrap();
    let mut event = LifecycleEvent {
        event_id: "event-1".to_owned(),
        invocation_id: fixture.invocation.invocation_id.clone(),
        sequence: 1,
        phase: LifecyclePhase::Inspect,
        state: "started".to_owned(),
        artifact_references: Vec::new(),
        diagnostics: Vec::new(),
    };
    fixture.invocation.lifecycle_event = Some(event.clone());
    let context = ExtensionContext::new(&fixture.invocation, &not_cancelled, &ignore_progress);
    registry.observe(&context, &event).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);

    event.phase = LifecyclePhase::Execute;
    assert!(matches!(
        registry.observe(&context, &event),
        Err(RegistryError::InvocationRejected { .. })
    ));
}

#[test]
fn rejects_duplicate_manifest_selection() {
    let fixture = embedded_fixture("example.duplicate", CapabilityKind::Inspector, 1);
    let manifest = fixture.extension.manifest_path().to_path_buf();
    let lock = fixture.extension.lock_path().to_path_buf();

    assert!(ExtensionCatalog::load(&[manifest.clone(), manifest], &[lock]).is_err());
}

#[test]
fn rejects_unknown_manifest_fields_before_typed_decoding() {
    let fixture = embedded_fixture("example.closed", CapabilityKind::Inspector, 1);
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.extension.manifest_path()).unwrap()).unwrap();
    document["unexpected_authority"] = json!(true);
    let manifest = fixture
        ._directory
        .path()
        .join("unknown-field.manifest.json");
    fs::write(&manifest, serde_json::to_vec(&document).unwrap()).unwrap();

    assert!(
        ExtensionCatalog::load(&[manifest], &[fixture.extension.lock_path().to_path_buf()])
            .is_err()
    );
}

#[cfg(unix)]
fn process_fixture(script: &str, timeout_ms: u64) -> Fixture {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("provider.sh");
    fs::write(&executable, script).unwrap();
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).unwrap();
    let executable_digest = Digest::blake3(&fs::read(&executable).unwrap());
    let requested_effects = BTreeSet::from([
        ExtensionEffect::ArtifactRead,
        ExtensionEffect::IsolatedWorkspaceWrite,
        ExtensionEffect::Subprocess,
    ]);
    let manifest = manifest(
        "example.process",
        CapabilityKind::Inspector,
        EXTENSION_SDK_CONTRACT,
        vec![ExecutionModeDeclaration::Process {
            name: "json-stdio-v1".to_owned(),
            protocol: EXTENSION_INVOCATION_SCHEMA.to_owned(),
            arguments: Vec::new(),
            limits: limits(timeout_ms),
        }],
        requested_effects,
    );
    let process = LockedProcess {
        executable: NativePath::from_path(&executable),
        executable_digest,
        working_directory: NativePath::from_path(directory.path()),
    };
    write_fixture(
        directory,
        manifest,
        TrustMode::TrustedProcess,
        1,
        Some(process),
    )
}

#[cfg(unix)]
#[test]
fn process_provider_timeout_is_bounded_and_typed() {
    let fixture = process_fixture("#!/bin/sh\n/bin/sleep 1\n", 40);
    let client = ProcessExtensionClient::new(fixture.extension.clone()).unwrap();

    assert!(matches!(
        client.invoke(&fixture.invocation, || false),
        Err(ProcessExtensionError::Process(
            SubprocessError::Timeout { .. }
        ))
    ));
}

#[cfg(unix)]
#[test]
fn process_provider_cancellation_is_bounded_and_typed() {
    let fixture = process_fixture("#!/bin/sh\n/bin/sleep 1\n", 2000);
    let client = ProcessExtensionClient::new(fixture.extension.clone()).unwrap();
    let started = std::time::Instant::now();

    assert!(matches!(
        client.invoke(&fixture.invocation, || started.elapsed()
            > Duration::from_millis(25)),
        Err(ProcessExtensionError::Process(
            SubprocessError::Cancelled { .. }
        ))
    ));
}

#[cfg(unix)]
#[test]
fn process_provider_refuses_changed_executable_bytes() {
    let fixture = process_fixture("#!/bin/sh\nprintf 'not-used'\n", 1000);
    let client = ProcessExtensionClient::new(fixture.extension.clone()).unwrap();
    let executable = fixture
        .extension
        .lock()
        .process
        .as_ref()
        .unwrap()
        .executable
        .to_path_buf();
    fs::write(&executable, "#!/bin/sh\nprintf 'changed'\n").unwrap();

    assert!(matches!(
        client.invoke(&fixture.invocation, || false),
        Err(ProcessExtensionError::ExecutableChanged { .. })
    ));
}

#[cfg(unix)]
#[test]
fn malformed_process_output_never_crosses_the_boundary() {
    let fixture = process_fixture("#!/bin/sh\n/bin/cat >/dev/null\nprintf 'not-json'\n", 1000);
    let client = ProcessExtensionClient::new(fixture.extension.clone()).unwrap();

    assert!(matches!(
        client.invoke(&fixture.invocation, || false),
        Err(ProcessExtensionError::Process(
            SubprocessError::Parse { .. }
        ))
    ));
}

#[cfg(unix)]
#[test]
fn closed_process_result_rejects_unknown_fields() {
    let fixture = process_fixture(
        "#!/bin/sh\n/bin/cat >/dev/null\nexec /bin/cat result.json\n",
        1000,
    );
    let mut document =
        serde_json::to_value(empty_result(&fixture.extension, &fixture.invocation)).unwrap();
    document["unexpected_authority"] = json!(true);
    fs::write(
        fixture._directory.path().join("result.json"),
        serde_json::to_vec(&document).unwrap(),
    )
    .unwrap();
    let client = ProcessExtensionClient::new(fixture.extension.clone()).unwrap();

    assert!(matches!(
        client.invoke(&fixture.invocation, || false),
        Err(ProcessExtensionError::ResultRejected { .. })
    ));
}

#[cfg(unix)]
#[test]
fn process_crash_does_not_poison_checkpoint_recovery() {
    let mut fixture = process_fixture(
        "#!/bin/sh\nif [ ! -f first-crash ]; then\n  : > first-crash\n  exit 7\nfi\n/bin/cat >/dev/null\nexec /bin/cat result.json\n",
        1000,
    );
    let checkpoint = optiflow::extensions::ImmutableArtifactReference {
        artifact_id: "checkpoint-1".to_owned(),
        schema: "optiflow.test.checkpoint.v1".to_owned(),
        content_digest: Digest::blake3(b"checkpoint"),
    };
    fixture.invocation.checkpoint_references = vec![checkpoint];
    let result = empty_result(&fixture.extension, &fixture.invocation);
    fs::write(
        fixture._directory.path().join("result.json"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    let client = ProcessExtensionClient::new(fixture.extension.clone()).unwrap();

    assert!(matches!(
        client.invoke(&fixture.invocation, || false),
        Err(ProcessExtensionError::Process(SubprocessError::Exit {
            code: Some(7),
            ..
        }))
    ));
    let recovered = client.invoke(&fixture.invocation, || false).unwrap();
    assert_eq!(recovered.checkpoint_references.len(), 1);
}

#[test]
fn progress_callback_is_host_owned() {
    let fixture = embedded_fixture("example.progress", CapabilityKind::Inspector, 1);
    let observed = AtomicUsize::new(0);
    let progress = ProgressEvent {
        sequence: 1,
        completed: 1,
        total: Some(1),
        unit: "artifact".to_owned(),
        message: "done".to_owned(),
    };
    let record_progress = |_: &ProgressEvent| {
        observed.fetch_add(1, Ordering::Relaxed);
    };
    let context = ExtensionContext::new(&fixture.invocation, &not_cancelled, &record_progress);
    context.report_progress(&progress);
    assert_eq!(observed.load(Ordering::Relaxed), 1);
}
