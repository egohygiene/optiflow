//! MP4 policy projection within the existing configuration owner. Resolution is
//! pure after file selection and creates the existing closed producer request.
use std::{collections::BTreeMap,path::PathBuf};
use anyhow::{Context,Result,ensure};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use crate::{domain::EvidenceDigest,mp4_delivery::{self as mp4,RateControl,Request}};
use super::{PolicyProvenance,PolicySourceKind,ShadowedValue};

#[derive(Debug,Clone,Default,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LimitSettings {
    #[serde(skip_serializing_if="Option::is_none")] pub input_bytes:Option<u64>,
    #[serde(skip_serializing_if="Option::is_none")] pub candidate_bytes:Option<u64>,
    #[serde(skip_serializing_if="Option::is_none")] pub elapsed_ms:Option<u64>,
    #[serde(skip_serializing_if="Option::is_none")] pub memory_admission_bytes:Option<u64>,
    #[serde(skip_serializing_if="Option::is_none")] pub working_bytes:Option<u64>,
    #[serde(skip_serializing_if="Option::is_none")] pub minimum_free_bytes:Option<u64>,
}
#[derive(Debug,Clone,Default,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(skip_serializing_if="Option::is_none")] pub control:Option<RateControl>,
    #[serde(default)] pub limits:LimitSettings,
}
#[derive(Debug,Clone,Default,Serialize)]
pub struct State {
    values:BTreeMap<String,Value>,
    provenance:BTreeMap<String,PolicyProvenance>,
    shadowed_values:Vec<ShadowedValue>,
}
impl State {
    pub fn is_empty(&self)->bool {self.values.is_empty()}
    pub(crate) fn apply(&mut self,settings:&Settings,origin:PolicyProvenance)->Result<()> {
        let mut entries=BTreeMap::new();
        if let Some(control)=&settings.control {entries.insert("control".to_owned(),serde_json::to_value(control)?);}
        for (name,value) in serde_json::to_value(&settings.limits)?.as_object().context("MP4 limit object required")? {entries.insert(format!("limits.{name}"),value.clone());}
        for (name,value) in entries {
            if let (Some(old),Some(provenance))=(self.values.get(&name),self.provenance.get(&name)) {self.shadowed_values.push(ShadowedValue {setting:name.clone(),value:old.clone(),provenance:provenance.clone()});}
            let mut provenance=origin.clone();provenance.detail=format!("{}.{name}",origin.detail);
            self.values.insert(name.clone(),value);self.provenance.insert(name,provenance);
        }
        Ok(())
    }
    fn materialize(&self,mut base:Value)->Result<Request> {
        let object=base.as_object_mut().context("MP4 request object required")?;
        object.insert("schema".into(),Value::String(mp4::planning::REQUEST_SCHEMA.into()));
        if let Some(value)=self.values.get("control") {object.insert("control".into(),value.clone());}
        let limits=self.values.iter().filter_map(|(k,v)|k.strip_prefix("limits.").map(|k|(k.to_owned(),v.clone()))).collect::<serde_json::Map<_,_>>();
        object.insert("limits".into(),Value::Object(limits));
        let request:Request=serde_json::from_value(base).context("explicit MP4 control and all six resource limits are required after resolution")?;
        request.validate()?;Ok(request)
    }
}
#[derive(Debug,Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    schema:String,profile:String,source:PathBuf,ffmpeg:crate::video_sandbox::PinnedLauncher,
    ffprobe:crate::video_sandbox::PinnedLauncher,sandbox:crate::video_sandbox::LinuxVideoSandbox,
    private_workdir:PathBuf,artifact_root:PathBuf,audio:String,metadata:String,
    control:Option<RateControl>,#[serde(default)] limits:LimitSettings,
}
impl Selection {
    fn parts(self)->(Value,Settings) {
        (json!({"profile":self.profile,"source":self.source,"ffmpeg":self.ffmpeg,"ffprobe":self.ffprobe,"sandbox":self.sandbox,"private_workdir":self.private_workdir,"artifact_root":self.artifact_root,"audio":self.audio,"metadata":self.metadata}),Settings {control:self.control,limits:self.limits})
    }
}
#[derive(Debug,Serialize)]
pub struct Resolution {
    pub schema:String,pub request:Request,pub request_digest:EvidenceDigest,
    pub provenance:BTreeMap<String,PolicyProvenance>,pub shadowed_values:Vec<ShadowedValue>,
    pub defaults:String,pub provider_invoked:bool,pub source_mutation:bool,
}
fn origin(detail:&str)->PolicyProvenance {PolicyProvenance {source:PolicySourceKind::CommandLine,detail:detail.into(),path:None}}
pub fn resolve(bytes:&[u8],configured:&State,overrides:Option<&Settings>)->Result<Resolution> {
    ensure!(bytes.len()<=131_072,"MP4 selection exceeds 128 KiB");let raw:Value=serde_json::from_slice(bytes)?;
    let (base,explicit)=match raw["schema"].as_str() {
        Some("optiflow.mp4-delivery-request.v1")=>{let request:Request=serde_json::from_slice(bytes)?;let settings=Settings {control:Some(request.control.clone()),limits:serde_json::from_value(serde_json::to_value(&request.limits)?)?};(serde_json::to_value(request)?,settings)},
        Some("optiflow.mp4-delivery-selection.v1")=>{let selection:Selection=serde_json::from_slice(bytes)?;ensure!(selection.schema=="optiflow.mp4-delivery-selection.v1","MP4 selection schema differs");selection.parts()},
        _=>anyhow::bail!("explicit supported MP4 request or selection schema required"),
    };
    let mut state=configured.clone();state.apply(&explicit,origin("request_document"))?;
    if let Some(settings)=overrides {state.apply(settings,origin("cli_settings_document"))?;}
    let request=state.materialize(base)?;let request_digest=mp4::digest(&request)?;
    Ok(Resolution {schema:"optiflow.mp4-settings-resolution.v1".into(),request,request_digest,provenance:state.provenance,shadowed_values:state.shadowed_values,defaults:"none; control and resource limits must be explicitly selected".into(),provider_invoked:false,source_mutation:false})
}
#[derive(Debug,Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsDocument {schema:String,settings:Settings}
pub fn overrides(bytes:&[u8])->Result<Settings> {ensure!(bytes.len()<=131_072,"MP4 CLI settings exceed 128 KiB");let document:SettingsDocument=serde_json::from_slice(bytes)?;ensure!(document.schema=="optiflow.mp4-settings.v1","explicit MP4 settings schema required");Ok(document.settings)}

#[derive(Debug,Deserialize)]
#[serde(deny_unknown_fields)]
struct FolderSelection {schema:String,source_root:PathBuf,recursive:bool,template:Value,run_directory:PathBuf,batch:crate::video_folder::FolderLimits,scratch_cleanup:String}
#[derive(Debug,Serialize)]
pub struct FolderResolution {pub schema:String,pub request:mp4::folder::Request,pub request_digest:EvidenceDigest,pub template_resolution:Resolution}
pub fn resolve_folder(bytes:&[u8],configured:&State,overrides:Option<&Settings>)->Result<FolderResolution> {
    ensure!(bytes.len()<=131_072,"MP4 folder selection exceeds 128 KiB");let raw:Value=serde_json::from_slice(bytes)?;
    let selection:FolderSelection=match raw["schema"].as_str() {
        Some("optiflow.mp4-folder-request.v1")=>{let request:mp4::folder::Request=serde_json::from_slice(bytes)?;serde_json::from_value(serde_json::to_value(request)?)?},
        Some("optiflow.mp4-folder-selection.v1")=>serde_json::from_slice(bytes)?,
        _=>anyhow::bail!("explicit supported MP4 folder request or selection schema required"),
    };
    ensure!(matches!(selection.schema.as_str(),"optiflow.mp4-folder-request.v1"|"optiflow.mp4-folder-selection.v1"),"folder selection schema differs");
    let template_resolution=resolve(&serde_json::to_vec(&selection.template)?,configured,overrides)?;
    let request=mp4::folder::Request {schema:"optiflow.mp4-folder-request.v1".into(),source_root:selection.source_root,recursive:selection.recursive,template:template_resolution.request.clone(),run_directory:selection.run_directory,batch:selection.batch,scratch_cleanup:selection.scratch_cleanup};request.validate()?;
    Ok(FolderResolution {schema:"optiflow.mp4-folder-settings-resolution.v1".into(),request_digest:mp4::digest(&request)?,request,template_resolution})
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn settings_layers_keep_explicit_request_and_cli_precedence_without_enabling_effects(){
        let mut state=State::default();let a=Settings {control:Some(RateControl::Quality {crf:20,preset:mp4::Preset::Slow}),limits:LimitSettings {elapsed_ms:Some(1000),..Default::default()}};
        state.apply(&a,origin("config_fixture")).unwrap();let b=Settings {limits:LimitSettings {elapsed_ms:Some(500),..Default::default()},..Default::default()};state.apply(&b,origin("request_fixture")).unwrap();state.apply(&Settings {limits:LimitSettings {elapsed_ms:Some(250),..Default::default()},..Default::default()},origin("cli_fixture")).unwrap();
        assert_eq!(state.values["limits.elapsed_ms"],250);assert_eq!(state.values["control"]["crf"],20);assert_eq!(state.shadowed_values.len(),2);
        assert!(overrides(br#"{"schema":"optiflow.mp4-settings.v1","settings":{"discard_originals":true}}"#).is_err());
        assert!(State::default().materialize(json!({})).is_err());
    }
    #[test]fn pure_selection_records_effective_policy_without_binding_provenance_into_request_identity(){
        let pin=|path:&str|json!({"path":path,"digest":{"algorithm":"blake3-256","value":"0".repeat(64)}});
        let selection=json!({"schema":"optiflow.mp4-delivery-selection.v1","profile":mp4::PROFILE,"source":"/synthetic/input.mp4","ffmpeg":pin("/tools/ffmpeg"),"ffprobe":pin("/tools/ffprobe"),"sandbox":{"schema":mp4::planning::SANDBOX_SCHEMA,"systemd_run":pin("/tools/systemd-run"),"systemctl":pin("/tools/systemctl"),"helper":pin("/tools/optiflow"),"user_bus":"/run/user/1000/bus","provider_subtree_memory_bytes":1073741824u64,"provider_scratch_bytes":536870912u64,"minimum_free_bytes":0},"private_workdir":"/private/work","artifact_root":"/private/store","audio":"copy","metadata":"preserve_supported_allow_encoder_change"});
        let settings:Settings=serde_json::from_value(json!({"control":{"kind":"quality","crf":22,"preset":"slow"},"limits":{"input_bytes":1048576,"candidate_bytes":1048576,"elapsed_ms":1000,"memory_admission_bytes":2147483648u64,"working_bytes":1073741824u64,"minimum_free_bytes":8388608}})).unwrap();
        let mut a=State::default();a.apply(&settings,origin("first_fixture")).unwrap();let mut b=State::default();b.apply(&settings,origin("second_fixture")).unwrap();let bytes=serde_json::to_vec(&selection).unwrap();let first=resolve(&bytes,&a,None).unwrap();let second=resolve(&bytes,&b,None).unwrap();
        assert_eq!(first.request_digest,second.request_digest);assert_ne!(first.provenance,second.provenance);assert!(!first.provider_invoked&&!first.source_mutation);
        assert!(resolve(&bytes,&State::default(),None).is_err());
    }
}
