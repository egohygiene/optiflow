use std::ffi::OsString;
use std::process::ExitCode;

use clap::Parser;
use clap::error::ErrorKind;

use optiflow::cli::{Cli, OutputFormat};
use optiflow::contracts::{self, Contract};
use optiflow::outcome::{
    CommandResult, Diagnostic, DiagnosticClassification, DiagnosticCode, DiagnosticImpact,
    DiagnosticSeverity,
};
use optiflow::signals::SignalState;

fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().collect();
    let requested_format = requested_output_format(&arguments);
    let signals = match SignalState::install() {
        Ok(signals) => signals,
        Err(error) => {
            let result = CommandResult::internal_failure(
                "cli",
                format!("failed to initialize signal handling: {error}"),
            );
            return finish(&result, requested_format);
        }
    };

    match Cli::try_parse_from(&arguments) {
        Ok(cli) => {
            let (result, output_format) = optiflow::run(cli, &signals);
            finish(&result, output_format)
        }
        Err(error) => {
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                let _ = error.print();
                return ExitCode::SUCCESS;
            }
            let message = error.to_string();
            if requested_format == OutputFormat::Human {
                let _ = error.print();
                return ExitCode::from(2);
            }
            let result = CommandResult::failure(
                "cli",
                Diagnostic::new(
                    DiagnosticCode::InvalidInvocation,
                    DiagnosticSeverity::Error,
                    DiagnosticClassification::Input,
                    DiagnosticImpact::BlocksCommand,
                    message.trim(),
                ),
            );
            finish(&result, requested_format)
        }
    }
}

fn finish(result: &CommandResult, format: OutputFormat) -> ExitCode {
    if let Err(error) = contracts::validate(Contract::CommandResult, result) {
        let failure = CommandResult::failure(
            "cli",
            Diagnostic::new(
                DiagnosticCode::ArtifactValidationFailed,
                DiagnosticSeverity::Fatal,
                DiagnosticClassification::Artifact,
                DiagnosticImpact::BlocksCommand,
                format!("the command result failed contract validation: {error}"),
            ),
        );
        let _ = optiflow::render::render(&failure, format);
        return ExitCode::from(1);
    }
    let exit_code = result.outcome.exit_code.get();
    if let Err(error) = optiflow::render::render(result, format) {
        eprintln!("optiflow could not render its command result: {error}");
        return ExitCode::from(1);
    }
    ExitCode::from(exit_code)
}

fn requested_output_format(arguments: &[OsString]) -> OutputFormat {
    let mut iterator = arguments.iter().filter_map(|argument| argument.to_str());
    while let Some(argument) = iterator.next() {
        if argument == "--json" || argument == "--output-format=json" {
            return OutputFormat::Json;
        }
        if argument == "--output-format" && iterator.next() == Some("json") {
            return OutputFormat::Json;
        }
    }
    OutputFormat::Human
}
