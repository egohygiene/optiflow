//! Current complete probe/decode validation through the existing guarded video
//! provider. A saved receipt cannot construct the private live validation token.
use super::*;
use crate::{adapters::video::{VideoProviders,CaptureLimits,InvocationEvidence},video_budget::OperationBudget,video_io::BoundedFileObservation,video_production::VideoProductionRequest,domain::EvidenceDigest};
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {pub schema:String,pub review:StagedVideoReview,pub production:VideoProductionRequest}
impl Request {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{ensure!(bytes.len()<=4_194_304,"live video validation request exceeds 4 MiB");let raw:serde_json::Value=serde_json::from_slice(bytes)?;crate::contracts::validate(crate::contracts::Contract::VideoReplacementValidation,&raw)?;let r:Self=serde_json::from_slice(bytes)?;r.validate()?;Ok(r)}
    pub fn validate(&self)->Result<()> {
        self.review.validate()?;let profile=Profile::marker(&self.review.artifact_schema)?;
        ensure!(self.schema=="optiflow.video-replacement-validation-request.v1"
            &&Profile::production(&self.production.schema)?==profile
            &&NativePath::from_path(&self.production.source)==self.review.source.path
            &&self.production.artifact_root==diag(fs::native_path(&self.review.candidate_set))?.parent().context("candidate root absent")?
            &&matches!(self.production.operation,crate::video_candidate::VideoOperation::StreamCopy),"explicit validation source/profile/operation differs");
        crate::video_production::admission(&self.production)?;
        ensure!(self.review.source.size_bytes<=self.production.limits.input_bytes&&self.review.candidate.size_bytes<=self.production.limits.candidate_bytes,"current pair exceeds selected validation bounds");
        let policy=if profile==Profile::TinyV1{serde_json::to_vec(&(&self.production.sandbox,self.production.limits,"closed_ebml_stream_copy_only"))?}else{serde_json::to_vec(&(&self.production.sandbox,self.production.limits,profile.candidate_profile(),"closed_ebml_stream_copy_only"))?};
        ensure!(blake3::hash(&policy).to_hex().as_str()==self.review.policy_fingerprint,"validation resource policy differs from reviewed candidate policy");Ok(())
    }
}
#[derive(Debug,Serialize)]
pub struct Record {
    pub schema:String,pub review_fingerprint:String,pub request_fingerprint:String,
    pub source_blake3:String,pub candidate_blake3:String,pub decoded:EvidenceDigest,
    pub decoded_bytes_per_subject:u64,pub invocations:Vec<InvocationEvidence>,
    pub resource_receipts:Vec<crate::video_sandbox::SandboxReceipt>,
    pub current_decode_observed:bool,pub exact_decoded_samples:bool,
    pub native_qualified:bool,pub source_removal_authority:bool,
}
/// Not Clone, Serialize or Deserialize: actual byte/provider observations are
/// retained and rechecked within the same original cooperative deadline.
pub struct ValidatedPair {
    request:Request,record:Record,source:BoundedFileObservation,
    candidate:BoundedFileObservation,providers:VideoProviders,budget:OperationBudget,
}
impl ValidatedPair {
    pub fn record(&self)->&Record{&self.record}
    pub fn request(&self)->&Request{&self.request}
    pub fn recheck(&self,signals:&SignalState)->Result<()> {
        let cancel=||signals.is_cancelled();let guard=||self.budget.check(&cancel);
        self.source.verify_current_guarded(&guard)?;self.candidate.verify_current_guarded(&guard)?;
        self.providers.verify_current_with_cancel(&cancel)?;guard()
    }
}
fn current_review(request:&Request,signals:&SignalState)->Result<()> {
    let r=&request.review;let fresh=super::review(&diag(fs::native_path(&r.source.path))?,&diag(fs::native_path(&r.candidate_set))?,&r.artifact_schema,&diag(fs::native_path(&r.state.path))?,&diag(fs::native_path(&r.retention.path))?,&r.policy_fingerprint,signals)?;
    ensure!(fresh.fingerprint==r.fingerprint,"reviewed pair changed before/after current native validation");Ok(())
}
/// Starts exactly the selected guarded probe/decode calls when a product user
/// invokes this operation. It performs no optimization, namespace change,
/// writable transaction-journal access or original disposal.
pub fn validate(request:&Request,signals:&SignalState)->Result<ValidatedPair>{
    request.validate()?;ensure!(cfg!(target_os="linux"),"current video validation requires the existing Linux guard profile");
    let selected=&request.production;let profile=Profile::marker(&request.review.artifact_schema)?;
    let budget=OperationBudget::new(selected.limits.elapsed_ms)?;let cancel=||signals.is_cancelled();let guard=||budget.check(&cancel);guard()?;
    current_review(request,signals)?;guard()?;
    let source=BoundedFileObservation::open_guarded(&selected.source,selected.limits.input_bytes,&guard)?;
    let candidate_path=diag(fs::native_path(&request.review.candidate.path))?;
    let candidate=BoundedFileObservation::open_guarded(&candidate_path,selected.limits.candidate_bytes,&guard)?;
    let source_bytes=source.read_bytes_profile(profile,&guard)?;let candidate_bytes=candidate.read_bytes_profile(profile,&guard)?;
    let source_container=crate::video_container::inspect_profile(&source_bytes,profile)?;
    let candidate_container=crate::video_container::inspect_profile(&candidate_bytes,profile)?;
    crate::video_container::compare_stream_copy(&source_container,&candidate_container)?;guard()?;
    let providers=VideoProviders::lock_enforced_profile(&selected.ffmpeg,&selected.ffprobe,&selected.private_workdir,CaptureLimits {candidate_bytes:selected.limits.candidate_bytes as usize,probe_bytes:4_194_304,stderr_bytes:4096,elapsed_ms:selected.limits.elapsed_ms},budget,selected.sandbox.clone(),profile,&cancel)?;
    let sp=providers.probe_untrusted(&source_bytes,&cancel)?;let cp=providers.probe_untrusted(&candidate_bytes,&cancel)?;
    let source_probe=crate::video_probe::parse_profile(&sp.bytes,profile)?;let candidate_probe=crate::video_probe::parse_profile(&cp.bytes,profile)?;guard()?;
    ensure!(source_probe.decoded_sample_bytes>0&&source_probe.decoded_sample_bytes<=selected.limits.decoded_bytes
        &&source_probe.decoded_sample_bytes==candidate_probe.decoded_sample_bytes
        &&serde_json::to_value(&source_probe.media)?==serde_json::to_value(&candidate_probe.media)?
        &&serde_json::to_value(&source_probe.frames)?==serde_json::to_value(&candidate_probe.frames)?
        &&source_probe.chroma_location==candidate_probe.chroma_location
        &&source_probe.frames.len()==source_container.packets.len()&&candidate_probe.frames.len()==candidate_container.packets.len()
        &&source_probe.media.width==source_container.width&&source_probe.media.height==source_container.height
        &&source_container.track_contract.get("default").map(String::as_str)==Some(if source_probe.media.disposition=="default"{"1"}else{"0"}),"current complete probe/container/timing properties differ");
    let directory=artifact::open_directory(&diag(fs::native_path(&request.review.candidate_set))?)?;
    let saved:serde_json::Value=serde_json::from_slice(&artifact::read_member(&directory,video::EVIDENCE_NAME,profile.evidence_bytes())?)?;
    ensure!(saved["request"]["producer"]==serde_json::to_value(providers.guarded_copy_evidence(&source_probe.media.disposition)?)?
        &&saved["request"]["probe"]==serde_json::to_value(providers.probe_evidence()?)?
        &&saved["request"]["producer_build_fingerprint"]==serde_json::to_value(providers.producer_build_fingerprint())?
        &&saved["request"]["probe_build_fingerprint"]==serde_json::to_value(providers.probe_build_fingerprint())?,"current decoder/probe identity differs from selected artifact provider");
    let sd=providers.decode_untrusted(&source_bytes,source_probe.decoded_sample_bytes as usize,&cancel)?;
    let cd=providers.decode_untrusted(&candidate_bytes,candidate_probe.decoded_sample_bytes as usize,&cancel)?;guard()?;
    ensure!(sd.bytes.len() as u64==source_probe.decoded_sample_bytes&&sd.bytes==cd.bytes,"current complete decoded sample bytes differ");
    let decoded=EvidenceDigest {algorithm:"blake3-256".into(),value:blake3::hash(&sd.bytes).to_hex().to_string()};
    let invocations=vec![sp.evidence,cp.evidence,sd.evidence,cd.evidence];
    ensure!(invocations.iter().all(|i|i.resource_guard.is_some()),"every current probe/decode needs observed guard receipts");
    source.verify_current_guarded(&guard)?;candidate.verify_current_guarded(&guard)?;providers.verify_current_with_cancel(&cancel)?;
    current_review(request,signals)?;guard()?;
    let record=Record {schema:"optiflow.video-replacement-validation-record.v1".into(),review_fingerprint:request.review.fingerprint.clone(),request_fingerprint:hash(request)?,source_blake3:source.digest().value.clone(),candidate_blake3:candidate.digest().value.clone(),decoded,decoded_bytes_per_subject:source_probe.decoded_sample_bytes,invocations,resource_receipts:providers.resource_receipts()?,current_decode_observed:true,exact_decoded_samples:true,native_qualified:false,source_removal_authority:false};
    crate::contracts::validate(crate::contracts::Contract::VideoReplacementValidationRecord,&record)?;
    Ok(ValidatedPair {request:request.clone(),record,source,candidate,providers,budget})
}
