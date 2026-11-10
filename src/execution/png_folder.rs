//! PNG-first bounded folder planning. Discovery starts no provider and mutates
//! no collection paths. Discard authority is explicit and independent of plans.
use std::{path::{Path,PathBuf},time::{Instant,Duration}};
use anyhow::{Result,ensure,Context};
use serde::{Serialize,Deserialize};
use super::{filesystem as fs,model::{FileBinding,DirectoryBinding}};
use super::replacement::{Limits,diag,hash,pin};
use crate::{domain::NativePath,signals::SignalState};
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum OriginalDisposition {RetainForRestore,DiscardAfterDurableReplacement}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderBounds {
    pub maximum_entries:u64,pub maximum_files:u64,pub maximum_actions:u64,
    pub maximum_depth:u64,pub maximum_total_input_bytes:u64,pub elapsed_ms:u64,
}
impl FolderBounds {pub fn validate(&self)->Result<()>{ensure!((1..=4096).contains(&self.maximum_entries)&&(1..=256).contains(&self.maximum_files)&&self.maximum_files<=self.maximum_entries
    &&(1..=32).contains(&self.maximum_actions)&&self.maximum_actions<=self.maximum_files&&(1..=32).contains(&self.maximum_depth)
    &&(1..=268_435_456).contains(&self.maximum_total_input_bytes)&&(1..=600_000).contains(&self.elapsed_ms),"PNG folder bounds exceed selected serial profile");Ok(())}}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema:String,pub source_root:PathBuf,pub oxipng:PathBuf,
    pub state_directory:PathBuf,pub retention_directory:PathBuf,
    pub artifact_root:PathBuf,pub run_directory:PathBuf,
    pub policy_fingerprint:String,pub original_disposition:OriginalDisposition,
    pub recursive:bool,pub include_hidden:bool,pub per_file:Limits,pub batch:FolderBounds,
}
impl Request {pub fn validate(&self)->Result<()>{
    ensure!(self.schema=="optiflow.png-folder-request.v1"&&pin(&self.policy_fingerprint),"explicit PNG folder schema/policy required");self.per_file.validate()?;self.batch.validate()?;
    ensure!(serde_json::to_vec(self)?.len()<=131_072,"PNG folder request exceeds 128 KiB");
    let paths=[&self.source_root,&self.state_directory,&self.retention_directory,&self.artifact_root,&self.run_directory];
    let mut directories=Vec::new();for path in paths {directories.push(diag(fs::directory(path))?);}
    for (i,path) in paths.iter().enumerate(){for (j,other) in directories.iter().enumerate(){if i!=j {diag(fs::outside_directories(path,&[other]))?;}}}
    for path in &paths[1..] {crate::candidate_artifact::validate_private_root(path)?;}
    ensure!(directories[0].identity.filesystem_id==directories[2].identity.filesystem_id,"original retention must share source filesystem");
    diag(fs::native_path(&NativePath::from_path(&self.oxipng)))?;Ok(())
}}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {pub path:NativePath,pub disposition:String,pub reason:String,pub source:Option<FileBinding>}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub schema:String,pub request_fingerprint:String,pub fingerprint:String,
    pub source_root:DirectoryBinding,pub entries_seen:u64,pub files_seen:u64,
    pub observed_input_bytes:u64,pub entries:Vec<Entry>,
    pub one_file_in_flight:bool,pub physical_space_reclaimed:bool,
}
impl Inventory {pub fn validate(&self,request:&Request)->Result<()>{
    request.per_file.validate()?;request.batch.validate()?;ensure!(request.schema=="optiflow.png-folder-request.v1"&&pin(&request.policy_fingerprint),"invalid folder request identity");
    let mut copy=self.clone();copy.fingerprint.clear();ensure!(self.schema=="optiflow.png-folder-inventory.v1"&&self.request_fingerprint==hash(request)?&&self.fingerprint==hash(&copy)?
        &&self.entries_seen<=request.batch.maximum_entries&&self.files_seen<=request.batch.maximum_files&&self.observed_input_bytes<=request.batch.maximum_total_input_bytes
        &&self.entries.len() as u64<=request.batch.maximum_entries&&self.one_file_in_flight&&!self.physical_space_reclaimed,"PNG inventory identity or bounds differ");
    let mut previous=None::<PathBuf>;for e in &self.entries {let path=diag(fs::native_path(&e.path))?;ensure!(path.starts_with(&request.source_root)&&previous.as_ref().is_none_or(|p|p<&path),"inventory entries must be unique sorted confined paths");previous=Some(path);ensure!(matches!(e.disposition.as_str(),"eligible"|"skipped"|"unsupported"),"unknown PNG entry disposition");if e.disposition=="eligible"{ensure!(e.source.as_ref().is_some_and(|s|s.path==e.path&&s.size_bytes>0&&s.size_bytes<=request.per_file.maximum_file_bytes&&s.identity.link_count==Some(1)),"eligible PNG entry lacks exact source binding");}}
    Ok(())
}}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchApproval {
    pub schema:String,pub authorization_id:String,pub request_fingerprint:String,
    pub inventory_fingerprint:String,pub disposition:OriginalDisposition,
    pub approved_by:String,pub approved_at:String,pub maximum_actions:u64,
}
impl BatchApproval {pub fn validate(&self,request:&Request,inventory:&Inventory)->Result<()>{
    inventory.validate(request)?;let mut copy=self.clone();copy.authorization_id.clear();
    ensure!(self.schema=="optiflow.png-folder-approval.v1"&&self.authorization_id==hash(&copy)?&&self.request_fingerprint==hash(request)?&&self.inventory_fingerprint==inventory.fingerprint&&self.disposition==request.original_disposition
        &&self.maximum_actions==request.batch.maximum_actions&&!self.approved_by.trim().is_empty()&&self.approved_by.len()<=128&&!self.approved_by.chars().any(char::is_control)&&chrono::DateTime::parse_from_rfc3339(&self.approved_at).is_ok(),"explicit batch authority differs");Ok(())
}}
pub fn approve(request:&Request,inventory:&Inventory,approved_by:String)->Result<BatchApproval>{
    let mut a=BatchApproval {schema:"optiflow.png-folder-approval.v1".into(),authorization_id:String::new(),request_fingerprint:hash(request)?,inventory_fingerprint:inventory.fingerprint.clone(),disposition:request.original_disposition,approved_by,approved_at:chrono::Utc::now().to_rfc3339(),maximum_actions:request.batch.maximum_actions};a.authorization_id=hash(&a)?;a.validate(request,inventory)?;Ok(a)
}
/// Bounded enumeration is completed before any provider can be started. All
/// reads use no-follow bindings; same-device mount equivalence is not attested.
pub fn discover(request:&Request,signals:&SignalState)->Result<Inventory>{
    request.validate()?;let started=Instant::now();
    let guard=||{ensure!(!signals.is_cancelled()&&started.elapsed()<Duration::from_millis(request.batch.elapsed_ms),"PNG folder discovery cancelled or timed out");Ok(())};
    #[cfg(unix)] {
        use std::os::unix::{fs::MetadataExt,ffi::OsStrExt};
        let root=diag(fs::directory(&request.source_root))?;let mut stack=vec![(request.source_root.clone(),0u64)];
        let mut entries=Vec::new();let mut seen=0u64;let mut files=0u64;let mut observed=0u64;
        while let Some((path,depth))=stack.pop(){guard()?;let held=crate::candidate_artifact::open_directory(&path)?;
            let before=crate::filesystem::identity::FileStateSignature::from_file_metadata(&held.metadata()?);
            for item in std::fs::read_dir(&path)? {guard()?;seen+=1;ensure!(seen<=request.batch.maximum_entries,"entry bound exceeded; no provider started");let item=item?;let leaf=item.file_name();let path=item.path();let m=std::fs::symlink_metadata(&path)?;
                let mut entry=Entry {path:NativePath::from_path(&path),disposition:"skipped".into(),reason:String::new(),source:None};
                if !request.include_hidden&&leaf.as_bytes().first()==Some(&b'.'){entry.reason="hidden_entry_excluded".into();}
                else if m.file_type().is_symlink(){entry.reason="symlink_not_followed".into();}
                else if m.dev().to_string()!=root.identity.filesystem_id{entry.reason="filesystem_boundary_skipped".into();}
                else if m.is_dir(){if request.recursive&&depth+1<request.batch.maximum_depth{stack.push((path,depth+1));continue;}else{entry.reason="recursion_or_depth_boundary".into();}}
                else if !m.is_file(){entry.reason="special_file_not_opened".into();}
                else {files+=1;ensure!(files<=request.batch.maximum_files,"file bound exceeded; no provider started");
                    if m.nlink()!=1||m.len()==0||m.len()>request.per_file.maximum_file_bytes{entry.reason="link_empty_or_file_bound".into();}
                    else if observed.checked_add(m.len()).is_none_or(|n|n>request.batch.maximum_total_input_bytes){entry.reason="aggregate_input_bound".into();}
                    else {let observation=crate::video_io::BoundedFileObservation::open_guarded(&path,request.per_file.maximum_file_bytes,&guard)?;let data=observation.read_bytes_guarded(&guard)?;observed=observed.checked_add(data.len() as u64).context("input accounting overflow")?;
                        let valid=crate::png_validation::validate_png_source(&data,crate::png_validation::ByteValidationLimits {source_bytes:request.per_file.maximum_file_bytes as usize,candidate_bytes:request.per_file.maximum_file_bytes as usize,decoded_bytes_per_image:67_108_864,chunks_per_image:256,decoder_allocation_bytes:67_108_864});
                        observation.verify_current_guarded(&guard)?;
                        if valid.is_ok(){entry.source=Some(diag(fs::snapshot(&path,signals,request.per_file.maximum_file_bytes))?);ensure!(entry.source.as_ref().is_some_and(|s|s.blake3==observation.digest().value),"PNG source changed after classification");entry.disposition="eligible".into();entry.reason="independent_supported_PNG_observed".into();}
                        else {entry.disposition="unsupported".into();entry.reason="outside_supported_static_PNG_profile".into();}
                    }
                }
                entries.push(entry);ensure!(serde_json::to_vec(&entries)?.len()<=4_194_304,"inventory exceeds 4 MiB");
            }
            guard()?;let now=crate::candidate_artifact::open_directory(&path)?;ensure!(crate::filesystem::identity::FileStateSignature::from_file_metadata(&now.metadata()?)==before&&crate::filesystem::identity::FileStateSignature::from_file_metadata(&held.metadata()?)==before,"directory changed during bounded discovery");
        }
        entries.sort_by_key(|e|e.path.to_path_buf());
        let mut inventory=Inventory {schema:"optiflow.png-folder-inventory.v1".into(),request_fingerprint:hash(request)?,fingerprint:String::new(),source_root:root,entries_seen:seen,files_seen:files,observed_input_bytes:observed,entries,one_file_in_flight:true,physical_space_reclaimed:false};inventory.fingerprint=hash(&inventory)?;inventory.validate(request)?;guard()?;Ok(inventory)
    }
    #[cfg(not(unix))] {let _=guard;anyhow::bail!("PNG folder discovery requires no-follow Unix filesystem bindings")}
}

#[cfg(test)] mod tests;
