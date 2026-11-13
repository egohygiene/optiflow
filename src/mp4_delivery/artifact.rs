//! Private digest-pinned candidate commitment over existing no-follow/fsync/
//! no-replace primitives. Readback never reruns the native codec or original.
use super::*;
use anyhow::Context;
use std::fs::File;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {pub directory:PathBuf,pub manifest_digest:EvidenceDigest,pub candidate_digest:EvidenceDigest,pub candidate_bytes:u64,pub evidence_digest:EvidenceDigest}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {schema:String,profile:String,id:String,state:String,candidate_digest:EvidenceDigest,candidate_bytes:u64,evidence_digest:EvidenceDigest,evidence_bytes:u64,quality_review:String,replacement_authority:bool,upload_authority:bool}
#[derive(Debug,Serialize)]
pub struct Inspection {pub schema:String,pub artifact:Pin,pub candidate:container::Summary,pub source_digest:EvidenceDigest,pub source_bytes:u64,pub request_digest:EvidenceDigest,pub tools:Vec<planning::Tool>,pub evidence_scope:String,pub current_native_decode:bool,pub native_qualified:bool,pub quality_review:String,pub replacement_authority:bool,pub upload_authority:bool}
fn same(path:&Path,file:&File)->Result<()> {
    #[cfg(unix)]{let current=crate::candidate_artifact::open_directory(path)?;let a=rustix::fs::fstat(file)?;let b=rustix::fs::fstat(&current)?;ensure!(a.st_dev==b.st_dev&&a.st_ino==b.st_ino,"MP4 publication directory identity changed");Ok(())}
    #[cfg(not(unix))]{let _=(path,file);anyhow::bail!("MP4 publication unsupported")}
}
pub(crate) fn publish(root:&Path,candidate:&[u8],evidence:&validation::Evidence,precommit:impl Fn()->Result<()>)->Result<Pin>{
    #[cfg(not(target_os="linux"))]{let _=(root,candidate,evidence,precommit);anyhow::bail!("MP4 candidate commitment requires the selected Linux profile");}
    #[cfg(target_os="linux")]{
        crate::candidate_artifact::validate_private_root(root)?;let parent=crate::candidate_artifact::open_directory(root)?;let id=uuid::Uuid::now_v7().to_string();let staging_name=format!(".mp4-staging-{id}");
        use rustix::fs::{Mode,OFlags,RenameFlags,mkdirat,openat,renameat_with};mkdirat(&parent,staging_name.as_str(),Mode::RUSR|Mode::WUSR|Mode::XUSR)?;let staging=File::from(openat(&parent,staging_name.as_str(),OFlags::RDONLY|OFlags::DIRECTORY|OFlags::NOFOLLOW|OFlags::CLOEXEC,Mode::empty())?);
        let body=serde_json::to_vec(evidence)?;ensure!(body.len()<=METADATA_BYTES,"MP4 evidence exceeds bound");runtime::write_member(&staging,"candidate.mp4",candidate)?;runtime::write_member(&staging,"evidence.json",&body)?;
        let manifest=Manifest {schema:"optiflow.mp4-delivery-artifact-set.v1".into(),profile:PROFILE.into(),id:id.clone(),state:"committed".into(),candidate_digest:hash(candidate),candidate_bytes:candidate.len() as u64,evidence_digest:hash(&body),evidence_bytes:body.len() as u64,quality_review:"required; explicitly lossy video".into(),replacement_authority:false,upload_authority:false};let bytes=serde_json::to_vec(&manifest)?;ensure!(bytes.len()<=16384,"MP4 marker exceeds bound");runtime::write_member(&staging,"mp4-artifact.json",&bytes)?;staging.sync_all()?;
        precommit().context("retain MP4 staging; current source/tool/resource recheck refused")?;same(root,&parent)?;same(&root.join(&staging_name),&staging)?;
        let name=format!("mp4-{id}");renameat_with(&parent,staging_name.as_str(),&parent,name.as_str(),RenameFlags::NOREPLACE)?;parent.sync_all().context("MP4 artifact visible but parent durability uncertain; retain it")?;
        let pin=Pin {directory:root.join(name),manifest_digest:hash(&bytes),candidate_digest:manifest.candidate_digest,candidate_bytes:manifest.candidate_bytes,evidence_digest:manifest.evidence_digest};inspect(&pin)?;Ok(pin)
    }
}
pub fn inspect(pin:&Pin)->Result<Inspection>{
    normalized(&pin.directory)?;planning::valid_digest(&pin.manifest_digest)?;let root=crate::candidate_artifact::open_directory(&pin.directory)?;crate::candidate_artifact::require_private_root(&root)?;
    let entries=std::fs::read_dir(&pin.directory)?.map(|e|e.map(|e|e.file_name())).collect::<std::io::Result<Vec<_>>>()?;ensure!(entries.len()==3&&entries.iter().all(|n|["candidate.mp4","evidence.json","mp4-artifact.json"].iter().any(|s|n==*s)),"unknown/incomplete MP4 artifact membership");
    let raw=crate::candidate_artifact::read_member(&root,"mp4-artifact.json",16384)?;ensure!(hash(&raw)==pin.manifest_digest,"externally retained MP4 marker digest differs");let m:Manifest=serde_json::from_slice(&raw)?;
    ensure!(m.schema=="optiflow.mp4-delivery-artifact-set.v1"&&m.profile==PROFILE&&m.state=="committed"&&uuid::Uuid::parse_str(&m.id).is_ok()&&pin.directory.file_name().and_then(|s|s.to_str())==Some(format!("mp4-{}",m.id).as_str())&&m.candidate_digest==pin.candidate_digest&&m.candidate_bytes==pin.candidate_bytes&&m.evidence_digest==pin.evidence_digest&&m.evidence_bytes<=METADATA_BYTES as u64&&m.quality_review=="required; explicitly lossy video"&&!m.replacement_authority&&!m.upload_authority,"MP4 marker scope/membership differs; staging cannot be adopted");
    let body=crate::candidate_artifact::read_member(&root,"evidence.json",METADATA_BYTES)?;ensure!(body.len() as u64==m.evidence_bytes&&hash(&body)==m.evidence_digest,"MP4 evidence changed");let e:validation::Evidence=serde_json::from_slice(&body)?;
    let candidate=BoundedFileObservation::open_mp4(&pin.directory.join("candidate.mp4"),OUTPUT_BYTES,&||Ok(()))?;ensure!(candidate.snapshot().link_count==1&&candidate.snapshot().logical_bytes==m.candidate_bytes&&candidate.digest()==&m.candidate_digest,"MP4 candidate changed");let parsed=container::observe(&candidate,&||Ok(()))?;
    validation::retained(&e,&parsed.summary,m.candidate_bytes,&m.candidate_digest)?;candidate.verify_current()?;same(&pin.directory,&root)?;
    ensure!(crate::candidate_artifact::read_member(&root,"mp4-artifact.json",16384)?==raw&&crate::candidate_artifact::read_member(&root,"evidence.json",METADATA_BYTES)?==body,"MP4 record changed during inspection");
    Ok(Inspection {schema:"optiflow.mp4-delivery-artifact-inspection.v1".into(),artifact:pin.clone(),candidate:parsed.summary,source_digest:e.plan.payload.source.content_digest.clone(),source_bytes:e.plan.payload.source.snapshot.logical_bytes,request_digest:digest(&e.plan.payload.request)?,tools:e.plan.payload.tools,evidence_scope:"current_candidate_bytes_structure_and_retained_validation_records; original_tools_native_decode_and_release_not_reobserved".into(),current_native_decode:false,native_qualified:false,quality_review:m.quality_review,replacement_authority:false,upload_authority:false})
}
