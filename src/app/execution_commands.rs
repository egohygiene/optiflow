use super::Execution;
use crate::cli::{
    ApplyArgs, ApproveArgs, ExecutionCommand, ExecutionFinalizeArgs,
    ExecutionFinalizeAuthorizeArgs, ExecutionPlanArgs, ExecutionRecoveryArgs,
};
use crate::configuration::EffectivePolicyV1;
use crate::contracts::Contract;
use crate::domain::NativePath;
use crate::execution::{self, finalization, model};
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
        if args.dry_run {
            execution::dry_run(&plan, &approval, state, policy, signals)
                .map(|run| serde_json::to_value(run).expect("serializable dry-run evidence"))
        } else {
            execution::apply_quarantine(&plan, &approval, state, policy, signals)
                .map(|run| serde_json::to_value(run).expect("serializable mutation evidence"))
        }
    })();
    match result {
        Ok(run) => {
            let mut result = Execution::success(&run);
            if let Some(diagnostics) = run.get("diagnostics") {
                result.diagnostics =
                    serde_json::from_value(diagnostics.clone()).unwrap_or_default();
            } else if let Some(diagnostics) =
                run.get("validation").and_then(|v| v.get("diagnostics"))
            {
                result.diagnostics =
                    serde_json::from_value(diagnostics.clone()).unwrap_or_default();
            }
            result
        }
        Err(diagnostic) => Execution::failure(*diagnostic),
    }
}

fn recover_documents(
    args: &ExecutionRecoveryArgs,
) -> execution::Result<(model::ExecutionPlan, model::Approval)> {
    Ok((
        execution::load_plan(&args.plan)?,
        execution::load_approval(&args.approval)?,
    ))
}

pub(super) fn recovery(
    command: ExecutionCommand,
    state: &Path,
    policy: &EffectivePolicyV1,
    signals: &SignalState,
) -> Execution {
    let artifact = match &command {
        ExecutionCommand::Finalize(args) if !args.commit => args.output.as_ref().map(|path| {
            (
                "execution_finalization_preview",
                finalization::PREVIEW_SCHEMA,
                path.clone(),
            )
        }),
        ExecutionCommand::AuthorizeFinalization(args) => Some((
            "execution_finalization_authorization",
            finalization::AUTHORIZATION_SCHEMA,
            args.output.clone(),
        )),
        _ => None,
    };
    let result = (|| match command {
        ExecutionCommand::Status(args) => {
            let final_status = finalization::status(state, &args.run)?;
            if final_status.events.is_empty() {
                Ok(serde_json::to_value(final_status.recovery_history)
                    .expect("serializable recovery"))
            } else {
                Ok(serde_json::to_value(final_status).expect("serializable finalization"))
            }
        }
        ExecutionCommand::Resume(args) => {
            let (plan, approval) = recover_documents(&args)?;
            execution::resume(&plan, &approval, state, policy, &args.run, signals)
                .map(|v| serde_json::to_value(v).expect("serializable recovery"))
        }
        ExecutionCommand::Restore(args) => {
            let (plan, approval) = recover_documents(&args.recovery)?;
            execution::restore(
                &plan,
                &approval,
                state,
                policy,
                &args.recovery.run,
                &args.action,
                signals,
            )
            .map(|v| serde_json::to_value(v).expect("serializable recovery"))
        }
        ExecutionCommand::Cleanup(args) => {
            let (plan, approval) = recover_documents(&args)?;
            execution::cleanup(&plan, &approval, state, policy, &args.run, signals)
                .map(|v| serde_json::to_value(v).expect("serializable recovery"))
        }
        ExecutionCommand::Finalize(args) => finalize(&args, state, policy, signals),
        ExecutionCommand::AuthorizeFinalization(args) => authorize_finalization(&args),
    })();
    match result {
        Ok(report) => {
            let mut execution = Execution::success(&report);
            if let Some((kind, schema, path)) = artifact {
                execution.artifacts.push(ArtifactReference {
                    kind: kind.to_owned(),
                    schema: schema.to_owned(),
                    run_id: report
                        .get("body")
                        .and_then(|v| v.get("run_id"))
                        .and_then(|v| v.as_str())
                        .map(str::to_owned),
                    path: NativePath::from_path(&path),
                });
            }
            execution
        }
        Err(diagnostic) => Execution::failure(*diagnostic),
    }
}

fn finalize(
    args: &ExecutionFinalizeArgs,
    state: &Path,
    policy: &EffectivePolicyV1,
    signals: &SignalState,
) -> execution::Result<serde_json::Value> {
    let plan = execution::load_plan(&args.plan)?;
    let approval = execution::load_approval(&args.approval)?;
    if args.commit {
        let preview = finalization::load_preview(
            args.preview
                .as_deref()
                .ok_or_else(|| input("--preview is required for --commit"))?,
        )?;
        let authorization = finalization::load_authorization(
            args.authorization
                .as_deref()
                .ok_or_else(|| input("--authorization is required for --commit"))?,
        )?;
        let result = finalization::commit(
            &plan,
            &approval,
            state,
            policy,
            &preview,
            &authorization,
            signals,
        )?;
        Ok(serde_json::to_value(result).expect("serializable finalization"))
    } else {
        let run = args
            .run
            .as_deref()
            .ok_or_else(|| input("--run is required for preview"))?;
        let output = args
            .output
            .as_deref()
            .ok_or_else(|| input("--output is required for preview"))?;
        let result =
            finalization::preview(&plan, &approval, state, policy, run, &args.action, signals)?;
        execution::write_finalization_document(
            output,
            &result,
            &plan,
            Contract::ExecutionFinalizationPreview,
        )?;
        Ok(serde_json::to_value(result).expect("serializable preview"))
    }
}

fn authorize_finalization(
    args: &ExecutionFinalizeAuthorizeArgs,
) -> execution::Result<serde_json::Value> {
    let plan = execution::load_plan(&args.plan)?;
    let preview = finalization::load_preview(&args.preview)?;
    if preview.body.plan_fingerprint != plan.fingerprint {
        return Err(input("preview belongs to a different plan"));
    }
    let authorization = finalization::authorize(&preview, &args.fingerprint, &args.approved_by)?;
    // Preview loading rejects symlinks and malformed documents; output is create-only.
    execution::write_finalization_document(
        &args.output,
        &authorization,
        &plan,
        Contract::ExecutionFinalizationAuthorization,
    )?;
    Ok(serde_json::to_value(authorization).expect("serializable authorization"))
}

fn input(message: &str) -> Box<Diagnostic> {
    Box::new(Diagnostic::new(
        DiagnosticCode::ExecutionPlanInvalid,
        DiagnosticSeverity::Error,
        DiagnosticClassification::Input,
        DiagnosticImpact::BlocksCommand,
        message,
    ))
}
