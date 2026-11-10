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
    pub staged: Option<FileBinding>,
    pub retained: Option<FileBinding>,
    pub published: Option<FileBinding>,
}
impl ReplacementEvent {
    pub(in crate::execution) fn from_json_slice(bytes:&[u8])->Result<Self>{
        ensure!(bytes.len()<=1_048_576,"replacement event exceeds 1 MiB");
        let raw:serde_json::Value=serde_json::from_slice(bytes)?;
        crate::contracts::validate(crate::contracts::Contract::ReplacementEvent,&raw)?;
        Ok(serde_json::from_slice(bytes)?)
    }
    pub fn validate_after(&self, prior: &[Self]) -> Result<()> {
        ensure!(prior.len()<32 && self.schema=="optiflow.replacement-event.v1"
            && pin(&self.plan_fingerprint)&&pin(&self.authorization_id)
            && self.sequence==prior.len() as u64,"replacement event identity/sequence differs");
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
    anyhow::bail!("PNG replacement native qualification is pending (#93/#95); no source mutation or writable journal access")
}
pub fn apply(plan:&ReplacementPlan,approval:&Approval,signals:&SignalState)->Result<ReplacementEvent>{
    approval.validate(plan)?;require_qualification()?;
    #[cfg(target_os="linux")] { apply_linux_source(plan,approval,signals) }
    #[cfg(not(target_os="linux"))] {let _=signals;anyhow::bail!("replacement property backend is not qualified on this platform")}
}
fn event(plan:&ReplacementPlan,approval:&Approval,sequence:u64,phase:&str,staged:Option<FileBinding>,retained:Option<FileBinding>,published:Option<FileBinding>)->ReplacementEvent {
    ReplacementEvent {schema:"optiflow.replacement-event.v1".into(),plan_fingerprint:plan.fingerprint.clone(),authorization_id:approval.authorization_id.clone(),sequence,phase:phase.into(),original_disposal_authorization:None,staged,retained,published}
}
fn append(journal:&mut Journal,plan:&ReplacementPlan,record:&ReplacementEvent)->Result<()> {diag(journal.append_replacement(record,plan.body.limits.journal_budget_bytes))}
fn moved_matches(actual:&FileBinding,expected:&FileBinding)->Result<()> {
    ensure!(actual.identity==expected.identity&&actual.size_bytes==expected.size_bytes
        && actual.modified_unix_ns==expected.modified_unix_ns&&actual.mode==expected.mode
        && actual.uid==expected.uid&&actual.gid==expected.gid&&actual.blake3==expected.blake3,
        "renamed subject differs; retain evidence and stop");Ok(())
}
#[cfg(target_os="linux")]
fn apply_linux_source(plan:&ReplacementPlan,approval:&Approval,signals:&SignalState)->Result<ReplacementEvent>{
    use std::io::Write;
    use rustix::fs::{Mode,OFlags,RenameFlags,mkdirat,openat,renameat_with};
    recheck_png(plan,approval,signals)?;
    let b=&plan.body;let mut journal=diag(Journal::open_replacement(&b.state))?;
    ensure!(diag(journal.replacement_events(&plan.fingerprint))?.is_empty(),"existing replacement run requires read-only inspection; no replay");
    recheck_png(plan,approval,signals)?;
    let retained_root=diag(fs::check_directory(&b.retention))?;diag(fs::writable_directory(&retained_root))?;
    let parent=diag(fs::check_directory(&b.source.directory))?;diag(fs::writable_directory(&parent))?;
    let capacity=diag(fs::capacity(&retained_root))?;
    let required=b.candidate.size_bytes.checked_add(b.limits.journal_budget_bytes).and_then(|v|v.checked_add(b.limits.free_space_reserve_bytes)).context("replacement reserve overflow")?;
    ensure!(!capacity.read_only&&capacity.available>=required,"replacement transient storage plus reserve unavailable");
    let planned=event(plan,approval,0,"planned",None,None,None);append(&mut journal,plan,&planned)?;
    mkdirat(&retained_root,plan.fingerprint.as_str(),Mode::RUSR|Mode::WUSR|Mode::XUSR)?;
    diag(fs::sync_directory(&retained_root))?;
    let namespace_path=diag(fs::native_path(&b.retention.path))?.join(&plan.fingerprint);
    let namespace=artifact::open_directory(&namespace_path)?;
    artifact::require_private_root(&namespace)?;
    let source=diag(fs::open_bound(&b.source))?;
    let properties=diag(super::super::mutation::properties(&source))?;
    let candidate_bytes=bytes(&diag(fs::native_path(&b.candidate.path))?,b.limits.maximum_file_bytes)?;
    ensure!(blake3::hash(&candidate_bytes).to_hex().as_str()==b.candidate.blake3&&!signals.is_cancelled(),"candidate changed or replacement cancelled");
    let mut staged=File::from(openat(&namespace,"replacement",OFlags::RDWR|OFlags::CREATE|OFlags::EXCL|OFlags::NOFOLLOW|OFlags::CLOEXEC,Mode::RUSR|Mode::WUSR)?);
    staged.write_all(&candidate_bytes)?;
    diag(super::super::mutation::set_properties(&staged,&properties))?;
    let after=diag(super::super::mutation::properties(&staged))?;
    ensure!(properties.uid==after.uid&&properties.gid==after.gid&&properties.mode==after.mode&&properties.mtime==after.mtime&&properties.xattrs==after.xattrs,"replacement properties did not preserve exactly");
    diag(fs::sync_file(&staged))?;diag(fs::sync_directory(&namespace))?;
    let staged_binding=diag(fs::snapshot(&namespace_path.join("replacement"),signals,b.limits.maximum_file_bytes))?;
    ensure!(staged_binding.blake3==b.candidate.blake3&&staged_binding.size_bytes==b.candidate.size_bytes,"staged replacement differs");
    let mut record=event(plan,approval,1,"staged",Some(staged_binding.clone()),None,None);append(&mut journal,plan,&record)?;
    recheck_png(plan,approval,signals)?;
    record.sequence+=1;record.phase="retain_pending".into();append(&mut journal,plan,&record)?;
    let original_path=diag(fs::native_path(&b.source.path))?;let name=original_path.file_name().context("original leaf absent")?;
    diag(fs::check_file(&source,&b.source))?;
    renameat_with(&parent,name,&namespace,"original",RenameFlags::NOREPLACE)?;
    diag(fs::sync_directory(&parent))?;diag(fs::sync_directory(&namespace))?;
    let original=diag(fs::snapshot(&namespace_path.join("original"),signals,b.limits.maximum_file_bytes))?;
    moved_matches(&original,&b.source)?;
    record.sequence+=1;record.phase="retained".into();record.retained=Some(original);append(&mut journal,plan,&record)?;
    ensure!(!signals.is_cancelled(),"cancelled after retention; inspect journal, never replay");
    record.sequence+=1;record.phase="publish_pending".into();append(&mut journal,plan,&record)?;
    renameat_with(&namespace,"replacement",&parent,name,RenameFlags::NOREPLACE)?;
    diag(fs::sync_directory(&parent))?;diag(fs::sync_directory(&namespace))?;
    let published=diag(fs::snapshot(&original_path,signals,b.limits.maximum_file_bytes))?;
    moved_matches(&published,&staged_binding)?;
    record.sequence+=1;record.phase="committed".into();record.published=Some(published);append(&mut journal,plan,&record)?;
    Ok(record)
}

/// Read only status uses SQLite read-only access and never classifies/replays an
/// uncertain transition. Source bindings must be reobserved before any recovery.
pub fn status(plan:&ReplacementPlan)->Result<Vec<ReplacementEvent>>{
    plan.validate()?;let state=diag(fs::check_directory(&plan.body.state))?;artifact::require_private_root(&state)?;
    let path=diag(fs::native_path(&plan.body.state.path))?.join("state.sqlite3");
    let _database=diag(fs::open(&path,false))?;
    let connection=rusqlite::Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let (count,total):(u64,u64)=connection.query_row("SELECT COUNT(*),COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM execution_replacement_events WHERE plan_fingerprint=?1",[&plan.fingerprint],|r|Ok((r.get(0)?,r.get(1)?)))?;
    ensure!(count<=32&&total<=plan.body.limits.journal_budget_bytes,"replacement journal exceeds count/byte bounds");
    let records:Vec<String>=connection.prepare("SELECT document_json FROM execution_replacement_events WHERE plan_fingerprint=?1 ORDER BY sequence")?.query_map([&plan.fingerprint],|r|r.get(0))?.collect::<std::result::Result<_,_>>()?;
    let mut out=Vec::new();let mut total=0u64;
    for text in records {total=total.checked_add(text.len() as u64).context("journal size overflow")?;ensure!(total<=plan.body.limits.journal_budget_bytes,"replacement journal exceeds bound");let e=ReplacementEvent::from_json_slice(text.as_bytes())?;e.validate_after(&out)?;ensure!(e.plan_fingerprint==plan.fingerprint,"another plan in journal");out.push(e);}
    Ok(out)
}
/// Restore source is deliberately no-replace and retains the optimized object.
/// It has the same native qualification gate as apply, before writable state.
pub fn restore(plan:&ReplacementPlan,approval:&Approval,signals:&SignalState)->Result<()> {
    approval.validate(plan)?;require_qualification()?;
    #[cfg(target_os="linux")] {
        use rustix::fs::{RenameFlags,renameat_with};
        let mut journal=diag(Journal::open_replacement(&plan.body.state))?;
        let records=diag(journal.replacement_events(&plan.fingerprint))?;
        let mut record=records.last().cloned().context("replacement journal absent")?;
        ensure!(record.phase=="committed"&&record.authorization_id==approval.authorization_id,"only exact committed approval can restore");
        let retained=record.retained.as_ref().context("retained original binding absent")?;
        let published=record.published.as_ref().context("published binding absent")?;
        let _original=diag(fs::open_bound(retained))?;let _replacement=diag(fs::open_bound(published))?;
        same_binding(retained,&diag(fs::snapshot(&diag(fs::native_path(&retained.path))?,signals,plan.body.limits.maximum_file_bytes))?)?;
        same_binding(published,&diag(fs::snapshot(&diag(fs::native_path(&published.path))?,signals,plan.body.limits.maximum_file_bytes))?)?;
        let parent=diag(fs::check_directory(&plan.body.source.directory))?;
        let root=diag(fs::native_path(&plan.body.retention.path))?.join(&plan.fingerprint);
        let namespace=artifact::open_directory(&root)?;artifact::require_private_root(&namespace)?;
        let source_path=diag(fs::native_path(&plan.body.source.path))?;let leaf=source_path.file_name().context("source leaf missing")?;
        record.sequence+=1;record.phase="restore_pending".into();append(&mut journal,plan,&record)?;
        ensure!(!signals.is_cancelled(),"restore cancelled; no replay");
        renameat_with(&parent,leaf,&namespace,"optimized-after-restore",RenameFlags::NOREPLACE)?;
        diag(fs::sync_directory(&parent))?;diag(fs::sync_directory(&namespace))?;
        renameat_with(&namespace,"original",&parent,leaf,RenameFlags::NOREPLACE)?;
        diag(fs::sync_directory(&parent))?;diag(fs::sync_directory(&namespace))?;
        moved_matches(&diag(fs::snapshot(&source_path,signals,plan.body.limits.maximum_file_bytes))?,retained)?;
        record.sequence+=1;record.phase="restored".into();append(&mut journal,plan,&record)?;Ok(())
    }
    #[cfg(not(target_os="linux"))] {let _=signals;anyhow::bail!("replacement restore property backend unavailable")}
}
