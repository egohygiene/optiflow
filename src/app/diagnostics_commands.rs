//! Pure support entry: no media configuration, state store or provider discovery.
use super::*;
use crate::{cli::DiagnosticsCommand, diagnostics};
pub(super) fn run(command: &DiagnosticsCommand, signals: &SignalState) -> Execution {
    let owner = diagnostics::owner();
    let cancel = || signals.is_cancelled();
    let result = (|| -> std::result::Result<serde_json::Value, diagnostics::Error> {
        match command {
            DiagnosticsCommand::Preview { request } => serde_json::to_value(
                owner.preview(&owner.read_request(request, &cancel)?, &cancel)?,
            ),
            DiagnosticsCommand::Bundle { request, output } => serde_json::to_value(
                owner.create_bundle(&owner.read_request(request, &cancel)?, output, &cancel)?,
            ),
            DiagnosticsCommand::Inspect { bundle } => {
                serde_json::to_value(owner.inspect_bundle(bundle, &cancel)?)
            }
            DiagnosticsCommand::Doctor => serde_json::to_value(owner.health()),
        }
        .map_err(|_| diagnostics::Error::Invalid)
    })();
    match result {
        Ok(value) => Execution::success(&value),
        Err(diagnostics::Error::Cancelled) if signals.is_cancelled() => {
            interruption_execution(signals.current(), None)
        }
        Err(error) => {
            let (code, class) = match error {
                diagnostics::Error::Unsupported => (
                    DiagnosticCode::ExecutionUnsupported,
                    DiagnosticClassification::Capability,
                ),
                diagnostics::Error::Changed => (
                    DiagnosticCode::ExecutionSourceStale,
                    DiagnosticClassification::State,
                ),
                diagnostics::Error::Io => (
                    DiagnosticCode::StateTransactionFailed,
                    DiagnosticClassification::Internal,
                ),
                diagnostics::Error::Cancelled => (
                    DiagnosticCode::ExecutionBoundsExceeded,
                    DiagnosticClassification::Input,
                ),
                _ => (
                    DiagnosticCode::InvalidCommandInput,
                    DiagnosticClassification::Input,
                ),
            };
            Execution::failure(Diagnostic::new(
                code,
                DiagnosticSeverity::Error,
                class,
                DiagnosticImpact::BlocksCommand,
                error.to_string(),
            ))
        }
    }
}
