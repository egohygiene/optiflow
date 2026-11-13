//! Thin routing through the existing configuration owner, without mutable state.
use anyhow::{Context,Result,ensure};
use super::Execution;
use crate::{cli::{Mp4DeliveryCommand,Mp4RequestArgs},configuration::{ConfigurationResolution,mp4 as settings},mp4_delivery as mp4,signals::SignalState};
use crate::outcome::{Diagnostic,DiagnosticCode,DiagnosticSeverity,DiagnosticClassification,DiagnosticImpact};
mod progress;

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
    let mut progress_health=None;
    let result=(||->Result<serde_json::Value>{
        let cancel=||signals.is_cancelled();
        if let Mp4DeliveryCommand::Plan(a)|Mp4DeliveryCommand::Resolve(a)|Mp4DeliveryCommand::Preflight(a)|Mp4DeliveryCommand::FolderPreview(a)|Mp4DeliveryCommand::FolderResolve(a)=command {ensure!(a.event_log.is_none()&&a.maximum_events.is_none()&&a.maximum_event_bytes.is_none(),"progress logging is only available for explicit run/folder-run/folder-resume");}
        match command {
            Mp4DeliveryCommand::Profiles=>Ok(mp4::profiles()),
            Mp4DeliveryCommand::Inspect(a)=>Ok(serde_json::to_value(mp4::inspect(&a.source,a.maximum_input_bytes,cancel)?)?),
            Mp4DeliveryCommand::Resolve(a)=>Ok(serde_json::to_value(resolve(a,configuration,signals)?)?),
            Mp4DeliveryCommand::Preflight(a)=>Ok(serde_json::to_value(mp4::preflight::observe(&resolve(a,configuration,signals)?.request,cancel)?)?),
            Mp4DeliveryCommand::Plan(a)|Mp4DeliveryCommand::Run(a)=>{
                let request=resolve(a,configuration,signals)?.request;
                if matches!(command,Mp4DeliveryCommand::Plan(_)){Ok(serde_json::to_value(mp4::plan(&request,cancel)?)?)}else{
                    let forbidden=forbidden(&request,None);let (value,health)=progress::execute(a,&forbidden,|observer|mp4::produce_with_progress(&request,cancel,observer))?;progress_health=Some(health);Ok(serde_json::to_value(value?)?)
                }
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
                    Mp4DeliveryCommand::FolderRun(_)|Mp4DeliveryCommand::FolderResume(_)=>{
                        let forbidden=forbidden(&request.template,Some(&request));let (value,health)=progress::execute(a,&forbidden,|observer|if matches!(command,Mp4DeliveryCommand::FolderRun(_)){mp4::folder::run_with_progress(&request,cancel,observer)}else{mp4::folder::resume_with_progress(&request,cancel,observer)})?;progress_health=Some(health);Ok(serde_json::to_value(value?)?)
                    },
                    _=>unreachable!(),
                }
            },
            Mp4DeliveryCommand::FolderResolve(a)=>Ok(serde_json::to_value(resolve_folder(a,configuration,signals)?)?),
            Mp4DeliveryCommand::FolderStatus(a)=>Ok(serde_json::to_value(mp4::folder::status(&a.run_directory)?)?),
        }
    })();
    let mut execution=match result {Ok(value)=>Execution::success(&value),Err(error)=>Execution::failure(Diagnostic::new(DiagnosticCode::ExecutionUnsupported,DiagnosticSeverity::Error,DiagnosticClassification::Capability,DiagnosticImpact::BlocksCommand,error.to_string()))};
    if progress_health.is_some_and(|h|h.degraded()){execution.diagnostics.push(Diagnostic::new(DiagnosticCode::Mp4ProgressDegraded,DiagnosticSeverity::Warning,DiagnosticClassification::Observation,DiagnosticImpact::None,"some operational progress was dropped or its sink failed; inspect authoritative work results/evidence separately"));}execution
}
fn forbidden<'a>(request:&'a mp4::Request,folder:Option<&'a mp4::folder::Request>)->Vec<&'a std::path::Path> {
    let mut paths=vec![request.source.as_path(),request.private_workdir.as_path(),request.artifact_root.as_path(),request.ffmpeg.path.as_path(),request.ffprobe.path.as_path(),request.sandbox.systemd_run.path.as_path(),request.sandbox.systemctl.path.as_path(),request.sandbox.helper.path.as_path(),request.sandbox.user_bus.as_path()];if let Some(folder)=folder{paths.push(&folder.source_root);paths.push(&folder.run_directory);}paths
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
