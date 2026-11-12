//! Profile-aware discovery before tools or writable namespaces are prepared.
use super::*;
#[derive(Debug,Clone,Copy,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum PreviewProfile {TinyV1,BoundedV2}
impl PreviewProfile {fn owner(self)->Profile{match self{Self::TinyV1=>Profile::TinyV1,Self::BoundedV2=>Profile::BoundedV2}}}
#[derive(Debug,Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct PreviewRequest {pub schema:String,pub source_root:PathBuf,pub profile:PreviewProfile,pub recursive:bool,pub input_bytes:u64,pub batch:FolderLimits}
#[derive(Debug,Serialize)]
pub struct ProfileDescription {pub profile:PreviewProfile,pub maximum_input_bytes:u64,pub maximum_candidate_bytes:u64,pub maximum_width:u32,pub maximum_height:u32,pub maximum_frames:usize,pub maximum_decoded_bytes:u64,pub subset:String,pub native_qualified:bool}
fn description(profile:PreviewProfile)->ProfileDescription {let p=profile.owner();ProfileDescription {profile,maximum_input_bytes:p.input_bytes(),maximum_candidate_bytes:p.candidate_bytes(),maximum_width:p.width(),maximum_height:p.height(),maximum_frames:p.frames(),maximum_decoded_bytes:p.decoded_bytes(),subset:"closed Matroska/FFV1 stream-copy SDR video-only; no audio/HDR/unknown features; current full native validation required".into(),native_qualified:false}}
pub fn profiles()->serde_json::Value {serde_json::json!({"schema":"optiflow.video-profile-catalog.v1","profiles":[description(PreviewProfile::TinyV1),description(PreviewProfile::BoundedV2)],"provider_invoked":false,"source_mutation":false,"qualification":"source_only; platform/resource/codec/native proof remains separate"})}
#[derive(Debug,Serialize)]
pub struct Preview {pub schema:String,pub profile:ProfileDescription,pub inventory:FolderInventory,pub eligible_files:u64,pub unsupported_files:u64,pub skipped_entries:u64,pub provider_invoked:bool,pub workspace_created:bool,pub candidate_acceptance:bool,pub physical_space_reclaimed:bool,pub replacement_authority:bool,pub next_step:String}
impl PreviewRequest {
    fn validate(&self)->Result<()> {
        ensure!(self.schema=="optiflow.video-folder-preview-request.v1"&&serde_json::to_vec(self)?.len()<=131_072,"closed bounded video preview request required");self.batch.validate_for(self.profile.owner())?;
        ensure!((1..=self.profile.owner().input_bytes()).contains(&self.input_bytes),"preview input bound exceeds selected profile; no implicit upgrade");
        ensure!(self.source_root.is_absolute()&&self.source_root.to_str().is_some_and(|s|s.len()<=4096)&&self.source_root.canonicalize()?==self.source_root,"canonical explicit source root required");Ok(())
    }
}
pub fn preview(request:&PreviewRequest,cancel:impl Fn()->bool)->Result<Preview> {
    request.validate()?;let budget=BatchBudget::new(request.batch.elapsed_ms)?;
    let scope=Discovery {source_root:request.source_root.clone(),batch:request.batch.clone(),recursive:request.recursive,input_bytes:request.input_bytes,profile:request.profile.owner(),request_fingerprint:digest(request)?,inventory_schema:"optiflow.video-folder-preview-inventory.v1".into()};
    #[cfg(target_os="linux")] let inventory=native::discover(&scope,&budget,&cancel)?;
    #[cfg(not(target_os="linux"))] {let _=(scope,budget,cancel);anyhow::bail!("video preview uses the existing Linux discovery owner; this host remains unsupported");}
    #[cfg(target_os="linux")] {let eligible_files=inventory.entries.iter().filter(|e|e.disposition==EntryDisposition::Eligible).count() as u64;let unsupported_files=inventory.entries.iter().filter(|e|e.disposition==EntryDisposition::Unsupported).count() as u64;let skipped_entries=inventory.entries.iter().filter(|e|e.disposition==EntryDisposition::Skipped).count() as u64;
        Ok(Preview {schema:"optiflow.video-folder-preview.v1".into(),profile:description(request.profile),inventory,eligible_files,unsupported_files,skipped_entries,provider_invoked:false,workspace_created:false,candidate_acceptance:false,physical_space_reclaimed:false,replacement_authority:false,next_step:"Review the explicit subset and exclusions; separately prepare an actual v1/v2 production request with pinned tools and Linux guard. Preview cannot be resumed or adopted as a run inventory.".into()})}
}
#[cfg(test)]mod tests {
    use super::*;
    fn request(root:PathBuf)->PreviewRequest {PreviewRequest {schema:"optiflow.video-folder-preview-request.v1".into(),source_root:root,profile:PreviewProfile::TinyV1,recursive:true,input_bytes:1024,batch:FolderLimits {maximum_entries:8,maximum_files:4,maximum_actions:1,maximum_depth:4,maximum_total_input_bytes:4096,maximum_publication_bytes:4096,elapsed_ms:10000}}}
    #[test]fn selected_v1_limit_does_not_silently_upgrade_to_v2(){let t=tempfile::tempdir().unwrap();let mut r=request(t.path().canonicalize().unwrap());r.input_bytes=Profile::TinyV1.input_bytes()+1;assert!(r.validate().is_err());r.profile=PreviewProfile::BoundedV2;r.validate().unwrap();}
    #[cfg(target_os="linux")]#[test]fn preview_classifies_without_tools_or_writable_namespaces(){let t=tempfile::tempdir().unwrap();let root=t.path().canonicalize().unwrap();std::fs::write(root.join("ordinary.txt"),b"synthetic non-video").unwrap();let result=preview(&request(root.clone()),||false).unwrap();assert_eq!(result.unsupported_files,1);assert!(!result.provider_invoked&&!result.workspace_created&&!result.candidate_acceptance&&!result.replacement_authority);assert_eq!(std::fs::read_dir(root).unwrap().count(),1);assert_eq!(result.inventory.schema,"optiflow.video-folder-preview-inventory.v1");}
}
