//! Explicit, bounded PNG candidate production and recovery entry points.

use std::path::{Component, Path, PathBuf};

#[cfg(unix)]
use anyhow::Context;
use anyhow::{Result, ensure};
use serde_json::json;
#[cfg(unix)]
use std::fs::File;
use uuid::Uuid;

use super::{Execution, interruption_execution};
use crate::candidate_artifact::{self, Inspection, Status};
use crate::cli::{CandidateCommand, CandidatePngArgs};
use crate::configuration::EffectivePolicyV1;
use crate::domain::{EvidenceDigest, NativePath};
use crate::outcome::{
    ArtifactReference, Diagnostic, DiagnosticClassification, DiagnosticCode, DiagnosticImpact,
    DiagnosticSeverity,
};
use crate::png_production::{self, ProductionFailure, ProductionLimits, ProductionRefusal};
use crate::signals::SignalState;

pub(super) fn run(
    command: CandidateCommand,
    state_directory: &Path,
    policy: &EffectivePolicyV1,
    signals: &SignalState,
) -> Execution {
    let state = match normalized_absolute(state_directory) {
        Ok(state) => state,
        Err(error) => return invalid_input(format!("unsafe state directory: {error}")),
    };
    let root = state.join("candidates");
    let recovered = matches!(&command, CandidateCommand::Recover(_));
    match command {
        CandidateCommand::Png(arguments) => png(&arguments, &state, &root, policy, signals),
        CandidateCommand::Status(arguments) | CandidateCommand::Recover(arguments) => {
            let id = match Uuid::parse_str(&arguments.id) {
                Ok(id) if arguments.id == id.to_string() => id,
                _ => return invalid_input("candidate ID must be a canonical UUID"),
            };
            if let Err(error) = candidate_artifact::validate_private_root(&root) {
                return state_failure(format!("candidate root is unavailable or unsafe: {error}"));
            }
            let inspection = if recovered {
                candidate_artifact::recover(&root, id)
            } else {
                candidate_artifact::inspect(&root.join(id.to_string()))
            };
            status(id, &root, inspection, recovered)
        }
    }
}

fn png(
    arguments: &CandidatePngArgs,
    state: &Path,
    root: &Path,
    policy: &EffectivePolicyV1,
    signals: &SignalState,
) -> Execution {
    let source = match normalized_absolute(&arguments.source) {
        Ok(source) => source,
        Err(error) => return invalid_input(format!("unsafe source path: {error}")),
    };
    let executable = match normalized_absolute(&arguments.oxipng) {
        Ok(executable) => executable,
        Err(error) => return invalid_input(format!("unsafe provider path: {error}")),
    };
    if let Err(error) = create_private_root(state, root) {
        return invalid_input(format!(
            "cannot establish a private candidate root: {error}"
        ));
    }
    let evidence_policy_fingerprint = EvidenceDigest {
        algorithm: policy.fingerprints.evidence_policy.algorithm.clone(),
        value: policy.fingerprints.evidence_policy.value.clone(),
    };
    let receipt = png_production::produce_png(
        &source,
        &executable,
        root,
        &evidence_policy_fingerprint,
        ProductionLimits::default(),
        || signals.is_cancelled(),
    );
    match receipt {
        Ok(receipt) => {
            let mut result = Execution::success(&json!({
                "set_id": receipt.set_id,
                "status": "committed",
                "directory": NativePath::from_path(&receipt.directory),
                "encoded_logical_reduction_bytes": receipt.evidence.encoded_logical_reduction_bytes,
                "physical_savings_bytes": receipt.evidence.physical_savings_bytes,
            }));
            result.artifacts = references(&receipt.directory);
            result
        }
        Err(failure) => production_failure(failure, signals),
    }
}

fn references(directory: &Path) -> Vec<ArtifactReference> {
    [
        (
            "png_candidate_artifact_set",
            candidate_artifact::SCHEMA,
            candidate_artifact::MARKER_NAME,
        ),
        (
            "png_candidate_evidence",
            candidate_artifact::EVIDENCE_SCHEMA,
            candidate_artifact::EVIDENCE_NAME,
        ),
        (
            "png_candidate",
            "image/png",
            candidate_artifact::CANDIDATE_NAME,
        ),
    ]
    .into_iter()
    .map(|(kind, schema, name)| ArtifactReference {
        kind: kind.to_owned(),
        schema: schema.to_owned(),
        run_id: None,
        path: NativePath::from_path(&directory.join(name)),
    })
    .collect()
}

fn status(id: Uuid, root: &Path, inspection: Inspection, recovered: bool) -> Execution {
    let discarded_stage = recovered
        && inspection
            .detail
            .starts_with("abandoned candidate staging discarded;");
    let status_name = match inspection.status {
        Status::Committed => "committed",
        Status::Incomplete => "incomplete",
        Status::Incompatible => "incompatible",
    };
    let mut result = Execution::success(&json!({
        "set_id": id,
        "status": status_name,
        "detail": inspection.detail,
        "manifest": inspection.manifest,
    }));
    match inspection.status {
        Status::Committed => {
            result.artifacts = references(&root.join(id.to_string()));
        }
        Status::Incomplete if discarded_stage => {}
        Status::Incomplete => {
            result.diagnostics.push(Diagnostic::new(
                DiagnosticCode::ArtifactSetIncomplete,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                "candidate has no verified committed set",
            ));
        }
        Status::Incompatible => {
            result.diagnostics.push(Diagnostic::new(
                DiagnosticCode::ArtifactSetIncompatible,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                "candidate set is incompatible or ambiguous; inspect it before taking further action",
            ));
        }
    }
    result
}

fn production_failure(failure: ProductionFailure, signals: &SignalState) -> Execution {
    if failure.reason == ProductionRefusal::Cancelled {
        return interruption_execution(
            signals.current(),
            failure.set_id.map(|id| id.to_string()).as_deref(),
        );
    }
    let (code, classification) = match failure.reason {
        ProductionRefusal::InvalidInput => (
            DiagnosticCode::CandidateInputInvalid,
            DiagnosticClassification::Input,
        ),
        ProductionRefusal::UnsupportedPng => (
            DiagnosticCode::CandidateUnsupportedPng,
            DiagnosticClassification::Input,
        ),
        ProductionRefusal::InvalidPng => (
            DiagnosticCode::CandidateInvalidPng,
            DiagnosticClassification::Input,
        ),
        ProductionRefusal::ChangedSource => (
            DiagnosticCode::CandidateSourceChanged,
            DiagnosticClassification::State,
        ),
        ProductionRefusal::ProviderUnavailable => (
            DiagnosticCode::CandidateProviderUnavailable,
            DiagnosticClassification::Capability,
        ),
        ProductionRefusal::ProviderChanged => (
            DiagnosticCode::CandidateProviderChanged,
            DiagnosticClassification::State,
        ),
        ProductionRefusal::ProviderFailed => (
            DiagnosticCode::CandidateProviderFailed,
            DiagnosticClassification::Capability,
        ),
        ProductionRefusal::ProviderTimedOut => (
            DiagnosticCode::CandidateProviderTimedOut,
            DiagnosticClassification::Capability,
        ),
        ProductionRefusal::OutputBoundExceeded => (
            DiagnosticCode::CandidateOutputBoundExceeded,
            DiagnosticClassification::Input,
        ),
        ProductionRefusal::CandidateChanged => (
            DiagnosticCode::CandidateValidationFailed,
            DiagnosticClassification::Capability,
        ),
        ProductionRefusal::CandidateNotSmaller => (
            DiagnosticCode::CandidateNotSmaller,
            DiagnosticClassification::Input,
        ),
        ProductionRefusal::ArtifactUncommitted => (
            DiagnosticCode::CandidateUncommitted,
            DiagnosticClassification::State,
        ),
        ProductionRefusal::Cancelled => unreachable!(),
    };
    let mut diagnostic = Diagnostic::new(
        code,
        DiagnosticSeverity::Error,
        classification,
        DiagnosticImpact::BlocksCommand,
        failure.detail,
    );
    let mut result = Execution::failure({
        diagnostic.context.run_id = failure.set_id.map(|id| id.to_string());
        diagnostic
    });
    if let Some(id) = failure.set_id {
        result.result = Some(json!({
            "set_id": id,
            "status": "uncommitted_or_ambiguous",
            "recovery": "candidate recover --id",
        }));
    }
    result
}

fn invalid_input(message: impl Into<String>) -> Execution {
    Execution::failure(Diagnostic::new(
        DiagnosticCode::CandidateInputInvalid,
        DiagnosticSeverity::Error,
        DiagnosticClassification::Input,
        DiagnosticImpact::BlocksCommand,
        message,
    ))
}

fn state_failure(message: impl Into<String>) -> Execution {
    Execution::failure(Diagnostic::new(
        DiagnosticCode::ArtifactSetIncompatible,
        DiagnosticSeverity::Error,
        DiagnosticClassification::State,
        DiagnosticImpact::BlocksCommand,
        message,
    ))
}

fn normalized_absolute(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    ensure!(
        !absolute
            .components()
            .any(|part| matches!(part, Component::ParentDir)),
        "parent-directory components are prohibited"
    );
    let normalized: PathBuf = absolute
        .components()
        .filter(|part| !matches!(part, Component::CurDir))
        .collect();
    ensure!(normalized.is_absolute(), "path is not absolute");
    Ok(normalized)
}

#[cfg(unix)]
fn create_private_root(state: &Path, root: &Path) -> Result<()> {
    use rustix::fs::{Mode, OFlags, mkdirat, open, openat};
    use rustix::io::Errno;

    ensure!(
        root == state.join("candidates"),
        "candidate root is not within state"
    );
    let flags =
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let mut directory = File::from(open("/", flags, Mode::empty())?);
    for part in root.components() {
        let Component::Normal(name) = part else {
            continue;
        };
        let child = match openat(&directory, name, flags, Mode::empty()) {
            Ok(child) => child,
            Err(Errno::NOENT) => {
                mkdirat(&directory, name, Mode::RUSR | Mode::WUSR | Mode::XUSR).with_context(
                    || format!("failed to create directory component of {}", root.display()),
                )?;
                directory
                    .sync_all()
                    .context("failed to synchronize candidate directory creation")?;
                openat(&directory, name, flags, Mode::empty())
                    .context("new candidate directory component changed")?
            }
            Err(error) => {
                return Err(error).context("unsafe or unavailable candidate path component");
            }
        };
        directory = File::from(child);
    }
    candidate_artifact::validate_private_root(root)
        .context("candidate root must be private and have no symlink components")?;
    Ok(())
}

#[cfg(not(unix))]
fn create_private_root(_state: &Path, _root: &Path) -> Result<()> {
    anyhow::bail!("candidate publication requires Unix no-follow directory handles")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_paths_refuse_parent_traversal() {
        assert!(normalized_absolute(Path::new("../source.png")).is_err());
        assert!(normalized_absolute(Path::new("/tmp/../source.png")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn candidate_root_rejects_symlinked_state_component() {
        use std::os::unix::fs::symlink;
        let base = tempfile::tempdir().unwrap();
        let actual = base.path().join("actual");
        std::fs::create_dir(&actual).unwrap();
        let link = base.path().join("link");
        symlink(&actual, &link).unwrap();
        assert!(create_private_root(&link, &link.join("candidates")).is_err());
        assert!(!actual.join("candidates").exists());
    }
}
