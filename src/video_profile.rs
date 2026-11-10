//! Internal closed profile selection. No implicit upgrade of v1 entry points.
use anyhow::{Result,bail};
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub(crate) enum Profile {TinyV1,BoundedV2}
impl Profile {
    pub(crate) fn production(schema:&str)->Result<Self>{match schema {"optiflow.video-production-request.v1"=>Ok(Self::TinyV1),"optiflow.video-production-request.v2"=>Ok(Self::BoundedV2),_=>bail!("unknown explicit video production profile")}}
    pub(crate) fn candidate(schema:&str,profile:&str)->Result<Self>{match (schema,profile) {("optiflow.video-candidate-request.v1","optiflow.matroska-ffv1-sdr-candidate.v1")=>Ok(Self::TinyV1),("optiflow.video-candidate-request.v2","optiflow.matroska-ffv1-sdr-candidate.v2")=>Ok(Self::BoundedV2),_=>bail!("candidate schema/profile differ; no implicit upgrade")}}
    pub(crate) fn evidence(schema:&str)->Result<Self>{match schema {"optiflow.video-candidate-evidence.v1"=>Ok(Self::TinyV1),"optiflow.video-candidate-evidence.v2"=>Ok(Self::BoundedV2),_=>bail!("unknown video evidence profile")}}
    pub(crate) fn marker(schema:&str)->Result<Self>{match schema {"optiflow.video-candidate-artifact-set.v1"=>Ok(Self::TinyV1),"optiflow.video-candidate-artifact-set.v2"=>Ok(Self::BoundedV2),_=>bail!("unknown video artifact profile")}}
    pub(crate) fn input_bytes(self)->u64{if self==Self::TinyV1{16_777_216}else{134_217_728}}
    pub(crate) fn candidate_bytes(self)->u64{self.input_bytes()}
    pub(crate) fn decoded_bytes(self)->u64{if self==Self::TinyV1{67_108_864}else{536_870_912}}
    pub(crate) fn temporary_bytes(self)->u64{if self==Self::TinyV1{33_554_432}else{536_870_912}}
    pub(crate) fn memory_bytes(self)->u64{if self==Self::TinyV1{268_435_456}else{2_147_483_648}}
    pub(crate) fn width(self)->u32{if self==Self::TinyV1{640}else{1280}}
    pub(crate) fn height(self)->u32{if self==Self::TinyV1{360}else{720}}
    pub(crate) fn frames(self)->usize{if self==Self::TinyV1{250}else{3000}}
    pub(crate) fn evidence_bytes(self)->usize{if self==Self::TinyV1{524_288}else{8_388_608}}
    pub(crate) fn publication_overhead(self)->u64{self.evidence_bytes() as u64+16_384}
    pub(crate) fn request_schema(self)->&'static str{if self==Self::TinyV1{"optiflow.video-candidate-request.v1"}else{"optiflow.video-candidate-request.v2"}}
    pub(crate) fn candidate_profile(self)->&'static str{if self==Self::TinyV1{"optiflow.matroska-ffv1-sdr-candidate.v1"}else{"optiflow.matroska-ffv1-sdr-candidate.v2"}}
    pub(crate) fn container_profile(self)->&'static str{if self==Self::TinyV1{"optiflow.matroska-ffv1-closed-ebml.v1"}else{"optiflow.matroska-ffv1-closed-ebml.v2"}}
    pub(crate) fn evidence_schema(self)->&'static str{if self==Self::TinyV1{"optiflow.video-candidate-evidence.v1"}else{"optiflow.video-candidate-evidence.v2"}}
    pub(crate) fn evidence_profile(self)->&'static str{if self==Self::TinyV1{"closed_matroska_ffv1_stream_copy_v1"}else{"closed_matroska_ffv1_stream_copy_v2"}}
    pub(crate) fn marker_schema(self)->&'static str{if self==Self::TinyV1{"optiflow.video-candidate-artifact-set.v1"}else{"optiflow.video-candidate-artifact-set.v2"}}
    pub(crate) fn request_contract(self)->crate::contracts::Contract{if self==Self::TinyV1{crate::contracts::Contract::VideoCandidateRequest}else{crate::contracts::Contract::VideoCandidateRequestV2}}
    pub(crate) fn production_contract(self)->crate::contracts::Contract{if self==Self::TinyV1{crate::contracts::Contract::VideoProductionRequest}else{crate::contracts::Contract::VideoProductionRequestV2}}
    pub(crate) fn evidence_contract(self)->crate::contracts::Contract{if self==Self::TinyV1{crate::contracts::Contract::VideoCandidateEvidence}else{crate::contracts::Contract::VideoCandidateEvidenceV2}}
    pub(crate) fn marker_contract(self)->crate::contracts::Contract{if self==Self::TinyV1{crate::contracts::Contract::VideoCandidateArtifactSet}else{crate::contracts::Contract::VideoCandidateArtifactSetV2}}
}
#[cfg(test)] mod tests;
