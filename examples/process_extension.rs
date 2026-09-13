//! Minimal bounded-process inspector for the extension SDK v1 contract.

use std::collections::BTreeSet;
use std::io::{Read, Write};

use anyhow::{Context, Result, bail};
use optiflow::contracts::{self, Contract};
use optiflow::extensions::{
    Cacheability, CapabilityDeclaration, CapabilityKind, ConfigurationSchemaDeclaration,
    CoverageDeclaration, DeclaredCoverage, Determinism, EXTENSION_INVOCATION_SCHEMA,
    EXTENSION_MANIFEST_SCHEMA, EXTENSION_RESULT_SCHEMA, EXTENSION_SDK_CONTRACT,
    EvidenceContribution, ExecutionModeDeclaration, ExtensionCoverage, ExtensionCoverageStatus,
    ExtensionEffect, ExtensionInvocationV1, ExtensionManifestV1, ExtensionOutcome,
    ExtensionProvenance, ExtensionResultV1, LocalityDeclaration, ProcessLimits, Publisher,
    ReplacementDeclaration, ResourceNeeds, fingerprint,
};
use serde_json::json;

const MAX_REQUEST_BYTES: u64 = 1024 * 1024;

fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--print-manifest") {
        println!("{}", serde_json::to_string_pretty(&reference_manifest())?);
        return Ok(());
    }

    let mut request = Vec::new();
    std::io::stdin()
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut request)
        .context("failed to read the bounded invocation")?;
    if request.len() as u64 > MAX_REQUEST_BYTES {
        bail!("invocation exceeds the provider's input limit");
    }
    let document: serde_json::Value =
        serde_json::from_slice(&request).context("invalid extension invocation JSON")?;
    contracts::validate(Contract::ExtensionInvocation, &document)
        .context("invocation failed the public contract")?;
    let invocation: ExtensionInvocationV1 =
        serde_json::from_value(document).context("invalid extension invocation")?;
    if invocation.schema != EXTENSION_INVOCATION_SCHEMA
        || invocation.extension.extension_id != "example.reference-inspector"
        || invocation.capability_id != "optiflow/inspect/reference-count"
        || invocation.capability_kind != CapabilityKind::Inspector
    {
        bail!("invocation does not target this provider and capability");
    }
    let value = json!({ "immutable_reference_count": invocation.input_artifacts.len() });
    let evidence = EvidenceContribution {
        evidence_id: format!("{}:reference-count", invocation.invocation_id),
        schema: "optiflow.example.reference-count.v1".to_owned(),
        fingerprint: fingerprint(&value)?,
        value,
    };
    let result = ExtensionResultV1 {
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
        evidence: vec![evidence],
        policy_contributions: Vec::new(),
        plan_operations: Vec::new(),
        validations: Vec::new(),
        reports: Vec::new(),
        diagnostics: Vec::new(),
        progress: Vec::new(),
        provenance: ExtensionProvenance {
            extension: invocation.extension.clone(),
            capability_id: invocation.capability_id.clone(),
            capability_version: "1.0.0".to_owned(),
            configuration_fingerprint: invocation.configuration.fingerprint.clone(),
            input_fingerprints: invocation
                .input_artifacts
                .iter()
                .map(|artifact| artifact.content_digest.clone())
                .collect(),
            authorization_lock_id: invocation.authorization.lock_id.clone(),
            authorization_lock_digest: invocation.authorization.lock_digest.clone(),
            grants_digest: invocation.authorization.grants_digest.clone(),
            execution_mode: "json-stdio-v1".to_owned(),
        },
        observed_effects: BTreeSet::new(),
        checkpoint_references: invocation.checkpoint_references,
    };
    let mut output = serde_json::to_vec(&result)?;
    output.push(b'\n');
    std::io::stdout().write_all(&output)?;
    Ok(())
}

fn reference_manifest() -> ExtensionManifestV1 {
    let configuration_document = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false
    });
    ExtensionManifestV1 {
        schema: EXTENSION_MANIFEST_SCHEMA.to_owned(),
        extension_id: "example.reference-inspector".to_owned(),
        version: "1.0.0".to_owned(),
        publisher: Publisher {
            id: "example.publisher".to_owned(),
            display_name: "Example Publisher".to_owned(),
        },
        sdk_contract: EXTENSION_SDK_CONTRACT.to_owned(),
        configuration_schemas: vec![ConfigurationSchemaDeclaration {
            schema_id: "optiflow.example.reference-inspector.config.v1".to_owned(),
            schema_digest: fingerprint(&configuration_document)
                .expect("static configuration schema is serializable"),
            document: configuration_document,
        }],
        capabilities: vec![CapabilityDeclaration {
            capability_id: "optiflow/inspect/reference-count".to_owned(),
            capability_version: "1.0.0".to_owned(),
            kind: CapabilityKind::Inspector,
            configuration_schema: "optiflow.example.reference-inspector.config.v1".to_owned(),
            accepts: vec!["optiflow.artifact-reference.v1".to_owned()],
            produces: vec!["optiflow.example.reference-count.v1".to_owned()],
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
                memory_bytes: 16 * 1024 * 1024,
                temporary_storage_bytes: 0,
                gpu: false,
            },
            locality: LocalityDeclaration {
                local_only: true,
                network_hosts: Vec::new(),
            },
            content_changes: false,
            effects: BTreeSet::new(),
        }],
        execution_modes: vec![ExecutionModeDeclaration::Process {
            name: "json-stdio-v1".to_owned(),
            protocol: EXTENSION_INVOCATION_SCHEMA.to_owned(),
            arguments: Vec::new(),
            limits: ProcessLimits {
                timeout_ms: 5000,
                max_stdin_bytes: MAX_REQUEST_BYTES,
                max_stdout_bytes: 1024 * 1024,
                max_stderr_bytes: 64 * 1024,
            },
        }],
        requested_effects: BTreeSet::from([ExtensionEffect::Subprocess]),
        replacement: ReplacementDeclaration {
            replaces: Vec::new(),
            fallbacks_for: Vec::new(),
        },
        observer_hooks: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_manifest_satisfies_the_public_contract() {
        contracts::validate(Contract::ExtensionManifest, &reference_manifest()).unwrap();
    }
}
