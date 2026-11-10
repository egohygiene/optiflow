//! Explicit pinned-record CLI; ordinary scan/plan/optimization does not invoke AI.
use anyhow::{Result,ensure};
use super::Execution;
use crate::{cli::{ExplainCommand,ExplanationInputs},explanation::{self,Packet,Candidate,Sidecar,SourceRecord},extensions::Digest,signals::SignalState};
use crate::outcome::{Diagnostic,DiagnosticCode,DiagnosticSeverity,DiagnosticClassification,DiagnosticImpact};
struct Records {report:Vec<u8>,report_digest:Digest,plan:Option<Vec<u8>>,plan_digest:Option<Digest>}
fn read(inputs:&ExplanationInputs,signals:&SignalState)->Result<Records>{
    ensure!(inputs.plan.is_some()==inputs.plan_blake3.is_some(),"plan locator and independent pin must be supplied together");
    let report=crate::record_io::read(&inputs.report,4_194_304,signals)?;
    let report_digest=Digest {algorithm:"blake3-256".into(),value:inputs.report_blake3.clone()};ensure!(Digest::blake3(&report)==report_digest,"report bytes differ from supplied pin");
    let plan=inputs.plan.as_ref().map(|p|crate::record_io::read(p,4_194_304,signals)).transpose()?;
    let plan_digest=inputs.plan_blake3.as_ref().map(|p|Digest {algorithm:"blake3-256".into(),value:p.clone()});
    if let (Some(bytes),Some(pin))=(&plan,&plan_digest){ensure!(Digest::blake3(bytes)==*pin,"plan bytes differ from supplied pin");}
    Ok(Records {report,report_digest,plan,plan_digest})
}
impl Records {
    fn packet(&self,signals:&SignalState)->Result<Packet>{explanation::prepare(true,SourceRecord {bytes:&self.report,digest:&self.report_digest},self.plan.as_ref().zip(self.plan_digest.as_ref()).map(|(bytes,digest)|SourceRecord {bytes,digest}),&||signals.is_cancelled())?.ok_or_else(||anyhow::anyhow!("explanation preparation unavailable"))}
    fn validate(&self,packet:&Packet,candidate:&Candidate,signals:&SignalState)->Result<Sidecar>{explanation::validate_current(SourceRecord {bytes:&self.report,digest:&self.report_digest},self.plan.as_ref().zip(self.plan_digest.as_ref()).map(|(bytes,digest)|SourceRecord {bytes,digest}),packet,candidate,&||signals.is_cancelled())}
}
pub(super) fn run(command:ExplainCommand,signals:&SignalState)->Execution {
    let result=(||->Result<serde_json::Value>{match command {
        ExplainCommand::Prepare(a)=>{let records=read(&a.inputs,signals)?;let p=records.packet(signals)?;
            ensure!(read(&a.inputs,signals)?.packet(signals)?==p,"pinned records changed before packet publication");crate::record_io::write_new(&a.output,&p,65_536)?;Ok(serde_json::to_value(p)?)},
        ExplainCommand::Accept(a)=>{
            let packet=Packet::from_json_slice(&crate::record_io::read(&a.packet,65_536,signals)?)?;
            let candidate=Candidate::from_json_slice(&crate::record_io::read(&a.candidate,65_536,signals)?)?;
            let sidecar=read(&a.inputs,signals)?.validate(&packet,&candidate,signals)?;
            ensure!(read(&a.inputs,signals)?.validate(&packet,&candidate,signals)?==sidecar,"pinned records changed before advisory publication");
            crate::record_io::write_new(&a.output,&sidecar,131_072)?;Ok(serde_json::to_value(sidecar)?)
        },
        ExplainCommand::Inspect(a)=>{
            let packet=Packet::from_json_slice(&crate::record_io::read(&a.packet,65_536,signals)?)?;
            let candidate=Candidate::from_json_slice(&crate::record_io::read(&a.candidate,65_536,signals)?)?;
            let bytes=crate::record_io::read(&a.sidecar,131_072,signals)?;let raw:serde_json::Value=serde_json::from_slice(&bytes)?;
            crate::contracts::validate(crate::contracts::Contract::ExplanationSidecar,&raw)?;let sidecar:Sidecar=serde_json::from_slice(&bytes)?;
            ensure!(read(&a.inputs,signals)?.validate(&packet,&candidate,signals)?==sidecar,"advisory no longer matches pinned records/packet/candidate");Ok(serde_json::to_value(sidecar)?)
        }
    }})();
    match result{Ok(v)=>Execution::success(&v),Err(e)=>Execution::failure(Diagnostic::new(DiagnosticCode::ExecutionPlanInvalid,DiagnosticSeverity::Error,DiagnosticClassification::Input,DiagnosticImpact::BlocksCommand,e.to_string()))}
}
