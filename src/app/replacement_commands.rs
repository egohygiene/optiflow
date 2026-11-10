use std::path::Path;
use anyhow::Result;
use super::Execution;
use crate::{cli::ReplacementCommand, configuration::EffectivePolicyV1, signals::SignalState};
use crate::execution::replacement::{self,ReplacementPlan,Approval,Limits,transaction};
use crate::outcome::{Diagnostic,DiagnosticCode,DiagnosticSeverity,DiagnosticClassification,DiagnosticImpact};
pub(super) fn run(command:ReplacementCommand,state:&Path,policy:&EffectivePolicyV1,signals:&SignalState)->Execution {
    let mutation=matches!(&command,ReplacementCommand::Apply(_)|ReplacementCommand::Restore(_));
    if mutation {if let Err(e)=transaction::require_qualification(){return Execution::failure(Diagnostic::new(DiagnosticCode::ExecutionUnsupported,DiagnosticSeverity::Error,DiagnosticClassification::Capability,DiagnosticImpact::BlocksCommand,e.to_string()));}}
    let outcome=(||->Result<serde_json::Value>{
        let read_plan=|path:&Path|ReplacementPlan::from_json_slice(&crate::record_io::read(path,4_194_304,signals)?);
        match command {
            ReplacementCommand::ReviewVideo(a)=>{
                let review=crate::execution::video_replacement::review(&a.source,&a.candidate_set,&a.artifact_schema,state,&a.retention,&a.video_policy,signals)?;
                crate::record_io::write_new(&a.output,&review,4_194_304)?;Ok(serde_json::to_value(review)?)
            }
            ReplacementCommand::Review(a)=>{
                let p=replacement::review_png(&a.source,&a.candidate_set,state,&a.retention,&policy.fingerprints.evidence_policy.value,Limits {maximum_file_bytes:a.maximum_file_bytes,free_space_reserve_bytes:a.reserve_bytes,journal_budget_bytes:1_048_576},signals)?;
                crate::record_io::write_new(&a.output,&p,4_194_304)?;Ok(serde_json::to_value(p)?)
            }
            ReplacementCommand::Approve(a)=>{let p=read_plan(&a.plan)?;let approval=replacement::approve(&p,a.approved_by)?;crate::record_io::write_new(&a.output,&approval,16_384)?;Ok(serde_json::to_value(approval)?)},
            ReplacementCommand::Status(a)=>Ok(serde_json::to_value(transaction::status(&read_plan(&a.plan)?)?)?),
            ReplacementCommand::Apply(a)|ReplacementCommand::Restore(a)=>{
                let _p=read_plan(&a.plan)?;let _approval:Approval=serde_json::from_slice(&crate::record_io::read(&a.approval,16_384,signals)?)?;
                anyhow::bail!("replacement mutation qualification pending")
            }
        }
    })();
    match outcome {Ok(v)=>Execution::success(&v),Err(e)=>Execution::failure(Diagnostic::new(DiagnosticCode::ExecutionPlanInvalid,DiagnosticSeverity::Error,DiagnosticClassification::Input,DiagnosticImpact::BlocksCommand,e.to_string()))}
}
