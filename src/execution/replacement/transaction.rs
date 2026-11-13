//! Restorable transaction source. Public mutation dispatch remains refused
//! pending #93/#95 native qualification; there is no user bypass flag.
use super::*;
use std::fs::File;
use super::super::journal::Journal;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplacementEvent {
    pub schema: String,
    pub plan_fingerprint: String,
    pub authorization_id: String,
    pub sequence: u64,
    pub phase: String,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub original_disposal_authorization: Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub video_action: Option<VideoAction>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub media_validation: Option<serde_json::Value>,
    pub staged: Option<FileBinding>,
    pub retained: Option<FileBinding>,
    pub published: Option<FileBinding>,
}
impl ReplacementEvent {
    pub(in crate::execution) fn from_json_slice(bytes:&[u8])->Result<Self>{
        ensure!(bytes.len()<=1_048_576,"replacement event exceeds 1 MiB");
        let raw:serde_json::Value=serde_json::from_slice(bytes)?;
        let contract=if raw["schema"]=="optiflow.replacement-event.v2" {crate::contracts::Contract::ReplacementEventV2}else{crate::contracts::Contract::ReplacementEvent};
        crate::contracts::validate(contract,&raw)?;
        Ok(serde_json::from_slice(bytes)?)
    }
    pub fn validate_after(&self, prior: &[Self]) -> Result<()> {
        ensure!(prior.len()<32 && matches!((self.schema.as_str(),self.video_action.is_some()),("optiflow.replacement-event.v1",false)|("optiflow.replacement-event.v2",true))
            && pin(&self.plan_fingerprint)&&pin(&self.authorization_id)
            && self.sequence==prior.len() as u64,"replacement event identity/sequence differs");
        if let Some(action)=&self.video_action {
            ensure!(self.original_disposal_authorization.is_none()&&!matches!(self.phase.as_str(),"discard_pending"|"discarded"|"cleanup_pending"|"cleaned"),"restorable video has no disposal path");
            ensure!(action.operation=="replace_validated_video_stream_copy.v1"&&pin(&action.review_fingerprint)&&pin(&action.validation_request_fingerprint)&&pin(&action.current_validation_fingerprint),"versioned video action identity required");
            if prior.is_empty(){let evidence=self.media_validation.as_ref().context("initial video event needs current validation evidence")?;crate::contracts::validate(crate::contracts::Contract::VideoReplacementValidationRecord,evidence)?;
                ensure!(hash(evidence)?==action.current_validation_fingerprint&&evidence["review_fingerprint"]==action.review_fingerprint&&evidence["request_fingerprint"]==action.validation_request_fingerprint,"video validation evidence differs from action");
            }else{ensure!(self.media_validation.is_none(),"current validation bytes belong only to initial event");}
        }else{ensure!(self.media_validation.is_none(),"legacy PNG event cannot carry video evidence");}
        let disposal=matches!(self.phase.as_str(),"discard_pending"|"discarded"|"cleanup_pending"|"cleaned");
        ensure!(if disposal {self.original_disposal_authorization.as_deref().is_some_and(pin)}else{self.original_disposal_authorization.is_none()},"separate original-disposal authority required");
        if let Some(old)=prior.last().and_then(|p|p.original_disposal_authorization.as_ref()){ensure!(self.original_disposal_authorization.as_ref()==Some(old),"disposal authority changed");}
        let previous=prior.last().map(|e|e.phase.as_str());
        ensure!(matches!((previous,self.phase.as_str()),
            (None,"planned")|(Some("planned"),"staged")|(Some("staged"),"retain_pending")
            |(Some("retain_pending"),"retained")|(Some("retained"),"publish_pending")
            |(Some("publish_pending"),"committed")|(Some("committed"),"restore_pending")
            |(Some("restore_pending"),"restored")|(Some("committed"),"discard_pending")
            |(Some("discard_pending"),"discarded")|(Some("discarded"),"cleanup_pending")
            |(Some("cleanup_pending"),"cleaned")),"uncertain or unsupported replacement transition");
        if let Some(p)=prior.last(){
            ensure!(p.schema==self.schema&&p.video_action==self.video_action,"replacement action kind/validation identity changed");
            ensure!(p.plan_fingerprint==self.plan_fingerprint&&p.authorization_id==self.authorization_id,"replacement event authority changed");
            for (old,new) in [(&p.staged,&self.staged),(&p.retained,&self.retained),(&p.published,&self.published)] {
                if let Some(old)=old {ensure!(new.as_ref().is_some_and(|new|hash(old).ok()==hash(new).ok()),"existing replacement binding changed");}
            }
        }
        if self.phase!="planned" {ensure!(self.staged.is_some(),"staged file binding missing");}
        if matches!(self.phase.as_str(),"retained"|"publish_pending"|"committed"|"restore_pending"|"restored"|"discard_pending"|"discarded"|"cleanup_pending"|"cleaned") {ensure!(self.retained.is_some(),"retained file binding missing");}
        if matches!(self.phase.as_str(),"committed"|"restore_pending"|"restored"|"discard_pending"|"discarded"|"cleanup_pending"|"cleaned") {ensure!(self.published.is_some(),"published file binding missing");}
        Ok(())
    }
}
/// Explicit source-level gate. Removing it requires the separately reviewed
/// native qualification work; JSON, approval, flags or an extension cannot do so.
pub fn require_qualification() -> Result<()> {
    anyhow::bail!("Replacement native qualification is pending (#93/#95/#146); no source mutation or writable journal access")
}
pub fn apply(plan:&ReplacementPlan,approval:&Approval,signals:&SignalState)->Result<ReplacementEvent>{
    approval.validate(plan)?;require_qualification()?;
    #[cfg(target_os="linux")] { apply_linux_source(plan,approval,signals) }
    #[cfg(not(target_os="linux"))] {let _=signals;anyhow::bail!("replacement property backend is not qualified on this platform")}
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoAction {pub operation:String,pub review_fingerprint:String,pub validation_request_fingerprint:String,pub current_validation_fingerprint:String}

pub(in crate::execution) struct TransactionLimits {pub maximum_file_bytes:u64,pub free_space_reserve_bytes:u64,pub journal_budget_bytes:u64}
pub(in crate::execution) struct TransactionBody {pub source:FileBinding,pub candidate:FileBinding,pub state:DirectoryBinding,pub retention:DirectoryBinding,pub limits:TransactionLimits}
pub(in crate::execution) struct TransactionView {pub fingerprint:String,pub authorization_id:String,pub body:TransactionBody,pub video_action:Option<VideoAction>,pub media_validation:Option<serde_json::Value>}
impl TransactionView {fn png(plan:&ReplacementPlan,approval:&Approval)->Self{Self {fingerprint:plan.fingerprint.clone(),authorization_id:approval.authorization_id.clone(),body:TransactionBody {source:plan.body.source.clone(),candidate:plan.body.candidate.clone(),state:plan.body.state.clone(),retention:plan.body.retention.clone(),limits:TransactionLimits {maximum_file_bytes:plan.body.limits.maximum_file_bytes,free_space_reserve_bytes:plan.body.limits.free_space_reserve_bytes,journal_budget_bytes:plan.body.limits.journal_budget_bytes}},video_action:None,media_validation:None}}}
fn event(view:&TransactionView,sequence:u64,phase:&str,staged:Option<FileBinding>,retained:Option<FileBinding>,published:Option<FileBinding>)->ReplacementEvent {
    ReplacementEvent {schema:if view.video_action.is_some(){"optiflow.replacement-event.v2"}else{"optiflow.replacement-event.v1"}.into(),plan_fingerprint:view.fingerprint.clone(),authorization_id:view.authorization_id.clone(),sequence,phase:phase.into(),original_disposal_authorization:None,video_action:view.video_action.clone(),media_validation:if sequence==0{view.media_validation.clone()}else{None},staged,retained,published}
}
fn append(journal:&mut Journal,view:&TransactionView,record:&ReplacementEvent)->Result<()> {diag(journal.append_replacement(record,view.body.limits.journal_budget_bytes))}
fn moved_matches(actual:&FileBinding,expected:&FileBinding)->Result<()> {
    ensure!(actual.identity==expected.identity&&actual.size_bytes==expected.size_bytes
        && actual.modified_unix_ns==expected.modified_unix_ns&&actual.mode==expected.mode
        && actual.uid==expected.uid&&actual.gid==expected.gid&&actual.blake3==expected.blake3,
        "renamed subject differs; retain evidence and stop");Ok(())
}
#[cfg(target_os="linux")]
fn preserved_properties(expected:&super::super::mutation::Properties,actual:&super::super::mutation::Properties)->Result<()> {
    // Reads may advance atime, and rename changes ctime. Neither can stand for
    // byte/property equality; ctime is independently checked by each binding.
    ensure!(expected.uid==actual.uid&&expected.gid==actual.gid&&expected.mode==actual.mode
        &&expected.mtime==actual.mtime&&expected.xattrs==actual.xattrs,
        "held replacement properties changed; retain evidence and stop");
    Ok(())
}
#[cfg(target_os="linux")]
fn check_held_subject(file:&mut File,binding:&FileBinding,properties:&super::super::mutation::Properties,maximum:u64,signals:&SignalState)->Result<()> {
    ensure!(binding.size_bytes>0&&binding.size_bytes<=maximum&&!signals.is_cancelled(),
        "replacement bound exceeded or cancelled; no namespace transition");
    diag(fs::check_file(file,binding))?;
    let current=diag(fs::hash_bound(file,binding.size_bytes,signals))?;
    ensure!(current==binding.blake3,"held replacement bytes changed; retain evidence and stop");
    preserved_properties(properties,&diag(super::super::mutation::properties(file))?)?;
    diag(fs::check_file(file,binding))?;
    ensure!(!signals.is_cancelled(),"replacement cancelled; no namespace transition");
    Ok(())
}
#[cfg(target_os="linux")]
fn check_directory_handle(file:&File,binding:&DirectoryBinding)->Result<()> {
    use std::os::unix::fs::MetadataExt;
    diag(fs::check_directory(binding))?;
    let m=diag(fs::metadata(file))?;
    ensure!(m.is_dir()&&binding.identity.platform=="linux"
        &&m.dev().to_string()==binding.identity.filesystem_id&&m.ino().to_string()==binding.identity.file_id
        &&m.mode()==binding.mode&&m.uid()==binding.uid&&m.gid()==binding.gid,
        "held replacement directory differs from its current path");
    diag(fs::writable_directory(file))?;
    Ok(())
}
/// Reuse the bounded execution hasher and property observer, without reopening
/// a subject for its bytes. These are observations, not an atomic snapshot or
/// permission to replay an intent when a check/rename/synchronization fails.
#[cfg(target_os="linux")]
fn check_transition_subjects(
    body:&TransactionBody,parent:&File,namespace:&File,namespace_binding:&DirectoryBinding,
    first:(&mut File,&FileBinding,&super::super::mutation::Properties),
    second:(&mut File,&FileBinding,&super::super::mutation::Properties),signals:&SignalState,
)->Result<()> {
    check_held_subject(first.0,first.1,first.2,body.limits.maximum_file_bytes,signals)?;
    check_held_subject(second.0,second.1,second.2,body.limits.maximum_file_bytes,signals)?;
    diag(fs::check_directory(&body.retention))?;
    check_directory_handle(parent,&body.source.directory)?;
    check_directory_handle(namespace,namespace_binding)?;
    artifact::require_private_root(namespace)?;
    // A second full read can be long: recheck the first subject's metadata too.
    diag(fs::check_file(first.0,first.1))?;
    diag(fs::check_file(second.0,second.1))?;
    ensure!(!signals.is_cancelled(),"replacement cancelled; inspect pending journal, never replay");
    Ok(())
}
#[cfg(target_os="linux")]
fn apply_linux_source(plan:&ReplacementPlan,approval:&Approval,signals:&SignalState)->Result<ReplacementEvent>{let view=TransactionView::png(plan,approval);apply_shared(&view,signals,&||recheck_png(plan,approval,signals))}
/// Private common engine: callers already own media-specific approval and live
/// validation. Legacy PNG validation and its byte ceiling remain unchanged.
#[cfg(target_os="linux")]
pub(in crate::execution) fn apply_shared(view:&TransactionView,signals:&SignalState,revalidate:&impl Fn()->Result<()>)->Result<ReplacementEvent>{
    use std::io::Write;
    use rustix::fs::{Mode,OFlags,RenameFlags,mkdirat,openat,renameat_with};
    revalidate()?;
    let b=&view.body;let mut journal=diag(Journal::open_replacement(&b.state))?;
    ensure!(diag(journal.replacement_events(&view.fingerprint))?.is_empty(),"existing replacement run requires read-only inspection; no replay");
    revalidate()?;
    let retained_root=diag(fs::check_directory(&b.retention))?;diag(fs::writable_directory(&retained_root))?;
    let parent=diag(fs::check_directory(&b.source.directory))?;diag(fs::writable_directory(&parent))?;
    let capacity=diag(fs::capacity(&retained_root))?;
    let required=b.candidate.size_bytes.checked_add(b.limits.journal_budget_bytes).and_then(|v|v.checked_add(b.limits.free_space_reserve_bytes)).context("replacement reserve overflow")?;
    ensure!(!capacity.read_only&&capacity.available>=required,"replacement transient storage plus reserve unavailable");
    let planned=event(view,0,"planned",None,None,None);append(&mut journal,view,&planned)?;
    mkdirat(&retained_root,view.fingerprint.as_str(),Mode::RUSR|Mode::WUSR|Mode::XUSR)?;
    diag(fs::sync_directory(&retained_root))?;
    let namespace_path=diag(fs::native_path(&b.retention.path))?.join(&view.fingerprint);
    let namespace=artifact::open_directory(&namespace_path)?;
    artifact::require_private_root(&namespace)?;
    let namespace_binding=diag(fs::directory(&namespace_path))?;
    check_directory_handle(&namespace,&namespace_binding)?;
    let mut source=diag(fs::open_bound(&b.source))?;
    let properties=diag(super::super::mutation::properties(&source))?;
    let candidate_bytes=bytes(&diag(fs::native_path(&b.candidate.path))?,b.limits.maximum_file_bytes)?;
    ensure!(blake3::hash(&candidate_bytes).to_hex().as_str()==b.candidate.blake3&&!signals.is_cancelled(),"candidate changed or replacement cancelled");
    let mut staged=File::from(openat(&namespace,"replacement",OFlags::RDWR|OFlags::CREATE|OFlags::EXCL|OFlags::NOFOLLOW|OFlags::CLOEXEC,Mode::RUSR|Mode::WUSR)?);
    staged.write_all(&candidate_bytes)?;
    diag(super::super::mutation::set_properties(&staged,&properties))?;
    let after=diag(super::super::mutation::properties(&staged))?;
    preserved_properties(&properties,&after)?;
    diag(fs::sync_file(&staged))?;diag(fs::sync_directory(&namespace))?;
    let staged_binding=diag(fs::snapshot(&namespace_path.join("replacement"),signals,b.limits.maximum_file_bytes))?;
    ensure!(staged_binding.blake3==b.candidate.blake3&&staged_binding.size_bytes==b.candidate.size_bytes,"staged replacement differs");
    let mut record=event(view,1,"staged",Some(staged_binding.clone()),None,None);append(&mut journal,view,&record)?;
    revalidate()?;
    record.sequence+=1;record.phase="retain_pending".into();append(&mut journal,view,&record)?;
    let original_path=diag(fs::native_path(&b.source.path))?;let name=original_path.file_name().context("original leaf absent")?;
    check_transition_subjects(b,&parent,&namespace,&namespace_binding,
        (&mut source,&b.source,&properties),(&mut staged,&staged_binding,&properties),signals)?;
    renameat_with(&parent,name,&namespace,"original",RenameFlags::NOREPLACE)?;
    diag(fs::sync_directory(&parent))?;diag(fs::sync_directory(&namespace))?;
    let original=diag(fs::snapshot(&namespace_path.join("original"),signals,b.limits.maximum_file_bytes))?;
    moved_matches(&original,&b.source)?;
    record.sequence+=1;record.phase="retained".into();record.retained=Some(original.clone());append(&mut journal,view,&record)?;
    ensure!(!signals.is_cancelled(),"cancelled after retention; inspect journal, never replay");
    record.sequence+=1;record.phase="publish_pending".into();append(&mut journal,view,&record)?;
    check_transition_subjects(b,&parent,&namespace,&namespace_binding,
        (&mut source,&original,&properties),(&mut staged,&staged_binding,&properties),signals)?;
    renameat_with(&namespace,"replacement",&parent,name,RenameFlags::NOREPLACE)?;
    diag(fs::sync_directory(&parent))?;diag(fs::sync_directory(&namespace))?;
    let published=diag(fs::snapshot(&original_path,signals,b.limits.maximum_file_bytes))?;
    moved_matches(&published,&staged_binding)?;
    check_transition_subjects(b,&parent,&namespace,&namespace_binding,
        (&mut source,&original,&properties),(&mut staged,&published,&properties),signals)?;
    record.sequence+=1;record.phase="committed".into();record.published=Some(published);append(&mut journal,view,&record)?;
    Ok(record)
}

/// Read only status uses SQLite read-only access and never classifies/replays an
/// uncertain transition. Source bindings must be reobserved before any recovery.
pub fn status(plan:&ReplacementPlan)->Result<Vec<ReplacementEvent>>{
    plan.validate()?;status_for(&plan.body.state,&plan.fingerprint,plan.body.limits.journal_budget_bytes)
}
pub(in crate::execution) fn status_for(bound_state:&DirectoryBinding,fingerprint:&str,budget:u64)->Result<Vec<ReplacementEvent>>{
    let state=diag(fs::check_directory(bound_state))?;artifact::require_private_root(&state)?;
    let path=diag(fs::native_path(&bound_state.path))?.join("state.sqlite3");
    match std::fs::symlink_metadata(&path) {Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Vec::new()),Err(e)=>return Err(e.into()),Ok(_)=>{}}
    let database=diag(fs::open(&path,false))?;
    ensure!(database.metadata()?.is_file(),"replacement journal is not a regular file");
    #[cfg(unix)] {use std::os::unix::fs::MetadataExt;ensure!(database.metadata()?.nlink()==1,"replacement journal hard-link alias refused");}
    let connection=rusqlite::Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let version:i64=connection.query_row("SELECT COALESCE(MAX(version),0) FROM schema_migrations",[],|r|r.get(0))?;
    ensure!(version<=10,"replacement journal was migrated by a newer implementation");
    if version<10 {return Ok(Vec::new());}
    let (count,total):(u64,u64)=connection.query_row("SELECT COUNT(*),COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM execution_replacement_events WHERE plan_fingerprint=?1",[fingerprint],|r|Ok((r.get(0)?,r.get(1)?)))?;
    ensure!(count<=32&&total<=budget,"replacement journal exceeds count/byte bounds");
    let records:Vec<String>=connection.prepare("SELECT document_json FROM execution_replacement_events WHERE plan_fingerprint=?1 ORDER BY sequence")?.query_map([fingerprint],|r|r.get(0))?.collect::<std::result::Result<_,_>>()?;
    let mut out=Vec::new();let mut total=0u64;
    for text in records {total=total.checked_add(text.len() as u64).context("journal size overflow")?;ensure!(total<=budget,"replacement journal exceeds bound");let e=ReplacementEvent::from_json_slice(text.as_bytes())?;e.validate_after(&out)?;ensure!(e.plan_fingerprint==fingerprint,"another plan in journal");out.push(e);}
    Ok(out)
}
/// Restore source is deliberately no-replace and retains the optimized object.
/// It has the same native qualification gate as apply, before writable state.
pub fn restore(plan:&ReplacementPlan,approval:&Approval,signals:&SignalState)->Result<()> {
    approval.validate(plan)?;require_qualification()?;
    #[cfg(target_os="linux")] {restore_shared(&TransactionView::png(plan,approval),signals)}
    #[cfg(not(target_os="linux"))] {let _=signals;anyhow::bail!("replacement restore property backend unavailable")}
}
#[cfg(target_os="linux")]
pub(in crate::execution) fn restore_shared(view:&TransactionView,signals:&SignalState)->Result<()> {
    use rustix::fs::{RenameFlags,renameat_with};
    let b=&view.body;let mut journal=diag(Journal::open_replacement(&b.state))?;
    let records=diag(journal.replacement_events(&view.fingerprint))?;
    let mut record=records.last().cloned().context("replacement journal absent")?;
    ensure!(record.phase=="committed"&&record.authorization_id==view.authorization_id
        &&record.video_action==view.video_action,"only exact committed action/approval can restore");
    let retained=record.retained.clone().context("retained original binding absent")?;
    let published=record.published.clone().context("published binding absent")?;
    moved_matches(&retained,&b.source)?;
    moved_matches(&published,record.staged.as_ref().context("staged binding absent")?)?;
    let mut original=diag(fs::open_bound(&retained))?;let mut replacement=diag(fs::open_bound(&published))?;
    same_binding(&retained,&diag(fs::snapshot(&diag(fs::native_path(&retained.path))?,signals,b.limits.maximum_file_bytes))?)?;
    same_binding(&published,&diag(fs::snapshot(&diag(fs::native_path(&published.path))?,signals,b.limits.maximum_file_bytes))?)?;
    let parent=diag(fs::check_directory(&b.source.directory))?;
    let root=diag(fs::native_path(&b.retention.path))?.join(&view.fingerprint);
    let namespace=artifact::open_directory(&root)?;artifact::require_private_root(&namespace)?;
    let namespace_binding=diag(fs::directory(&root))?;
    ensure!(retained.path==NativePath::from_path(&root.join("original"))&&retained.directory==namespace_binding
        &&published.path==b.source.path&&published.directory==b.source.directory,
        "recorded restore subjects are outside the exact source/retention namespace");
    let original_properties=diag(super::super::mutation::properties(&original))?;
    let replacement_properties=diag(super::super::mutation::properties(&replacement))?;
    preserved_properties(&original_properties,&replacement_properties)?;
    let source_path=diag(fs::native_path(&b.source.path))?;let leaf=source_path.file_name().context("source leaf missing")?;
    record.sequence+=1;record.phase="restore_pending".into();append(&mut journal,view,&record)?;
    check_transition_subjects(b,&parent,&namespace,&namespace_binding,
        (&mut original,&retained,&original_properties),(&mut replacement,&published,&replacement_properties),signals)?;
    renameat_with(&parent,leaf,&namespace,"optimized-after-restore",RenameFlags::NOREPLACE)?;
    diag(fs::sync_directory(&parent))?;diag(fs::sync_directory(&namespace))?;
    let optimized=diag(fs::snapshot(&root.join("optimized-after-restore"),signals,b.limits.maximum_file_bytes))?;
    moved_matches(&optimized,&published)?;
    check_transition_subjects(b,&parent,&namespace,&namespace_binding,
        (&mut original,&retained,&original_properties),(&mut replacement,&optimized,&replacement_properties),signals)?;
    renameat_with(&namespace,"original",&parent,leaf,RenameFlags::NOREPLACE)?;
    diag(fs::sync_directory(&parent))?;diag(fs::sync_directory(&namespace))?;
    let restored=diag(fs::snapshot(&source_path,signals,b.limits.maximum_file_bytes))?;
    moved_matches(&restored,&retained)?;
    check_transition_subjects(b,&parent,&namespace,&namespace_binding,
        (&mut original,&restored,&original_properties),(&mut replacement,&optimized,&replacement_properties),signals)?;
    record.sequence+=1;record.phase="restored".into();append(&mut journal,view,&record)?;Ok(())
}

#[cfg(all(test,target_os="linux"))]
#[path="integrity_tests.rs"]
mod integrity_tests;
