use std::path::Path;

use serde::Serialize;

use crate::adapters::ffprobe;
use crate::cli::{CacheCommand, Cli, Command, ConfigCommand, OutputFormat, PlanCommand};
use crate::configuration::ConfigurationResolution;
use crate::domain::{DoctorReport, NativePath};
use crate::outcome::{
    ArtifactReference, CommandResult, CoverageStatus, Diagnostic, DiagnosticClassification,
    DiagnosticCode, DiagnosticImpact, DiagnosticSeverity,
};
use crate::signals::{Interruption, SignalState};
use crate::state::StateStore;

mod extension_commands;
mod reporting;
mod scan;

pub(super) struct Execution {
    pub(super) coverage: Option<CoverageStatus>,
    pub(super) artifacts: Vec<ArtifactReference>,
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) result: Option<serde_json::Value>,
}

impl Execution {
    pub(super) fn success<T: Serialize>(result: &T) -> Self {
        Self {
            coverage: None,
            artifacts: Vec::new(),
            diagnostics: Vec::new(),
            result: serde_json::to_value(result).ok(),
        }
    }

    pub(super) fn failure(diagnostic: Diagnostic) -> Self {
        Self {
            coverage: None,
            artifacts: Vec::new(),
            diagnostics: vec![diagnostic],
            result: None,
        }
    }
}

pub fn run(cli: Cli, signals: &SignalState) -> (CommandResult, OutputFormat) {
    let command_name = cli.command_name();
    let fallback_output_format = cli.selected_output_format();
    let resolution = match crate::configuration::resolve(&cli) {
        Ok(resolution) => resolution,
        Err(diagnostic) => {
            return (
                CommandResult::failure(command_name, *diagnostic),
                fallback_output_format,
            );
        }
    };
    let output_format = resolution.runtime.output_format;
    let state_directory = resolution.runtime.state_directory.clone();
    let scan_options = resolution.runtime.scan.clone();
    let effective_policy = resolution.policy.clone();
    if signals.is_cancelled() {
        let execution = interruption_execution(signals.current(), None);
        return (
            CommandResult::resolve(
                command_name,
                execution.coverage,
                execution.artifacts,
                execution.diagnostics,
                execution.result,
            ),
            output_format,
        );
    }
    let execution = match cli.command {
        Command::Doctor => run_doctor(&state_directory),
        Command::Scan(arguments) => scan::run(
            &state_directory,
            &arguments,
            &scan_options,
            &effective_policy,
            signals,
        ),
        Command::Report(arguments) => {
            reporting::run_report(&state_directory, &arguments.run, &effective_policy)
        }
        Command::Plan(arguments) => match arguments.command {
            PlanCommand::ExactDuplicates(arguments) => reporting::run_plan(
                &state_directory,
                &arguments.run,
                arguments.output.as_deref(),
                &effective_policy,
            ),
        },
        Command::Cache(arguments) => match arguments.command {
            CacheCommand::Status => run_cache_status(&state_directory),
        },
        Command::Config(arguments) => run_config(&resolution, arguments.command),
        Command::Extensions(arguments) => extension_commands::run(arguments.command),
    };
    (
        CommandResult::resolve(
            command_name,
            execution.coverage,
            execution.artifacts,
            execution.diagnostics,
            execution.result,
        ),
        output_format,
    )
}

fn run_doctor(state_directory: &Path) -> Execution {
    let state_ready = StateStore::open(state_directory).is_ok();
    let report = DoctorReport {
        optiflow_version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        state_directory: state_directory.to_string_lossy().into_owned(),
        state_ready,
        tools: vec![
            ffprobe::status("ffprobe", "optional media stream inventory"),
            ffprobe::status("ffmpeg", "future transactional media validation"),
        ],
    };
    Execution::success(&report)
}

fn run_cache_status(state_directory: &Path) -> Execution {
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
    match store.cache_status() {
        Ok(status) => Execution::success(&status),
        Err(error) => internal_failure_with_code(
            DiagnosticCode::StateTransactionFailed,
            "failed to inspect cache state",
            &error,
        ),
    }
}

fn run_config(resolution: &ConfigurationResolution, command: ConfigCommand) -> Execution {
    match command {
        ConfigCommand::Validate => Execution::success(&serde_json::json!({
            "valid": true,
            "configuration_schema": crate::configuration::CONFIG_SCHEMA,
            "effective_policy_schema": resolution.policy.schema,
            "effective_configuration_digest": resolution
                .policy
                .fingerprints
                .effective_configuration,
            "evidence_policy_fingerprint": resolution.policy.fingerprints.evidence_policy,
            "loaded_source_count": resolution
                .sources
                .iter()
                .filter(|source| matches!(
                    source.status,
                    crate::configuration::ConfigurationSourceStatus::Loaded
                ))
                .count(),
        })),
        ConfigCommand::Show => Execution::success(resolution),
        ConfigCommand::Explain(arguments) => match resolution.explain(&arguments.setting) {
            Ok(explanation) => Execution::success(&explanation),
            Err(diagnostic) => Execution::failure(*diagnostic),
        },
    }
}

pub(super) fn artifact_reference(
    kind: &str,
    schema: &str,
    run_id: &str,
    path: &Path,
) -> ArtifactReference {
    ArtifactReference {
        kind: kind.to_owned(),
        schema: schema.to_owned(),
        run_id: Some(run_id.to_owned()),
        path: NativePath::from_path(path),
    }
}

pub(super) fn internal_failure(message: &str, error: &dyn std::fmt::Display) -> Execution {
    internal_failure_with_code(DiagnosticCode::InternalInvariantViolated, message, error)
}

pub(super) fn internal_failure_with_code(
    code: DiagnosticCode,
    message: &str,
    error: &dyn std::fmt::Display,
) -> Execution {
    Execution::failure(Diagnostic::new(
        code,
        DiagnosticSeverity::Fatal,
        DiagnosticClassification::Internal,
        DiagnosticImpact::BlocksCommand,
        format!("{message}: {error}"),
    ))
}

pub(super) fn interruption_execution(
    interruption: Option<Interruption>,
    run_id: Option<&str>,
) -> Execution {
    let (code, message) = match interruption {
        Some(Interruption::Terminate) => (
            DiagnosticCode::OperationTerminated,
            "the operation was terminated by SIGTERM",
        ),
        _ => (
            DiagnosticCode::OperationInterrupted,
            "the operation was interrupted by SIGINT",
        ),
    };
    let mut diagnostic = Diagnostic::new(
        code,
        DiagnosticSeverity::Error,
        DiagnosticClassification::Interruption,
        DiagnosticImpact::BlocksCommand,
        message,
    );
    diagnostic.context.run_id = run_id.map(str::to_owned);
    Execution::failure(diagnostic)
}
