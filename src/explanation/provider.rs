use super::*;
use crate::extensions::*;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub enabled: bool,
    pub candidate_fingerprint: Option<Digest>,
}
/// A supplied fake candidate through the existing reporting role. This performs
/// no inference and requires no new SDK, router, network or environment grant.
pub struct SuppliedExplanationProvider {
    extension: LoadedExtension,
    candidate: Option<Candidate>,
}
impl SuppliedExplanationProvider {
    pub fn new(extension: LoadedExtension, candidate: Option<Candidate>) -> Self {
        Self {
            extension,
            candidate,
        }
    }
}
impl ReportProvider for SuppliedExplanationProvider {
    fn report(
        &self,
        context: &ExtensionContext<'_>,
    ) -> std::result::Result<ExtensionResultV1, ExtensionFailure> {
        let failure = |_: anyhow::Error| {
            ExtensionFailure::new(
                "invalid_explanation",
                "bounded explanation configuration, packet or candidate refused",
                false,
            )
        };
        let inv = context.invocation;
        let cap = self
            .extension
            .capability(&inv.capability_id)
            .ok_or_else(|| {
                ExtensionFailure::new("missing_capability", "report capability unavailable", false)
            })?;
        let config: Configuration = serde_json::from_value(inv.configuration.values.clone())
            .map_err(|_| failure(anyhow::anyhow!("closed config")))?;
        if context.is_cancelled() {
            return Err(ExtensionFailure::new(
                "cancelled",
                "explanation cancelled",
                false,
            ));
        }
        if cap.kind != CapabilityKind::ReportProvider
            || inv.configuration.schema_id != CONFIGURATION_SCHEMA
            || (inv.input_artifacts.len() > 1
                || inv
                    .input_artifacts
                    .iter()
                    .any(|a| a.schema != PACKET_SCHEMA))
            || !inv.authorization.granted_effects.is_empty()
            || !inv.checkpoint_references.is_empty()
        {
            return Err(failure(anyhow::anyhow!(
                "no effects, paths, credentials or raw artifacts accepted"
            )));
        }
        let mut out = ExtensionResultV1 {
            schema: EXTENSION_RESULT_SCHEMA.into(),
            invocation_id: inv.invocation_id.clone(),
            extension: inv.extension.clone(),
            capability_id: inv.capability_id.clone(),
            outcome: ExtensionOutcome::Unavailable,
            coverage: ExtensionCoverage {
                status: ExtensionCoverageStatus::Partial,
                limitations: vec![
                    "supplied evidence-only advisory; no current source or model qualification"
                        .into(),
                ],
            },
            consumed_artifacts: vec![],
            evidence: vec![],
            policy_contributions: vec![],
            plan_operations: vec![],
            validations: vec![],
            reports: vec![],
            diagnostics: vec![],
            progress: vec![],
            provenance: ExtensionProvenance {
                extension: inv.extension.clone(),
                capability_id: inv.capability_id.clone(),
                capability_version: cap.capability_version.clone(),
                configuration_fingerprint: inv.configuration.fingerprint.clone(),
                input_fingerprints: inv
                    .input_artifacts
                    .iter()
                    .map(|a| a.content_digest.clone())
                    .collect(),
                authorization_lock_id: self.extension.lock().lock_id.clone(),
                authorization_lock_digest: self.extension.lock_digest().clone(),
                grants_digest: self.extension.lock().grants_digest.clone(),
                execution_mode: self.extension.lock().selected_execution_mode.clone(),
            },
            observed_effects: BTreeSet::new(),
            checkpoint_references: vec![],
        };
        if !config.enabled {
            out.diagnostics.push(ExtensionDiagnostic {
                code: "ai_disabled".into(),
                message: "optional explanation disabled; ordinary processing unchanged".into(),
                retryable: false,
            });
            return Ok(out);
        }
        let Some(candidate) = &self.candidate else {
            out.diagnostics.push(ExtensionDiagnostic {
                code: "candidate_unavailable".into(),
                message: "no supplied candidate; no model invocation or remote fallback".into(),
                retryable: false,
            });
            return Ok(out);
        };
        if config.candidate_fingerprint.as_ref() != Some(&fingerprint(candidate).map_err(failure)?)
            || inv.input_evidence.len() != 1
            || inv.input_artifacts.len() != 1
        {
            return Err(failure(anyhow::anyhow!(
                "candidate fingerprint or packet count differs"
            )));
        }
        let evidence = &inv.input_evidence[0];
        if evidence.schema != PACKET_SCHEMA
            || evidence.fingerprint != fingerprint(&evidence.value).map_err(failure)?
            || inv.input_artifacts[0].artifact_id != evidence.evidence_id
            || inv.input_artifacts[0].content_digest != evidence.fingerprint
        {
            return Err(failure(anyhow::anyhow!("packet evidence differs")));
        }
        let packet: Packet = serde_json::from_value(evidence.value.clone())
            .map_err(|_| failure(anyhow::anyhow!("closed packet")))?;
        let sidecar =
            validate_candidate(&packet, candidate, &|| context.is_cancelled()).map_err(failure)?;
        let value = serde_json::to_value(sidecar)
            .map_err(|_| failure(anyhow::anyhow!("sidecar encoding")))?;
        out.outcome = ExtensionOutcome::Produced;
        out.consumed_artifacts = vec![inv.input_artifacts[0].artifact_id.clone()];
        out.reports.push(ReportContribution {
            report_id: "evidence-explanation".into(),
            schema: SIDECAR_SCHEMA.into(),
            fingerprint: fingerprint(&value).map_err(failure)?,
            value,
        });
        Ok(out)
    }
}
