//! Typed source/output references over the existing producer. Upstream origin
//! remains caller-declared; no release routing or sibling implementation import.
use super::*;
use anyhow::Context;
use std::time::{Duration,Instant};
const PREPARED_BYTES:usize=METADATA_BYTES-131_072;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum Origin {
    SelectedFile,
    ExternalArtifact {tool:String,contract:String,artifact_id:String,manifest_digest:EvidenceDigest},
}
fn identifier(value:&str)->bool {!value.is_empty()&&value.len()<=256&&!value.chars().any(char::is_control)}
impl Origin {fn validate(&self)->Result<()> {if let Self::ExternalArtifact {tool,contract,artifact_id,manifest_digest}=self {
    ensure!(matches!(tool.as_str(),"aniflow"|"renderflow"|"optiflow")&&identifier(contract)&&identifier(artifact_id),"bounded explicit suite artifact origin required");
    ensure!(matches!(manifest_digest.algorithm.as_str(),"blake3-256"|"sha256")&&manifest_digest.value.len()==64&&manifest_digest.value.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)),"explicit supported origin-manifest digest required");
}Ok(())}}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputReference {pub schema:String,pub path:PathBuf,pub content_digest:EvidenceDigest,pub byte_count:u64,pub origin:Origin}
impl InputReference {pub fn validate(&self)->Result<()> {ensure!(self.schema=="optiflow.mp4-input-reference.v1"&&(1..=INPUT_BYTES).contains(&self.byte_count),"bounded MP4 input reference required");normalized(&self.path)?;ensure!(self.path.to_str().is_some_and(|p|p.len()<=4096&&!p.chars().any(char::is_control)),"bounded UTF8 input reference path required");planning::valid_digest(&self.content_digest)?;self.origin.validate()}}
pub fn observe_input(path:&Path,maximum:u64,origin:Origin,cancel:impl Fn()->bool)->Result<InputReference>{
    origin.validate()?;let source=super::inspect(path,maximum,cancel)?;Ok(InputReference {schema:"optiflow.mp4-input-reference.v1".into(),path:source.path,content_digest:source.content_digest,byte_count:source.snapshot.logical_bytes,origin})
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consumer {pub profile:String,pub maximum_bytes:u64,pub accept_lossy_video:bool}
impl Consumer {pub fn validate(&self)->Result<()> {ensure!(self.profile==PROFILE&&(1..=OUTPUT_BYTES).contains(&self.maximum_bytes)&&self.accept_lossy_video,"consumer must explicitly accept the supported lossy MP4 profile and bounded file size");Ok(())}}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum UnchangedPolicy {Refuse,KeepOriginal}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prepared {pub schema:String,pub plan:Plan,pub input:InputReference,pub consumer:Consumer,pub unchanged_policy:UnchangedPolicy,pub binding_digest:EvidenceDigest}
fn binding(plan:&Plan,input:&InputReference,consumer:&Consumer,unchanged:UnchangedPolicy)->Result<EvidenceDigest>{digest(&("optiflow.mp4-handoff-preparation.v1",&plan.plan_digest,input,consumer,unchanged))}
impl Prepared {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{ensure!(bytes.len()<=PREPARED_BYTES,"MP4 preparation exceeds bound");let value:Self=serde_json::from_slice(bytes)?;value.validate()?;Ok(value)}
    pub fn validate(&self)->Result<()> {
        ensure!(self.schema=="optiflow.mp4-handoff-preparation.v1"&&serde_json::to_vec(self)?.len()<=PREPARED_BYTES,"bounded explicit handoff preparation required");planning::retained(&self.plan)?;self.input.validate()?;self.consumer.validate()?;
        let source=&self.plan.payload.source;ensure!(self.input.path==source.path&&self.input.byte_count==source.snapshot.logical_bytes&&self.input.content_digest==source.content_digest&&self.plan.payload.allocation.maximum_candidate_bytes<=self.consumer.maximum_bytes,"prepared input identity or consumer cap differs");
        ensure!(self.binding_digest==binding(&self.plan,&self.input,&self.consumer,self.unchanged_policy)?,"prepared handoff binding differs");Ok(())
    }
}
pub fn prepare(request:&Request,input:InputReference,consumer:Consumer,unchanged_policy:UnchangedPolicy,cancel:impl Fn()->bool)->Result<Prepared>{
    input.validate()?;consumer.validate()?;request.validate()?;ensure!(input.path==request.source&&input.byte_count<=request.limits.input_bytes&&request.control.maximum(request.limits.candidate_bytes)<=consumer.maximum_bytes,"explicit source reference/consumer cap incompatible with selected request");
    let plan=planning::plan(request,cancel)?;let binding_digest=binding(&plan,&input,&consumer,unchanged_policy)?;let prepared=Prepared {schema:"optiflow.mp4-handoff-preparation.v1".into(),plan,input,consumer,unchanged_policy,binding_digest};prepared.validate()?;Ok(prepared)
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum SelectedKind {ValidatedCandidate,ExplicitUnchangedOriginal}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedOutput {pub kind:SelectedKind,pub path:PathBuf,pub content_digest:EvidenceDigest,pub byte_count:u64}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handoff {
    pub schema:String,pub prepared:Prepared,pub outcome:Outcome,pub selected_output:Option<SelectedOutput>,
    pub state:String,pub origin_verification:String,pub quality_review:String,pub native_qualified:bool,
    pub replacement_authority:bool,pub upload_authority:bool,
}
#[derive(Debug,Serialize)]
pub struct Produced {pub schema:String,pub handoff:Handoff,pub handoff_digest:EvidenceDigest}
fn original(input:&InputReference)->SelectedOutput{SelectedOutput {kind:SelectedKind::ExplicitUnchangedOriginal,path:input.path.clone(),content_digest:input.content_digest.clone(),byte_count:input.byte_count}}
fn candidate(pin:&artifact::Pin)->SelectedOutput{SelectedOutput {kind:SelectedKind::ValidatedCandidate,path:pin.directory.join("candidate.mp4"),content_digest:pin.candidate_digest.clone(),byte_count:pin.candidate_bytes}}
fn current_original(prepared:&Prepared,cancel:impl Fn()->bool)->Result<()> {
    let source=super::inspect(&prepared.input.path,prepared.plan.payload.request.limits.input_bytes,cancel)?;ensure!(source.content_digest==prepared.input.content_digest&&source.snapshot==prepared.plan.payload.source.snapshot,"unchanged original differs from exact prepared source");Ok(())
}
pub fn produce(prepared:&Prepared,cancel:impl Fn()->bool,observer:&progress::Observer<'_>)->Result<Produced>{
    prepared.validate()?;let started=Instant::now();let stop=||cancel()||started.elapsed()>=Duration::from_millis(prepared.plan.payload.request.limits.elapsed_ms);
    let expected=crate::video_folder::SourceBinding {snapshot:prepared.plan.payload.source.snapshot.clone(),digest:prepared.input.content_digest.clone()};
    let outcome=validation::produce_bound(&prepared.plan.payload.request,Some(&expected),Some(&prepared.plan.payload.tools),Some(&prepared.plan.plan_digest),&stop,observer,None)?;
    let (state,selected_output)=match &outcome {
        Outcome::Accepted {artifact,..}=>{artifact::inspect_for_plan(artifact,&prepared.plan)?;("candidate_ready_for_human_review",Some(candidate(artifact)))},
        Outcome::Unchanged {..}=>match prepared.unchanged_policy {
            UnchangedPolicy::Refuse=>("unchanged_not_selected",None),
            UnchangedPolicy::KeepOriginal=>if prepared.input.byte_count<=prepared.consumer.maximum_bytes&&current_original(prepared,&stop).is_ok(){("unchanged_original_explicitly_selected",Some(original(&prepared.input)))}else{("unchanged_original_unavailable",None)},
        },
    };
    let handoff=Handoff {schema:"optiflow.mp4-delivery-handoff.v1".into(),prepared:prepared.clone(),outcome,selected_output,state:state.into(),origin_verification:"caller_declared_origin; external_manifest_release_and_provenance_not_verified".into(),quality_review:"required_for_lossy_candidate; not_recorded_as_approved".into(),native_qualified:false,replacement_authority:false,upload_authority:false};
    ensure!(serde_json::to_vec(&handoff)?.len()<=METADATA_BYTES,"MP4 delivery handoff exceeds bound");let handoff_digest=digest(&handoff)?;Ok(Produced {schema:"optiflow.mp4-handoff-result.v1".into(),handoff,handoff_digest})
}
#[derive(Debug,Serialize)]
pub struct Inspection {pub schema:String,pub binding_digest:EvidenceDigest,pub selected_output:Option<SelectedOutput>,pub state:String,pub selected_bytes_reobserved:bool,pub scope:String,pub native_qualified:bool,pub human_quality_approved:bool,pub replacement_authority:bool,pub upload_authority:bool}
pub fn inspect(value:&Handoff,expected_digest:&EvidenceDigest)->Result<Inspection>{
    planning::valid_digest(expected_digest)?;ensure!(serde_json::to_vec(value)?.len()<=METADATA_BYTES&&digest(value)?==*expected_digest,"externally retained handoff semantic digest differs");value.prepared.validate()?;
    ensure!(value.schema=="optiflow.mp4-delivery-handoff.v1"&&value.origin_verification=="caller_declared_origin; external_manifest_release_and_provenance_not_verified"&&value.quality_review=="required_for_lossy_candidate; not_recorded_as_approved"&&!value.native_qualified&&!value.replacement_authority&&!value.upload_authority,"handoff scope/authority differs");
    let prepared=&value.prepared;let selected_bytes_reobserved=match &value.outcome {
        Outcome::Accepted {artifact,source_bytes,candidate_bytes,encoded_bytes_saved,quality_review,physical_space_reclaimed,attempt_directory}=>{
            artifact::inspect_for_plan(artifact,&prepared.plan)?;ensure!(*source_bytes==prepared.input.byte_count&&*candidate_bytes==artifact.candidate_bytes&&source_bytes.checked_sub(*candidate_bytes)==Some(*encoded_bytes_saved)&&quality_review=="required; explicitly lossy video"&&!*physical_space_reclaimed&&attempt_directory.parent()==Some(prepared.plan.payload.request.private_workdir.as_path())&&value.selected_output==Some(candidate(artifact))&&value.state=="candidate_ready_for_human_review","accepted handoff joins differ");true
        },
        Outcome::Unchanged {reason,source_bytes,candidate_bytes,candidate_digest,attempt_directory}=>{
            ensure!(*source_bytes==prepared.input.byte_count&&reason.len()<=2048&&candidate_bytes.is_some()==candidate_digest.is_some()&&candidate_bytes.is_some()==attempt_directory.is_some(),"unchanged handoff framing differs");
            if let Some(bytes)=candidate_bytes {ensure!(*bytes>=*source_bytes&&*bytes<=prepared.plan.payload.allocation.maximum_candidate_bytes&&attempt_directory.as_ref().and_then(|p|p.parent())==Some(prepared.plan.payload.request.private_workdir.as_path()),"nonshrinking handoff differs");planning::valid_digest(candidate_digest.as_ref().context("candidate digest absent")?)?;}else{ensure!(matches!(&prepared.plan.payload.request.control,RateControl::TargetSize {maximum_bytes,..} if *source_bytes<=*maximum_bytes),"unencoded original must already fit the selected target");}
            match (&value.selected_output,prepared.unchanged_policy) {
                (None,UnchangedPolicy::Refuse)=>{ensure!(value.state=="unchanged_not_selected","unchanged refusal state differs");false},
                (None,UnchangedPolicy::KeepOriginal)=>{ensure!(value.state=="unchanged_original_unavailable","unchanged unavailable state differs");false},
                (Some(output),UnchangedPolicy::KeepOriginal)=>{ensure!(*output==original(&prepared.input)&&output.byte_count<=prepared.consumer.maximum_bytes&&value.state=="unchanged_original_explicitly_selected","explicit original selection differs");current_original(prepared,||false)?;true},
                _=>anyhow::bail!("unchanged original selected without explicit policy"),
            }
        },
    };
    Ok(Inspection {schema:"optiflow.mp4-handoff-inspection.v1".into(),binding_digest:prepared.binding_digest.clone(),selected_output:value.selected_output.clone(),state:value.state.clone(),selected_bytes_reobserved,scope:"selected output bytes/structure and retained exact plan joins only; external origin/release, current native decode and human quality approval remain unverified".into(),native_qualified:false,human_quality_approved:false,replacement_authority:false,upload_authority:false})
}

#[derive(Debug,Serialize)]
pub struct Capabilities {
    pub schema:&'static str,pub profile:&'static str,pub input_container:&'static str,pub output_container:&'static str,
    pub video_codec:&'static str,pub audio_codec:&'static str,pub pixel_format:&'static str,pub color:&'static str,
    pub channels:[u16;2],pub audio_rates:[u32;2],pub maximum_input_bytes:u64,pub maximum_candidate_bytes:u64,
    pub maximum_width:u16,pub maximum_height:u16,pub maximum_frames:usize,pub maximum_duration_seconds:u64,
    pub frame_rate:&'static str,pub video_pixels:&'static str,pub audio_payload:&'static str,pub presentation_clocks:&'static str,
    pub metadata:&'static str,pub unchanged_selection:&'static str,pub provider_execution:&'static str,
    pub source_mutation:bool,pub release_route_available:bool,pub native_qualified:bool,pub quality_review:&'static str,
}
pub fn capabilities()->Capabilities {Capabilities {schema:"optiflow.mp4-delivery-capabilities.v1",profile:PROFILE,input_container:"unfragmented_self_contained_mp4",output_container:"unfragmented_self_contained_mp4",video_codec:"h264",audio_codec:"aac_lc",pixel_format:"yuv420p",color:"bt709_limited_sdr_progressive",channels:[1,2],audio_rates:[44100,48000],maximum_input_bytes:INPUT_BYTES,maximum_candidate_bytes:OUTPUT_BYTES,maximum_width:4096,maximum_height:2160,maximum_frames:MAX_FRAMES,maximum_duration_seconds:900,frame_rate:"constant_1_to_120_fps",video_pixels:"explicitly_lossy; exact_pixel_equivalence_unavailable",audio_payload:"exact_copied_aac_configuration_payload_and_edit_scope_required",presentation_clocks:"exact_rational_video_audio_clock_and_edit_preservation_required",metadata:"supported_utf8_fields_preserved; technical_encoder_field_may_change; unknown_features_refuse",unchanged_selection:"refuse_unless_explicit_keep_original_and_current_exact_input_match",provider_execution:"selected_linux_guard_only; exact_ffmpeg_ffprobe_8.0.1; native_qualification_unverified",source_mutation:false,release_route_available:false,native_qualified:false,quality_review:"human_review_required; never_automatically_approved"}}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn consumer_must_explicitly_accept_lossy_profile_and_size_limit(){let mut value=Consumer {profile:PROFILE.into(),maximum_bytes:100_000_000,accept_lossy_video:false};assert!(value.validate().is_err());value.accept_lossy_video=true;assert!(value.validate().is_ok());value.maximum_bytes=OUTPUT_BYTES+1;assert!(value.validate().is_err());}
    #[test]fn an_origin_label_never_substitutes_for_a_valid_explicit_content_reference(){let reference=InputReference {schema:"optiflow.mp4-input-reference.v1".into(),path:PathBuf::from("/synthetic/selected.mp4"),content_digest:hash(b"synthetic only"),byte_count:10,origin:Origin::ExternalArtifact {tool:"renderflow".into(),contract:"declared-publication/v1".into(),artifact_id:"declared-artifact".into(),manifest_digest:hash(b"declared manifest")}};reference.validate().unwrap();let mut changed=reference.clone();changed.content_digest.value="missing".into();assert!(changed.validate().is_err());let catalog=capabilities();assert!(!catalog.release_route_available&&!catalog.native_qualified&&!catalog.source_mutation);}
}
