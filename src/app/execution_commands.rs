use super::Execution;
use crate::cli::{ApplyArgs, ApproveArgs, ExecutionPlanArgs};
use crate::configuration::EffectivePolicyV1;
use crate::domain::NativePath;
use crate::execution::{self, model};
use crate::outcome::ArtifactReference;
use crate::outcome::{
    Diagnostic, DiagnosticClassification, DiagnosticCode, DiagnosticImpact, DiagnosticSeverity,
};
use crate::signals::SignalState;
use std::path::Path;

pub(super) fn plan(
    args: &ExecutionPlanArgs,
    state: &Path,
    policy: &EffectivePolicyV1,
    signals: &SignalState,
) -> Execution {
    let result = (|| {
        let plan = execution::create_plan(args, state, policy, signals)?;
        execution::write_document(&args.output, &plan, &plan)?;
        Ok::<_, Box<Diagnostic>>(plan)
    })();
    match result {
        Ok(plan) => {
            let mut result = Execution::success(&plan);
            result.artifacts.push(ArtifactReference {
                kind: "execution_plan".to_owned(),
                schema: model::PLAN_SCHEMA.to_owned(),
                run_id: None,
                path: NativePath::from_path(&args.output),
            });
            result
        }
        Err(diagnostic) => Execution::failure(*diagnostic),
    }
}

pub(super) fn approve(args: &ApproveArgs) -> Execution {
    let result = (|| {
        let plan = execution::load_plan(&args.plan)?;
        let approval = execution::approve(&plan, &args.fingerprint, &args.approved_by)?;
        execution::write_document(&args.output, &approval, &plan)?;
        Ok::<_, Box<Diagnostic>>(approval)
    })();
    match result {
        Ok(approval) => {
            let mut result = Execution::success(&approval);
            result.artifacts.push(ArtifactReference {
                kind: "execution_approval".to_owned(),
                schema: model::APPROVAL_SCHEMA.to_owned(),
                run_id: None,
                path: NativePath::from_path(&args.output),
            });
            result
        }
        Err(diagnostic) => Execution::failure(*diagnostic),
    }
}

pub(super) fn apply(
    args: &ApplyArgs,
    state: &Path,
    policy: &EffectivePolicyV1,
    signals: &SignalState,
) -> Execution {
    if !args.dry_run {
        return Execution::failure(Diagnostic::new(
            DiagnosticCode::ExecutionUnsupported,
            DiagnosticSeverity::Error,
            DiagnosticClassification::Capability,
            DiagnosticImpact::BlocksCommand,
            "only apply --dry-run is supported; source mutation is disabled",
        ));
    }
    let Some(approval_path) = &args.approval else {
        return Execution::failure(Diagnostic::new(
            DiagnosticCode::ExecutionApprovalRequired,
            DiagnosticSeverity::Error,
            DiagnosticClassification::Input,
            DiagnosticImpact::BlocksCommand,
            "--approval is required; a review plan or plan file alone does not authorize execution",
        ));
    };
    let result = (|| {
        let plan = execution::load_plan(&args.plan)?;
        let approval = execution::load_approval(approval_path)?;
        execution::dry_run(&plan, &approval, state, policy, signals)
    })();
    match result {
        Ok(run) => {
            let mut result = Execution::success(&run);
            result.diagnostics = run.validation.diagnostics;
            result
        }
        Err(diagnostic) => Execution::failure(*diagnostic),
    }
}
