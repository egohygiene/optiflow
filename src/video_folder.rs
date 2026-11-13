//! Bounded serial source-preserving candidates, using the existing producer.
//! No replacement/disposal, uncertain-attempt replay or source-tree staging.
use std::{path::{Path,PathBuf},time::{Duration,Instant}};
use anyhow::{Result,Context,ensure};
use serde::{Deserialize,Serialize};
use crate::domain::{EvidenceDigest,NativePath};
use crate::video_candidate::{RequestedLimits,VideoOperation};
use crate::video_production::{VideoProductionRequest,VideoProductionOutcome};
pub use crate::video_io::FileSnapshot;
const JOURNAL_BYTES:usize=16_777_216;
use crate::video_profile::Profile;
const INVENTORY_BYTES:usize=4_194_304;
mod preview;
pub use preview::{PreviewProfile,PreviewRequest,Preview,profiles,preview};
struct Discovery {source_root:PathBuf,batch:FolderLimits,recursive:bool,input_bytes:u64,profile:Option<Profile>,request_fingerprint:EvidenceDigest,inventory_schema:String}
impl Discovery {fn for_run(request:&VideoFolderRequest)->Result<Self>{Ok(Self {source_root:request.source_root.clone(),batch:request.batch.clone(),recursive:request.recursive,input_bytes:request.limits.input_bytes,profile:Some(request.profile()?),request_fingerprint:digest(request)?,inventory_schema:request.document_schema("inventory")?})}}
pub(crate) fn discover_mp4(root:&Path,batch:&FolderLimits,recursive:bool,input_bytes:u64,fingerprint:EvidenceDigest,cancel:&impl Fn()->bool)->Result<FolderInventory>{
    let scope=Discovery {source_root:root.into(),batch:batch.clone(),recursive,input_bytes,profile:None,request_fingerprint:fingerprint,inventory_schema:"optiflow.mp4-folder-inventory.v1".into()};let budget=BatchBudget::new(batch.elapsed_ms)?;
    #[cfg(target_os="linux")]{native::discover(&scope,&budget,cancel)}
    #[cfg(not(target_os="linux"))]{let _=(scope,budget,cancel);anyhow::bail!("MP4 folder discovery requires the existing Linux owner")}
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderLimits {
    pub maximum_entries:u64,pub maximum_files:u64,pub maximum_actions:u64,pub maximum_depth:u64,
    pub maximum_total_input_bytes:u64,pub maximum_publication_bytes:u64,pub elapsed_ms:u64,
}
impl FolderLimits {
    pub fn validate(&self)->Result<()> {self.validate_for(Profile::TinyV1)}
    fn validate_for(&self,profile:Profile)->Result<()> {
        let aggregate=if profile==Profile::TinyV1 {268_435_456}else{1_073_741_824};
        ensure!((1..=4096).contains(&self.maximum_entries)&&(1..=256).contains(&self.maximum_files)
            &&self.maximum_files<=self.maximum_entries&&(1..=32).contains(&self.maximum_actions)&&self.maximum_actions<=self.maximum_files
            &&(1..=32).contains(&self.maximum_depth)&&(1..=aggregate).contains(&self.maximum_total_input_bytes)
            &&(1..=aggregate).contains(&self.maximum_publication_bytes)&&(1..=600_000).contains(&self.elapsed_ms),"folder bounds exceed the selected serial subset");Ok(())
    }
}
fn yes()->bool {true}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoFolderRequest {
    pub schema:String,pub source_root:PathBuf,pub ffmpeg:PathBuf,pub ffprobe:PathBuf,
    pub private_workdir:PathBuf,pub artifact_root:PathBuf,pub run_directory:PathBuf,
    pub limits:RequestedLimits,pub sandbox:crate::video_sandbox::LinuxVideoSandbox,
    pub batch:FolderLimits,#[serde(default="yes")] pub recursive:bool,
}
impl VideoFolderRequest {
    fn profile(&self)->Result<Profile>{match self.schema.as_str(){"optiflow.video-folder-request.v1"=>Ok(Profile::TinyV1),"optiflow.video-folder-request.v2"=>Ok(Profile::BoundedV2),_=>anyhow::bail!("unknown explicit video folder profile")}}
    fn contract(&self)->Result<crate::contracts::Contract>{Ok(if self.profile()?==Profile::TinyV1{crate::contracts::Contract::VideoFolderProtocol}else{crate::contracts::Contract::VideoFolderProtocolV2})}
    fn document_schema(&self,kind:&str)->Result<String>{Ok(format!("optiflow.video-folder-{kind}.v{}",if self.profile()?==Profile::TinyV1{1}else{2}))}
    fn overhead(&self)->Result<u64>{Ok(self.profile()?.publication_overhead())}
    fn production(&self,source:PathBuf)->VideoProductionRequest {
        VideoProductionRequest {schema:if self.schema=="optiflow.video-folder-request.v2" {"optiflow.video-production-request.v2"}else{"optiflow.video-production-request.v1"}.into(),source,ffmpeg:self.ffmpeg.clone(),ffprobe:self.ffprobe.clone(),
            private_workdir:self.private_workdir.clone(),artifact_root:self.artifact_root.clone(),operation:VideoOperation::StreamCopy,limits:self.limits,sandbox:self.sandbox.clone()}
    }
    pub fn validate(&self)->Result<()> {
        self.batch.validate_for(self.profile()?)?;
        ensure!(serde_json::to_vec(self)?.len()<=131_072,"folder request exceeds 128 KiB");
        crate::contracts::validate(self.contract()?,self)?;
        crate::video_production::admission(&self.production(self.source_root.join("source-placeholder")))?;
        let namespaces=[&self.source_root,&self.private_workdir,&self.artifact_root,&self.run_directory];
        for (i,path) in namespaces.iter().enumerate() {
            ensure!(path.is_absolute()&&path.to_str().is_some_and(|s|s.len()<=4096)&&path.canonicalize()?==**path,"folder namespaces require canonical absolute UTF-8 paths");
            for other in &namespaces[..i] {ensure!(!path.starts_with(other)&&!other.starts_with(path),"source/work/store/journal namespaces must be disjoint");}
            crate::candidate_artifact::open_directory(path)?;
        }
        for private in [&self.private_workdir,&self.artifact_root,&self.run_directory] {crate::candidate_artifact::validate_private_root(private)?;}Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum EntryDisposition {Eligible,Skipped,Unsupported,Failed}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBinding {pub snapshot:FileSnapshot,pub digest:EvidenceDigest}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderEntry {
    pub relative_path:NativePath,pub disposition:EntryDisposition,pub reason:String,
    pub source:Option<SourceBinding>,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderInventory {
    /// New runs freeze local tool bytes; legacy inventories remain inspectable.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub components:Option<FolderComponents>,
    pub schema:String,pub request_fingerprint:EvidenceDigest,pub entries_seen:u64,pub files_seen:u64,
    pub hashed_input_bytes:u64,pub entries:Vec<FolderEntry>,pub scope:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderComponents {pub ffmpeg:SourceBinding,pub ffprobe:SourceBinding}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderRecord {
    pub index:u64,pub relative_path:NativePath,pub status:String,pub reason:String,
    pub production:Option<VideoProductionOutcome>,pub detail:Option<String>,
}
#[derive(Debug,Clone,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderCounts {
    pub inspected_files:u64,pub eligible_files:u64,pub optimized_files:u64,pub unchanged_files:u64,
    pub unsupported_files:u64,pub skipped_entries:u64,pub failed_files:u64,
}
fn counts(inventory:&FolderInventory,records:&[FolderRecord])->FolderCounts {
    FolderCounts {inspected_files:inventory.entries.iter().filter(|e|e.source.is_some()).count() as u64,
        eligible_files:inventory.entries.iter().filter(|e|e.disposition==EntryDisposition::Eligible).count() as u64,
        optimized_files:records.iter().filter(|r|r.status=="accepted").count() as u64,unchanged_files:records.iter().filter(|r|r.status=="unchanged").count() as u64,
        unsupported_files:records.iter().filter(|r|r.status=="unsupported").count() as u64,skipped_entries:records.iter().filter(|r|r.status=="skipped").count() as u64,
        failed_files:records.iter().filter(|r|r.status=="failed").count() as u64}
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderReport {
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub continuation:Option<FolderContinuation>,
    pub schema:String,pub request_fingerprint:EvidenceDigest,pub inventory_fingerprint:EvidenceDigest,
    pub state:String,pub stop_reason:Option<String>,pub actions_started:u64,pub records:Vec<FolderRecord>,
    pub counts:FolderCounts,
    pub publication_admission_bytes:u64,pub encoded_bytes_saved:u64,
    pub elapsed_ms:u64,pub physical_space_reclaimed:bool,pub replacement_authority:bool,pub qualification:String,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderContinuation {
    pub sequence:u64,pub previous_report_fingerprint:Option<EvidenceDigest>,
    pub prior_result_count:u64,pub prior_results_fingerprint:EvidenceDigest,
    pub elapsed_policy:String,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderRecovery {
    pub schema:String,pub request_fingerprint:EvidenceDigest,pub recorded_results:Vec<FolderRecord>,
    pub unfinished_attempts:Vec<u64>,pub unstarted_eligible:u64,pub completed_report:Option<FolderReport>,
    pub replay_authority:bool,pub source_freshness:String,
}
fn digest<T:Serialize>(value:&T)->Result<EvidenceDigest> {Ok(EvidenceDigest {algorithm:"blake3-256".into(),value:blake3::hash(&serde_json::to_vec(value)?).to_hex().to_string()})}
struct BatchBudget {started:Instant,deadline:Instant}
impl BatchBudget {
    fn new(ms:u64)->Result<Self> {let started=Instant::now();Ok(Self {started,deadline:started.checked_add(Duration::from_millis(ms)).context("folder deadline overflow")?})}
    fn cancelled_or_expired(&self,cancel:&impl Fn()->bool)->bool {cancel()||Instant::now()>=self.deadline}
    fn check(&self,cancel:&impl Fn()->bool)->Result<()> {ensure!(!self.cancelled_or_expired(cancel),"folder cancelled or cooperative deadline exhausted");Ok(())}
    fn remaining_ms(&self)->u64 {self.deadline.saturating_duration_since(Instant::now()).as_millis().min(u128::from(u64::MAX)) as u64}
}
pub fn discover(request:&VideoFolderRequest,cancel:impl Fn()->bool)->Result<FolderInventory> {
    request.validate()?;let budget=BatchBudget::new(request.batch.elapsed_ms)?;
    #[cfg(target_os="linux")] {native::discover(&Discovery::for_run(request)?,&budget,&cancel)}
    #[cfg(not(target_os="linux"))] {let _=(budget,cancel);anyhow::bail!("video folder lane currently requires Linux");}
}
pub fn run(request:&VideoFolderRequest,cancel:impl Fn()->bool)->Result<FolderReport> {
    request.validate()?;let budget=BatchBudget::new(request.batch.elapsed_ms)?;
    #[cfg(target_os="linux")] {native::run(request,&budget,&cancel)}
    #[cfg(not(target_os="linux"))] {let _=(budget,cancel);anyhow::bail!("video folder lane currently requires Linux");}
}
/// Continue only never-started entries after exact source/tool/store/journal
/// reconciliation. An uncertain start or failed publication is never replayed.
pub fn resume(request:&VideoFolderRequest,cancel:impl Fn()->bool)->Result<FolderReport> {
    request.validate()?;let budget=BatchBudget::new(request.batch.elapsed_ms)?;
    #[cfg(target_os="linux")] {native::resume(request,&budget,&cancel)}
    #[cfg(not(target_os="linux"))] {let _=(budget,cancel);anyhow::bail!("video folder continuation currently requires Linux");}
}
/// Read-only reconciliation. Incomplete starts are never replayed or adopted;
/// candidate cleanup/native qualification stay with their existing owners.
pub fn inspect_recovery(root:&Path)->Result<FolderRecovery> {
    #[cfg(target_os="linux")] {native::inspect_recovery(root)}
    #[cfg(not(target_os="linux"))] {let _=root;anyhow::bail!("video folder recovery currently requires Linux");}
}

#[cfg(target_os="linux")]
mod native {
    use super::*;
    use std::{fs::{File,ReadDir},os::unix::fs::MetadataExt};
    use crate::filesystem::identity::FileStateSignature;
    use crate::video_io::BoundedFileObservation;
    fn same(path:&Path,bound:&File)->Result<()> {
        let current=crate::candidate_artifact::open_directory(path)?.metadata()?;let expected=bound.metadata()?;
        ensure!(current.dev()==expected.dev()&&current.ino()==expected.ino(),"folder directory namespace changed");Ok(())
    }
    struct Directory {path:PathBuf,bound:File,before:FileStateSignature,entries:ReadDir,depth:u64}
    impl Directory {
        fn open(path:PathBuf,depth:u64)->Result<Self> {
            let bound=crate::candidate_artifact::open_directory(&path)?;let before=FileStateSignature::from_file_metadata(&bound.metadata()?);
            let entries=std::fs::read_dir(&path)?;same(&path,&bound)?;Ok(Self {path,bound,before,entries,depth})
        }
        fn check(&self)->Result<()> {same(&self.path,&self.bound)?;ensure!(FileStateSignature::from_file_metadata(&self.bound.metadata()?)==self.before,"source directory changed during discovery");Ok(())}
    }
    pub(super) fn discover(request:&Discovery,budget:&BatchBudget,cancel:&impl Fn()->bool)->Result<FolderInventory> {
        budget.check(cancel)?;let root=Directory::open(request.source_root.clone(),0)?;let device=root.bound.metadata()?.dev();
        let mut stack=vec![root];let mut entries=Vec::<(PathBuf,FolderEntry,FileStateSignature)>::new();let mut seen=0u64;let mut files=0u64;let mut hashed=0u64;let mut retained=0usize;
        while !stack.is_empty() {
            budget.check(cancel)?;
            let directory=stack.last_mut().unwrap();directory.check()?;
            let next=directory.entries.next();let depth=directory.depth+1;
            let Some(next)=next else {directory.check()?;stack.pop();continue;};
            seen+=1;ensure!(seen<=request.batch.maximum_entries,"folder discovery entry bound exceeded; no candidate started");
            let entry=next.context("incomplete folder traversal; no candidate started")?;directory.check()?;
            let path=entry.path();let relative=path.strip_prefix(&request.source_root)?.to_path_buf();let metadata=std::fs::symlink_metadata(&path)?;
            let mut record=FolderEntry {relative_path:NativePath::from_path(&relative),disposition:EntryDisposition::Skipped,reason:String::new(),source:None};
            if metadata.file_type().is_symlink(){record.reason="symlink_not_followed".into();}
            else if metadata.dev()!=device {record.reason="device_boundary_not_crossed".into();}
            else if metadata.is_dir() {
                if !request.recursive {record.reason="recursion_disabled".into();}
                else if depth>=request.batch.maximum_depth {record.reason="depth_limit".into();}
                else {stack.push(Directory::open(path,depth)?);continue;}
            }
            else if !metadata.is_file(){record.reason="special_file_not_opened".into();}
            else {
                files+=1;ensure!(files<=request.batch.maximum_files,"folder discovery file bound exceeded; no candidate started");
                if metadata.nlink()!=1 {record.reason="hard_link_alias_not_admitted".into();}
                else if path.to_str().is_none(){record.reason="production_source_requires_utf8_locator".into();}
                else if metadata.len()==0 {record.reason="empty_file".into();}
                else if metadata.len()>request.input_bytes {record.reason="per_file_input_byte_limit".into();}
                else {
                    record.disposition=EntryDisposition::Eligible;record.reason="classification_pending".into();
                }
            }
            budget.check(cancel)?;retained=retained.checked_add(serde_json::to_vec(&record)?.len()).context("inventory byte overflow")?;
            ensure!(retained<=INVENTORY_BYTES,"folder inventory retention exceeds 4 MiB; no candidate started");entries.push((relative,record,FileStateSignature::from_symlink_metadata(&metadata)));
        }
        entries.sort_by(|(a,_,_),(b,_,_)|a.cmp(b));retained=0;
        // Apply byte admission in exact path order, independent of directory
        // enumeration order, after bounded discovery has completed.
        for (relative,record,before) in &mut entries {
            budget.check(cancel)?;
            if record.disposition==EntryDisposition::Eligible {
                let path=request.source_root.join(relative.as_path());
                ensure!(FileStateSignature::from_symlink_metadata(&std::fs::symlink_metadata(&path)?)==*before,"source changed between traversal and classification");
                if hashed.checked_add(before.logical_size_bytes).is_none_or(|v|v>request.batch.maximum_total_input_bytes){record.disposition=EntryDisposition::Skipped;record.reason="batch_input_read_byte_limit".into();}
                else {
                    let guard=||budget.check(cancel);let observation=if request.profile.is_some(){BoundedFileObservation::open_guarded(&path,request.input_bytes,&guard)?}else{BoundedFileObservation::open_mp4(&path,request.input_bytes,&guard)?};
                    ensure!(observation.snapshot().device_id==device&&observation.snapshot().link_count==1,"source identity/device/link changed during discovery");
                    hashed+=observation.snapshot().logical_bytes;
                    record.source=Some(SourceBinding {snapshot:observation.snapshot().clone(),digest:observation.digest().clone()});
                    if let Some(profile)=request.profile {let bytes=observation.read_bytes_profile(profile,&guard)?;let kind=infer::get(&bytes).map(|v|crate::inventory::classify_content_type(v.mime_type()));let looks_video=matches!(kind,Some(crate::domain::MediaKind::Video))||bytes.starts_with(&[0x1a,0x45,0xdf,0xa3]);
                        if !looks_video {record.disposition=EntryDisposition::Unsupported;record.reason="not_a_supported_video_content_type".into();}else if crate::video_container::inspect_profile(&bytes,profile).is_err(){record.disposition=EntryDisposition::Unsupported;record.reason="outside_closed_matroska_ffv1_subset".into();}else{record.reason="closed_container_observed_native_validation_pending".into();}
                    }else if crate::mp4_delivery::container::observe(&observation,&guard).is_err(){record.disposition=EntryDisposition::Unsupported;record.reason="outside_bounded_mp4_h264_aac_subset".into();}else{record.reason="mp4_container_observed_native_validation_pending".into();}
                    observation.verify_current_guarded(&guard)?;
                }
            }
            retained=retained.checked_add(serde_json::to_vec(record)?.len()).context("inventory byte overflow")?;ensure!(retained<=INVENTORY_BYTES,"folder inventory retention exceeds 4 MiB; no candidate started");
        }
        budget.check(cancel)?;
        Ok(FolderInventory {components:None,schema:request.inventory_schema.clone(),request_fingerprint:request.request_fingerprint.clone(),entries_seen:seen,files_seen:files,hashed_input_bytes:hashed,entries:entries.into_iter().map(|(_,v,_)|v).collect(),
            scope:"bounded observed directory walk; no symlink following; device boundaries skipped; same-device bind mounts are not attested".into()})
    }
    fn lock(directory:&File)->Result<()> {rustix::fs::flock(directory,rustix::fs::FlockOperation::NonBlockingLockExclusive).context("another folder owner holds this private directory")}
    fn free(path:&Path,required:u64)->Result<()> {let fd=crate::candidate_artifact::open_directory(path)?;let v=rustix::fs::fstatvfs(&fd)?;
        ensure!(v.f_frsize>0&&v.f_bavail.checked_mul(v.f_frsize).is_some_and(|n|n>=required),"folder free-space reserve unavailable");Ok(())}
    fn write(root:&Path,bound:&File,name:&str,bytes:&[u8])->Result<()> {
        ensure!(bytes.len()<=JOURNAL_BYTES,"folder journal member exceeds bound");same(root,bound)?;
        let total=journal_size(root)?;ensure!(total.checked_add(bytes.len() as u64).is_some_and(|v|v<=JOURNAL_BYTES as u64),"folder journal aggregate byte limit exceeded");
        crate::candidate_artifact::write_new(bound,name,bytes)?;bound.sync_all()?;same(root,bound)
    }
    fn journal_size(root:&Path)->Result<u64> {
        let mut count=0;let mut total=0u64;
        for item in std::fs::read_dir(root)? {count+=1;ensure!(count<=99,"folder journal member count exceeds bounds");let item=item?;let meta=std::fs::symlink_metadata(item.path())?;
            ensure!(meta.is_file()&&!meta.file_type().is_symlink()&&meta.nlink()==1,"folder journal contains an alias or special member");
            total=total.checked_add(meta.len()).context("journal size overflow")?;ensure!(total<=JOURNAL_BYTES as u64,"folder journal exceeds aggregate bound");
        }Ok(total)
    }
    fn read<T:serde::de::DeserializeOwned>(bound:&File,name:&str)->Result<T> {Ok(serde_json::from_slice(&crate::candidate_artifact::read_member(bound,name,JOURNAL_BYTES)?)?)}
    fn entry_path(root:&Path,path:&NativePath)->Result<PathBuf> {
        let relative=path.to_path_buf();ensure!(!relative.as_os_str().is_empty()&&NativePath::from_path(&relative)==*path
            &&relative.components().all(|v|matches!(v,std::path::Component::Normal(_))),"folder entry locator is not an exact confined relative path");Ok(root.join(relative))
    }
    fn store_members(request:&VideoFolderRequest,known:&[String])->Result<()> {
        let mut count=0;for item in std::fs::read_dir(&request.artifact_root)? {count+=1;ensure!(count<=request.batch.maximum_actions as usize,"video store contains unowned members");let item=item?;
            ensure!(item.file_type()?.is_dir()&&known.iter().any(|id|item.file_name()==std::ffi::OsStr::new(id)),"video store has an unowned or unresolved member; stop and reconcile");}
        ensure!(count==known.len(),"video store lost an accepted member");Ok(())
    }
    fn accepted(request:&VideoFolderRequest,entry:&FolderEntry,outcome:&VideoProductionOutcome)->Result<()> {
        if let VideoProductionOutcome::Unchanged {source_bytes,candidate_bytes,..}=outcome {
            ensure!(entry.source.as_ref().is_some_and(|s|s.snapshot.logical_bytes==*source_bytes)&&candidate_bytes>=source_bytes
                &&*candidate_bytes<=request.limits.candidate_bytes,"unchanged folder result differs from its bounded original source");
        }
        if let VideoProductionOutcome::Accepted {artifact_set_id,artifact_directory,source_bytes,candidate_bytes,encoded_bytes_saved}=outcome {
            ensure!(uuid::Uuid::parse_str(artifact_set_id)?.to_string()==*artifact_set_id&&artifact_directory==&request.artifact_root.join(artifact_set_id),"folder artifact locator escaped its store");
            let manifest=crate::video_artifact::inspect(artifact_directory)?;ensure!(manifest.schema==request.profile()?.marker_schema(),"folder candidate profile differs from explicit request");
            let bound=crate::candidate_artifact::open_directory(artifact_directory)?;
            let evidence:serde_json::Value=serde_json::from_slice(&crate::candidate_artifact::read_member(&bound,crate::video_artifact::EVIDENCE_NAME,request.profile()?.evidence_bytes())?)?;
            let source=entry.source.as_ref().context("accepted folder source binding missing")?;
            ensure!(evidence["source"]["content"]["digest"]==serde_json::to_value(&source.digest)?&&evidence["source"]["snapshot"]==serde_json::to_value(&source.snapshot)?
                &&evidence["source"]["path"]==serde_json::to_value(NativePath::from_path(&entry_path(&request.source_root,&entry.relative_path)?))?
                &&source_bytes==&source.snapshot.logical_bytes&&evidence["source"]["content"]["logical_bytes"].as_u64()==Some(*source_bytes)
                &&evidence["candidate"]["content"]["logical_bytes"].as_u64()==Some(*candidate_bytes)&&evidence["encoded_bytes_saved"].as_u64()==Some(*encoded_bytes_saved),"folder candidate/source/outcome binding differs");
        }Ok(())
    }
    fn components(request:&VideoFolderRequest,budget:&BatchBudget,cancel:&impl Fn()->bool)->Result<FolderComponents> {
        let guard=||budget.check(cancel);
        let observe=|path:&Path|->Result<SourceBinding> {
            ensure!(path.canonicalize()?==path,"folder tool paths must be canonical without symlink ancestors");
            let value=BoundedFileObservation::open_guarded(path,134_217_728,&guard)?;
            value.verify_current_guarded(&guard)?;Ok(SourceBinding {snapshot:value.snapshot().clone(),digest:value.digest().clone()})
        };
        Ok(FolderComponents {ffmpeg:observe(&request.ffmpeg)?,ffprobe:observe(&request.ffprobe)?})
    }
    fn require_components(request:&VideoFolderRequest,inventory:&FolderInventory,budget:&BatchBudget,cancel:&impl Fn()->bool)->Result<()> {
        ensure!(inventory.components.as_ref()==Some(&components(request,budget,cancel)?),"folder tools changed or legacy inventory lacks exact baseline; reconcile without replay");Ok(())
    }
    fn ledger(request:&VideoFolderRequest,records:&[FolderRecord])->Result<(u64,u64,Vec<String>)> {
        let mut publication=0u64;let mut saved=0u64;let mut known=Vec::new();
        for record in records {if let Some(VideoProductionOutcome::Accepted {artifact_set_id,source_bytes,candidate_bytes,encoded_bytes_saved,..})=&record.production {
            ensure!(candidate_bytes<source_bytes&&source_bytes-candidate_bytes==*encoded_bytes_saved&&!known.contains(artifact_set_id),"folder accepted savings or unique artifact identity differs");
            publication=publication.checked_add(candidate_bytes.checked_add(request.overhead()?).context("publication accounting overflow")?).context("publication accounting overflow")?;
            if record.status=="accepted" {saved=saved.checked_add(*encoded_bytes_saved).context("savings accounting overflow")?;}known.push(artifact_set_id.clone());
        }}Ok((publication,saved,known))
    }
    pub(super) fn run(request:&VideoFolderRequest,budget:&BatchBudget,cancel:&impl Fn()->bool)->Result<FolderReport> {
        budget.check(cancel)?;
        let journal=crate::candidate_artifact::open_directory(&request.run_directory)?;let store=crate::candidate_artifact::open_directory(&request.artifact_root)?;
        lock(&journal)?;lock(&store)?;
        ensure!(std::fs::read_dir(&request.run_directory)?.next().is_none(),"folder journal must be empty; inspect retained recovery state instead of replaying");store_members(request,&[])?;
        let source_root=crate::candidate_artifact::open_directory(&request.source_root)?;
        let mut inventory=discover(&Discovery::for_run(request)?,budget,cancel)?;inventory.components=Some(components(request,budget,cancel)?);
        ensure!(serde_json::to_vec(&inventory)?.len()<=INVENTORY_BYTES,"complete folder inventory including tool baseline exceeds 4 MiB");
        free(&request.run_directory,request.sandbox.minimum_free_bytes.checked_add(JOURNAL_BYTES as u64).context("journal reserve overflow")?)?;
        write(&request.run_directory,&journal,"request.json",&serde_json::to_vec(request)?)?;
        write(&request.run_directory,&journal,"inventory.json",&serde_json::to_vec(&inventory)?)?;
        let report=FolderReport {continuation:None,schema:request.document_schema("report")?,request_fingerprint:digest(request)?,inventory_fingerprint:digest(&inventory)?,state:"complete".into(),stop_reason:None,actions_started:0,records:Vec::new(),counts:FolderCounts::default(),publication_admission_bytes:0,encoded_bytes_saved:0,elapsed_ms:0,physical_space_reclaimed:false,replacement_authority:false,qualification:"source_only_native_host_unqualified".into()};
        advance(request,&inventory,budget,cancel,&journal,&store,&source_root,report,Vec::new(),&[],"report.json")
    }
    fn advance(request:&VideoFolderRequest,inventory:&FolderInventory,budget:&BatchBudget,cancel:&impl Fn()->bool,journal:&File,store:&File,source_root:&File,
        mut report:FolderReport,mut known:Vec<String>,prior:&[FolderRecord],report_name:&str)->Result<FolderReport> {
        for (index,entry) in inventory.entries.iter().enumerate() {
            if let Some(record)=prior.iter().find(|r|r.index==index as u64){report.records.push(record.clone());continue;}
            let mut record=FolderRecord {index:index as u64,relative_path:entry.relative_path.clone(),status:"skipped".into(),reason:entry.reason.clone(),production:None,detail:None};
            if entry.disposition!=EntryDisposition::Eligible {
                record.status=match entry.disposition {EntryDisposition::Unsupported=>"unsupported",EntryDisposition::Failed=>"failed",_=>"skipped"}.into();report.records.push(record);continue;
            }
            if report.stop_reason.is_none() {
                if budget.cancelled_or_expired(cancel){report.stop_reason=Some("cancelled_or_cooperative_deadline".into());}
                else if report.actions_started>=request.batch.maximum_actions {report.stop_reason=Some("action_limit".into());}
                else if report.publication_admission_bytes.checked_add(request.limits.candidate_bytes+request.overhead()?).is_none_or(|v|v>request.batch.maximum_publication_bytes) {report.stop_reason=Some("publication_byte_limit".into());}
            }
            if let Some(reason)=&report.stop_reason {record.reason=reason.clone();report.records.push(record);continue;}
            // Namespace, capacity and source checks precede the immutable start.
            same(&request.source_root,&source_root)?;same(&request.artifact_root,&store)?;store_members(request,&known)?;
            require_components(request,inventory,budget,cancel)?;
            let source=entry_path(&request.source_root,&entry.relative_path)?;
            let guard=||budget.check(cancel);
            let expected=entry.source.as_ref().context("eligible source binding missing")?;
            let observed=BoundedFileObservation::open_guarded(&source,request.limits.input_bytes,&guard)?;
            if observed.snapshot()!=&expected.snapshot||observed.digest()!=&expected.digest {
                record.status="failed".into();record.reason="source_changed_since_discovery".into();report.stop_reason=Some(record.reason.clone());report.records.push(record);continue;
            }
            let mut production=request.production(source);production.limits.elapsed_ms=production.limits.elapsed_ms.min(budget.remaining_ms());
            if production.limits.elapsed_ms==0 {report.stop_reason=Some("cooperative_deadline".into());record.reason="cooperative_deadline".into();report.records.push(record);continue;}
            free(&request.artifact_root,request.sandbox.minimum_free_bytes.checked_add(production.limits.candidate_bytes+request.overhead()?).context("folder store reserve overflow")?)?;
            free(&request.run_directory,request.sandbox.minimum_free_bytes.checked_add(JOURNAL_BYTES as u64).context("journal reserve overflow")?)?;
            write(&request.run_directory,&journal,&format!("{index:04}-start.json"),&serde_json::to_vec(&serde_json::json!({"schema":request.document_schema("start")?,"index":index,"request_fingerprint":report.request_fingerprint,"inventory_fingerprint":report.inventory_fingerprint,"source":entry.source,"effective_limits":production.limits}))?)?;
            report.actions_started+=1;
            match crate::video_production::produce(&production,||budget.cancelled_or_expired(cancel)) {
                Ok(outcome)=>{
                    match &outcome {
                        VideoProductionOutcome::Accepted {artifact_set_id,candidate_bytes,encoded_bytes_saved,..}=>{
                            // Account before readback; any uncertain readback stops further work.
                            report.publication_admission_bytes+=candidate_bytes+request.overhead()?;
                            known.push(artifact_set_id.clone());record.status="accepted".into();record.reason="validated_strictly_smaller_source_preserved".into();
                            if let Err(error)=accepted(request,entry,&outcome){record.status="failed".into();record.reason="published_candidate_readback_uncertain".into();record.detail=Some(error.to_string().chars().take(4096).collect());report.stop_reason=Some(record.reason.clone());}
                            else {report.encoded_bytes_saved+=encoded_bytes_saved;}
                        },
                        VideoProductionOutcome::Unchanged {..}=>{record.status="unchanged".into();record.reason="validated_candidate_not_strictly_smaller".into();},
                    }record.production=Some(outcome);
                },
                Err(error)=>{record.status="failed".into();record.reason="producer_refused_or_failed_reconcile_before_next_candidate".into();record.detail=Some(error.to_string().chars().take(4096).collect());report.stop_reason=Some(record.reason.clone());},
            }
            // Always retain the observed result even if cancellation arrived at
            // publication. A write failure leaves the start unresolved.
            if report.stop_reason.is_none()&&!budget.cancelled_or_expired(cancel)&&require_components(request,inventory,budget,cancel).is_err(){
                if record.status=="accepted" {if let Some(VideoProductionOutcome::Accepted {encoded_bytes_saved,..})=&record.production {report.encoded_bytes_saved-=encoded_bytes_saved;}}
                record.status="failed".into();record.reason="tool_binding_changed_after_candidate".into();report.stop_reason=Some(record.reason.clone());
            }
            write(&request.run_directory,&journal,&format!("{index:04}-result.json"),&serde_json::to_vec(&record)?)?;
            if report.stop_reason.is_none() {
                if budget.cancelled_or_expired(cancel){report.stop_reason=Some("cancelled_or_cooperative_deadline".into());}
                else if observed.verify_current_guarded(&||budget.check(cancel)).is_err(){report.stop_reason=Some("source_changed_or_deadline_after_candidate".into());}
                else if store_members(request,&known).is_err(){report.stop_reason=Some("store_contains_unreconciled_members".into());}
            }
            report.records.push(record);
        }
        if report.stop_reason.is_none()&&budget.cancelled_or_expired(cancel){report.stop_reason=Some("cancelled_or_cooperative_deadline".into());}
        if report.stop_reason.is_some(){report.state="stopped".into();}
        report.counts=counts(&inventory,&report.records);
        report.elapsed_ms=budget.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        write(&request.run_directory,&journal,report_name,&serde_json::to_vec(&report)?)?;Ok(report)
    }
    pub(super) fn inspect_recovery(root:&Path)->Result<FolderRecovery> {
        let bound=crate::candidate_artifact::open_directory(root)?;crate::candidate_artifact::require_private_root(&bound)?;lock(&bound)?;
        inspect_locked(root,&bound)
    }
    fn inspect_locked(root:&Path,bound:&File)->Result<FolderRecovery> {
        journal_size(root)?;
        let request:VideoFolderRequest=read(&bound,"request.json")?;request.batch.validate_for(request.profile()?)?;
        crate::contracts::validate(request.contract()?,&request)?;
        ensure!(request.run_directory==root,"folder recovery locator differs");
        let inventory:FolderInventory=read(&bound,"inventory.json")?;
        crate::contracts::validate(request.contract()?,&inventory)?;
        ensure!(inventory.schema==request.document_schema("inventory")?&&inventory.request_fingerprint==digest(&request)?&&inventory.entries.len()<=request.batch.maximum_entries as usize,"folder inventory/request binding differs");
        let mut allowed=std::collections::BTreeSet::from([std::ffi::OsString::from("request.json"),"inventory.json".into(),"report.json".into()]);
        let mut results=Vec::new();let mut unfinished=Vec::new();let mut pending=0;let mut gap=false;
        for (index,entry) in inventory.entries.iter().enumerate() {
            entry_path(&request.source_root,&entry.relative_path)?;
            let start_name=format!("{index:04}-start.json");let result_name=format!("{index:04}-result.json");
            let start=root.join(&start_name).symlink_metadata();let result=root.join(&result_name).symlink_metadata();
            let has_start=match start {Ok(_)=>true,Err(e)if e.kind()==std::io::ErrorKind::NotFound=>false,Err(e)=>return Err(e.into())};
            let has_result=match result {Ok(_)=>true,Err(e)if e.kind()==std::io::ErrorKind::NotFound=>false,Err(e)=>return Err(e.into())};
            ensure!(!has_result||has_start,"folder result has no immutable start");
            if has_start {
                ensure!(!gap,"folder attempt order skipped a never-started eligible predecessor");
                ensure!(entry.disposition==EntryDisposition::Eligible,"attempt names an ineligible entry");
                let start:serde_json::Value=read(&bound,&start_name)?;
                crate::contracts::validate(request.contract()?,&start)?;
                let effective:RequestedLimits=serde_json::from_value(start["effective_limits"].clone())?;let mut expected_limits=request.limits;
                ensure!(effective.elapsed_ms>0&&effective.elapsed_ms<=expected_limits.elapsed_ms,"folder effective time budget was widened");expected_limits.elapsed_ms=effective.elapsed_ms;
                ensure!(serde_json::to_value(expected_limits)?==start["effective_limits"],"folder effective production bounds differ");
                ensure!(start["schema"]==request.document_schema("start")?&&start["index"].as_u64()==Some(index as u64)
                    &&start["request_fingerprint"]==serde_json::to_value(digest(&request)?)?&&start["inventory_fingerprint"]==serde_json::to_value(digest(&inventory)?)?
                    &&start["source"]==serde_json::to_value(&entry.source)?,"folder start identity differs");
                allowed.insert(start_name.into());
                if has_result {let record:FolderRecord=read(&bound,&result_name)?;ensure!(record.index==index as u64&&record.relative_path==entry.relative_path,"folder result subject differs");
                    ensure!(matches!((&*record.status,&record.production),("accepted",Some(VideoProductionOutcome::Accepted {..}))|("unchanged",Some(VideoProductionOutcome::Unchanged {..}))|("failed",None)|("failed",Some(_))),"folder result status/production evidence disagree");
                    if let Some(outcome)=&record.production {accepted(&request,entry,outcome)?;}
                    results.push(record);allowed.insert(result_name.into());
                }else{unfinished.push(index as u64);}
            }else if entry.disposition==EntryDisposition::Eligible {pending+=1;gap=true;}
        }
        let mut report:Option<FolderReport>=None;let mut report_gap=false;
        for sequence in 0..=32u64 {
            let name=if sequence==0 {"report.json".into()}else{format!("report-{sequence:04}.json")};
            match root.join(&name).symlink_metadata() {Ok(_)=>{},Err(e)if e.kind()==std::io::ErrorKind::NotFound=>{if sequence>0 {report_gap=true;}continue;},Err(e)=>return Err(e.into())}
            ensure!(!report_gap,"folder continuation report sequence has a gap");let current:FolderReport=read(bound,&name)?;
            let prefix=results.get(..usize::try_from(current.actions_started)?).context("folder report claims unrecorded results")?;
            let (publication,saved,_)=ledger(&request,prefix)?;
            ensure!(current.schema==request.document_schema("report")?&&current.request_fingerprint==digest(&request)?&&current.inventory_fingerprint==digest(&inventory)?
                &&current.counts==counts(&inventory,&current.records)&&!current.replacement_authority&&!current.physical_space_reclaimed
                &&current.actions_started<=request.batch.maximum_actions&&current.publication_admission_bytes==publication&&current.encoded_bytes_saved==saved&&publication<=request.batch.maximum_publication_bytes
                &&current.records.len()==inventory.entries.len()&&current.records.iter().enumerate().all(|(i,r)|r.index==i as u64&&r.relative_path==inventory.entries[i].relative_path)
                &&prefix.iter().all(|r|serde_json::to_value(r).ok()==current.records.get(r.index as usize).and_then(|v|serde_json::to_value(v).ok()))
                &&current.records.iter().filter(|r|r.production.is_some()).all(|r|prefix.iter().any(|old|old.index==r.index))
                &&(current.state=="complete")==current.stop_reason.is_none(),"folder completion/result/accounting binding differs");
            if current.state=="complete" {ensure!(current.actions_started==inventory.entries.iter().filter(|e|e.disposition==EntryDisposition::Eligible).count() as u64,"complete folder report skipped eligible work");}
            if sequence==0 {ensure!(current.continuation.is_none(),"initial report cannot claim continuation");}
            else {
                let c=current.continuation.as_ref().context("continuation report lacks its binding")?;
                let earlier=prefix.get(..usize::try_from(c.prior_result_count)?).context("continuation prior result count is widened")?;
                ensure!(c.sequence==sequence&&c.previous_report_fingerprint==report.as_ref().map(digest).transpose()?
                    &&c.prior_results_fingerprint==digest(&earlier)?&&c.elapsed_policy=="fresh_invocation; cumulative_actions_and_publication"
                    &&report.as_ref().is_none_or(|old|c.prior_result_count>=old.actions_started),"continuation report history differs");
            }
            crate::contracts::validate(request.contract()?,&current)?;allowed.insert(name.into());report=Some(current);
        }
        let mut count=0;let mut total=0u64;for item in std::fs::read_dir(root)? {count+=1;ensure!(count<=2*request.batch.maximum_actions+35,"folder journal member count exceeds bounds");let item=item?;
            ensure!(allowed.contains(&item.file_name())&&item.file_type()?.is_file(),"folder journal contains an unowned member");total=total.checked_add(item.metadata()?.len()).context("journal size overflow")?;ensure!(total<=JOURNAL_BYTES as u64,"folder journal exceeds aggregate bound");}
        same(root,&bound)?;
        Ok(FolderRecovery {schema:request.document_schema("recovery")?,request_fingerprint:digest(&request)?,recorded_results:results,unfinished_attempts:unfinished,unstarted_eligible:pending,completed_report:report,replay_authority:false,
            source_freshness:"historical source binding only; current source is not revalidated by journal inspection".into()})
    }
    pub(super) fn resume(request:&VideoFolderRequest,budget:&BatchBudget,cancel:&impl Fn()->bool)->Result<FolderReport> {
        budget.check(cancel)?;
        let journal=crate::candidate_artifact::open_directory(&request.run_directory)?;let store=crate::candidate_artifact::open_directory(&request.artifact_root)?;
        lock(&journal)?;lock(&store)?;let source_root=crate::candidate_artifact::open_directory(&request.source_root)?;
        let recovery=inspect_locked(&request.run_directory,&journal)?;
        let original:VideoFolderRequest=read(&journal,"request.json")?;
        ensure!(digest(&original)?==digest(request)?&&recovery.request_fingerprint==digest(request)?,"folder continuation requires the exact original request and limits");
        ensure!(recovery.unfinished_attempts.is_empty()&&!recovery.recorded_results.iter().any(|r|r.status=="failed")
            &&recovery.completed_report.as_ref().is_none_or(|r|!r.records.iter().any(|row|row.status=="failed")),"unresolved start or failed/uncertain result blocks continuation; no replay/adoption is permitted");
        let inventory:FolderInventory=read(&journal,"inventory.json")?;require_components(request,&inventory,budget,cancel)?;
        let mut current=discover(&Discovery::for_run(request)?,budget,cancel)?;current.components=Some(components(request,budget,cancel)?);
        ensure!(digest(&current)?==digest(&inventory)?,"folder source inventory or tool bytes changed; preserve prior state and reconcile");
        let (publication,saved,known)=ledger(request,&recovery.recorded_results)?;
        ensure!(recovery.recorded_results.len() as u64<=request.batch.maximum_actions&&publication<=request.batch.maximum_publication_bytes,"folder cumulative admission already exceeded");
        same(&request.source_root,&source_root)?;same(&request.artifact_root,&store)?;store_members(request,&known)?;
        if let Some(report)=&recovery.completed_report {
            if report.actions_started==recovery.recorded_results.len() as u64&&(recovery.unstarted_eligible==0||report.actions_started>=request.batch.maximum_actions
                ||publication.checked_add(request.limits.candidate_bytes+request.overhead()?).is_none_or(|n|n>request.batch.maximum_publication_bytes)){return Ok(report.clone());}
        }
        let sequence=recovery.completed_report.as_ref().and_then(|r|r.continuation.as_ref()).map(|c|c.sequence).unwrap_or(0)+1;
        ensure!(sequence<=32,"folder continuation report limit exhausted; reconcile before another run");
        let report=FolderReport {continuation:Some(FolderContinuation {sequence,previous_report_fingerprint:recovery.completed_report.as_ref().map(digest).transpose()?,
            prior_result_count:recovery.recorded_results.len() as u64,prior_results_fingerprint:digest(&recovery.recorded_results)?,elapsed_policy:"fresh_invocation; cumulative_actions_and_publication".into()}),
            schema:request.document_schema("report")?,request_fingerprint:digest(request)?,inventory_fingerprint:digest(&inventory)?,state:"complete".into(),stop_reason:None,
            actions_started:recovery.recorded_results.len() as u64,records:Vec::new(),counts:FolderCounts::default(),publication_admission_bytes:publication,encoded_bytes_saved:saved,
            elapsed_ms:0,physical_space_reclaimed:false,replacement_authority:false,qualification:"source_only_native_host_unqualified".into()};
        free(&request.run_directory,request.sandbox.minimum_free_bytes.checked_add(JOURNAL_BYTES as u64).context("journal reserve overflow")?)?;
        advance(request,&inventory,budget,cancel,&journal,&store,&source_root,report,known,&recovery.recorded_results,&format!("report-{sequence:04}.json"))
    }
}

#[cfg(all(test,target_os="linux"))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt,symlink};
    struct Fixture { _temp:tempfile::TempDir,request:VideoFolderRequest }
    fn fixture()->Fixture {
        let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();
        for name in ["source","work","store","journal"] {let path=root.join(name);std::fs::create_dir(&path).unwrap();std::fs::set_permissions(&path,std::fs::Permissions::from_mode(0o700)).unwrap();}
        let old:VideoProductionRequest=serde_json::from_str(include_str!("../examples/video-production-request-v1.json")).unwrap();
        for name in ["ffmpeg","ffprobe"] {let path=root.join(name);std::fs::write(&path,format!("synthetic inert {name}; no launch")).unwrap();std::fs::set_permissions(path,std::fs::Permissions::from_mode(0o700)).unwrap();}
        Fixture {_temp:temp,request:VideoFolderRequest {schema:"optiflow.video-folder-request.v1".into(),source_root:root.join("source"),ffmpeg:root.join("ffmpeg"),ffprobe:root.join("ffprobe"),
            private_workdir:root.join("work"),artifact_root:root.join("store"),run_directory:root.join("journal"),limits:old.limits,sandbox:old.sandbox,
            batch:FolderLimits {maximum_entries:64,maximum_files:32,maximum_actions:8,maximum_depth:8,maximum_total_input_bytes:67_108_864,maximum_publication_bytes:67_108_864,elapsed_ms:600_000},recursive:true}}
    }
    #[test]
    fn nested_paths_are_ordered_and_extensions_never_grant_video_eligibility() {
        let f=fixture();std::fs::write(f.request.source_root.join("z.mkv"),b"ordinary text").unwrap();std::fs::create_dir(f.request.source_root.join("nested")).unwrap();
        std::fs::write(f.request.source_root.join("nested/a.mp4"),[0x1a,0x45,0xdf,0xa3,0]).unwrap();
        let inventory=discover(&f.request,||false).unwrap();assert_eq!(inventory.entries.len(),2);
        assert_eq!(inventory.entries[0].relative_path.to_path_buf(),PathBuf::from("nested/a.mp4"));
        assert!(inventory.entries.iter().all(|e|e.disposition==EntryDisposition::Unsupported));
        assert_eq!(std::fs::read(f.request.source_root.join("z.mkv")).unwrap(),b"ordinary text");assert!(std::fs::read_dir(&f.request.artifact_root).unwrap().next().is_none());
    }
    #[test]
    fn symlinks_and_hard_link_aliases_are_skipped_without_opening_targets() {
        let f=fixture();let source=f.request.source_root.join("file");std::fs::write(&source,b"text").unwrap();
        std::fs::hard_link(&source,f.request.source_root.join("alias")).unwrap();symlink("/missing/target",f.request.source_root.join("link")).unwrap();
        let inventory=discover(&f.request,||false).unwrap();assert_eq!(inventory.hashed_input_bytes,0);
        assert_eq!(inventory.entries.iter().filter(|e|e.reason=="hard_link_alias_not_admitted").count(),2);
        assert_eq!(inventory.entries.iter().filter(|e|e.reason=="symlink_not_followed").count(),1);
    }
    #[test]
    fn input_byte_cap_is_applied_in_path_order_and_recursion_is_explicit() {
        let mut f=fixture();std::fs::write(f.request.source_root.join("z"),b"xyz").unwrap();std::fs::write(f.request.source_root.join("a"),b"abc").unwrap();
        std::fs::create_dir(f.request.source_root.join("nested")).unwrap();std::fs::write(f.request.source_root.join("nested/hidden"),b"value").unwrap();
        f.request.batch.maximum_total_input_bytes=3;f.request.recursive=false;
        let inventory=discover(&f.request,||false).unwrap();assert_eq!(inventory.hashed_input_bytes,3);
        assert_eq!(inventory.entries[0].relative_path.to_path_buf(),PathBuf::from("a"));assert!(inventory.entries[0].source.is_some());
        assert!(inventory.entries.iter().any(|e|e.reason=="recursion_disabled"));assert!(inventory.entries.iter().any(|e|e.relative_path.to_path_buf()==PathBuf::from("z")&&e.reason=="batch_input_read_byte_limit"));
    }
    #[test]
    fn discovery_overflow_and_prestart_cancellation_create_no_candidate_or_journal() {
        let mut f=fixture();std::fs::write(f.request.source_root.join("a"),b"one").unwrap();std::fs::write(f.request.source_root.join("b"),b"two").unwrap();
        f.request.batch.maximum_entries=1;f.request.batch.maximum_files=1;f.request.batch.maximum_actions=1;
        assert!(discover(&f.request,||false).is_err());assert!(run(&f.request,||true).is_err());
        assert!(std::fs::read_dir(&f.request.run_directory).unwrap().next().is_none());assert!(std::fs::read_dir(&f.request.artifact_root).unwrap().next().is_none());
    }
    #[test]
    fn unsupported_only_batch_is_durable_and_repeated_recovery_never_replays() {
        let f=fixture();std::fs::write(f.request.source_root.join("text.mkv"),b"text").unwrap();
        let report=run(&f.request,||false).unwrap();assert_eq!(report.actions_started,0);assert_eq!(report.counts.unsupported_files,1);assert!(!report.physical_space_reclaimed);
        let bytes=std::fs::read(f.request.run_directory.join("report.json")).unwrap();
        for _ in 0..2 {let recovery=inspect_recovery(&f.request.run_directory).unwrap();assert!(!recovery.replay_authority);assert!(recovery.completed_report.is_some());}
        assert!(run(&f.request,||false).is_err());assert_eq!(std::fs::read(f.request.run_directory.join("report.json")).unwrap(),bytes);
    }
    fn interrupted_fixture()->Fixture {
        let f=fixture();std::fs::write(f.request.source_root.join("source.mkv"),b"synthetic pending identity only").unwrap();
        let mut inventory=discover(&f.request,||false).unwrap();inventory.entries[0].disposition=EntryDisposition::Eligible;inventory.entries[0].reason="synthetic_interruption_fixture".into();
        let bound=crate::candidate_artifact::open_directory(&f.request.run_directory).unwrap();
        crate::candidate_artifact::write_new(&bound,"request.json",&serde_json::to_vec(&f.request).unwrap()).unwrap();crate::candidate_artifact::write_new(&bound,"inventory.json",&serde_json::to_vec(&inventory).unwrap()).unwrap();
        let start=serde_json::json!({"schema":"optiflow.video-folder-start.v1","index":0,"request_fingerprint":digest(&f.request).unwrap(),"inventory_fingerprint":digest(&inventory).unwrap(),"source":inventory.entries[0].source,"effective_limits":f.request.limits});
        crate::candidate_artifact::write_new(&bound,"0000-start.json",&serde_json::to_vec(&start).unwrap()).unwrap();f
    }
    #[test]
    fn incomplete_start_stays_unreconciled_on_every_read_and_is_never_adopted() {
        let f=interrupted_fixture();let before=std::fs::read(f.request.run_directory.join("0000-start.json")).unwrap();
        for _ in 0..2 {let recovery=inspect_recovery(&f.request.run_directory).unwrap();assert_eq!(recovery.unfinished_attempts,[0]);assert!(recovery.recorded_results.is_empty());assert!(!recovery.replay_authority);}
        assert_eq!(std::fs::read(f.request.run_directory.join("0000-start.json")).unwrap(),before);assert!(std::fs::read_dir(&f.request.artifact_root).unwrap().next().is_none());
    }
    #[test]
    fn result_without_start_and_aliased_journal_member_are_refused() {
        let f=interrupted_fixture();std::fs::remove_file(f.request.run_directory.join("0000-start.json")).unwrap();std::fs::write(f.request.run_directory.join("0000-result.json"),b"{}").unwrap();
        assert!(inspect_recovery(&f.request.run_directory).is_err());
        std::fs::remove_file(f.request.run_directory.join("0000-result.json")).unwrap();symlink("/missing/target",f.request.run_directory.join("report.json")).unwrap();assert!(inspect_recovery(&f.request.run_directory).is_err());
    }
    #[test]
    fn native_non_utf8_path_is_retained_losslessly_and_not_misrouted_to_utf8_production() {
        use std::os::unix::ffi::OsStringExt;let f=fixture();let name=std::ffi::OsString::from_vec(b"clip-\xff.mkv".to_vec());std::fs::write(f.request.source_root.join(&name),b"data").unwrap();
        let inventory=discover(&f.request,||false).unwrap();assert_eq!(inventory.entries[0].relative_path.to_path_buf(),PathBuf::from(name));assert_eq!(inventory.entries[0].reason,"production_source_requires_utf8_locator");assert!(inventory.entries[0].source.is_none());
    }
    #[test]
    fn completed_folder_resume_is_freshly_checked_and_idempotent() {
        let f=fixture();std::fs::write(f.request.source_root.join("text"),b"ordinary text").unwrap();let original=run(&f.request,||false).unwrap();
        let bytes=std::fs::read(f.request.run_directory.join("report.json")).unwrap();
        let resumed=resume(&f.request,||false).unwrap();assert_eq!(serde_json::to_value(original).unwrap(),serde_json::to_value(resumed).unwrap());
        assert_eq!(std::fs::read(f.request.run_directory.join("report.json")).unwrap(),bytes);assert!(!f.request.run_directory.join("report-0001.json").exists());
    }
    #[test]
    fn missing_final_report_can_be_reconciled_without_replaying_any_attempt() {
        let f=fixture();std::fs::write(f.request.source_root.join("text"),b"ordinary text").unwrap();run(&f.request,||false).unwrap();
        std::fs::remove_file(f.request.run_directory.join("report.json")).unwrap();let report=resume(&f.request,||false).unwrap();
        assert_eq!(report.actions_started,0);assert_eq!(report.continuation.unwrap().sequence,1);
        let recovery=inspect_recovery(&f.request.run_directory).unwrap();assert!(recovery.completed_report.is_some());assert!(recovery.recorded_results.is_empty());assert!(!recovery.replay_authority);
        assert!(std::fs::read_dir(&f.request.artifact_root).unwrap().next().is_none());
    }
    #[test]
    fn resume_keeps_incomplete_starts_and_failed_results_blocked() {
        let f=interrupted_fixture();let start=std::fs::read(f.request.run_directory.join("0000-start.json")).unwrap();assert!(resume(&f.request,||false).is_err());
        assert_eq!(std::fs::read(f.request.run_directory.join("0000-start.json")).unwrap(),start);assert!(!f.request.run_directory.join("0000-result.json").exists());
        let record=FolderRecord {index:0,relative_path:NativePath::from_path(Path::new("source.mkv")),status:"failed".into(),reason:"uncertain_native_attempt".into(),production:None,detail:None};
        let bound=crate::candidate_artifact::open_directory(&f.request.run_directory).unwrap();crate::candidate_artifact::write_new(&bound,"0000-result.json",&serde_json::to_vec(&record).unwrap()).unwrap();
        assert!(resume(&f.request,||false).is_err());assert!(!f.request.run_directory.join("report-0001.json").exists());
    }
    #[test]
    fn changed_sources_tools_or_request_cannot_resume_a_previous_inventory() {
        let f=fixture();let path=f.request.source_root.join("text");std::fs::write(&path,b"original").unwrap();run(&f.request,||false).unwrap();std::fs::write(path,b"changed").unwrap();assert!(resume(&f.request,||false).is_err());
        let f=fixture();run(&f.request,||false).unwrap();std::fs::write(&f.request.ffmpeg,b"changed tool").unwrap();assert!(resume(&f.request,||false).is_err());
        let mut f=fixture();run(&f.request,||false).unwrap();f.request.batch.maximum_actions-=1;assert!(resume(&f.request,||false).is_err());
    }
    #[test]
    fn orphan_store_members_block_continuation_and_are_preserved() {
        let f=fixture();run(&f.request,||false).unwrap();let orphan=f.request.artifact_root.join("unresolved");std::fs::create_dir(&orphan).unwrap();
        assert!(resume(&f.request,||false).is_err());assert!(orphan.is_dir());assert!(!f.request.run_directory.join("report-0001.json").exists());
    }
    #[test]
    fn continuation_history_and_cumulative_accounting_cannot_be_widened() {
        let f=fixture();run(&f.request,||false).unwrap();std::fs::remove_file(f.request.run_directory.join("report.json")).unwrap();resume(&f.request,||false).unwrap();
        let path=f.request.run_directory.join("report-0001.json");let mut report:serde_json::Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();report["publication_admission_bytes"]=serde_json::json!(1);
        std::fs::write(&path,serde_json::to_vec(&report).unwrap()).unwrap();assert!(inspect_recovery(&f.request.run_directory).is_err());assert!(resume(&f.request,||false).is_err());
    }
    #[test] fn v2_folder_is_explicit_and_preserves_v1_per_file_limits(){let mut f=fixture();f.request.limits.input_bytes=134_217_728;assert!(f.request.validate().is_err());f.request.schema="optiflow.video-folder-request.v2".into();let p:VideoProductionRequest=serde_json::from_str(include_str!("../examples/video-production-request-v2.json")).unwrap();f.request.limits=p.limits;f.request.sandbox=p.sandbox;f.request.validate().unwrap();assert_eq!(f.request.production(f.request.source_root.join("x.mkv")).schema,"optiflow.video-production-request.v2");}
    #[test] fn v2_budget_does_not_widen_old_folder_schema(){let mut limits=fixture().request.batch;limits.maximum_publication_bytes=1_073_741_824;assert!(limits.validate().is_err());limits.validate_for(Profile::BoundedV2).unwrap();}
    #[test] fn unknown_folder_version_never_selects_latest_profile(){let mut f=fixture();f.request.schema="optiflow.video-folder-request.v3".into();assert!(f.request.profile().is_err());}
    #[test] fn video_folder_cli_requires_explicit_request_and_has_no_disposition_flag(){use clap::Parser;assert!(crate::cli::Cli::try_parse_from(["optiflow","video-folder","run"]).is_err());assert!(crate::cli::Cli::try_parse_from(["optiflow","video-folder","run","--request","/records/r.json","--discard-originals"]).is_err());}

}
