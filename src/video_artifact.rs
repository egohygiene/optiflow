//! Separate video candidate protocol reusing the existing no-follow private
//! descriptor and durable no-replace publication primitives. PNG is unchanged.
use std::path::{Path,PathBuf};
use anyhow::{Result,Context,ensure};
use serde_json::Value;
use uuid::Uuid;
use crate::candidate_artifact::{self,Manifest,Member};
use crate::contracts::{self,Contract};

pub const SCHEMA: &str = "optiflow.video-candidate-artifact-set.v1";
pub const CANDIDATE_NAME: &str = "candidate.mkv";
pub const EVIDENCE_NAME: &str = "evidence.json";
pub const MARKER_NAME: &str = "video-candidate-artifact-set.json";
const MAX_EVIDENCE: usize = 524_288;
const MAX_CANDIDATE: usize = 16_777_216;

fn validate_evidence(evidence: &Value,id: Uuid,candidate: &[u8]) -> Result<()> {
    contracts::validate(Contract::VideoCandidateEvidence,evidence)?;
    ensure!(evidence["artifact_set_id"].as_str()==Some(id.to_string().as_str()),"video candidate evidence belongs to another set");
    let candidate_digest=blake3::hash(candidate).to_hex().to_string();
    ensure!(evidence.pointer("/candidate/content/logical_bytes").and_then(Value::as_u64)==Some(candidate.len() as u64)
        && evidence.pointer("/candidate/content/digest/value").and_then(Value::as_str)==Some(candidate_digest.as_str()),"video candidate content differs from evidence");
    let source_bytes=evidence.pointer("/source/content/logical_bytes").and_then(Value::as_u64).context("missing video source size")?;
    let saved=source_bytes.checked_sub(candidate.len() as u64).filter(|v| *v>0).context("video candidate is not strictly smaller")?;
    ensure!(evidence["encoded_bytes_saved"].as_u64()==Some(saved),"encoded-byte reduction disagrees");
    ensure!(evidence.pointer("/validation/source_decoded_digest")==evidence.pointer("/validation/candidate_decoded_digest"),"video decode identities differ");
    let source:crate::video_container::ContainerObservation=serde_json::from_value(evidence.pointer("/validation/source_container").context("missing source container")?.clone())?;
    let recorded:crate::video_container::ContainerObservation=serde_json::from_value(evidence.pointer("/validation/candidate_container").context("missing candidate container")?.clone())?;
    let actual=crate::video_container::inspect(candidate)?;
    ensure!(actual==recorded,"current candidate structure differs from recorded host inspection");
    crate::video_container::compare_stream_copy(&source,&actual)?;
    let request:crate::video_candidate::VideoCandidateRequest=serde_json::from_value(evidence["request"].clone())?;
    crate::video_candidate::prepare_candidate(&request)?;
    ensure!(matches!(request.operation,crate::video_candidate::VideoOperation::StreamCopy),"video artifact does not support lossy acceptance");
    ensure!(serde_json::to_value(crate::video_candidate::request_fingerprint(&request))?==evidence["request_fingerprint"],"video candidate request fingerprint changed");
    ensure!(evidence.pointer("/source/content/digest")==Some(&serde_json::to_value(&request.source.content_digest)?)
        && request.source.logical_bytes==source_bytes,"video candidate source identity differs from request");
    let invocations=evidence["invocations"].as_array().context("missing native invocation receipts")?;
    ensure!(invocations.len()==5,"video candidate requires independent source/probe/decode/candidate invocations");
    for (index,input_digest) in [(0,&request.source.content_digest.value),(1,&request.source.content_digest.value),
        (2,&request.source.content_digest.value),(3,&candidate_digest),(4,&candidate_digest)] {
        ensure!(invocations[index].pointer("/input_digest/value").and_then(Value::as_str)==Some(input_digest.as_str()),"video invocation subject differs from observed bytes");
    }
    ensure!(invocations[2].pointer("/stdout_digest/value").and_then(Value::as_str)==Some(candidate_digest.as_str())
        && invocations[1]["stdout_digest"]==evidence["validation"]["source_decoded_digest"]
        && invocations[4]["stdout_digest"]==evidence["validation"]["candidate_decoded_digest"],"video capture/decode output joins differ");
    let policy:crate::video_sandbox::LinuxVideoSandbox=serde_json::from_value(evidence["resources"]["selected_policy"].clone())?;
    policy.validate()?;
    let receipts:Vec<crate::video_sandbox::SandboxReceipt>=serde_json::from_value(evidence["resources"]["kernel_observations"].clone())?;
    ensure!(receipts.len()==7,"video kernel observation count differs from fixed workflow");
    for receipt in &receipts {crate::video_sandbox::check_receipt(receipt,policy.provider_subtree_memory_bytes,policy.provider_scratch_bytes)?;}
    for (index,invocation) in invocations.iter().enumerate() {
        ensure!(serde_json::to_value(&receipts[index+2])?==invocation["resource_guard"],"video invocation lost its resource observation");
        let expected=if index==0 || index==3 {&request.probe} else {&request.producer};
        ensure!(invocation.pointer("/provider/binary_digest")==Some(&serde_json::to_value(&expected.binary_digest)?)
            && invocation.pointer("/provider/executable")==Some(&serde_json::to_value(&expected.executable)?),"video provider byte/path identity differs");
    }
    ensure!(invocations[2]["provider"]==serde_json::to_value(&request.producer)?
        && invocations[0]["provider"]==serde_json::to_value(&request.probe)?
        && invocations[3]["provider"]==serde_json::to_value(&request.probe)?,"video invocation recipe fingerprint differs");
    let decoded=evidence.pointer("/validation/decoded_bytes").and_then(Value::as_u64).context("missing complete decode count")?;
    ensure!(decoded>0 && decoded<=request.limits.decoded_bytes
        && invocations[1]["captured_stdout_bytes"].as_u64()==Some(decoded)
        && invocations[4]["captured_stdout_bytes"].as_u64()==Some(decoded),"video complete-decode byte joins differ");
    Ok(())
}

fn same_directory(path: &Path,bound: &std::fs::File) -> Result<()> {
    #[cfg(unix)] {use std::os::unix::fs::MetadataExt;
        let current=candidate_artifact::open_directory(path)?.metadata()?;let original=bound.metadata()?;
        ensure!(current.dev()==original.dev() && current.ino()==original.ino(),"video directory namespace changed");}
    Ok(())
}

/// Only the coordinator can call publication after live source/provider checks.
pub(crate) fn publish(root: &Path,id: Uuid,candidate: &[u8],evidence: &Value,precommit: impl FnOnce()->Result<()>) -> Result<PathBuf> {
    ensure!(!candidate.is_empty() && candidate.len()<=MAX_CANDIDATE,"invalid video publication size");
    validate_evidence(evidence,id,candidate)?;
    let mut bytes=serde_json::to_vec_pretty(evidence)?;bytes.push(b'\n');ensure!(bytes.len()<=MAX_EVIDENCE,"video evidence exceeds protocol bound");
    #[cfg(not(any(target_os="linux",target_os="macos")))] {let _=(root,id,precommit);anyhow::bail!("video no-replace publication unsupported")}
    #[cfg(any(target_os="linux",target_os="macos"))] {
        use rustix::fs::{Mode,RenameFlags,mkdirat,renameat_with};
        let root_file=candidate_artifact::open_directory(root)?;candidate_artifact::require_private_root(&root_file)?;
        let staging_name=format!(".video-staging-{id}");
        mkdirat(&root_file,staging_name.as_str(),Mode::RUSR|Mode::WUSR|Mode::XUSR).context("video staging namespace already exists or cannot be reserved")?;
        root_file.sync_all()?;
        let staging=candidate_artifact::open_directory(&root.join(&staging_name))?;
        candidate_artifact::write_new(&staging,CANDIDATE_NAME,candidate)?;
        candidate_artifact::write_new(&staging,EVIDENCE_NAME,&bytes)?;
        let manifest=Manifest {schema:SCHEMA.to_owned(),set_id:id.to_string(),created_at:chrono::Utc::now().to_rfc3339(),state:"committed".to_owned(),
            members:vec![candidate_artifact::member("candidate_video",CANDIDATE_NAME,candidate),candidate_artifact::member("evidence",EVIDENCE_NAME,&bytes)]};
        contracts::validate(Contract::VideoCandidateArtifactSet,&manifest)?;
        candidate_artifact::write_new(&staging,MARKER_NAME,&serde_json::to_vec(&manifest)?)?;staging.sync_all()?;
        precommit().context("video source/provider/deadline/free-space precommit refused; staging retained")?;
        same_directory(root,&root_file)?;same_directory(&root.join(&staging_name),&staging)?;
        let name=id.to_string();renameat_with(&root_file,staging_name.as_str(),&root_file,name.as_str(),RenameFlags::NOREPLACE)?;
        root_file.sync_all().context("video candidate visible but parent durability uncertain")?;
        let directory=root.join(name);inspect(&directory)?;Ok(directory)
    }
}

/// Read-only verification of committed current bytes. Staging is never adopted
/// or deleted, and production evidence grants no replacement/disposal authority.
pub fn inspect(directory: &Path) -> Result<Manifest> {
    let name=directory.file_name().and_then(|v| v.to_str()).context("video candidate set name missing")?;
    let id=Uuid::parse_str(name).context("video staging/unknown directory is not a committed set")?;
    ensure!(name==id.to_string(),"noncanonical video candidate UUID");
    let root=directory.parent().context("video candidate root missing")?;
    let root_file=candidate_artifact::open_directory(root)?;candidate_artifact::require_private_root(&root_file)?;
    let handle=candidate_artifact::open_directory(directory)?;candidate_artifact::require_private_root(&handle)?;
    let marker=candidate_artifact::read_member(&handle,MARKER_NAME,16_384)?;
    let manifest:Manifest=serde_json::from_slice(&marker)?;contracts::validate(Contract::VideoCandidateArtifactSet,&manifest)?;
    ensure!(manifest.set_id==id.to_string(),"video candidate marker names a different set");
    let candidate=candidate_artifact::read_member(&handle,CANDIDATE_NAME,MAX_CANDIDATE)?;
    let evidence=candidate_artifact::read_member(&handle,EVIDENCE_NAME,MAX_EVIDENCE)?;
    let expected:Vec<Member>=vec![candidate_artifact::member("candidate_video",CANDIDATE_NAME,&candidate),candidate_artifact::member("evidence",EVIDENCE_NAME,&evidence)];
    ensure!(manifest.members==expected,"video marker size/hash/member joins differ from current files");
    validate_evidence(&serde_json::from_slice(&evidence)?,id,&candidate)?;
    // Refuse changing/replaced namespaces and any unowned extra member.
    let mut names=std::fs::read_dir(directory)?.map(|v| v.map(|e| e.file_name())).collect::<std::io::Result<Vec<_>>>()?;names.sort();
    let mut allowed:Vec<_>=[CANDIDATE_NAME,EVIDENCE_NAME,MARKER_NAME].into_iter().map(std::ffi::OsString::from).collect();allowed.sort();
    ensure!(names==allowed,"video candidate contains unowned members");
    ensure!(candidate_artifact::read_member(&handle,MARKER_NAME,16_384)?==marker
        && candidate_artifact::read_member(&handle,CANDIDATE_NAME,MAX_CANDIDATE)?==candidate
        && candidate_artifact::read_member(&handle,EVIDENCE_NAME,MAX_EVIDENCE)?==evidence,"video candidate changed during observation");
    same_directory(directory,&handle)?;same_directory(root,&root_file)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_looking_staging_is_never_adopted_or_deleted(){
        let root=tempfile::tempdir().unwrap();let stage=root.path().join(format!(".video-staging-{}",Uuid::now_v7()));
        std::fs::create_dir(&stage).unwrap();std::fs::write(stage.join(MARKER_NAME),b"{\"state\":\"committed\"}").unwrap();
        assert!(inspect(&stage).is_err());assert!(stage.join(MARKER_NAME).exists());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_candidate_namespace_is_refused_without_touching_original(){
        let root=tempfile::tempdir().unwrap();let outside=tempfile::tempdir().unwrap();let id=Uuid::now_v7();
        std::os::unix::fs::symlink(outside.path(),root.path().join(id.to_string())).unwrap();
        assert!(inspect(&root.path().join(id.to_string())).is_err());assert!(outside.path().exists());
    }
}
