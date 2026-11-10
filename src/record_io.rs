//! Small immutable JSON records; never source media or overwrite operations.
use std::path::Path;
use anyhow::{Result, Context, ensure};
use serde::Serialize;
use crate::signals::SignalState;
pub(crate) fn read(path:&Path,maximum:u64,signals:&SignalState)->Result<Vec<u8>>{
    ensure!((1..=16_777_216).contains(&maximum),"invalid record read bound");
    let guard=||{ensure!(!signals.is_cancelled(),"record read cancelled");Ok(())};
    let record=crate::video_io::BoundedFileObservation::open_guarded(path,maximum,&guard)?;
    let bytes=record.read_bytes_guarded(&guard)?;record.verify_current_guarded(&guard)?;Ok(bytes)
}
pub(crate) fn preflight_new(path:&Path)->Result<()> {
    ensure!(path.is_absolute(),"absolute record output required");
    let parent=crate::candidate_artifact::open_directory(path.parent().context("record parent absent")?)?;
    crate::candidate_artifact::require_private_root(&parent)?;
    ensure!(path.file_name().and_then(|s|s.to_str()).is_some_and(|s|!s.is_empty()&&s!="."&&s!=".."),"UTF-8 record leaf required");
    ensure!(std::fs::symlink_metadata(path).is_err_and(|e|e.kind()==std::io::ErrorKind::NotFound),"record output already exists or cannot be inspected");Ok(())
}
pub(crate) fn write_new<T:Serialize>(path:&Path,record:&T,maximum:usize)->Result<()> {
    preflight_new(path)?;
    ensure!(path.is_absolute()&&(1..=16_777_216).contains(&maximum),"absolute bounded record output required");
    let parent=crate::candidate_artifact::open_directory(path.parent().context("record parent absent")?)?;
    crate::candidate_artifact::require_private_root(&parent)?;
    let name=path.file_name().and_then(|v|v.to_str()).context("UTF-8 record leaf required")?;
    ensure!(!name.is_empty()&&name!="."&&name!="..","record leaf invalid");
    let mut data=serde_json::to_vec_pretty(record)?;data.push(b'\n');
    ensure!(data.len()<=maximum,"record output exceeds bound");
    crate::candidate_artifact::write_new(&parent,name,&data)?;parent.sync_all()?;Ok(())
}
