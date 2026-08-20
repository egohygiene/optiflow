use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::NativePath;

pub const COMMAND_RESULT_SCHEMA: &str = "optiflow.command-result.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOutcomeClass {
    Success,
    InternalFailure,
    InvalidInput,
    PartialSuccess,
    CapabilityUnavailable,
    StaleState,
    Interrupted,
    Terminated,
}

impl CommandOutcomeClass {
    pub const fn exit_code(self) -> ProcessExitCode {
        match self {
            Self::Success => ProcessExitCode::Success,
            Self::InternalFailure => ProcessExitCode::InternalFailure,
            Self::InvalidInput => ProcessExitCode::InvalidInput,
            Self::PartialSuccess => ProcessExitCode::PartialSuccess,
            Self::CapabilityUnavailable => ProcessExitCode::CapabilityUnavailable,
            Self::StaleState => ProcessExitCode::StaleState,
            Self::Interrupted => ProcessExitCode::InterruptedBySigint,
            Self::Terminated => ProcessExitCode::InterruptedBySigterm,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProcessExitCode {
    Success = 0,
    InternalFailure = 1,
    InvalidInput = 2,
    PartialSuccess = 3,
    CapabilityUnavailable = 4,
    StaleState = 5,
    InterruptedBySigint = 130,
    InterruptedBySigterm = 143,
}

impl ProcessExitCode {
    pub const fn get(self) -> u8 {
        self as u8
    }
}

impl Serialize for ProcessExitCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(self.get())
    }
}

impl<'de> Deserialize<'de> for ProcessExitCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(Self::Success),
            1 => Ok(Self::InternalFailure),
            2 => Ok(Self::InvalidInput),
            3 => Ok(Self::PartialSuccess),
            4 => Ok(Self::CapabilityUnavailable),
            5 => Ok(Self::StaleState),
            130 => Ok(Self::InterruptedBySigint),
            143 => Ok(Self::InterruptedBySigterm),
            _ => Err(serde::de::Error::custom(
                "unknown optiflow process exit code",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Complete,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Information,
    Warning,
    Error,
    Fatal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticClassification {
    Input,
    Capability,
    Coverage,
    Observation,
    State,
    Artifact,
    Interruption,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticImpact {
    None,
    DegradesCoverage,
    BlocksCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    InvalidInvocation,
    InvalidCommandInput,
    RequiredCapabilityUnavailable,
    OptionalCapabilityUnavailable,
    PartialInventory,
    SourceRunPartial,
    SourceRunInterrupted,
    StoredRunNotFound,
    StoredStateIncompatible,
    StaleEvidencePreventsOperation,
    ArtifactValidationFailed,
    ArtifactCommitFailed,
    StateTransactionFailed,
    InternalInvariantViolated,
    OperationInterrupted,
    OperationTerminated,
    StructuredOutputFailed,
    OutputDestinationInvalid,
    ResultProducedWithExclusions,
    ConfigurationNotFound,
    ConfigurationUnreadable,
    ConfigurationNotRegularFile,
    ConfigurationParseFailed,
    ConfigurationSchemaMissing,
    ConfigurationSchemaUnsupported,
    ConfigurationUnknownKey,
    ConfigurationValueInvalid,
    ConfigurationValueOutOfRange,
    ConfigurationConflict,
    ConfigurationSelectorConflict,
    ConfigurationEnvironmentInvalid,
    ConfigurationPathResolutionFailed,
    ConfigurationDiscoveryFailed,
    ConfigurationSourceReplacedDuringRead,
    ConfigurationOverrideShadowed,
    ConfigurationDeprecatedKey,
    ConfigurationLockedInvariant,
    EffectivePolicyInvalid,
    EffectivePolicySerializationFailed,
    EffectivePolicyFingerprintMismatch,
    HistoricalPolicyUnknown,
    SourcePolicyDiffers,
    ConfigurationSettingUnknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<NativePath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setting: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_variable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub classification: DiagnosticClassification,
    pub impact: DiagnosticImpact,
    pub message: String,
    #[serde(default, skip_serializing_if = "is_default_context")]
    pub context: DiagnosticContext,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

fn is_default_context(context: &DiagnosticContext) -> bool {
    context == &DiagnosticContext::default()
}

impl Diagnostic {
    pub fn new(
        code: DiagnosticCode,
        severity: DiagnosticSeverity,
        classification: DiagnosticClassification,
        impact: DiagnosticImpact,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity,
            classification,
            impact,
            message: message.into(),
            context: DiagnosticContext::default(),
            retryable: None,
        }
    }

    pub fn with_path(mut self, path: &std::path::Path) -> Self {
        self.context.path = Some(NativePath::from_path(path));
        self
    }

    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.context.run_id = Some(run_id.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReference {
    pub kind: String,
    pub schema: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub path: NativePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub class: CommandOutcomeClass,
    pub exit_code: ProcessExitCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    pub status: CoverageStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub schema: String,
    pub command: String,
    pub outcome: Outcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<Coverage>,
    pub artifacts: Vec<ArtifactReference>,
    pub diagnostics: Vec<Diagnostic>,
    pub result: Option<Value>,
}

impl CommandResult {
    pub fn resolve(
        command: impl Into<String>,
        coverage: Option<CoverageStatus>,
        artifacts: Vec<ArtifactReference>,
        mut diagnostics: Vec<Diagnostic>,
        result: Option<Value>,
    ) -> Self {
        diagnostics.sort_by_key(|diagnostic| serde_json::to_string(diagnostic).unwrap_or_default());
        diagnostics = coalesce_diagnostics(diagnostics);

        let class = resolve_class(coverage, &artifacts, &diagnostics, result.as_ref());
        let resolved = Self {
            schema: COMMAND_RESULT_SCHEMA.to_owned(),
            command: command.into(),
            outcome: Outcome {
                class,
                exit_code: class.exit_code(),
            },
            coverage: coverage.map(|status| Coverage { status }),
            artifacts,
            diagnostics,
            result,
        };

        if let Err(message) = resolved.validate() {
            return Self::internal_failure("cli", message);
        }
        resolved
    }

    pub fn failure(command: impl Into<String>, diagnostic: Diagnostic) -> Self {
        Self::resolve(command, None, Vec::new(), vec![diagnostic], None)
    }

    pub fn internal_failure(command: impl Into<String>, message: impl Into<String>) -> Self {
        let command = command.into();
        Self {
            schema: COMMAND_RESULT_SCHEMA.to_owned(),
            command,
            outcome: Outcome {
                class: CommandOutcomeClass::InternalFailure,
                exit_code: ProcessExitCode::InternalFailure,
            },
            coverage: None,
            artifacts: Vec::new(),
            diagnostics: vec![Diagnostic::new(
                DiagnosticCode::InternalInvariantViolated,
                DiagnosticSeverity::Fatal,
                DiagnosticClassification::Internal,
                DiagnosticImpact::BlocksCommand,
                message,
            )],
            result: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != COMMAND_RESULT_SCHEMA {
            return Err("command-result schema identifier is inconsistent".to_owned());
        }
        if self.outcome.exit_code != self.outcome.class.exit_code() {
            return Err("declared exit code does not match the outcome class".to_owned());
        }
        if self.outcome.class == CommandOutcomeClass::Success
            && self
                .coverage
                .is_some_and(|coverage| coverage.status == CoverageStatus::Partial)
        {
            return Err("success cannot declare partial coverage".to_owned());
        }
        if self.outcome.class == CommandOutcomeClass::PartialSuccess
            && self.artifacts.is_empty()
            && self.result.is_none()
        {
            return Err("partial success requires a valid result or committed artifact".to_owned());
        }
        Ok(())
    }
}

fn coalesce_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut coalesced: Vec<Diagnostic> = Vec::with_capacity(diagnostics.len());
    for diagnostic in diagnostics {
        if let Some(previous) = coalesced
            .last_mut()
            .filter(|previous| equivalent_except_count(previous, &diagnostic))
        {
            previous.context.count = Some(
                previous
                    .context
                    .count
                    .unwrap_or(1)
                    .saturating_add(diagnostic.context.count.unwrap_or(1)),
            );
        } else {
            coalesced.push(diagnostic);
        }
    }
    coalesced
}

fn equivalent_except_count(left: &Diagnostic, right: &Diagnostic) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.context.count = None;
    right.context.count = None;
    left == right
}

fn resolve_class(
    coverage: Option<CoverageStatus>,
    artifacts: &[ArtifactReference],
    diagnostics: &[Diagnostic],
    result: Option<&Value>,
) -> CommandOutcomeClass {
    let blocking = |classification| {
        diagnostics.iter().any(|diagnostic| {
            diagnostic.classification == classification
                && diagnostic.impact == DiagnosticImpact::BlocksCommand
        })
    };
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::OperationInterrupted)
    {
        return CommandOutcomeClass::Interrupted;
    }
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::OperationTerminated)
    {
        return CommandOutcomeClass::Terminated;
    }
    if blocking(DiagnosticClassification::Internal) || blocking(DiagnosticClassification::Artifact)
    {
        return CommandOutcomeClass::InternalFailure;
    }
    if blocking(DiagnosticClassification::State) {
        return CommandOutcomeClass::StaleState;
    }
    if blocking(DiagnosticClassification::Capability) {
        return CommandOutcomeClass::CapabilityUnavailable;
    }
    if blocking(DiagnosticClassification::Input) {
        return CommandOutcomeClass::InvalidInput;
    }
    if coverage == Some(CoverageStatus::Partial)
        || diagnostics
            .iter()
            .any(|diagnostic| diagnostic.impact == DiagnosticImpact::DegradesCoverage)
    {
        if artifacts.is_empty() && result.is_none() {
            return CommandOutcomeClass::InternalFailure;
        }
        return CommandOutcomeClass::PartialSuccess;
    }
    CommandOutcomeClass::Success
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_contract_is_stable() {
        let cases = [
            (CommandOutcomeClass::Success, 0),
            (CommandOutcomeClass::InternalFailure, 1),
            (CommandOutcomeClass::InvalidInput, 2),
            (CommandOutcomeClass::PartialSuccess, 3),
            (CommandOutcomeClass::CapabilityUnavailable, 4),
            (CommandOutcomeClass::StaleState, 5),
            (CommandOutcomeClass::Interrupted, 130),
            (CommandOutcomeClass::Terminated, 143),
        ];
        for (class, code) in cases {
            assert_eq!(class.exit_code().get(), code);
        }
    }

    #[test]
    fn coverage_impact_is_distinct_from_warning_severity() {
        let informational_warning = Diagnostic::new(
            DiagnosticCode::OptionalCapabilityUnavailable,
            DiagnosticSeverity::Warning,
            DiagnosticClassification::Capability,
            DiagnosticImpact::None,
            "optional capability unavailable",
        );
        let complete = CommandResult::resolve(
            "scan",
            Some(CoverageStatus::Complete),
            Vec::new(),
            vec![informational_warning],
            Some(serde_json::json!({})),
        );
        assert_eq!(complete.outcome.class, CommandOutcomeClass::Success);

        let partial = CommandResult::resolve(
            "scan",
            Some(CoverageStatus::Partial),
            Vec::new(),
            vec![Diagnostic::new(
                DiagnosticCode::PartialInventory,
                DiagnosticSeverity::Warning,
                DiagnosticClassification::Coverage,
                DiagnosticImpact::DegradesCoverage,
                "partial inventory",
            )],
            Some(serde_json::json!({})),
        );
        assert_eq!(partial.outcome.class, CommandOutcomeClass::PartialSuccess);
    }

    #[test]
    fn blocking_precedence_is_semantic() {
        let result = CommandResult::resolve(
            "scan",
            Some(CoverageStatus::Partial),
            Vec::new(),
            vec![
                Diagnostic::new(
                    DiagnosticCode::PartialInventory,
                    DiagnosticSeverity::Warning,
                    DiagnosticClassification::Coverage,
                    DiagnosticImpact::DegradesCoverage,
                    "partial",
                ),
                Diagnostic::new(
                    DiagnosticCode::StateTransactionFailed,
                    DiagnosticSeverity::Fatal,
                    DiagnosticClassification::Internal,
                    DiagnosticImpact::BlocksCommand,
                    "state failure",
                ),
            ],
            Some(serde_json::json!({})),
        );
        assert_eq!(result.outcome.class, CommandOutcomeClass::InternalFailure);
    }

    #[test]
    fn serialized_contract_names_are_stable() {
        assert_eq!(
            serde_json::to_value(CommandOutcomeClass::PartialSuccess).unwrap(),
            "partial_success"
        );
        assert_eq!(
            serde_json::to_value(DiagnosticCode::StoredRunNotFound).unwrap(),
            "stored_run_not_found"
        );
    }

    #[test]
    fn resolver_coalesces_duplicate_diagnostics_and_preserves_count() {
        let diagnostic = Diagnostic::new(
            DiagnosticCode::PartialInventory,
            DiagnosticSeverity::Warning,
            DiagnosticClassification::Coverage,
            DiagnosticImpact::DegradesCoverage,
            "path was unavailable",
        );
        let result = CommandResult::resolve(
            "scan",
            Some(CoverageStatus::Partial),
            Vec::new(),
            vec![diagnostic.clone(), diagnostic],
            Some(serde_json::json!({})),
        );

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].context.count, Some(2));
    }

    #[test]
    fn signal_outcome_overrides_other_blocking_diagnostics() {
        let result = CommandResult::resolve(
            "scan",
            Some(CoverageStatus::Partial),
            Vec::new(),
            vec![
                Diagnostic::new(
                    DiagnosticCode::StateTransactionFailed,
                    DiagnosticSeverity::Fatal,
                    DiagnosticClassification::Internal,
                    DiagnosticImpact::BlocksCommand,
                    "state failed",
                ),
                Diagnostic::new(
                    DiagnosticCode::OperationTerminated,
                    DiagnosticSeverity::Error,
                    DiagnosticClassification::Interruption,
                    DiagnosticImpact::BlocksCommand,
                    "terminated",
                ),
            ],
            Some(serde_json::json!({})),
        );

        assert_eq!(result.outcome.class, CommandOutcomeClass::Terminated);
        assert_eq!(result.outcome.exit_code.get(), 143);
    }

    #[test]
    fn blocking_classifications_resolve_to_stable_classes() {
        let cases = [
            (
                DiagnosticClassification::Input,
                DiagnosticCode::InvalidCommandInput,
                CommandOutcomeClass::InvalidInput,
            ),
            (
                DiagnosticClassification::Capability,
                DiagnosticCode::RequiredCapabilityUnavailable,
                CommandOutcomeClass::CapabilityUnavailable,
            ),
            (
                DiagnosticClassification::State,
                DiagnosticCode::StoredRunNotFound,
                CommandOutcomeClass::StaleState,
            ),
            (
                DiagnosticClassification::Artifact,
                DiagnosticCode::ArtifactCommitFailed,
                CommandOutcomeClass::InternalFailure,
            ),
        ];

        for (classification, code, expected) in cases {
            let result = CommandResult::resolve(
                "scan",
                None,
                Vec::new(),
                vec![Diagnostic::new(
                    code,
                    DiagnosticSeverity::Error,
                    classification,
                    DiagnosticImpact::BlocksCommand,
                    "wording is irrelevant",
                )],
                None,
            );
            assert_eq!(result.outcome.class, expected);
        }
    }
}
