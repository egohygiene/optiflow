//! Read-only staged video handoff. Saved native receipts are historical claims;
//! current byte/namespace checks do not manufacture live decode qualification.
use std::path::Path;
use anyhow::{Result,ensure,Context};
use serde::{Serialize,Deserialize};
use super::{filesystem as fs,model::{FileBinding,DirectoryBinding},replacement::{diag,hash,pin}};
use crate::{candidate_artifact as artifact,video_artifact as video,domain::NativePath,signals::SignalState,video_profile::Profile};
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedVideoReview {
    pub schema:String,pub fingerprint:String,pub operation:String,pub artifact_schema:String,
    pub source:FileBinding,pub candidate:FileBinding,pub candidate_set:NativePath,
    pub marker_blake3:String,pub evidence_blake3:String,pub policy_fingerprint:String,
    pub state:DirectoryBinding,pub retention:DirectoryBinding,pub encoded_logical_reduction_bytes:u64,
    pub current_source_content_bound:bool,pub independent_container_comparison:bool,
    pub live_decode_observed:bool,pub native_properties_preserved:bool,
    pub replacement_qualified:bool,pub apply_authority:bool,pub original_disposal_authority:bool,
}
impl StagedVideoReview {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{ensure!(bytes.len()<=4_194_304,"staged video review exceeds 4 MiB");let raw:serde_json::Value=serde_json::from_slice(bytes)?;crate::contracts::validate(crate::contracts::Contract::StagedVideoReplacement,&raw)?;let r:Self=serde_json::from_slice(bytes)?;r.validate()?;Ok(r)}
    pub fn validate(&self)->Result<()>{
    let profile=Profile::marker(&self.artifact_schema)?;let mut copy=self.clone();copy.fingerprint.clear();
    ensure!(self.schema=="optiflow.staged-video-replacement-review.v1"&&self.operation=="stage_video_replacement.v1"&&self.fingerprint==hash(&copy)?
        &&pin(&self.marker_blake3)&&pin(&self.evidence_blake3)&&pin(&self.policy_fingerprint)
        &&self.source.size_bytes<=profile.input_bytes()&&self.candidate.size_bytes>0&&self.candidate.size_bytes<self.source.size_bytes
        &&self.encoded_logical_reduction_bytes==self.source.size_bytes-self.candidate.size_bytes
        &&self.source.identity.filesystem_id==self.retention.identity.filesystem_id
        &&self.current_source_content_bound&&self.independent_container_comparison&&!self.live_decode_observed
        &&!self.native_properties_preserved&&!self.replacement_qualified&&!self.apply_authority&&!self.original_disposal_authority,"staged video review identity or authority differs");
    for path in [&self.source.path,&self.candidate.path,&self.candidate_set,&self.state.path,&self.retention.path]{diag(fs::native_path(path))?;}Ok(())
}}
/// Inspect current source, marker/evidence and complete stream-copy payloads.
/// No ffmpeg/decoder/model, writable journal, original replacement or disposal.
pub fn review(source:&Path,set:&Path,expected_artifact_schema:&str,state:&Path,retention:&Path,policy:&str,signals:&SignalState)->Result<StagedVideoReview>{
    let profile=Profile::marker(expected_artifact_schema)?;ensure!(pin(policy)&&!signals.is_cancelled(),"invalid video review policy or cancellation");
    let guard=||{ensure!(!signals.is_cancelled(),"staged video review cancelled");Ok(())};
    let source_observation=crate::video_io::BoundedFileObservation::open_guarded(source,profile.input_bytes(),&guard)?;
    let bound_source=diag(fs::snapshot(source,signals,profile.input_bytes()))?;
    ensure!(bound_source.blake3==source_observation.digest().value,"source changed while binding video review");
    artifact::validate_private_root(state)?;artifact::validate_private_root(retention)?;
    let state=diag(fs::directory(state))?;let retention=diag(fs::directory(retention))?;
    diag(fs::outside_directories(source.parent().context("source parent absent")?,&[&state,&retention]))?;
    let manifest=video::inspect(set)?;ensure!(manifest.schema==expected_artifact_schema&&set.file_name().and_then(|v|v.to_str())==Some(manifest.set_id.as_str()),"video artifact identity differs from explicit selected schema");
    let directory=artifact::open_directory(set)?;
    let marker=artifact::read_member(&directory,video::MARKER_NAME,16_384)?;
    let evidence_bytes=artifact::read_member(&directory,video::EVIDENCE_NAME,profile.evidence_bytes())?;
    let evidence:serde_json::Value=serde_json::from_slice(&evidence_bytes)?;
    ensure!(evidence["schema"]==profile.evidence_schema()&&evidence["source"]["path"]==serde_json::to_value(&bound_source.path)?
        &&evidence["source"]["snapshot"]==serde_json::to_value(source_observation.snapshot())?
        &&evidence["source"]["content"]["digest"]==serde_json::to_value(source_observation.digest())?
        &&evidence["request"]["policy"]["effective_policy_fingerprint"]["algorithm"]=="blake3-256"&&evidence["request"]["policy"]["effective_policy_fingerprint"]["value"]==policy,"video evidence does not bind current source/policy");
    let candidate_path=set.join(video::CANDIDATE_NAME);let candidate=diag(fs::snapshot(&candidate_path,signals,profile.candidate_bytes()))?;
    let original_bytes=source_observation.read_bytes_profile(profile,&guard)?;
    let candidate_bytes=artifact::read_member(&directory,video::CANDIDATE_NAME,profile.candidate_bytes() as usize)?;
    ensure!(blake3::hash(&candidate_bytes).to_hex().as_str()==candidate.blake3,"candidate changed during staged video review");
    let original_container=crate::video_container::inspect_profile(&original_bytes,profile)?;
    let candidate_container=crate::video_container::inspect_profile(&candidate_bytes,profile)?;
    crate::video_container::compare_stream_copy(&original_container,&candidate_container)?;
    ensure!(evidence["validation"]["source_container"]==serde_json::to_value(&original_container)?&&evidence["validation"]["candidate_container"]==serde_json::to_value(&candidate_container)?,"current container evidence differs");
    source_observation.verify_current_guarded(&guard)?;
    let current=diag(fs::snapshot(&candidate_path,signals,profile.candidate_bytes()))?;
    ensure!(hash(&current)?==hash(&candidate)?&&artifact::read_member(&directory,video::MARKER_NAME,16_384)?==marker&&artifact::read_member(&directory,video::EVIDENCE_NAME,profile.evidence_bytes())?==evidence_bytes,"video candidate records changed");
    ensure!(video::inspect(set)?==manifest,"video commitment changed after independent review");
    let saved=bound_source.size_bytes.checked_sub(candidate.size_bytes).filter(|n|*n>0).context("video candidate not strictly smaller")?;
    let mut r=StagedVideoReview {schema:"optiflow.staged-video-replacement-review.v1".into(),fingerprint:String::new(),operation:"stage_video_replacement.v1".into(),artifact_schema:expected_artifact_schema.into(),source:bound_source,candidate,candidate_set:NativePath::from_path(set),marker_blake3:blake3::hash(&marker).to_hex().to_string(),evidence_blake3:blake3::hash(&evidence_bytes).to_hex().to_string(),policy_fingerprint:policy.into(),state,retention,encoded_logical_reduction_bytes:saved,current_source_content_bound:true,independent_container_comparison:true,live_decode_observed:false,native_properties_preserved:false,replacement_qualified:false,apply_authority:false,original_disposal_authority:false};r.fingerprint=hash(&r)?;r.validate()?;guard()?;Ok(r)
}
/// Explicit bridge to the existing replacement owner. It refuses before any
/// writable state: a staged review cannot enter the PNG-only transaction lane.
pub fn admit_to_transaction(review:&StagedVideoReview)->Result<()> {review.validate()?;anyhow::bail!("staged video needs live decode/property validation and native replacement qualification; PNG approval/discard authority cannot be reused")}

#[cfg(test)] mod tests;
