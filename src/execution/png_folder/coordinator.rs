//! One-file coordinator source. No public mutation before native qualification.
use super::*;
use super::super::{replacement::{self,transaction::{self,ReplacementEvent}},journal::Journal};
use crate::{candidate_artifact as artifact,png_production};
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemResult {
    pub schema:String,pub index:u64,pub batch_authorization_id:String,
    pub source:NativePath,pub status:String,pub plan_fingerprint:Option<String>,
    pub encoded_logical_reduction_bytes:u64,pub temporaries_cleaned:bool,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema:String,pub request_fingerprint:String,pub inventory_fingerprint:String,
    pub batch_authorization_id:String,pub results:Vec<ItemResult>,pub uncertain_start:Option<u64>,
    pub encoded_logical_reduction_bytes:u64,pub physical_reclaimed_bytes:Option<u64>,
    pub one_file_in_flight:bool,pub qualification:String,
}
fn name(prefix:&str,index:u64)->String {format!("{prefix}-{index:06}.json")}
fn write<T:Serialize>(request:&Request,root:&std::fs::File,name:&str,value:&T)->Result<()> {
    let bytes=journal_members(request)?;
    let data=serde_json::to_vec(value)?;ensure!(bytes.checked_add(data.len() as u64).is_some_and(|v|v<=16_777_216),"folder run journal exceeds 16 MiB");
    artifact::write_new(root,name,&data)?;root.sync_all()?;Ok(())
}
fn journal_members(request:&Request)->Result<u64>{
    #[cfg(unix)] use std::os::unix::fs::MetadataExt;
    let mut count=0u64;let mut total=0u64;
    for item in std::fs::read_dir(&request.run_directory)? {
        count+=1;ensure!(count<=200,"folder journal member bound exceeded");let item=item?;
        let name=item.file_name();let name=name.to_str().context("unexpected non-UTF8 run member")?;
        let known=["request.json","inventory.json","approval.json","provider.json"].contains(&name)
            ||(0..request.batch.maximum_actions).any(|i|["start","result","replacement-plan","replacement-approval","candidate-cleanup"].iter().any(|prefix|name==self::name(prefix,i)));
        let metadata=std::fs::symlink_metadata(item.path())?;
        ensure!(known&&metadata.is_file()&&!metadata.file_type().is_symlink(),"unknown or redirected folder journal member");
        #[cfg(unix)] ensure!(metadata.nlink()==1,"folder journal hard-link alias refused");
        total=total.checked_add(metadata.len()).context("run journal accounting overflow")?;
        ensure!(metadata.len()<=4_194_304&&total<=16_777_216,"folder run journal exceeds bounds");
    }Ok(total)
}
fn validate_terminal(request:&Request,inventory:&Inventory,batch:&BatchApproval,entry:&Entry,result:&ItemResult,root:&std::fs::File,signals:&SignalState)->Result<()> {
    ensure!(!signals.is_cancelled(),"folder recovery cancelled");
    let source=entry.source.as_ref().context("completed folder item has no inventory binding")?;
    if result.status=="unchanged" {ensure!(result.plan_fingerprint.is_none()&&result.encoded_logical_reduction_bytes==0,"unchanged item claims replacement/savings");return source_now(source,request.per_file.maximum_file_bytes,signals);}
    let plan:replacement::ReplacementPlan=read(root,&name("replacement-plan",result.index))?;
    let single:replacement::Approval=read(root,&name("replacement-approval",result.index))?;
    single.validate(&plan)?;
    ensure!(result.plan_fingerprint.as_deref()==Some(plan.fingerprint.as_str())&&hash(&plan.body.source)?==hash(source)?
        &&single.approved_by==format!("batch:{}",batch.authorization_id)&&result.encoded_logical_reduction_bytes==plan.body.encoded_logical_reduction_bytes
        &&plan.body.state.path==NativePath::from_path(&request.state_directory)&&plan.body.retention.path==NativePath::from_path(&request.retention_directory),"terminal replacement plan/approval/source differs");
    batch.validate(request,inventory)?;
    let records=transaction::status(&plan)?;let last=records.last().context("terminal replacement journal absent")?;
    ensure!(last.authorization_id==single.authorization_id,"replacement journal approval differs");
    source_now(last.published.as_ref().context("terminal published binding absent")?,request.per_file.maximum_file_bytes,signals)?;
    match batch.disposition {
        OriginalDisposition::RetainForRestore=>{ensure!(result.status=="committed_retained"&&last.phase=="committed"&&last.original_disposal_authorization.is_none(),"retention disposition differs from terminal history");source_now(last.retained.as_ref().context("terminal retained binding absent")?,request.per_file.maximum_file_bytes,signals)?;},
        OriginalDisposition::DiscardAfterDurableReplacement=>{ensure!(result.status=="committed_discarded"&&last.phase=="cleaned"&&last.original_disposal_authorization.as_deref()==Some(batch.authorization_id.as_str())&&!request.retention_directory.join(&plan.fingerprint).exists(),"discard/cleanup is not terminal under exact batch authority");},
    }
    let cleanup:serde_json::Value=read(root,&name("candidate-cleanup",result.index))?;
    ensure!(cleanup["plan_fingerprint"]==plan.fingerprint&&cleanup["batch_authorization_id"]==batch.authorization_id,"candidate cleanup identity differs");Ok(())
}
fn read<T:serde::de::DeserializeOwned>(root:&std::fs::File,name:&str)->Result<T>{Ok(serde_json::from_slice(&artifact::read_member(root,name,4_194_304)?)?)}
fn empty(path:&Path)->Result<()> {ensure!(std::fs::read_dir(path)?.next().is_none(),"owned temporary store is not empty; reconcile before next item");Ok(())}
fn source_now(expected:&FileBinding,maximum:u64,signals:&SignalState)->Result<()> {
    let actual=diag(fs::snapshot(&diag(fs::native_path(&expected.path))?,signals,maximum))?;
    ensure!(hash(expected)?==hash(&actual)?,"source changed; no provider/replacement started");Ok(())
}
fn provider(request:&Request,signals:&SignalState)->Result<FileBinding>{diag(fs::snapshot(&request.oxipng,signals,67_108_864))}
fn report(request:&Request,inventory:&Inventory,approval:&BatchApproval,results:Vec<ItemResult>,uncertain_start:Option<u64>)->Result<Report>{
    let saved=results.iter().try_fold(0u64,|total,r|total.checked_add(r.encoded_logical_reduction_bytes).context("encoded reduction overflow"))?;
    Ok(Report {schema:"optiflow.png-folder-report.v1".into(),request_fingerprint:hash(request)?,inventory_fingerprint:inventory.fingerprint.clone(),batch_authorization_id:approval.authorization_id.clone(),results,uncertain_start,encoded_logical_reduction_bytes:saved,physical_reclaimed_bytes:None,one_file_in_flight:true,qualification:"unqualified_source_checkpoint".into()})
}
/// Read-only recovery: an uncertain start is reported, never adopted/replayed.
pub fn status(request:&Request,inventory:&Inventory,approval:&BatchApproval)->Result<Report>{status_guarded(request,inventory,approval,&SignalState::default())}
pub fn status_guarded(request:&Request,inventory:&Inventory,approval:&BatchApproval,signals:&SignalState)->Result<Report>{
    approval.validate(request,inventory)?;let root=artifact::open_directory(&request.run_directory)?;artifact::require_private_root(&root)?;
    let saved_request:Request=read(&root,"request.json")?;let saved_inventory:Inventory=read(&root,"inventory.json")?;let saved_approval:BatchApproval=read(&root,"approval.json")?;
    ensure!(hash(&saved_request)?==hash(request)?&&hash(&saved_inventory)?==hash(inventory)?&&hash(&saved_approval)?==hash(approval)?,"saved folder basis differs");
    journal_members(request)?;
    let eligible:Vec<_>=inventory.entries.iter().filter(|e|e.disposition=="eligible").collect();
    let mut results=Vec::new();let mut uncertain=None;
    for (index,entry) in eligible.iter().take(request.batch.maximum_actions as usize).enumerate(){
        let start=name("start",index as u64);let result=name("result",index as u64);
        let started=std::fs::symlink_metadata(request.run_directory.join(&start)).is_ok();
        let finished=std::fs::symlink_metadata(request.run_directory.join(&result)).is_ok();
        ensure!(!finished||started,"result without exact start record");
        if !started {
            for later in index+1..eligible.len().min(request.batch.maximum_actions as usize) {ensure!(!request.run_directory.join(name("start",later as u64)).exists()&&!request.run_directory.join(name("result",later as u64)).exists(),"non-prefix folder history requires reconciliation");}
            break;
        }
        let begin:serde_json::Value=read(&root,&start)?;
        ensure!(begin.as_object().is_some_and(|v|v.len()==4)&&begin["schema"]=="optiflow.png-folder-item-start.v1"&&begin["batch_authorization_id"]==approval.authorization_id&&begin["index"].as_u64()==Some(index as u64)&&begin["source"]==serde_json::to_value(&entry.path)?,"start identity differs");
        if !finished {uncertain=Some(index as u64);break;}
        let r:ItemResult=read(&root,&result)?;
        ensure!(r.schema=="optiflow.png-folder-item-result.v1"&&r.index==index as u64&&r.batch_authorization_id==approval.authorization_id&&r.source==entry.path&&r.temporaries_cleaned&&matches!(r.status.as_str(),"committed_retained"|"committed_discarded"|"unchanged"),"terminal item record differs");
        validate_terminal(request,inventory,approval,entry,&r,&root,signals)?;
        results.push(r);
    }
    report(request,inventory,approval,results,uncertain)
}
/// Public apply/resume cannot bypass #93/#95 native qualification.
pub fn run(request:&Request,inventory:&Inventory,approval:&BatchApproval,resume:bool,signals:&SignalState)->Result<Report>{
    approval.validate(request,inventory)?;transaction::require_qualification()?;
    #[cfg(target_os="linux")] {run_linux_source(request,inventory,approval,resume,signals)}
    #[cfg(not(target_os="linux"))] {let _=(resume,signals);anyhow::bail!("serial replacement/disposal requires a qualified property backend")}
}
#[cfg(target_os="linux")]
fn run_linux_source(request:&Request,inventory:&Inventory,approval:&BatchApproval,resume:bool,signals:&SignalState)->Result<Report>{
    request.validate()?;let started=Instant::now();let check=||{ensure!(!signals.is_cancelled()&&started.elapsed()<Duration::from_millis(request.batch.elapsed_ms),"folder operation cancelled or timed out");Ok(())};
    let root=artifact::open_directory(&request.run_directory)?;let store=artifact::open_directory(&request.artifact_root)?;
    rustix::fs::flock(&root,rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
    rustix::fs::flock(&store,rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
    let current_provider=provider(request,signals)?;
    if !resume {empty(&request.run_directory)?;empty(&request.artifact_root)?;
        let fresh=discover(request,signals)?;ensure!(fresh.fingerprint==inventory.fingerprint,"folder inventory changed before first action");
        for (name,value) in [("request.json",serde_json::to_value(request)?),("inventory.json",serde_json::to_value(inventory)?),("approval.json",serde_json::to_value(approval)?),("provider.json",serde_json::to_value(&current_provider)?)]{write(request,&root,name,&value)?;}
    } else {
        let old_request:Request=read(&root,"request.json")?;let old_inventory:Inventory=read(&root,"inventory.json")?;let old_approval:BatchApproval=read(&root,"approval.json")?;let old_provider:FileBinding=read(&root,"provider.json")?;
        ensure!(hash(&old_request)?==hash(request)?&&old_inventory.fingerprint==inventory.fingerprint&&old_approval.authorization_id==approval.authorization_id&&hash(&old_provider)?==hash(&current_provider)?,"resume basis/provider differs");
    }
    let prior=status_guarded(request,inventory,approval,signals)?;ensure!(prior.uncertain_start.is_none(),"uncertain prior start; inspect without replay");
    empty(&request.artifact_root)?;let mut results=prior.results;
    let eligible:Vec<_>=inventory.entries.iter().filter(|e|e.disposition=="eligible").take(request.batch.maximum_actions as usize).collect();
    for (index,entry) in eligible.iter().enumerate().skip(results.len()){
        check()?;empty(&request.artifact_root)?;
        let capacity=diag(fs::capacity(&store))?;
        let transient=request.per_file.maximum_file_bytes.checked_mul(2).and_then(|n|n.checked_add(request.per_file.journal_budget_bytes)).and_then(|n|n.checked_add(request.per_file.free_space_reserve_bytes)).context("folder transient reservation overflow")?;
        ensure!(!capacity.read_only&&capacity.available>=transient,"per-file candidate/stage/journal admission plus reserve unavailable");
        ensure!(hash(&provider(request,signals)?)?==hash(&current_provider)?,"provider changed before next item");
        let source=entry.source.as_ref().context("eligible source binding missing")?;source_now(source,request.per_file.maximum_file_bytes,signals)?;
        write(request,&root,&name("start",index as u64),&serde_json::json!({"schema":"optiflow.png-folder-item-start.v1","index":index,"batch_authorization_id":approval.authorization_id,"source":entry.path}))?;
        let outcome=png_production::produce_png(&diag(fs::native_path(&entry.path))?,&request.oxipng,&request.artifact_root,&crate::domain::EvidenceDigest {algorithm:"blake3-256".into(),value:request.policy_fingerprint.clone()},png_production::ProductionLimits {source_bytes:request.per_file.maximum_file_bytes as usize,candidate_bytes:request.per_file.maximum_file_bytes as usize,..Default::default()},||signals.is_cancelled()||started.elapsed()>=Duration::from_millis(request.batch.elapsed_ms));
        let item=match outcome {
            Err(e) if e.reason==png_production::ProductionRefusal::CandidateNotSmaller=>{empty(&request.artifact_root)?;source_now(source,request.per_file.maximum_file_bytes,signals)?;ItemResult {schema:"optiflow.png-folder-item-result.v1".into(),index:index as u64,batch_authorization_id:approval.authorization_id.clone(),source:entry.path.clone(),status:"unchanged".into(),plan_fingerprint:None,encoded_logical_reduction_bytes:0,temporaries_cleaned:true}},
            Err(e)=>return Err(anyhow::anyhow!("provider attempt retained as uncertain; no next item: {e}")),
            Ok(receipt)=>{
                check()?;source_now(source,request.per_file.maximum_file_bytes,signals)?;
                let plan=replacement::review_png(&diag(fs::native_path(&entry.path))?,&receipt.directory,&request.state_directory,&request.retention_directory,&request.policy_fingerprint,request.per_file.clone(),signals)?;
                let single=replacement::approve(&plan,format!("batch:{}",approval.authorization_id))?;
                write(request,&root,&name("replacement-plan",index as u64),&plan)?;write(request,&root,&name("replacement-approval",index as u64),&single)?;
                let committed=transaction::apply(&plan,&single,signals)?;ensure!(committed.phase=="committed","replacement is not durably committed");
                if approval.disposition==OriginalDisposition::DiscardAfterDurableReplacement {discard_original(request,inventory,approval,&plan,&single,signals)?;}
                // Persist cleanup intent before removing only this owned set.
                write(request,&root,&name("candidate-cleanup",index as u64),&serde_json::json!({"set_id":receipt.set_id,"plan_fingerprint":plan.fingerprint,"batch_authorization_id":approval.authorization_id}))?;
                remove_candidate(&receipt.directory,receipt.set_id,&plan)?;empty(&request.artifact_root)?;check()?;
                ItemResult {schema:"optiflow.png-folder-item-result.v1".into(),index:index as u64,batch_authorization_id:approval.authorization_id.clone(),source:entry.path.clone(),status:if approval.disposition==OriginalDisposition::RetainForRestore{"committed_retained"}else{"committed_discarded"}.into(),plan_fingerprint:Some(plan.fingerprint),encoded_logical_reduction_bytes:receipt.evidence.encoded_logical_reduction_bytes,temporaries_cleaned:true}
            }
        };
        write(request,&root,&name("result",index as u64),&item)?;results.push(item);check()?;
    }
    report(request,inventory,approval,results,None)
}
#[cfg(target_os="linux")]
fn discard_original(request:&Request,inventory:&Inventory,batch:&BatchApproval,plan:&replacement::ReplacementPlan,single:&replacement::Approval,signals:&SignalState)->Result<()> {
    use rustix::fs::{unlinkat,AtFlags};
    batch.validate(request,inventory)?;single.validate(plan)?;
    ensure!(batch.disposition==OriginalDisposition::DiscardAfterDurableReplacement&&inventory.entries.iter().any(|e|e.source.as_ref().is_some_and(|s|hash(s).ok()==hash(&plan.body.source).ok()))&&plan.body.evidence_policy_fingerprint==request.policy_fingerprint,"exact batch discard scope required");
    let mut journal=diag(Journal::open_replacement(&plan.body.state))?;let events=diag(journal.replacement_events(&plan.fingerprint))?;
    let mut e=events.last().cloned().context("committed replacement absent")?;ensure!(e.phase=="committed"&&e.authorization_id==single.authorization_id,"only committed exact replacement may discard");
    source_now(e.published.as_ref().context("published binding absent")?,plan.body.limits.maximum_file_bytes,signals)?;
    source_now(e.retained.as_ref().context("retained original binding absent")?,plan.body.limits.maximum_file_bytes,signals)?;
    let root=diag(fs::check_directory(&plan.body.retention))?;let path=request.retention_directory.join(&plan.fingerprint);let namespace=artifact::open_directory(&path)?;artifact::require_private_root(&namespace)?;
    ensure!(std::fs::read_dir(&path)?.take(2).collect::<std::io::Result<Vec<_>>>()?.len()==1,"original namespace contains unexpected work");
    e.sequence+=1;e.phase="discard_pending".into();e.original_disposal_authorization=Some(batch.authorization_id.clone());diag(journal.append_replacement(&e,plan.body.limits.journal_budget_bytes))?;
    ensure!(!signals.is_cancelled(),"discard cancelled; uncertain intent is never replayed");
    unlinkat(&namespace,"original",AtFlags::empty())?;diag(fs::sync_directory(&namespace))?;
    e.sequence+=1;e.phase="discarded".into();diag(journal.append_replacement(&e,plan.body.limits.journal_budget_bytes))?;
    e.sequence+=1;e.phase="cleanup_pending".into();diag(journal.append_replacement(&e,plan.body.limits.journal_budget_bytes))?;
    empty(&path)?;unlinkat(&root,plan.fingerprint.as_str(),AtFlags::REMOVEDIR)?;diag(fs::sync_directory(&root))?;
    e.sequence+=1;e.phase="cleaned".into();diag(journal.append_replacement(&e,plan.body.limits.journal_budget_bytes))?;Ok(())
}
#[cfg(target_os="linux")]
fn remove_candidate(path:&Path,id:uuid::Uuid,plan:&replacement::ReplacementPlan)->Result<()> {
    use rustix::fs::{unlinkat,AtFlags};
    ensure!(path==diag(fs::native_path(&plan.body.candidate_set))?,"candidate cleanup escaped plan");
    let inspection=artifact::inspect(path);ensure!(inspection.status==artifact::Status::Committed&&inspection.manifest.as_ref().is_some_and(|m|m.set_id==id.to_string()),"candidate set changed before cleanup");
    let parent=artifact::open_directory(path.parent().context("candidate parent absent")?)?;artifact::require_private_root(&parent)?;let root=artifact::open_directory(path)?;
    for (name,maximum,pin) in [(artifact::MARKER_NAME,16_384,plan.body.marker_blake3.as_str()),(artifact::EVIDENCE_NAME,artifact::MAX_EVIDENCE_BYTES,plan.body.evidence_blake3.as_str()),(artifact::CANDIDATE_NAME,plan.body.limits.maximum_file_bytes as usize,plan.body.candidate.blake3.as_str())]{ensure!(blake3::hash(&artifact::read_member(&root,name,maximum)?).to_hex().as_str()==pin,"cleanup member identity changed");}
    ensure!(std::fs::read_dir(path)?.take(4).collect::<std::io::Result<Vec<_>>>()?.len()==3,"candidate has unexpected members; no recursive cleanup");
    for name in [artifact::CANDIDATE_NAME,artifact::EVIDENCE_NAME,artifact::MARKER_NAME]{unlinkat(&root,name,AtFlags::empty())?;}
    root.sync_all()?;unlinkat(&parent,id.to_string().as_str(),AtFlags::REMOVEDIR)?;parent.sync_all()?;Ok(())
}
