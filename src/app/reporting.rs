use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::artifact_set::{
    self, ARTIFACT_SET_SCHEMA, ArtifactPayload, ArtifactSetInspection, ArtifactSetManifest,
    ArtifactSetStatus,
};
use crate::configuration::EffectivePolicyV1;
use crate::contracts::{self, Contract};
use crate::domain::{
    MediaProfileCoverageStatus, PLAN_SCHEMA_VERSION, REPORT_SCHEMA_VERSION,
    REPORT_SCHEMA_VERSION_V1, REPORT_SCHEMA_VERSION_V2, REPORT_SCHEMA_VERSION_V3,
    REPORT_SCHEMA_VERSION_V4, REPORT_SCHEMA_VERSION_V5, ScanReport,
};
use crate::outcome::{
    ArtifactReference, CoverageStatus, Diagnostic, DiagnosticClassification, DiagnosticCode,
    DiagnosticImpact, DiagnosticSeverity,
};
use crate::planning::exact_duplicate_plan;
use crate::state::StateStore;

use super::{Execution, artifact_reference, internal_failure_with_code};

struct LoadedReport {
    report: ScanReport,
    path: PathBuf,
    artifact_set: Option<ArtifactSetManifest>,
}

pub(super) fn run_report(
    state_directory: &Path,
    run_or_path: &str,
    current_policy: &EffectivePolicyV1,
) -> Execution {
    let store = match StateStore::open(state_directory) {
        Ok(store) => store,
        Err(error) => {
            return internal_failure_with_code(
                DiagnosticCode::StateTransactionFailed,
                "failed to open persistent state",
                &error,
            );
        }
    };
    let loaded = match load_report(&store, run_or_path) {
        Ok(loaded) => loaded,
        Err(diagnostic) => return Execution::failure(*diagnostic),
    };
    let partial = report_is_partial(&loaded.report);
    let mut diagnostics = if partial {
        vec![source_run_partial_diagnostic(&loaded.report)]
    } else {
        Vec::new()
    };
    let (policy_artifact, mut policy_diagnostics) =
        load_source_policy(&loaded.report, current_policy);
    diagnostics.append(&mut policy_diagnostics);
    let mut artifacts = vec![artifact_reference(
        "report",
        &loaded.report.schema_version,
        &loaded.report.run.run_id,
        &loaded.path,
    )];
    if loaded.artifact_set.is_some() {
        let marker_path = loaded
            .path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(artifact_set::SCAN_MARKER_FILE_NAME);
        artifacts.push(artifact_reference(
            "artifact_set",
            ARTIFACT_SET_SCHEMA,
            &loaded.report.run.run_id,
            &marker_path,
        ));
    }
    if let Some(policy_artifact) = policy_artifact {
        artifacts.push(policy_artifact);
    }
    Execution {
        coverage: Some(if partial {
            CoverageStatus::Partial
        } else {
            CoverageStatus::Complete
        }),
        artifacts,
        diagnostics,
        result: serde_json::to_value(&loaded.report).ok(),
    }
}

pub(super) fn run_plan(
    state_directory: &Path,
    run_or_path: &str,
    output: Option<&Path>,
    current_policy: &EffectivePolicyV1,
) -> Execution {
    let store = match StateStore::open(state_directory) {
        Ok(store) => store,
        Err(error) => {
            return internal_failure_with_code(
                DiagnosticCode::StateTransactionFailed,
                "failed to open persistent state",
                &error,
            );
        }
    };
    let loaded = match load_report(&store, run_or_path) {
        Ok(loaded) => loaded,
        Err(diagnostic) => return Execution::failure(*diagnostic),
    };
    let partial = report_is_partial(&loaded.report);
    let plan = exact_duplicate_plan(&loaded.report);
    let output_path = output.map_or_else(
        || Path::new(&loaded.report.run.artifact_directory).join("plan-exact-duplicates.json"),
        Path::to_path_buf,
    );
    if let Err(error) = contracts::validate(Contract::Plan, &plan) {
        return Execution::failure(Diagnostic::new(
            DiagnosticCode::ArtifactValidationFailed,
            DiagnosticSeverity::Fatal,
            DiagnosticClassification::Artifact,
            DiagnosticImpact::BlocksCommand,
            format!("the generated plan failed contract validation: {error}"),
        ));
    }
    let plan_payload = match output_path
        .file_name()
        .map(|file_name| ArtifactPayload::json("plan", PLAN_SCHEMA_VERSION, file_name, &plan))
    {
        Some(Ok(payload)) => payload,
        Some(Err(error)) => {
            return internal_failure_with_code(
                DiagnosticCode::ArtifactCommitFailed,
                "failed to serialize the plan artifact set",
                &error,
            );
        }
        None => {
            return Execution::failure(
                Diagnostic::new(
                    DiagnosticCode::OutputDestinationInvalid,
                    DiagnosticSeverity::Error,
                    DiagnosticClassification::Input,
                    DiagnosticImpact::BlocksCommand,
                    "the plan output destination has no file name",
                )
                .with_path(&output_path),
            );
        }
    };
    let source_set_id = loaded
        .artifact_set
        .as_ref()
        .map(|manifest| manifest.set_id.as_str());
    if let Err(error) = artifact_set::commit_plan_set(
        &output_path,
        &loaded.report.run.run_id,
        source_set_id,
        plan_payload,
    ) {
        let mut diagnostic = Diagnostic::new(
            DiagnosticCode::OutputDestinationInvalid,
            DiagnosticSeverity::Error,
            DiagnosticClassification::Input,
            DiagnosticImpact::BlocksCommand,
            format!("the plan output destination could not be committed: {error}"),
        )
        .with_path(&output_path);
        diagnostic.retryable = Some(false);
        return Execution::failure(diagnostic);
    }
    let (policy_artifact, mut diagnostics) = load_source_policy(&loaded.report, current_policy);
    if partial {
        diagnostics.push(source_run_partial_diagnostic(&loaded.report));
    }
    let mut artifacts = vec![artifact_reference(
        "plan",
        PLAN_SCHEMA_VERSION,
        &loaded.report.run.run_id,
        &output_path,
    )];
    artifacts.push(artifact_reference(
        "artifact_set",
        ARTIFACT_SET_SCHEMA,
        &loaded.report.run.run_id,
        &artifact_set::plan_marker_path(&output_path),
    ));
    if let Some(policy_artifact) = policy_artifact {
        artifacts.push(policy_artifact);
    }
    Execution {
        coverage: Some(if partial {
            CoverageStatus::Partial
        } else {
            CoverageStatus::Complete
        }),
        artifacts,
        diagnostics,
        result: serde_json::to_value(&plan).ok(),
    }
}

fn load_report(store: &StateStore, run_or_path: &str) -> Result<LoadedReport, Box<Diagnostic>> {
    let candidate = Path::new(run_or_path);
    if candidate.exists() {
        let report_path = if candidate.is_dir() {
            candidate.join("report.json")
        } else {
            candidate.to_path_buf()
        };
        let bytes = fs::read(&report_path).map_err(|error| {
            Box::new(
                Diagnostic::new(
                    DiagnosticCode::StoredStateIncompatible,
                    DiagnosticSeverity::Error,
                    DiagnosticClassification::State,
                    DiagnosticImpact::BlocksCommand,
                    format!("the requested report could not be read: {error}"),
                )
                .with_path(&report_path),
            )
        })?;
        let report = decode_report(&bytes, &report_path)?;
        let artifact_set = if report_requires_artifact_set(&report.schema_version) {
            let directory = report_path.parent().unwrap_or_else(|| Path::new("."));
            Some(require_report_artifact_set(
                artifact_set::inspect_scan_set(directory),
                directory,
                &report,
            )?)
        } else {
            None
        };
        return Ok(LoadedReport {
            report,
            path: report_path,
            artifact_set,
        });
    }

    if Uuid::parse_str(run_or_path).is_err() {
        return Err(Box::new(Diagnostic::new(
            DiagnosticCode::InvalidCommandInput,
            DiagnosticSeverity::Error,
            DiagnosticClassification::Input,
            DiagnosticImpact::BlocksCommand,
            "the run reference is neither an existing report path nor a valid run identifier",
        )));
    }

    let report = store.load_report(run_or_path).map_err(|error| {
        Box::new(
            Diagnostic::new(
                DiagnosticCode::StoredStateIncompatible,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                format!("stored scan state could not be decoded: {error}"),
            )
            .with_run_id(run_or_path),
        )
    })?;
    if let Some(report) = report {
        let directory = Path::new(&report.run.artifact_directory);
        let artifact_set = if report_requires_artifact_set(&report.schema_version) {
            Some(require_report_artifact_set(
                artifact_set::inspect_scan_set(directory),
                directory,
                &report,
            )?)
        } else {
            None
        };
        return Ok(LoadedReport {
            path: Path::new(&report.run.artifact_directory).join("report.json"),
            report,
            artifact_set,
        });
    }

    let status = store.load_run_status(run_or_path).map_err(|error| {
        Box::new(
            Diagnostic::new(
                DiagnosticCode::StoredStateIncompatible,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                format!("stored scan status could not be inspected: {error}"),
            )
            .with_run_id(run_or_path),
        )
    })?;
    let (code, message) = if status.as_deref() == Some("interrupted") {
        (
            DiagnosticCode::SourceRunInterrupted,
            "the requested scan run was interrupted and has no complete report",
        )
    } else {
        (
            DiagnosticCode::StoredRunNotFound,
            "the requested completed scan run was not found",
        )
    };
    Err(Box::new(
        Diagnostic::new(
            code,
            DiagnosticSeverity::Error,
            DiagnosticClassification::State,
            DiagnosticImpact::BlocksCommand,
            message,
        )
        .with_run_id(run_or_path),
    ))
}

fn require_committed_artifact_set(
    inspection: ArtifactSetInspection,
    path: &Path,
) -> Result<ArtifactSetManifest, Box<Diagnostic>> {
    match inspection.status {
        ArtifactSetStatus::Committed => inspection.manifest.ok_or_else(|| {
            Box::new(
                Diagnostic::new(
                    DiagnosticCode::InternalInvariantViolated,
                    DiagnosticSeverity::Fatal,
                    DiagnosticClassification::Internal,
                    DiagnosticImpact::BlocksCommand,
                    "artifact-set inspection omitted its committed manifest",
                )
                .with_path(path),
            )
        }),
        ArtifactSetStatus::Incomplete => Err(Box::new(
            Diagnostic::new(
                DiagnosticCode::ArtifactSetIncomplete,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                inspection.detail,
            )
            .with_path(path),
        )),
        ArtifactSetStatus::Incompatible => Err(Box::new(
            Diagnostic::new(
                DiagnosticCode::ArtifactSetIncompatible,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                inspection.detail,
            )
            .with_path(path),
        )),
    }
}

fn require_report_artifact_set(
    inspection: ArtifactSetInspection,
    path: &Path,
    report: &ScanReport,
) -> Result<ArtifactSetManifest, Box<Diagnostic>> {
    let manifest = require_committed_artifact_set(inspection, path)?;
    if report.run.artifact_set_id.as_deref() != Some(manifest.set_id.as_str()) {
        return Err(Box::new(
            Diagnostic::new(
                DiagnosticCode::ArtifactSetIncompatible,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                "the report and artifact-set marker declare different set identities",
            )
            .with_path(path),
        ));
    }
    Ok(manifest)
}

fn decode_report(bytes: &[u8], path: &Path) -> Result<ScanReport, Box<Diagnostic>> {
    let document: serde_json::Value = serde_json::from_slice(bytes).map_err(|error| {
        Box::new(
            Diagnostic::new(
                DiagnosticCode::InvalidCommandInput,
                DiagnosticSeverity::Error,
                DiagnosticClassification::Input,
                DiagnosticImpact::BlocksCommand,
                format!("the supplied report is not valid JSON: {error}"),
            )
            .with_path(path),
        )
    })?;
    let schema_version = document
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    if !matches!(
        schema_version.as_deref(),
        Some(
            REPORT_SCHEMA_VERSION
                | REPORT_SCHEMA_VERSION_V1
                | REPORT_SCHEMA_VERSION_V2
                | REPORT_SCHEMA_VERSION_V3
                | REPORT_SCHEMA_VERSION_V4
                | REPORT_SCHEMA_VERSION_V5
        )
    ) {
        return Err(Box::new(
            Diagnostic::new(
                DiagnosticCode::StoredStateIncompatible,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                "the supplied report declares an unsupported schema",
            )
            .with_path(path),
        ));
    }
    let report: ScanReport = serde_json::from_value(document).map_err(|error| {
        Box::new(
            Diagnostic::new(
                DiagnosticCode::StoredStateIncompatible,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                format!("the supplied report is incompatible with its declared schema: {error}"),
            )
            .with_path(path),
        )
    })?;
    if schema_version.as_deref() == Some(REPORT_SCHEMA_VERSION) {
        contracts::validate(Contract::Report, &report).map_err(|error| {
            Box::new(
                Diagnostic::new(
                    DiagnosticCode::StoredStateIncompatible,
                    DiagnosticSeverity::Error,
                    DiagnosticClassification::State,
                    DiagnosticImpact::BlocksCommand,
                    format!("the supplied report failed contract validation: {error}"),
                )
                .with_path(path),
            )
        })?;
    }
    Ok(report)
}

fn report_is_partial(report: &ScanReport) -> bool {
    report.summary.unstable_observation_count > 0
        || report.summary.unreadable_files > 0
        || !report.run.warnings.is_empty()
        || report.media_profile_evidence.iter().any(|evidence| {
            matches!(
                evidence.coverage.status,
                MediaProfileCoverageStatus::Partial | MediaProfileCoverageStatus::Unavailable
            )
        })
}

fn report_requires_artifact_set(schema_version: &str) -> bool {
    matches!(
        schema_version,
        REPORT_SCHEMA_VERSION | REPORT_SCHEMA_VERSION_V5
    )
}

fn load_source_policy(
    report: &ScanReport,
    current_policy: &EffectivePolicyV1,
) -> (Option<ArtifactReference>, Vec<Diagnostic>) {
    let path = Path::new(&report.run.artifact_directory).join("effective-policy.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let diagnostic = Diagnostic::new(
                DiagnosticCode::HistoricalPolicyUnknown,
                DiagnosticSeverity::Information,
                DiagnosticClassification::State,
                DiagnosticImpact::None,
                "the historical run predates persisted effective-policy evidence",
            )
            .with_run_id(report.run.run_id.clone());
            return (None, vec![diagnostic]);
        }
        Err(error) => {
            let diagnostic = Diagnostic::new(
                DiagnosticCode::HistoricalPolicyUnknown,
                DiagnosticSeverity::Warning,
                DiagnosticClassification::State,
                DiagnosticImpact::None,
                format!("the historical effective policy could not be read: {error}"),
            )
            .with_run_id(report.run.run_id.clone());
            return (None, vec![diagnostic]);
        }
    };
    let source_policy: EffectivePolicyV1 = match serde_json::from_slice(&bytes) {
        Ok(policy) => policy,
        Err(error) => {
            let diagnostic = Diagnostic::new(
                DiagnosticCode::HistoricalPolicyUnknown,
                DiagnosticSeverity::Warning,
                DiagnosticClassification::State,
                DiagnosticImpact::None,
                format!("the historical effective policy is incompatible: {error}"),
            )
            .with_run_id(report.run.run_id.clone());
            return (None, vec![diagnostic]);
        }
    };
    if let Err(error) = contracts::validate(Contract::EffectivePolicy, &source_policy)
        .and_then(|()| crate::configuration::validate_fingerprints(&source_policy))
    {
        let diagnostic = Diagnostic::new(
            DiagnosticCode::EffectivePolicyFingerprintMismatch,
            DiagnosticSeverity::Warning,
            DiagnosticClassification::State,
            DiagnosticImpact::None,
            format!("the historical effective policy failed integrity validation: {error}"),
        )
        .with_run_id(report.run.run_id.clone());
        return (None, vec![diagnostic]);
    }

    let mut diagnostics = Vec::new();
    if source_policy.fingerprints.evidence_policy != current_policy.fingerprints.evidence_policy {
        diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::SourcePolicyDiffers,
                DiagnosticSeverity::Information,
                DiagnosticClassification::State,
                DiagnosticImpact::None,
                "the source run used a different evidence policy than the current command",
            )
            .with_run_id(report.run.run_id.clone()),
        );
    }
    (
        Some(artifact_reference(
            "effective_policy",
            &source_policy.schema,
            &report.run.run_id,
            &path,
        )),
        diagnostics,
    )
}

fn source_run_partial_diagnostic(report: &ScanReport) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::SourceRunPartial,
        DiagnosticSeverity::Warning,
        DiagnosticClassification::Coverage,
        DiagnosticImpact::DegradesCoverage,
        "the source run has incomplete or excluded evidence",
    )
    .with_run_id(report.run.run_id.clone())
}
