use anyhow::{Result,ensure};
use super::Execution;
use crate::{cli::{VideoRequestArgs,VideoFolderCommand},signals::SignalState};
use crate::outcome::{Diagnostic,DiagnosticCode,DiagnosticSeverity,DiagnosticClassification,DiagnosticImpact};
fn outcome(result:Result<serde_json::Value>)->Execution {match result{Ok(v)=>Execution::success(&v),Err(e)=>Execution::failure(Diagnostic::new(DiagnosticCode::ExecutionUnsupported,DiagnosticSeverity::Error,DiagnosticClassification::Capability,DiagnosticImpact::BlocksCommand,e.to_string()))}}
pub(super) fn candidate(a:&VideoRequestArgs,signals:&SignalState)->Execution {outcome((||{
    let bytes=crate::record_io::read(&a.request,131_072,signals)?;let raw:serde_json::Value=serde_json::from_slice(&bytes)?;
    let schema=raw["schema"].as_str().ok_or_else(||anyhow::anyhow!("explicit video production schema required"))?;
    let profile=crate::video_profile::Profile::production(schema)?;crate::contracts::validate(profile.production_contract(),&raw)?;
    let request:crate::video_production::VideoProductionRequest=serde_json::from_slice(&bytes)?;
    Ok(serde_json::to_value(crate::video_production::produce(&request,||signals.is_cancelled())?)?)
})())}
pub(super) fn folder(command:VideoFolderCommand,signals:&SignalState)->Execution {outcome((||{
    if let VideoFolderCommand::Status(a)=&command {return Ok(serde_json::to_value(crate::video_folder::inspect_recovery(&a.run_directory)?)?);}
    let a=match &command{VideoFolderCommand::Discover(a)|VideoFolderCommand::Run(a)|VideoFolderCommand::Resume(a)=>a,_=>unreachable!()};
    let bytes=crate::record_io::read(&a.request,131_072,signals)?;let raw:serde_json::Value=serde_json::from_slice(&bytes)?;
    let schema=raw["schema"].as_str().unwrap_or("");ensure!(matches!(schema,"optiflow.video-folder-request.v1"|"optiflow.video-folder-request.v2"),"explicit supported video folder schema required");
    crate::contracts::validate(if schema.ends_with(".v2"){crate::contracts::Contract::VideoFolderProtocolV2}else{crate::contracts::Contract::VideoFolderProtocol},&raw)?;
    let request:crate::video_folder::VideoFolderRequest=serde_json::from_slice(&bytes)?;
    match command {VideoFolderCommand::Discover(_)=>Ok(serde_json::to_value(crate::video_folder::discover(&request,||signals.is_cancelled())?)?),VideoFolderCommand::Run(_)=>Ok(serde_json::to_value(crate::video_folder::run(&request,||signals.is_cancelled())?)?),VideoFolderCommand::Resume(_)=>Ok(serde_json::to_value(crate::video_folder::resume(&request,||signals.is_cancelled())?)?),_=>unreachable!()}
})())}
