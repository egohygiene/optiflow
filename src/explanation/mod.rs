//! Optional pinned-record explanations through the existing SDK v1 report role.
use std::collections::BTreeSet;
use anyhow::{Result,ensure,Context};
use serde::{Serialize,Deserialize};
use crate::domain::{ScanReport,Plan,REPORT_SCHEMA_VERSION,PLAN_SCHEMA_VERSION};
use crate::extensions::{Digest,fingerprint};
use crate::contracts::{self,Contract};
mod provider;
pub use provider::{SuppliedExplanationProvider,Configuration};
#[cfg(test)] mod tests;
pub const PACKET_SCHEMA:&str="optiflow.explanation-packet.v1";
pub const CANDIDATE_SCHEMA:&str="optiflow.explanation-candidate.v1";
pub const SIDECAR_SCHEMA:&str="optiflow.explanation-sidecar.v1";
pub const CONFIGURATION_SCHEMA:&str="optiflow.evidence-explanation-config.v1";
/// Caller supplies immutable report bytes from its existing artifact boundary.
/// A matching external digest proves bytes, not current collection freshness.
pub struct SourceRecord<'a> {pub bytes:&'a [u8],pub digest:&'a Digest}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {pub record_id:String,pub schema_version:String,pub digest:Digest,pub byte_count:u64}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {pub fact_id:String,pub record_id:String,pub recorded_value:u64}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {pub schema:String,pub packet_fingerprint:Digest,pub sources:Vec<SourceIdentity>,pub facts:Vec<Fact>,pub omissions:Vec<String>,pub source_freshness:String,pub physical_space_reclaimed:bool,pub mutation_authority:bool}
fn omissions()->Vec<String>{["source_files_not_reobserved","raw_paths_names_and_content_omitted","record_semantics_not_independently_recomputed","physical_reclamation_unknown","no_provider_or_model_execution","no_plan_approval_or_mutation"].into_iter().map(String::from).collect()}
fn id(s:&str)->bool{!s.is_empty()&&s.len()<=96&&s.bytes().all(|c|c.is_ascii_alphanumeric()||b"._-".contains(&c))}
fn digest(d:&Digest)->bool{d.algorithm=="blake3-256"&&d.value.len()==64&&d.value.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))}
fn packet_fingerprint(p:&Packet)->Result<Digest>{let mut p=p.clone();p.packet_fingerprint=Digest::blake3(b"");fingerprint(&p)}
impl Packet {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{ensure!(bytes.len()<=65_536,"explanation packet exceeds64KiB");let p:Self=serde_json::from_slice(bytes).context("closed normalized explanation packet required")?;p.validate()?;Ok(p)}
    pub fn validate(&self)->Result<()> {ensure!(self.schema==PACKET_SCHEMA&&self.packet_fingerprint==packet_fingerprint(self)?&&self.sources.len()>=1&&self.sources.len()<=2&&self.facts.len()<=20&&!self.facts.is_empty()&&self.omissions==omissions()&&self.source_freshness=="pinned_report_bytes_only; current_collection_not_observed"&&!self.physical_space_reclaimed&&!self.mutation_authority,"explanation packet identity/coverage differs");let mut ids=BTreeSet::new();for s in &self.sources{ensure!(matches!(s.record_id.as_str(),"report"|"plan")&&ids.insert(&s.record_id)&&digest(&s.digest)&&s.byte_count>0&&s.byte_count<=4_194_304&&s.schema_version==(if s.record_id=="report"{REPORT_SCHEMA_VERSION}else{PLAN_SCHEMA_VERSION}),"bounded unique pinned report/plan reference required");}let mut facts=BTreeSet::new();for f in &self.facts{ensure!(id(&f.fact_id)&&facts.insert(&f.fact_id)&&ids.contains(&f.record_id),"explanation fact reference absent or duplicate");}contracts::validate(Contract::ExplanationPacket,self)
    }
}
fn checked_source<'a>(s:&'a SourceRecord<'_>)->Result<serde_json::Value>{ensure!(!s.bytes.is_empty()&&s.bytes.len()<=4_194_304&&digest(s.digest)&&Digest::blake3(s.bytes)==*s.digest,"report/plan bytes changed or exceed bound");serde_json::from_slice(s.bytes).context("bounded report/plan JSON required")}
/// No filesystem access, state write, model call, environment or remote fallback.
pub fn prepare(enabled:bool,report:SourceRecord<'_>,plan:Option<SourceRecord<'_>>,cancel:&impl Fn()->bool)->Result<Option<Packet>>{
    if !enabled{return Ok(None);}ensure!(!cancel(),"explanation cancelled");let raw=checked_source(&report)?;contracts::validate(Contract::Report,&raw)?;let r:ScanReport=serde_json::from_value(raw)?;ensure!(r.schema_version==REPORT_SCHEMA_VERSION&&r.observations.len()<=16_384,"current bounded report v6 required");
    let mut sources=vec![SourceIdentity {record_id:"report".into(),schema_version:r.schema_version.clone(),digest:report.digest.clone(),byte_count:report.bytes.len() as u64}];let s=&r.summary;let mut facts=Vec::new();
    for (name,value) in [("recorded_files",s.file_count),("recorded_logical_bytes",s.total_bytes),("recorded_media_files",s.media_files),("recorded_unsupported_files",s.unsupported_files),("recorded_unreadable_files",s.unreadable_files),("recorded_exact_groups",s.exact_duplicate_groups),("recorded_duplicate_files",s.exact_duplicate_files),("recorded_potential_logical_bytes",s.reclaimable_bytes),("recorded_unstable_observations",s.unstable_observation_count),("recorded_hardlink_alias_paths",s.hard_link_alias_path_count),("recorded_warning_count",r.run.warnings.len() as u64)]{facts.push(Fact {fact_id:name.into(),record_id:"report".into(),recorded_value:value});}
    if let Some(record)=plan {let raw=checked_source(&record)?;contracts::validate(Contract::Plan,&raw)?;let p:Plan=serde_json::from_value(raw)?;ensure!(p.schema_version==PLAN_SCHEMA_VERSION&&p.source_run_id==r.run.run_id&&p.source_artifact_set_id==r.run.artifact_set_id&&p.source_artifact_set_id.is_some()&&!p.safety.mutates_files&&p.safety.requires_explicit_apply&&p.actions.len()<=4096&&p.actions.len() as u64==p.summary.action_count,"plan must be a bounded review-only record of the selected report run/set");sources.push(SourceIdentity {record_id:"plan".into(),schema_version:p.schema_version,digest:record.digest.clone(),byte_count:record.bytes.len() as u64});for (name,value) in [("recorded_plan_actions",p.summary.action_count),("recorded_plan_candidates",p.summary.candidate_file_count),("recorded_plan_potential_logical_bytes",p.summary.potential_reclaimable_bytes)]{facts.push(Fact {fact_id:name.into(),record_id:"plan".into(),recorded_value:value});}}
    ensure!(!cancel(),"explanation cancelled");let mut p=Packet {schema:PACKET_SCHEMA.into(),packet_fingerprint:Digest::blake3(b""),sources,facts,omissions:omissions(),source_freshness:"pinned_report_bytes_only; current_collection_not_observed".into(),physical_space_reclaimed:false,mutation_authority:false};p.packet_fingerprint=packet_fingerprint(&p)?;p.validate()?;Ok(Some(p))
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredIdentity {pub provider_id:String,pub model_id:String,pub runtime_revision:String,pub profile_fingerprint:Digest,pub template_fingerprint:Digest,pub output_schema_digest:Digest}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {pub text:String,pub fact_ids:Vec<String>}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {pub schema:String,pub packet_fingerprint:Digest,pub declared_identity:DeclaredIdentity,pub observed_fact_ids:Vec<String>,pub suggestions:Vec<Note>,pub questions:Vec<Note>}
impl Candidate {pub fn from_json_slice(bytes:&[u8])->Result<Self>{ensure!(bytes.len()<=65_536,"explanation candidate exceeds64KiB");let c:Self=serde_json::from_slice(bytes).context("closed supplied explanation candidate required")?;contracts::validate(Contract::ExplanationCandidate,&c)?;Ok(c)}}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sidecar {pub schema:String,pub sidecar_fingerprint:Digest,pub packet_fingerprint:Digest,pub candidate_fingerprint:Digest,pub declared_identity:DeclaredIdentity,pub observed_record_facts:Vec<Fact>,pub suggestions:Vec<Note>,pub questions:Vec<Note>,pub omissions:Vec<String>,pub semantic_grounding:String,pub provider_execution_observed:bool,pub current_source_eligibility_established:bool,pub physical_space_reclaimed:bool,pub apply_authority:bool}
pub fn validate_candidate(packet:&Packet,c:&Candidate,cancel:&impl Fn()->bool)->Result<Sidecar>{
    ensure!(!cancel(),"explanation cancelled");packet.validate()?;contracts::validate(Contract::ExplanationCandidate,c)?;ensure!(c.schema==CANDIDATE_SCHEMA&&c.packet_fingerprint==packet.packet_fingerprint,"candidate cites a different exact packet");let identity=&c.declared_identity;
    for n in [&identity.provider_id,&identity.model_id,&identity.runtime_revision]{ensure!(!n.is_empty()&&n.len()<=128&&n.bytes().all(|b|b.is_ascii_alphanumeric()||b"._-:/+".contains(&b)),"bounded declared provider/model name required");}
    ensure!(digest(&identity.profile_fingerprint)&&digest(&identity.template_fingerprint)&&identity.output_schema_digest==Digest::blake3(include_bytes!("../../schemas/explanation-candidate-v1.schema.json")),"exact declared profile/template/output-schema identity required");
    let mut seen=BTreeSet::new();let mut observed=Vec::new();for id in &c.observed_fact_ids{ensure!(seen.insert(id),"duplicate observed fact citation");observed.push(packet.facts.iter().find(|f|&f.fact_id==id).context("unknown observed fact citation")?.clone());}
    for n in c.suggestions.iter().chain(&c.questions){ensure!(!n.text.is_empty()&&n.text.len()<=1024&&!n.text.chars().any(char::is_control)&&!n.fact_ids.is_empty()&&n.fact_ids.len()<=8,"bounded note text and1..8 fact citations required");let mut seen=BTreeSet::new();for f in &n.fact_ids{ensure!(seen.insert(f)&&packet.facts.iter().any(|v|&v.fact_id==f),"unknown/duplicate suggestion or question citation");}ensure!(!cancel(),"explanation cancelled");}
    let mut out=Sidecar {schema:SIDECAR_SCHEMA.into(),sidecar_fingerprint:Digest::blake3(b""),packet_fingerprint:packet.packet_fingerprint.clone(),candidate_fingerprint:fingerprint(c)?,declared_identity:identity.clone(),observed_record_facts:observed,suggestions:c.suggestions.clone(),questions:c.questions.clone(),omissions:packet.omissions.clone(),semantic_grounding:"exact_record_facts_and_citation_membership_only; suggestion_meaning_unverified".into(),provider_execution_observed:false,current_source_eligibility_established:false,physical_space_reclaimed:false,apply_authority:false};out.sidecar_fingerprint=fingerprint(&out)?;contracts::validate(Contract::ExplanationSidecar,&out)?;ensure!(!cancel(),"explanation cancelled");Ok(out)
}
/// Caller re-supplies its CURRENT pinned immutable artifact bytes. This detects
/// changed records, not current on-disk collection content or mutation preconditions.
pub fn validate_current(report:SourceRecord<'_>,plan:Option<SourceRecord<'_>>,packet:&Packet,candidate:&Candidate,cancel:&impl Fn()->bool)->Result<Sidecar>{ensure!(prepare(true,report,plan,cancel)?.as_ref()==Some(packet),"explanation packet differs from current pinned record bytes");validate_candidate(packet,candidate,cancel)}
