use std::path::Path;
use anyhow::{Result,ensure};
use serde::de::DeserializeOwned;
use super::Execution;
use crate::{cli::OptimizeCommand,configuration::EffectivePolicyV1,signals::SignalState,execution::{png_folder::{self,Request,Inventory,BatchApproval,coordinator},replacement::transaction}};
use crate::outcome::{Diagnostic,DiagnosticCode,DiagnosticSeverity,DiagnosticClassification,DiagnosticImpact};
fn read<T:DeserializeOwned>(path:&Path,signals:&SignalState)->Result<T>{let bytes=crate::record_io::read(path,4_194_304,signals)?;let raw:serde_json::Value=serde_json::from_slice(&bytes)?;crate::contracts::validate(crate::contracts::Contract::PngFolderProtocol,&raw)?;Ok(serde_json::from_slice(&bytes)?)}
pub(super) fn run(command:OptimizeCommand,policy:&EffectivePolicyV1,signals:&SignalState)->Execution {
    if matches!(&command,OptimizeCommand::Run(_)|OptimizeCommand::Resume(_)){if let Err(e)=transaction::require_qualification(){return Execution::failure(Diagnostic::new(DiagnosticCode::ExecutionUnsupported,DiagnosticSeverity::Error,DiagnosticClassification::Capability,DiagnosticImpact::BlocksCommand,e.to_string()));}}
    let resume=matches!(&command,OptimizeCommand::Resume(_));let inspect=matches!(&command,OptimizeCommand::Status(_));
    let result=(||->Result<serde_json::Value>{match command {
        OptimizeCommand::Discover(a)=>{let r:Request=read(&a.request,signals)?;ensure!(r.policy_fingerprint==policy.fingerprints.evidence_policy.value,"request policy differs from effective policy");let i=png_folder::discover(&r,signals)?;crate::record_io::write_new(&a.output,&i,4_194_304)?;Ok(serde_json::to_value(i)?)},
        OptimizeCommand::Approve(a)=>{let r:Request=read(&a.request,signals)?;let i:Inventory=read(&a.inventory,signals)?;let approval=png_folder::approve(&r,&i,a.approved_by)?;crate::record_io::write_new(&a.output,&approval,16_384)?;Ok(serde_json::to_value(approval)?)},
        OptimizeCommand::Run(a)|OptimizeCommand::Resume(a)|OptimizeCommand::Status(a)=>{let r:Request=read(&a.request,signals)?;let i:Inventory=read(&a.inventory,signals)?;let approval:BatchApproval=read(&a.approval,signals)?;ensure!(r.policy_fingerprint==policy.fingerprints.evidence_policy.value,"request policy differs from effective policy");let report=if inspect{coordinator::status_guarded(&r,&i,&approval,signals)?}else{coordinator::run(&r,&i,&approval,resume,signals)?};Ok(serde_json::to_value(report)?)},
    }})();
    match result{Ok(v)=>Execution::success(&v),Err(e)=>Execution::failure(Diagnostic::new(DiagnosticCode::ExecutionPlanInvalid,DiagnosticSeverity::Error,DiagnosticClassification::Input,DiagnosticImpact::BlocksCommand,e.to_string()))}
}
