//! Fixed serial MP4 composition over existing descriptor discovery and producer.
//! Only never-started work may continue; uncertain starts are never replayed.
use super::*;
use anyhow::Context;
use std::{collections::BTreeSet,fs::File,time::{Duration,Instant}};
use crate::video_folder::{FolderLimits,FolderInventory,SourceBinding,EntryDisposition};
use progress::{Observer,Phase,Transition};
const JOURNAL_BYTES:u64=16_777_216;
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema:String,pub source_root:PathBuf,pub recursive:bool,pub template:planning::Request,
    pub run_directory:PathBuf,pub batch:FolderLimits,pub scratch_cleanup:String,
}
impl Request {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{ensure!(bytes.len()<=131_072,"MP4 folder request exceeds bound");let r:Self=serde_json::from_slice(bytes)?;r.validate()?;Ok(r)}
    pub(crate) fn validate(&self)->Result<()> {
        self.template.validate()?;normalized(&self.source_root)?;normalized(&self.run_directory)?;ensure!(self.run_directory.to_str().is_some_and(|s|s.len()<=4096&&!s.chars().any(char::is_control)),"bounded UTF8 journal path required");
        ensure!(self.schema=="optiflow.mp4-folder-request.v1"&&self.template.source==self.source_root&&self.scratch_cleanup=="remove_only_known_generated_candidate_after_durable_result","explicit MP4 root template and generated-only scratch cleanup policy required");
        let b=&self.batch;ensure!((1..=4096).contains(&b.maximum_entries)&&(1..=256).contains(&b.maximum_files)&&b.maximum_files<=b.maximum_entries&&(1..=32).contains(&b.maximum_actions)&&b.maximum_actions<=b.maximum_files&&(1..=32).contains(&b.maximum_depth)&&(1..=8_589_934_592).contains(&b.maximum_total_input_bytes)&&(1..=8_589_934_592).contains(&b.maximum_publication_bytes)&&(1..=86_400_000).contains(&b.elapsed_ms),"bounded explicit MP4 folder policy required");
        for p in [&self.source_root,&self.template.private_workdir,&self.template.artifact_root,&self.template.ffmpeg.path,&self.template.ffprobe.path,&self.template.sandbox.helper.path,&self.template.sandbox.systemd_run.path,&self.template.sandbox.systemctl.path]{ensure!(!self.run_directory.starts_with(p)&&!p.starts_with(&self.run_directory),"MP4 journal overlaps source/work/store/tool namespace");}
        Ok(())
    }
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {schema:String,index:usize,request_digest:EvidenceDigest,source:SourceBinding,child_request:planning::Request}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Cleanup {schema:String,index:usize,record_digest:EvidenceDigest,generated_candidate_removed:bool,candidate_digest:Option<EvidenceDigest>,candidate_bytes:Option<u64>,attempt_directory:Option<PathBuf>,source_mutation:bool}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {pub schema:String,pub index:usize,pub status:String,pub reason:String,pub outcome:Option<Outcome>}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {schema:String,sequence:usize,request_digest:EvidenceDigest,inventory_digest:EvidenceDigest,results_digest:EvidenceDigest,result_count:usize,actions_started:u64,encoded_bytes_saved:u64,publication_admission_bytes:u64,state:String,elapsed_policy:String,physical_space_reclaimed:bool,replacement_authority:bool}
#[derive(Debug,Serialize)]
pub struct Status {pub schema:String,pub request_digest:EvidenceDigest,pub inventory_digest:EvidenceDigest,pub state:String,pub records:Vec<Record>,pub unfinished_attempts:Vec<usize>,pub unstarted_eligible:u64,pub actions_started:u64,pub encoded_bytes_saved:u64,pub publication_admission_bytes:u64,pub source_freshness:String,pub replay_authority:bool,pub native_qualified:bool,pub physical_space_reclaimed:bool,pub replacement_authority:bool,pub quality_review:String}
struct Basis {request:Request,inventory:FolderInventory,tools:Vec<planning::Tool>,results:Vec<Record>,starts:Vec<Start>,reports:Vec<Report>,unfinished:Vec<usize>}
fn path(root:&Path,relative:&crate::domain::NativePath)->Result<PathBuf>{let relative=relative.to_path_buf();ensure!(!relative.as_os_str().is_empty()&&relative.components().all(|c|matches!(c,std::path::Component::Normal(_))),"bounded relative MP4 entry required");Ok(root.join(relative))}
fn same(root:&Path,bound:&File)->Result<()> {
    #[cfg(unix)]{let current=crate::candidate_artifact::open_directory(root)?;let a=rustix::fs::fstat(bound)?;let b=rustix::fs::fstat(&current)?;ensure!(a.st_dev==b.st_dev&&a.st_ino==b.st_ino,"MP4 journal directory changed");Ok(())}
    #[cfg(not(unix))]{let _=(root,bound);anyhow::bail!("MP4 folder journal unavailable")}
}
fn locked(root:&Path)->Result<File>{let bound=crate::candidate_artifact::open_directory(root)?;crate::candidate_artifact::require_private_root(&bound)?;
    #[cfg(unix)]{rustix::fs::flock(&bound,rustix::fs::FlockOperation::NonBlockingLockExclusive).context("another MP4 folder operation holds the journal")?;}
    #[cfg(not(unix))]{anyhow::bail!("MP4 folder locking unavailable");}
    same(root,&bound)?;Ok(bound)
}
fn create(root:&Path)->Result<File>{normalized(root)?;
    #[cfg(unix)]{use rustix::fs::{mkdirat,Mode};let parent=crate::candidate_artifact::open_directory(root.parent().context("journal parent missing")?)?;crate::candidate_artifact::require_private_root(&parent)?;mkdirat(&parent,root.file_name().context("journal leaf missing")?,Mode::RUSR|Mode::WUSR|Mode::XUSR)?;parent.sync_all()?;locked(root)}
    #[cfg(not(unix))]{anyhow::bail!("MP4 folder journal creation unavailable")}
}
fn members(root:&Path)->Result<BTreeSet<String>> {let mut names=BTreeSet::new();let mut total=0u64;for e in std::fs::read_dir(root)?{let e=e?;ensure!(e.file_type()?.is_file(),"unknown symbolic/directory MP4 journal member refused");let name=e.file_name().into_string().map_err(|_|anyhow::anyhow!("journal member UTF8 required"))?;let m=e.metadata()?;total=total.checked_add(m.len()).context("journal byte overflow")?;ensure!(names.len()<8500&&total<=JOURNAL_BYTES,"MP4 journal aggregate bound exceeded");names.insert(name);}Ok(names)}
fn read<T:for<'de>Deserialize<'de>>(bound:&File,name:&str)->Result<T>{Ok(serde_json::from_slice(&crate::candidate_artifact::read_member(bound,name,JOURNAL_BYTES as usize)?)?)}
fn write<T:Serialize>(root:&Path,bound:&File,name:&str,value:&T)->Result<()> {same(root,bound)?;let bytes=serde_json::to_vec(value)?;let used=std::fs::read_dir(root)?.try_fold(0u64,|n,e|->Result<u64>{Ok(n.checked_add(e?.metadata()?.len()).context("journal byte overflow")?)})?;ensure!(used.checked_add(bytes.len() as u64).is_some_and(|n|n<=JOURNAL_BYTES),"MP4 journal byte admission exhausted");runtime::write_member(bound,name,&bytes)?;bound.sync_all()?;same(root,bound)}
pub fn preview(request:&Request,cancel:impl Fn()->bool)->Result<FolderInventory>{request.validate()?;crate::video_folder::discover_mp4(&request.source_root,&request.batch,request.recursive,request.template.limits.input_bytes,digest(request)?,&cancel)}
fn admission(request:&Request,actions:u64)->Result<u64>{request.template.control.maximum(request.template.limits.candidate_bytes).checked_add(134_234_112).and_then(|n|n.checked_mul(actions)).context("MP4 artifact plus retained audit admission overflow")}
fn cleanup_subject(record:&Record)->Result<(Option<PathBuf>,Option<EvidenceDigest>,Option<u64>)>{match record.outcome.as_ref().context("successful owner outcome required for cleanup")? {
    Outcome::Accepted {artifact,attempt_directory,..}=>Ok((Some(attempt_directory.clone()),Some(artifact.candidate_digest.clone()),Some(artifact.candidate_bytes))),
    Outcome::Unchanged {attempt_directory,candidate_digest,candidate_bytes,..}=>{ensure!(attempt_directory.is_some()==candidate_digest.is_some()&&candidate_digest.is_some()==candidate_bytes.is_some(),"unchanged generated scratch framing differs");Ok((attempt_directory.clone(),candidate_digest.clone(),*candidate_bytes))},
}}
fn cleanup(request:&Request,record:&Record)->Result<Cleanup>{let (attempt,digest_value,size)=cleanup_subject(record)?;
    if let Some(path)=&attempt{ensure!(path.parent()==Some(request.template.private_workdir.as_path())&&path.file_name().and_then(|n|n.to_str()).is_some_and(|n|n.starts_with("mp4-attempt-")),"cleanup confined to the exact owned generated attempt");let bound=crate::candidate_artifact::open_directory(path)?;crate::candidate_artifact::require_private_root(&bound)?;
        let allowed=BTreeSet::from(["intent.json","validation.json","candidate.mp4","source-video.framehash","source-audio.framehash","candidate-video.framehash","candidate-audio.framehash"]);let mut names=BTreeSet::new();let mut audit_bytes=0u64;
        for entry in std::fs::read_dir(path)?{let entry=entry?;let name=entry.file_name().into_string().map_err(|_|anyhow::anyhow!("unknown generated attempt name retained"))?;ensure!(entry.file_type()?.is_file()&&allowed.contains(name.as_str())&&names.len()<7,"unknown generated attempt members retained");if name!="candidate.mp4"{audit_bytes=audit_bytes.checked_add(entry.metadata()?.len()).context("generated audit size overflow")?;ensure!(audit_bytes<=100_663_296,"generated audit exceeds admitted retention");}names.insert(name);}
        ensure!(names.contains("candidate.mp4"),"generated candidate absent; cleanup cannot be inferred");
        let candidate=BoundedFileObservation::open_mp4(&path.join("candidate.mp4"),OUTPUT_BYTES,&||Ok(()))?;ensure!(Some(candidate.digest())==digest_value.as_ref()&&Some(candidate.snapshot().logical_bytes)==size,"generated scratch candidate changed; retain it");
        if let Some(Outcome::Accepted {artifact,..})=&record.outcome{artifact::inspect(artifact)?;}
        same(path,&bound)?;candidate.verify_current()?;
        #[cfg(unix)]{rustix::fs::unlinkat(&bound,"candidate.mp4",rustix::fs::AtFlags::empty())?;bound.sync_all()?;}
        #[cfg(not(unix))]{anyhow::bail!("generated candidate cleanup unavailable");}
    }
    Ok(Cleanup {schema:"optiflow.mp4-folder-generated-cleanup.v1".into(),index:record.index,record_digest:digest(record)?,generated_candidate_removed:attempt.is_some(),candidate_digest:digest_value,candidate_bytes:size,attempt_directory:attempt,source_mutation:false})
}
fn root_members(root:&Path)->Result<BTreeSet<PathBuf>>{let mut members=BTreeSet::new();for entry in std::fs::read_dir(root)?{let entry=entry?;ensure!(members.len()<32&&entry.file_type()?.is_dir(),"unknown/excess MP4 store member refused");members.insert(entry.path());}Ok(members)}
fn reconcile(root:&Path,bound:&File)->Result<Basis>{
    same(root,bound)?;let names=members(root)?;let request:Request=read(bound,"request.json")?;request.validate()?;ensure!(request.run_directory==root,"journal moved from its explicit request root");let inventory:FolderInventory=read(bound,"inventory.json")?;let tools:Vec<planning::Tool>=read(bound,"tools.json")?;
    ensure!(inventory.schema=="optiflow.mp4-folder-inventory.v1"&&inventory.request_fingerprint==digest(&request)?&&inventory.entries.len()<=request.batch.maximum_entries as usize,"MP4 frozen inventory/request differs");
    let mut expected=BTreeSet::from(["request.json".into(),"inventory.json".into(),"tools.json".into()]);let mut results=vec![];let mut starts=vec![];let mut unfinished=vec![];let mut gap=false;let mut store=BTreeSet::new();
    for (index,entry) in inventory.entries.iter().enumerate(){let source=path(&request.source_root,&entry.relative_path)?;let s=format!("{index:04}-start.json");let r=format!("{index:04}-result.json");
        if names.contains(&s){ensure!(!gap,"MP4 start after an unresolved predecessor refused");expected.insert(s.clone());let start:Start=read(bound,&s)?;ensure!(entry.disposition==EntryDisposition::Eligible&&start.schema=="optiflow.mp4-folder-start.v1"&&start.index==index&&start.request_digest==digest(&request)?&&entry.source.as_ref()==Some(&start.source)&&start.child_request.source==source,"MP4 start subject differs");
            let mut expected_child=request.template.clone();expected_child.source=source;expected_child.limits.elapsed_ms=start.child_request.limits.elapsed_ms;ensure!(start.child_request.limits.elapsed_ms>0&&start.child_request.limits.elapsed_ms<=request.template.limits.elapsed_ms&&digest(&expected_child)?==digest(&start.child_request)?,"MP4 child request scope differs");starts.push(start);if !names.contains(&r){unfinished.push(index);}}
        if names.contains(&r){ensure!(!gap,"MP4 result after an uncommitted predecessor refused");expected.insert(r.clone());let record:Record=read(bound,&r)?;ensure!(record.schema=="optiflow.mp4-folder-result.v1"&&record.index==index&&record.reason.len()<=2048,"MP4 result framing differs");
            if entry.disposition==EntryDisposition::Eligible {let start=starts.iter().find(|s|s.index==index).context("MP4 result without start refused")?;match (&record.status[..],&record.outcome){
                ("accepted",Some(Outcome::Accepted {artifact,source_bytes,candidate_bytes,encoded_bytes_saved,quality_review,physical_space_reclaimed,..}))=>{
                    let current=artifact::inspect(artifact)?;ensure!(artifact.directory.parent()==Some(request.template.artifact_root.as_path())&&store.insert(artifact.directory.clone())&&current.source_digest==start.source.digest&&current.source_bytes==start.source.snapshot.logical_bytes&&current.request_digest==digest(&start.child_request)?&&current.tools==tools&&*source_bytes==current.source_bytes&&*candidate_bytes==artifact.candidate_bytes&&source_bytes.checked_sub(*candidate_bytes)==Some(*encoded_bytes_saved)&&quality_review=="required; explicitly lossy video"&&!*physical_space_reclaimed,"MP4 accepted result/source/store differs");
                },
                ("unchanged",Some(Outcome::Unchanged {source_bytes,..}))=>ensure!(*source_bytes==start.source.snapshot.logical_bytes,"unchanged result source differs"),
                ("failed",None)=>{},_=>anyhow::bail!("eligible MP4 outcome state differs"),
            }}else{let expected_status=if entry.disposition==EntryDisposition::Unsupported{"unsupported"}else{"skipped"};ensure!(!names.contains(&s)&&record.outcome.is_none()&&record.status==expected_status,"noneligible MP4 result granted an action");}
            if eligible_success(entry,&record){let name=format!("{index:04}-cleanup.json");if names.contains(&name){expected.insert(name.clone());let c:Cleanup=read(bound,&name)?;let (attempt,hash_value,size)=cleanup_subject(&record)?;ensure!(c.schema=="optiflow.mp4-folder-generated-cleanup.v1"&&c.index==index&&c.record_digest==digest(&record)?&&c.attempt_directory==attempt&&c.candidate_digest==hash_value&&c.candidate_bytes==size&&c.generated_candidate_removed==attempt.is_some()&&!c.source_mutation,"generated cleanup evidence differs");}else{unfinished.push(index);gap=true;}}
            if record.status=="failed"{gap=true;}
            results.push(record);
        }else{gap=true;}
    }
    let mut reports=vec![];let mut prior_count=0;for sequence in 0..64 {let name=format!("report-{sequence:04}.json");if !names.contains(&name){break;}expected.insert(name.clone());let r:Report=read(bound,&name)?;ensure!(r.schema=="optiflow.mp4-folder-report.v1"&&r.sequence==sequence&&r.request_digest==digest(&request)?&&r.inventory_digest==digest(&inventory)?&&r.result_count>=prior_count&&r.result_count<=results.len()&&r.results_digest==digest(&results[..r.result_count])?&&!r.physical_space_reclaimed&&!r.replacement_authority&&r.elapsed_policy=="new_explicit_bounded_window_for_never_started_work; cumulative_actions_and_publication_caps","retained MP4 report scope differs");
        let prefix=&results[..r.result_count];let actions=starts.iter().filter(|s|s.index<r.result_count).count() as u64;let saved=prefix.iter().try_fold(0u64,|n,v|->Result<u64>{Ok(n.checked_add(match &v.outcome{Some(Outcome::Accepted {encoded_bytes_saved,..})=>*encoded_bytes_saved,_=>0}).context("retained report savings overflow")?)})?;let state=if prefix.iter().any(|v|v.status=="failed"){"reconciliation_required"}else if r.result_count==inventory.entries.len(){"retained_complete"}else{"never_started_work_remains"};ensure!(r.actions_started==actions&&r.encoded_bytes_saved==saved&&r.publication_admission_bytes==admission(&request,actions)?&&r.state==state,"retained MP4 report totals/state differ");prior_count=r.result_count;reports.push(r);}
    ensure!(names==expected,"unknown/orphan MP4 journal member refused");ensure!(root_members(&request.template.artifact_root)?==store,"unaccounted MP4 artifact/staging remains; do not adopt or replay");same(root,bound)?;
    Ok(Basis {request,inventory,tools,results,starts,reports,unfinished})
}
fn eligible_success(entry:&crate::video_folder::FolderEntry,record:&Record)->bool{entry.disposition==EntryDisposition::Eligible&&matches!(record.status.as_str(),"accepted"|"unchanged")}
fn summary(b:&Basis)->Result<Status>{let actions=b.starts.len() as u64;let publication=admission(&b.request,actions)?;ensure!(actions<=b.request.batch.maximum_actions&&publication<=b.request.batch.maximum_publication_bytes,"cumulative MP4 action/publication budget exceeded");let saved=b.results.iter().try_fold(0u64,|n,r|->Result<u64>{Ok(n.checked_add(match &r.outcome{Some(Outcome::Accepted {encoded_bytes_saved,..})=>*encoded_bytes_saved,_=>0}).context("MP4 savings overflow")?)})?;
    let uncertain=!b.unfinished.is_empty()||b.results.iter().any(|r|r.status=="failed");let state=if uncertain{"reconciliation_required"}else if b.results.len()==b.inventory.entries.len(){"retained_complete"}else{"never_started_work_remains"};
    Ok(Status {schema:"optiflow.mp4-folder-status.v1".into(),request_digest:digest(&b.request)?,inventory_digest:digest(&b.inventory)?,state:state.into(),records:b.results.clone(),unfinished_attempts:b.unfinished.clone(),unstarted_eligible:b.inventory.entries.iter().enumerate().filter(|(i,e)|e.disposition==EntryDisposition::Eligible&&!b.starts.iter().any(|s|s.index==*i)).count() as u64,actions_started:actions,encoded_bytes_saved:saved,publication_admission_bytes:publication,source_freshness:"not_reobserved_by_status; explicit_resume_reconciles_current_inventory_and_tools".into(),replay_authority:false,native_qualified:false,physical_space_reclaimed:false,replacement_authority:false,quality_review:"required_for_each_lossy_candidate".into()})
}
pub fn status(root:&Path)->Result<Status>{let bound=locked(root)?;summary(&reconcile(root,&bound)?)}
fn drive(root:&Path,bound:&File,cancel:&impl Fn()->bool,started:Instant,observer:&Observer<'_>)->Result<Status>{
    observer.phase(Phase::FolderReconciliation,Transition::Started,None);let mut basis=reconcile(root,bound)?;let previous=summary(&basis)?;ensure!(previous.state!="reconciliation_required","uncertain/failed MP4 work cannot be replayed");ensure!(basis.reports.len()<64,"MP4 continuation report count exhausted");observer.phase(Phase::FolderReconciliation,Transition::Finished,None);
    let request=basis.request.clone();let deadline=Duration::from_millis(request.batch.elapsed_ms);let mut actions=previous.actions_started;
    for index in basis.results.len()..basis.inventory.entries.len(){if cancel()||started.elapsed()>=deadline{break;}let entry=&basis.inventory.entries[index];let eligible=entry.disposition==EntryDisposition::Eligible;
        if eligible&&(actions>=request.batch.maximum_actions||admission(&request,actions+1)?>request.batch.maximum_publication_bytes){break;}
        observer.entries(Transition::Started,index,basis.results.len() as u64,basis.inventory.entries.len() as u64);
        let mut record=Record {schema:"optiflow.mp4-folder-result.v1".into(),index,status:if entry.disposition==EntryDisposition::Unsupported{"unsupported"}else{"skipped"}.into(),reason:entry.reason.clone(),outcome:None};
        if eligible {
            let mut child=request.template.clone();child.source=path(&request.source_root,&entry.relative_path)?;child.limits.elapsed_ms=child.limits.elapsed_ms.min(deadline.checked_sub(started.elapsed()).context("MP4 folder budget exhausted")?.as_millis().try_into()?);ensure!(child.limits.elapsed_ms>0,"MP4 folder budget exhausted");
            let source=entry.source.clone().context("eligible MP4 source binding absent")?;let start=Start {schema:"optiflow.mp4-folder-start.v1".into(),index,request_digest:digest(&request)?,source:source.clone(),child_request:child.clone()};write(root,bound,&format!("{index:04}-start.json"),&start)?;actions+=1;
            let result=validation::produce_bound(&child,Some(&source),Some(&basis.tools),||cancel()||started.elapsed()>=deadline,observer,Some(index));
            match result {Ok(value)=>{record.status=if matches!(&value,Outcome::Accepted {..}){"accepted"}else{"unchanged"}.into();record.reason="owner outcome retained; technical acceptance is not human quality approval".into();record.outcome=Some(value);},Err(error)=>{record.status="failed".into();record.reason=format!("{error}").chars().take(1024).collect();}}
        }
        write(root,bound,&format!("{index:04}-result.json"),&record)?;if eligible_success(entry,&record){let c=cleanup(&request,&record)?;write(root,bound,&format!("{index:04}-cleanup.json"),&c)?;}let failed=record.status=="failed";basis.results.push(record);observer.entries(if failed{Transition::Failed}else{Transition::Finished},index,basis.results.len() as u64,basis.inventory.entries.len() as u64);if failed{break;}
    }
    let current=reconcile(root,bound)?;let value=summary(&current)?;ensure!(current.reports.len()<64,"MP4 continuation report count exhausted");observer.phase(Phase::FolderReport,Transition::Started,None);
    let report=Report {schema:"optiflow.mp4-folder-report.v1".into(),sequence:current.reports.len(),request_digest:value.request_digest.clone(),inventory_digest:value.inventory_digest.clone(),results_digest:digest(&current.results)?,result_count:current.results.len(),actions_started:value.actions_started,encoded_bytes_saved:value.encoded_bytes_saved,publication_admission_bytes:value.publication_admission_bytes,state:value.state.clone(),elapsed_policy:"new_explicit_bounded_window_for_never_started_work; cumulative_actions_and_publication_caps".into(),physical_space_reclaimed:false,replacement_authority:false};write(root,bound,&format!("report-{:04}.json",report.sequence),&report)?;observer.phase(Phase::FolderReport,Transition::Finished,None);Ok(value)
}
pub fn run(request:&Request,cancel:impl Fn()->bool)->Result<Status>{run_with_progress(request,cancel,&Observer::silent())}
pub fn run_with_progress(request:&Request,cancel:impl Fn()->bool,observer:&Observer<'_>)->Result<Status>{observed(observer,||run_inner(request,cancel,observer))}
fn run_inner(request:&Request,cancel:impl Fn()->bool,observer:&Observer<'_>)->Result<Status>{request.validate()?;let started=Instant::now();let stop=||cancel()||started.elapsed()>=Duration::from_millis(request.batch.elapsed_ms);observer.phase(Phase::FolderDiscovery,Transition::Started,None);let inventory=preview(request,&stop)?;let tools=planning::bind_tools(&request.template,&||{ensure!(!stop(),"MP4 folder cancelled");Ok(())})?;observer.phase(Phase::FolderDiscovery,Transition::Finished,None);
    crate::candidate_artifact::validate_private_root(&request.template.artifact_root)?;ensure!(root_members(&request.template.artifact_root)?.is_empty(),"new MP4 folder requires a dedicated empty artifact store");let bound=create(&request.run_directory)?;write(&request.run_directory,&bound,"request.json",request)?;write(&request.run_directory,&bound,"inventory.json",&inventory)?;write(&request.run_directory,&bound,"tools.json",&tools)?;drive(&request.run_directory,&bound,&cancel,started,observer)
}
pub fn resume(request:&Request,cancel:impl Fn()->bool)->Result<Status>{resume_with_progress(request,cancel,&Observer::silent())}
pub fn resume_with_progress(request:&Request,cancel:impl Fn()->bool,observer:&Observer<'_>)->Result<Status>{observed(observer,||resume_inner(request,cancel,observer))}
fn observed(observer:&Observer<'_>,operation:impl FnOnce()->Result<Status>)->Result<Status>{observer.phase(Phase::Operation,Transition::Started,None);let result=operation();let transition=match &result{Ok(value) if value.state=="retained_complete"=>Transition::Finished,Ok(_)=>Transition::Stopped,Err(_)=>Transition::Failed};observer.phase(Phase::Operation,transition,None);result}
fn resume_inner(request:&Request,cancel:impl Fn()->bool,observer:&Observer<'_>)->Result<Status>{request.validate()?;let started=Instant::now();let bound=locked(&request.run_directory)?;observer.phase(Phase::FolderReconciliation,Transition::Started,None);let basis=reconcile(&request.run_directory,&bound)?;ensure!(digest(&basis.request)?==digest(request)?,"MP4 continuation request differs");ensure!(summary(&basis)?.state!="reconciliation_required","failed/unfinished MP4 starts cannot be replayed");let stop=||cancel()||started.elapsed()>=Duration::from_millis(request.batch.elapsed_ms);let current_inventory=preview(request,&stop)?;ensure!(digest(&current_inventory)?==digest(&basis.inventory)?,"current MP4 inventory/source identities differ");let tools=planning::bind_tools(&request.template,&||{ensure!(!stop(),"MP4 continuation cancelled");Ok(())})?;ensure!(tools==basis.tools,"current MP4 tool baseline differs");observer.phase(Phase::FolderReconciliation,Transition::Finished,None);drive(&request.run_directory,&bound,&cancel,started,observer)}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn folder_paths_do_not_allow_a_frozen_entry_to_escape(){let p=crate::domain::NativePath::from_path(Path::new("../escape.mp4"));assert!(path(Path::new("/synthetic/source"),&p).is_err());let p=crate::domain::NativePath::from_path(Path::new("nested/曲.mp4"));assert_eq!(path(Path::new("/synthetic/source"),&p).unwrap(),Path::new("/synthetic/source/nested/曲.mp4"));}
    #[test]fn explicit_discovery_keeps_nonvideo_sources_and_creates_no_workspace(){let t=tempfile::tempdir().unwrap();let root=t.path().canonicalize().unwrap();std::fs::write(root.join("keep.txt"),b"synthetic nonvideo").unwrap();let batch=FolderLimits {maximum_entries:8,maximum_files:4,maximum_actions:1,maximum_depth:4,maximum_total_input_bytes:4096,maximum_publication_bytes:4096,elapsed_ms:10000};#[cfg(target_os="linux")]{let result=crate::video_folder::discover_mp4(&root,&batch,true,1024,hash(b"synthetic"),&||false).unwrap();assert_eq!(result.entries[0].disposition,EntryDisposition::Unsupported);assert_eq!(std::fs::read_dir(root).unwrap().count(),1);}}
}
