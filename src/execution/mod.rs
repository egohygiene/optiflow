//! Approved exact-duplicate dry runs. Mutation and restore remain unsupported.
mod filesystem;
mod journal;
pub mod model;
mod mutation;
mod recovery;
mod validation;

pub use journal::load_execution;
pub use mutation::{MutationRun, MutationStatus, apply_quarantine, load_mutation};
pub use recovery::{RecoveryReport, cleanup, restore, resume, status as execution_status};
pub use validation::{approve, create_plan, dry_run, load_approval, load_plan, write_document};

use crate::outcome::{
    Diagnostic, DiagnosticClassification, DiagnosticCode, DiagnosticImpact, DiagnosticSeverity,
};

pub type Result<T> = std::result::Result<T, Box<Diagnostic>>;

fn failure(code: DiagnosticCode, message: impl Into<String>) -> Box<Diagnostic> {
    let classification = match code {
        DiagnosticCode::ExecutionPlanInvalid
        | DiagnosticCode::ExecutionApprovalRequired
        | DiagnosticCode::ExecutionApprovalMismatch
        | DiagnosticCode::ExecutionBoundsExceeded
        | DiagnosticCode::ExecutionScopeInvalid => DiagnosticClassification::Input,
        DiagnosticCode::ExecutionReadOnly
        | DiagnosticCode::ExecutionCapacityUnavailable
        | DiagnosticCode::ExecutionCapacityInsufficient
        | DiagnosticCode::ExecutionUnsupported => DiagnosticClassification::Capability,
        DiagnosticCode::OperationInterrupted | DiagnosticCode::OperationTerminated => {
            DiagnosticClassification::Interruption
        }
        DiagnosticCode::StateTransactionFailed => DiagnosticClassification::Internal,
        _ => DiagnosticClassification::State,
    };
    Box::new(Diagnostic::new(
        code,
        DiagnosticSeverity::Error,
        classification,
        DiagnosticImpact::BlocksCommand,
        message,
    ))
}

fn digest<T: serde::Serialize>(value: &T) -> Result<String> {
    // serde_json's default map is sorted: whitespace and object key order do not
    // change identity; array order and every declared semantic field do.
    let canonical = serde_json::to_value(value)
        .and_then(|v| serde_json::to_vec(&v))
        .map_err(|e| failure(DiagnosticCode::ExecutionPlanInvalid, e.to_string()))?;
    Ok(blake3::hash(&canonical).to_hex().to_string())
}
