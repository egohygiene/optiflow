//! Thin routing through the existing configuration owner, without mutable state.
use anyhow::{Context,Result};
use super::Execution;
use crate::{cli::{Mp4DeliveryCommand,Mp4RequestArgs},configuration::{ConfigurationResolution,mp4 as settings},mp4_delivery as mp4,signals::SignalState};
use crate::outcome::{Diagnostic,DiagnosticCode,DiagnosticSeverity,DiagnosticClassification,DiagnosticImpact};

fn overrides(a:&Mp4RequestArgs,signals:&SignalState)->Result<Option<settings::Settings>> {a.settings.as_ref().map(|path|settings::overrides(&crate::record_io::read(path,131_072,signals)?)).transpose()}
fn resolve(a:&Mp4RequestArgs,configuration:Option<&ConfigurationResolution>,signals:&SignalState)->Result<settings::Resolution> {
    let configured=&configuration.context("MP4 configuration resolution absent")?.mp4_delivery;
    let bytes=crate::record_io::read(&a.request,131_072,signals)?;settings::resolve(&bytes,configured,overrides(a,signals)?.as_ref())
}
fn resolve_folder(a:&Mp4RequestArgs,configuration:Option<&ConfigurationResolution>,signals:&SignalState)->Result<settings::FolderResolution> {
    let configured=&configuration.context("MP4 configuration resolution absent")?.mp4_delivery;
    let bytes=crate::record_io::read(&a.request,131_072,signals)?;settings::resolve_folder(&bytes,configured,overrides(a,signals)?.as_ref())
}
pub(super) fn run(command:&Mp4DeliveryCommand,configuration:Option<&ConfigurationResolution>,signals:&SignalState)->Execution {
    let result=(||->Result<serde_json::Value>{
        let cancel=||signals.is_cancelled();
        match command {
            Mp4DeliveryCommand::Profiles=>Ok(mp4::profiles()),
            Mp4DeliveryCommand::Inspect(a)=>Ok(serde_json::to_value(mp4::inspect(&a.source,a.maximum_input_bytes,cancel)?)?),
            Mp4DeliveryCommand::Resolve(a)=>Ok(serde_json::to_value(resolve(a,configuration,signals)?)?),
            Mp4DeliveryCommand::Preflight(a)=>Ok(serde_json::to_value(mp4::preflight::observe(&resolve(a,configuration,signals)?.request,cancel)?)?),
            Mp4DeliveryCommand::Plan(a)|Mp4DeliveryCommand::Run(a)=>{
                let request=resolve(a,configuration,signals)?.request;
                if matches!(command,Mp4DeliveryCommand::Plan(_)){Ok(serde_json::to_value(mp4::plan(&request,cancel)?)?)}else{Ok(serde_json::to_value(mp4::produce(&request,cancel)?)?)}
            },
            Mp4DeliveryCommand::Status(a)=>{
                let bytes=crate::record_io::read(&a.pin,131_072,signals)?;
                let pin:mp4::artifact::Pin=serde_json::from_slice(&bytes)?;
                Ok(serde_json::to_value(mp4::artifact::inspect(&pin)?)?)
            },
            Mp4DeliveryCommand::FolderPreview(a)|Mp4DeliveryCommand::FolderRun(a)|Mp4DeliveryCommand::FolderResume(a)=>{
                let request=resolve_folder(a,configuration,signals)?.request;
                match command {
                    Mp4DeliveryCommand::FolderPreview(_)=>Ok(serde_json::to_value(mp4::folder::preview(&request,cancel)?)?),
                    Mp4DeliveryCommand::FolderRun(_)=>Ok(serde_json::to_value(mp4::folder::run(&request,cancel)?)?),
                    Mp4DeliveryCommand::FolderResume(_)=>Ok(serde_json::to_value(mp4::folder::resume(&request,cancel)?)?),
                    _=>unreachable!(),
                }
            },
            Mp4DeliveryCommand::FolderResolve(a)=>Ok(serde_json::to_value(resolve_folder(a,configuration,signals)?)?),
            Mp4DeliveryCommand::FolderStatus(a)=>Ok(serde_json::to_value(mp4::folder::status(&a.run_directory)?)?),
        }
    })();
    match result {Ok(value)=>Execution::success(&value),Err(error)=>Execution::failure(Diagnostic::new(DiagnosticCode::ExecutionUnsupported,DiagnosticSeverity::Error,DiagnosticClassification::Capability,DiagnosticImpact::BlocksCommand,error.to_string()))}
}

#[cfg(test)]mod tests {
    use crate::cli::Cli;
    use clap::Parser;
    #[test]fn mp4_cli_requires_explicit_requests_and_has_no_replacement_or_upload_path(){
        assert!(Cli::try_parse_from(["optiflow","mp4-delivery","run"]).is_err());
        for flag in ["--replace-original","--upload"]{assert!(Cli::try_parse_from(["optiflow","mp4-delivery","run","--request","/synthetic/request.json",flag]).is_err());}
        let parsed=Cli::try_parse_from(["optiflow","--json","mp4-delivery","inspect","--source","/synthetic/曲.mp4","--maximum-input-bytes","1048576"]).unwrap();
        assert_eq!(parsed.command_name(),"mp4-delivery inspect");
        assert!(matches!(parsed.command,crate::cli::Command::Mp4Delivery(crate::cli::Mp4DeliveryArgs {command:crate::cli::Mp4DeliveryCommand::Inspect(a)}) if a.source==std::path::Path::new("/synthetic/曲.mp4")&&a.maximum_input_bytes==1048576));
    }
}
