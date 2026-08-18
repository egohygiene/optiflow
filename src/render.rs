use std::io::{self, Write};

use anyhow::{Context, Result};

use crate::cli::OutputFormat;
use crate::outcome::{CommandOutcomeClass, CommandResult, CoverageStatus, DiagnosticSeverity};

pub fn render(result: &CommandResult, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => render_json(result),
        OutputFormat::Human => render_human(result),
    }
}

fn render_json(result: &CommandResult) -> Result<()> {
    let mut document = serde_json::to_vec_pretty(result)
        .context("failed to serialize the structured command result")?;
    document.push(b'\n');
    io::stdout()
        .lock()
        .write_all(&document)
        .context("failed to write the structured command result")
}

fn render_human(result: &CommandResult) -> Result<()> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "optiflow {}", result.command)?;
    writeln!(stdout, "  Outcome: {}", outcome_label(result.outcome.class))?;
    if result.outcome.exit_code.get() != 0 {
        writeln!(stdout, "  Exit code: {}", result.outcome.exit_code.get())?;
    }
    if let Some(coverage) = result.coverage {
        let label = match coverage.status {
            CoverageStatus::Complete => "complete",
            CoverageStatus::Partial => "partial",
        };
        writeln!(stdout, "  Coverage: {label}")?;
    }
    for artifact in &result.artifacts {
        writeln!(
            stdout,
            "  {} artifact: {}",
            artifact.kind,
            artifact.path.display()
        )?;
    }
    if result.command.starts_with("config ") {
        if let Some(value) = &result.result {
            writeln!(
                stdout,
                "{}",
                serde_json::to_string_pretty(value)
                    .context("failed to render the configuration result")?
            )?;
        }
    }
    stdout.flush()?;

    let mut stderr = io::stderr().lock();
    for diagnostic in &result.diagnostics {
        writeln!(
            stderr,
            "  {} [{}]: {}",
            severity_label(diagnostic.severity),
            serde_json::to_value(diagnostic.code)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown_diagnostic".to_owned()),
            escape_terminal_text(&diagnostic.message)
        )?;
    }
    stderr.flush()?;
    Ok(())
}

fn severity_label(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Information => "info",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Fatal => "fatal",
    }
}

fn escape_terminal_text(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
}

fn outcome_label(class: CommandOutcomeClass) -> &'static str {
    match class {
        CommandOutcomeClass::Success => "success",
        CommandOutcomeClass::InternalFailure => "internal failure",
        CommandOutcomeClass::InvalidInput => "invalid input",
        CommandOutcomeClass::PartialSuccess => "partial success",
        CommandOutcomeClass::CapabilityUnavailable => "capability unavailable",
        CommandOutcomeClass::StaleState => "stale state",
        CommandOutcomeClass::Interrupted => "interrupted",
        CommandOutcomeClass::Terminated => "terminated",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_text_cannot_forge_lines_or_control_sequences() {
        assert_eq!(
            escape_terminal_text("line one\n\u{1b}[31mline two"),
            "line one\\n\\u{1b}[31mline two"
        );
    }
}
