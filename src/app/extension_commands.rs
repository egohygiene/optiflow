use std::path::Path;

use serde::Serialize;

use crate::cli::{ExtensionSourcesArgs, ExtensionsCommand};
use crate::extensions::{CatalogError, ExtensionCatalog, ExtensionResolutionStatus};
use crate::outcome::{
    CoverageStatus, Diagnostic, DiagnosticClassification, DiagnosticCode, DiagnosticImpact,
    DiagnosticSeverity,
};

use super::Execution;

pub(super) fn run(command: ExtensionsCommand) -> Execution {
    match command {
        ExtensionsCommand::List(sources) => {
            let catalog = match load_extension_catalog(&sources) {
                Ok(catalog) => catalog,
                Err(execution) => return execution,
            };
            let entries = catalog.list();
            let unavailable = entries.iter().filter(|entry| !entry.available).count();
            extension_execution(entries, unavailable, 0)
        }
        ExtensionsCommand::Inspect(arguments) => {
            let catalog = match load_extension_catalog(&arguments.sources) {
                Ok(catalog) => catalog,
                Err(execution) => return execution,
            };
            match catalog.inspect(&arguments.extension_id) {
                Ok(inspection) => {
                    let unavailable = usize::from(!inspection.extension.available);
                    extension_execution(inspection, unavailable, 0)
                }
                Err(error) => extension_catalog_failure(error),
            }
        }
        ExtensionsCommand::Doctor(sources) => {
            let catalog = match load_extension_catalog(&sources) {
                Ok(catalog) => catalog,
                Err(execution) => return execution,
            };
            let report = catalog.doctor();
            let unavailable = report
                .extensions
                .iter()
                .filter(|extension| !extension.available)
                .count();
            let conflicts = report
                .resolutions
                .iter()
                .filter(|resolution| resolution.status == ExtensionResolutionStatus::Conflict)
                .count();
            extension_execution(report, unavailable, conflicts)
        }
    }
}

fn load_extension_catalog(sources: &ExtensionSourcesArgs) -> Result<ExtensionCatalog, Execution> {
    ExtensionCatalog::load(&sources.manifests, &sources.locks).map_err(extension_catalog_failure)
}

fn extension_catalog_failure(error: CatalogError) -> Execution {
    let path = error.path().map(Path::to_path_buf);
    let mut diagnostic = Diagnostic::new(
        DiagnosticCode::InvalidCommandInput,
        DiagnosticSeverity::Error,
        DiagnosticClassification::Input,
        DiagnosticImpact::BlocksCommand,
        error.to_string(),
    );
    if let Some(path) = path {
        diagnostic = diagnostic.with_path(&path);
    }
    Execution::failure(diagnostic)
}

fn extension_execution<T: Serialize>(result: T, unavailable: usize, conflicts: usize) -> Execution {
    let mut execution = Execution::success(&result);
    let affected = unavailable.saturating_add(conflicts);
    if affected > 0 {
        execution.coverage = Some(CoverageStatus::Partial);
        execution.diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::OptionalCapabilityUnavailable,
                DiagnosticSeverity::Warning,
                DiagnosticClassification::Capability,
                DiagnosticImpact::DegradesCoverage,
                format!(
                    "extension inspection found {unavailable} unavailable provider(s) and {conflicts} precedence conflict(s)"
                ),
            )
            .with_count(u64::try_from(affected).unwrap_or(u64::MAX)),
        );
    }
    execution
}
