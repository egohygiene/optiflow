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
const PUBLICATION_OVERHEAD:u64=540_672;
const INVENTORY_BYTES:usize=4_194_304;
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderLimits {
    pub maximum_entries:u64,pub maximum_files:u64,pub maximum_actions:u64,pub maximum_depth:u64,
    pub maximum_total_input_bytes:u64,pub maximum_publication_bytes:u64,pub elapsed_ms:u64,
}
impl FolderLimits {
    pub fn validate(&self)->Result<()> {
        ensure!((1..=4096).contains(&self.maximum_entries)&&(1..=256).contains(&self.maximum_files)
            &&self.maximum_files<=self.maximum_entries&&(1..=32).contains(&self.maximum_actions)&&self.maximum_actions<=self.maximum_files
            &&(1..=32).contains(&self.maximum_depth)&&(1..=268_435_456).contains(&self.maximum_total_input_bytes)
            &&(1..=268_435_456).contains(&self.maximum_publication_bytes)&&(1..=600_000).contains(&self.elapsed_ms),"folder bounds exceed the selected serial subset");Ok(())
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
    fn production(&self,source:PathBuf)->VideoProductionRequest {
        VideoProductionRequest {schema:"optiflow.video-production-request.v1".into(),source,ffmpeg:self.ffmpeg.clone(),ffprobe:self.ffprobe.clone(),
            private_workdir:self.private_workdir.clone(),artifact_root:self.artifact_root.clone(),operation:VideoOperation::StreamCopy,limits:self.limits,sandbox:self.sandbox.clone()}
    }
    pub fn validate(&self)->Result<()> {
        ensure!(self.schema=="optiflow.video-folder-request.v1","unsupported video folder request");self.batch.validate()?;
        ensure!(serde_json::to_vec(self)?.len()<=131_072,"folder request exceeds 128 KiB");
        crate::contracts::validate(crate::contracts::Contract::VideoFolderProtocol,self)?;
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
    pub schema:String,pub request_fingerprint:EvidenceDigest,pub entries_seen:u64,pub files_seen:u64,
    pub hashed_input_bytes:u64,pub entries:Vec<FolderEntry>,pub scope:String,
}
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
    pub schema:String,pub request_fingerprint:EvidenceDigest,pub inventory_fingerprint:EvidenceDigest,
    pub state:String,pub stop_reason:Option<String>,pub actions_started:u64,pub records:Vec<FolderRecord>,
    pub counts:FolderCounts,
    pub publication_admission_bytes:u64,pub encoded_bytes_saved:u64,
    pub elapsed_ms:u64,pub physical_space_reclaimed:bool,pub replacement_authority:bool,pub qualification:String,
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
    #[cfg(target_os="linux")] {native::discover(request,&budget,&cancel)}
    #[cfg(not(target_os="linux"))] {let _=(budget,cancel);anyhow::bail!("video folder lane currently requires Linux");}
}
pub fn run(request:&VideoFolderRequest,cancel:impl Fn()->bool)->Result<FolderReport> {
    request.validate()?;let budget=BatchBudget::new(request.batch.elapsed_ms)?;
    #[cfg(target_os="linux")] {native::run(request,&budget,&cancel)}
    #[cfg(not(target_os="linux"))] {let _=(budget,cancel);anyhow::bail!("video folder lane currently requires Linux");}
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
    pub(super) fn discover(request:&VideoFolderRequest,budget:&BatchBudget,cancel:&impl Fn()->bool)->Result<FolderInventory> {
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
                else if metadata.len()>request.limits.input_bytes {record.reason="per_file_input_byte_limit".into();}
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
                    let guard=||budget.check(cancel);let observation=BoundedFileObservation::open_guarded(&path,request.limits.input_bytes,&guard)?;
                    ensure!(observation.snapshot().device_id==device&&observation.snapshot().link_count==1,"source identity/device/link changed during discovery");
                    let bytes=observation.read_bytes_guarded(&guard)?;hashed+=bytes.len() as u64;
                    let kind=infer::get(&bytes).map(|v|crate::inventory::classify_content_type(v.mime_type()));
                    let looks_video=matches!(kind,Some(crate::domain::MediaKind::Video))||bytes.starts_with(&[0x1a,0x45,0xdf,0xa3]);
                    record.source=Some(SourceBinding {snapshot:observation.snapshot().clone(),digest:observation.digest().clone()});
                    if !looks_video {record.disposition=EntryDisposition::Unsupported;record.reason="not_a_supported_video_content_type".into();}
                    else if crate::video_container::inspect(&bytes).is_err() {record.disposition=EntryDisposition::Unsupported;record.reason="outside_closed_matroska_ffv1_subset".into();}
                    else {record.reason="closed_container_observed_native_validation_pending".into();}
                    observation.verify_current_guarded(&guard)?;
                }
            }
            retained=retained.checked_add(serde_json::to_vec(record)?.len()).context("inventory byte overflow")?;ensure!(retained<=INVENTORY_BYTES,"folder inventory retention exceeds 4 MiB; no candidate started");
        }
        budget.check(cancel)?;
        Ok(FolderInventory {schema:"optiflow.video-folder-inventory.v1".into(),request_fingerprint:digest(request)?,entries_seen:seen,files_seen:files,hashed_input_bytes:hashed,entries:entries.into_iter().map(|(_,v,_)|v).collect(),
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
        for item in std::fs::read_dir(root)? {count+=1;ensure!(count<=67,"folder journal member count exceeds bounds");let item=item?;let meta=std::fs::symlink_metadata(item.path())?;
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
        if let VideoProductionOutcome::Accepted {artifact_set_id,artifact_directory,source_bytes,candidate_bytes,encoded_bytes_saved}=outcome {
            ensure!(uuid::Uuid::parse_str(artifact_set_id)?.to_string()==*artifact_set_id&&artifact_directory==&request.artifact_root.join(artifact_set_id),"folder artifact locator escaped its store");
            crate::video_artifact::inspect(artifact_directory)?;
            let bound=crate::candidate_artifact::open_directory(artifact_directory)?;
            let evidence:serde_json::Value=serde_json::from_slice(&crate::candidate_artifact::read_member(&bound,crate::video_artifact::EVIDENCE_NAME,524_288)?)?;
            let source=entry.source.as_ref().context("accepted folder source binding missing")?;
            ensure!(evidence["source"]["content"]["digest"]==serde_json::to_value(&source.digest)?&&evidence["source"]["snapshot"]==serde_json::to_value(&source.snapshot)?
                &&evidence["source"]["path"]==serde_json::to_value(NativePath::from_path(&entry_path(&request.source_root,&entry.relative_path)?))?
                &&source_bytes==&source.snapshot.logical_bytes&&evidence["source"]["content"]["logical_bytes"].as_u64()==Some(*source_bytes)
                &&evidence["candidate"]["content"]["logical_bytes"].as_u64()==Some(*candidate_bytes)&&evidence["encoded_bytes_saved"].as_u64()==Some(*encoded_bytes_saved),"folder candidate/source/outcome binding differs");
        }Ok(())
    }
    pub(super) fn run(request:&VideoFolderRequest,budget:&BatchBudget,cancel:&impl Fn()->bool)->Result<FolderReport> {
        budget.check(cancel)?;
        let journal=crate::candidate_artifact::open_directory(&request.run_directory)?;let store=crate::candidate_artifact::open_directory(&request.artifact_root)?;
        lock(&journal)?;lock(&store)?;
        ensure!(std::fs::read_dir(&request.run_directory)?.next().is_none(),"folder journal must be empty; inspect retained recovery state instead of replaying");store_members(request,&[])?;
        let source_root=crate::candidate_artifact::open_directory(&request.source_root)?;
        let inventory=discover(request,budget,cancel)?;
        free(&request.run_directory,request.sandbox.minimum_free_bytes.checked_add(JOURNAL_BYTES as u64).context("journal reserve overflow")?)?;
        write(&request.run_directory,&journal,"request.json",&serde_json::to_vec(request)?)?;
        write(&request.run_directory,&journal,"inventory.json",&serde_json::to_vec(&inventory)?)?;
        let mut report=FolderReport {schema:"optiflow.video-folder-report.v1".into(),request_fingerprint:digest(request)?,inventory_fingerprint:digest(&inventory)?,state:"complete".into(),stop_reason:None,actions_started:0,records:Vec::new(),counts:FolderCounts::default(),publication_admission_bytes:0,encoded_bytes_saved:0,elapsed_ms:0,physical_space_reclaimed:false,replacement_authority:false,qualification:"source_only_native_host_unqualified".into()};
        let mut known=Vec::<String>::new();
        for (index,entry) in inventory.entries.iter().enumerate() {
            let mut record=FolderRecord {index:index as u64,relative_path:entry.relative_path.clone(),status:"skipped".into(),reason:entry.reason.clone(),production:None,detail:None};
            if entry.disposition!=EntryDisposition::Eligible {
                record.status=match entry.disposition {EntryDisposition::Unsupported=>"unsupported",EntryDisposition::Failed=>"failed",_=>"skipped"}.into();report.records.push(record);continue;
            }
            if report.stop_reason.is_none() {
                if budget.cancelled_or_expired(cancel){report.stop_reason=Some("cancelled_or_cooperative_deadline".into());}
                else if report.actions_started>=request.batch.maximum_actions {report.stop_reason=Some("action_limit".into());}
                else if report.publication_admission_bytes.checked_add(request.limits.candidate_bytes+PUBLICATION_OVERHEAD).is_none_or(|v|v>request.batch.maximum_publication_bytes) {report.stop_reason=Some("publication_byte_limit".into());}
            }
            if let Some(reason)=&report.stop_reason {record.reason=reason.clone();report.records.push(record);continue;}
            // Namespace, capacity and source checks precede the immutable start.
            same(&request.source_root,&source_root)?;same(&request.artifact_root,&store)?;store_members(request,&known)?;
            let source=entry_path(&request.source_root,&entry.relative_path)?;
            let guard=||budget.check(cancel);
            let expected=entry.source.as_ref().context("eligible source binding missing")?;
            let observed=BoundedFileObservation::open_guarded(&source,request.limits.input_bytes,&guard)?;
            if observed.snapshot()!=&expected.snapshot||observed.digest()!=&expected.digest {
                record.status="failed".into();record.reason="source_changed_since_discovery".into();report.stop_reason=Some(record.reason.clone());report.records.push(record);continue;
            }
            let mut production=request.production(source);production.limits.elapsed_ms=production.limits.elapsed_ms.min(budget.remaining_ms());
            if production.limits.elapsed_ms==0 {report.stop_reason=Some("cooperative_deadline".into());record.reason="cooperative_deadline".into();report.records.push(record);continue;}
            free(&request.artifact_root,request.sandbox.minimum_free_bytes.checked_add(production.limits.candidate_bytes+PUBLICATION_OVERHEAD).context("folder store reserve overflow")?)?;
            free(&request.run_directory,request.sandbox.minimum_free_bytes.checked_add(JOURNAL_BYTES as u64).context("journal reserve overflow")?)?;
            write(&request.run_directory,&journal,&format!("{index:04}-start.json"),&serde_json::to_vec(&serde_json::json!({"schema":"optiflow.video-folder-start.v1","index":index,"request_fingerprint":report.request_fingerprint,"inventory_fingerprint":report.inventory_fingerprint,"source":entry.source,"effective_limits":production.limits}))?)?;
            report.actions_started+=1;
            match crate::video_production::produce(&production,||budget.cancelled_or_expired(cancel)) {
                Ok(outcome)=>{
                    match &outcome {
                        VideoProductionOutcome::Accepted {artifact_set_id,candidate_bytes,encoded_bytes_saved,..}=>{
                            // Account before readback; any uncertain readback stops further work.
                            report.publication_admission_bytes+=candidate_bytes+PUBLICATION_OVERHEAD;
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
        write(&request.run_directory,&journal,"report.json",&serde_json::to_vec(&report)?)?;Ok(report)
    }
    pub(super) fn inspect_recovery(root:&Path)->Result<FolderRecovery> {
        let bound=crate::candidate_artifact::open_directory(root)?;crate::candidate_artifact::require_private_root(&bound)?;lock(&bound)?;
        journal_size(root)?;
        let request:VideoFolderRequest=read(&bound,"request.json")?;request.batch.validate()?;
        crate::contracts::validate(crate::contracts::Contract::VideoFolderProtocol,&request)?;
        ensure!(request.run_directory==root,"folder recovery locator differs");
        let inventory:FolderInventory=read(&bound,"inventory.json")?;
        crate::contracts::validate(crate::contracts::Contract::VideoFolderProtocol,&inventory)?;
        ensure!(inventory.schema=="optiflow.video-folder-inventory.v1"&&inventory.request_fingerprint==digest(&request)?&&inventory.entries.len()<=request.batch.maximum_entries as usize,"folder inventory/request binding differs");
        let mut allowed=std::collections::BTreeSet::from([std::ffi::OsString::from("request.json"),"inventory.json".into(),"report.json".into()]);
        let mut results=Vec::new();let mut unfinished=Vec::new();let mut pending=0;
        for (index,entry) in inventory.entries.iter().enumerate() {
            entry_path(&request.source_root,&entry.relative_path)?;
            let start_name=format!("{index:04}-start.json");let result_name=format!("{index:04}-result.json");
            let start=root.join(&start_name).symlink_metadata();let result=root.join(&result_name).symlink_metadata();
            let has_start=match start {Ok(_)=>true,Err(e)if e.kind()==std::io::ErrorKind::NotFound=>false,Err(e)=>return Err(e.into())};
            let has_result=match result {Ok(_)=>true,Err(e)if e.kind()==std::io::ErrorKind::NotFound=>false,Err(e)=>return Err(e.into())};
            ensure!(!has_result||has_start,"folder result has no immutable start");
            if has_start {
                ensure!(entry.disposition==EntryDisposition::Eligible,"attempt names an ineligible entry");
                let start:serde_json::Value=read(&bound,&start_name)?;
                crate::contracts::validate(crate::contracts::Contract::VideoFolderProtocol,&start)?;
                let effective:RequestedLimits=serde_json::from_value(start["effective_limits"].clone())?;let mut expected_limits=request.limits;
                ensure!(effective.elapsed_ms>0&&effective.elapsed_ms<=expected_limits.elapsed_ms,"folder effective time budget was widened");expected_limits.elapsed_ms=effective.elapsed_ms;
                ensure!(serde_json::to_value(expected_limits)?==start["effective_limits"],"folder effective production bounds differ");
                ensure!(start["schema"]=="optiflow.video-folder-start.v1"&&start["index"].as_u64()==Some(index as u64)
                    &&start["request_fingerprint"]==serde_json::to_value(digest(&request)?)?&&start["inventory_fingerprint"]==serde_json::to_value(digest(&inventory)?)?
                    &&start["source"]==serde_json::to_value(&entry.source)?,"folder start identity differs");
                allowed.insert(start_name.into());
                if has_result {let record:FolderRecord=read(&bound,&result_name)?;ensure!(record.index==index as u64&&record.relative_path==entry.relative_path,"folder result subject differs");
                    ensure!(matches!((&*record.status,&record.production),("accepted",Some(VideoProductionOutcome::Accepted {..}))|("unchanged",Some(VideoProductionOutcome::Unchanged {..}))|("failed",None)|("failed",Some(VideoProductionOutcome::Accepted {..}))),"folder result status/production evidence disagree");
                    if let Some(outcome)=&record.production {accepted(&request,entry,outcome)?;}
                    results.push(record);allowed.insert(result_name.into());
                }else{unfinished.push(index as u64);}
            }else if entry.disposition==EntryDisposition::Eligible {pending+=1;}
        }
        let report=match root.join("report.json").symlink_metadata() {Ok(_)=>Some(read::<FolderReport>(&bound,"report.json")?),Err(e)if e.kind()==std::io::ErrorKind::NotFound=>None,Err(e)=>return Err(e.into())};
        if let Some(report)=&report {ensure!(report.schema=="optiflow.video-folder-report.v1"&&report.request_fingerprint==digest(&request)?&&report.inventory_fingerprint==digest(&inventory)?&&unfinished.is_empty()
            &&report.counts==counts(&inventory,&report.records)&&!report.replacement_authority&&!report.physical_space_reclaimed
            &&report.actions_started==results.len() as u64&&report.actions_started<=request.batch.maximum_actions&&report.publication_admission_bytes<=request.batch.maximum_publication_bytes
            &&report.records.len()==inventory.entries.len()&&report.records.iter().enumerate().all(|(i,r)|r.index==i as u64&&r.relative_path==inventory.entries[i].relative_path)
            &&results.iter().all(|r|serde_json::to_value(r).ok()==report.records.get(r.index as usize).and_then(|v|serde_json::to_value(v).ok())),"folder completion/result binding differs");}
        if let Some(report)=&report {crate::contracts::validate(crate::contracts::Contract::VideoFolderProtocol,report)?;}
        let mut count=0;let mut total=0u64;for item in std::fs::read_dir(root)? {count+=1;ensure!(count<=2*request.batch.maximum_actions+3,"folder journal member count exceeds bounds");let item=item?;
            ensure!(allowed.contains(&item.file_name())&&item.file_type()?.is_file(),"folder journal contains an unowned member");total=total.checked_add(item.metadata()?.len()).context("journal size overflow")?;ensure!(total<=JOURNAL_BYTES as u64,"folder journal exceeds aggregate bound");}
        same(root,&bound)?;
        Ok(FolderRecovery {schema:"optiflow.video-folder-recovery.v1".into(),request_fingerprint:digest(&request)?,recorded_results:results,unfinished_attempts:unfinished,unstarted_eligible:pending,completed_report:report,replay_authority:false,
            source_freshness:"historical source binding only; current source is not revalidated by journal inspection".into()})
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
        Fixture {_temp:temp,request:VideoFolderRequest {schema:"optiflow.video-folder-request.v1".into(),source_root:root.join("source"),ffmpeg:old.ffmpeg,ffprobe:old.ffprobe,
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
}
