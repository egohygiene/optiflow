//! Bounded read-only prerequisites. No subprocess, service connection, native
//! version probe, tool search, writable state or guard qualification.
use super::*;
#[cfg(unix)] use anyhow::Context;
use std::time::{Duration,Instant};

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize)]
#[serde(rename_all="snake_case")]
pub enum State {Observed,Unavailable,Unverified}
#[derive(Debug,Serialize)]
pub struct Check {
    pub id:String,pub state:State,pub detail:String,
    pub available_bytes:Option<u64>,pub required_bytes:Option<u64>,
}
#[derive(Debug,Serialize)]
pub struct Report {
    pub schema:String,pub profile:String,pub request_digest:EvidenceDigest,
    pub state:String,pub checks:Vec<Check>,pub plan:Option<Plan>,
    pub observation_scope:String,pub provider_invoked:bool,pub service_connected:bool,
    pub space_reserved:bool,pub runtime_guard_verified:bool,pub native_qualified:bool,
    pub source_mutation:bool,pub replacement_authority:bool,pub upload_authority:bool,
}
fn check(id:&str,state:State,detail:&str)->Check {Check {id:id.into(),state,detail:detail.into(),available_bytes:None,required_bytes:None}}
fn user_bus(path:&Path)->Result<()> {
    #[cfg(unix)]{
        use std::os::unix::fs::{FileTypeExt,MetadataExt};
        let parent=path.parent().context("user bus parent absent")?;
        let bound=crate::candidate_artifact::open_directory(parent)?;let owner=bound.metadata()?.uid();
        ensure!(path==Path::new(&format!("/run/user/{owner}/bus"))&&std::fs::symlink_metadata(path)?.file_type().is_socket(),"explicit local user bus socket unavailable");
        ensure!(path.canonicalize()?==path,"user bus locator changed");Ok(())
    }
    #[cfg(not(unix))]{let _=path;anyhow::bail!("local user-manager observation unavailable")}
}
pub fn observe(request:&Request,cancel:impl Fn()->bool)->Result<Report> {
    request.validate()?;let started=Instant::now();let deadline=Duration::from_millis(request.limits.elapsed_ms.min(30_000));
    let guard=||{ensure!(!cancel()&&started.elapsed()<deadline,"MP4 preflight cancelled or cooperative observation deadline exhausted");Ok(())};
    guard()?;let mut checks=vec![if cfg!(target_os="linux"){check("host",State::Observed,"selected Linux host family; actual native guard remains unverified")}else{check("host",State::Unavailable,"production requires the selected Linux user-manager/cgroup/tmpfs guard") }];
    let source=match super::inspect(&request.source,request.limits.input_bytes,||cancel()||started.elapsed()>=deadline){
        Ok(source)=>{checks.push(check("source",State::Observed,"bounded source identity and independent supported container observed; native decode remains required"));Some(source)},
        Err(_)=>{guard()?;checks.push(check("source",State::Unavailable,"select a current singly-linked regular source within the declared MP4 subset and input bound"));None},
    };
    let mut tools=vec![];
    for (name,pin) in [("ffmpeg",&request.ffmpeg),("ffprobe",&request.ffprobe),("systemd_run",&request.sandbox.systemd_run),("systemctl",&request.sandbox.systemctl),("worker",&request.sandbox.helper)] {
        guard()?;match planning::observe_tool(name,pin,&guard){Ok(tool)=>{tools.push(tool);checks.push(check(name,State::Observed,"exact executable bytes and mode observed; version/build/codec support is not executed"));},Err(_)=>{guard()?;checks.push(check(name,State::Unavailable,"select the exact pinned regular executable with supported non-privileged execution mode"));}}
    }
    let required=runtime::required_free(request)?;
    for (name,path) in [("private_workdir",&request.private_workdir),("artifact_root",&request.artifact_root)] {
        guard()?;let result=(||->Result<u64>{crate::candidate_artifact::validate_private_root(path)?;runtime::available_free(path)})();
        let mut item=match result {
            Ok(available)=>{let mut item=check(name,if available>=required{State::Observed}else{State::Unavailable},if available>=required{"private root and requested free-space allowance observed; no space is reserved"}else{"observed available bytes are below the requested working allowance; free space or explicitly revise the budget"});item.available_bytes=Some(available);item},
            Err(_)=>check(name,State::Unavailable,"prepare the explicit no-follow private directory and supported storage observation before running"),
        };item.required_bytes=Some(required);checks.push(item);
    }
    guard()?;checks.push(if user_bus(&request.sandbox.user_bus).is_ok(){check("user_bus",State::Observed,"explicit local socket observed without connecting; user manager health/delegation is unverified")}else{check("user_bus",State::Unavailable,"select an existing supported /run/user/UID/bus socket; no service is started automatically")});
    let plan=if let Some(source)=source {if tools.len()==5 {match planning::assemble(request,source,tools){Ok(plan)=>{checks.push(check("admission",State::Observed,"source-specific rate and whole-work estimates fit selected limits; runtime enforcement remains separate"));Some(plan)},Err(_)=>{checks.push(check("admission",State::Unavailable,"copied audio/mux, input/candidate scratch, host buffer or provider quota exceeds selected admission; revise explicit settings"));None}}}else{checks.push(check("admission",State::Unverified,"source/tool observations are required before assembling a bound plan"));None}}else{checks.push(check("admission",State::Unverified,"a supported observed source is required before assembling a bound plan"));None};
    checks.push(check("runtime_guard",State::Unverified,"actual cgroup delegation, confinement, aggregate tmpfs and native decode are checked only by explicit production"));guard()?;
    let report=Report {schema:"optiflow.mp4-preflight.v1".into(),profile:PROFILE.into(),request_digest:digest(request)?,state:if checks.iter().any(|c|c.state==State::Unavailable){"prerequisites_unavailable"}else{"prerequisites_observed_guard_unverified"}.into(),checks,plan,observation_scope:"bounded point observations; no native version/codec execution, reservation, delegation or current supported-host qualification; producer rechecks before and during execution".into(),provider_invoked:false,service_connected:false,space_reserved:false,runtime_guard_verified:false,native_qualified:false,source_mutation:false,replacement_authority:false,upload_authority:false};
    ensure!(serde_json::to_vec(&report)?.len()<=METADATA_BYTES,"MP4 preflight report exceeds bound");Ok(report)
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn unavailable_prerequisites_remain_read_only_and_do_not_claim_native_success(){
        let temporary=tempfile::tempdir().unwrap();let root=temporary.path().canonicalize().unwrap();let pin=|name:&str|crate::video_sandbox::PinnedLauncher {path:root.join(name),digest:hash(b"synthetic missing pin")};
        let request=Request {schema:planning::REQUEST_SCHEMA.into(),profile:PROFILE.into(),source:root.join("missing.mp4"),ffmpeg:pin("ffmpeg"),ffprobe:pin("ffprobe"),sandbox:crate::video_sandbox::LinuxVideoSandbox {schema:planning::SANDBOX_SCHEMA.into(),systemd_run:pin("systemd-run"),systemctl:pin("systemctl"),helper:pin("optiflow"),user_bus:PathBuf::from("/run/user/4294967294/bus"),provider_subtree_memory_bytes:1_073_741_824,provider_scratch_bytes:536_870_912,minimum_free_bytes:0},private_workdir:root.join("work"),artifact_root:root.join("store"),control:RateControl::Quality {crf:22,preset:Preset::Slow},audio:"copy".into(),metadata:"preserve_supported_allow_encoder_change".into(),limits:Limits {input_bytes:1_048_576,candidate_bytes:1_048_576,elapsed_ms:10000,memory_admission_bytes:2_147_483_648,working_bytes:1_073_741_824,minimum_free_bytes:0}};
        let report=observe(&request,||false).unwrap();assert_eq!(report.state,"prerequisites_unavailable");assert!(report.plan.is_none());assert!(!report.provider_invoked&&!report.service_connected&&!report.native_qualified&&!report.runtime_guard_verified&&!report.space_reserved);assert_eq!(std::fs::read_dir(&root).unwrap().count(),0);
    }
}
