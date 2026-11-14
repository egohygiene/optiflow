//! Explicit private CLI event sink. A partial/missing log is never work evidence.
use std::{cell::RefCell,fs::File,io::Write,path::{Path,PathBuf}};
use anyhow::{Context,Result,ensure};
use crate::{cli::Mp4RequestArgs,mp4_delivery::{self as mp4,progress::{Event,Observer,Statistics}}};

struct EventFile {path:PathBuf,parent:File,file:RefCell<File>,written:std::cell::Cell<u64>,maximum:u64}
impl EventFile {
    fn create(path:&Path,maximum:u64,forbidden:&[&Path])->Result<Self> {
        mp4::normalized(path)?;ensure!(path.to_str().is_some_and(|p|p.len()<=4096&&!p.chars().any(char::is_control)),"bounded UTF8 progress path required");
        ensure!(forbidden.iter().all(|p|!path.starts_with(p)&&!p.starts_with(path)),"progress log must be outside source/tool/work/store/journal namespaces");
        crate::record_io::preflight_new(path)?;
        #[cfg(unix)]{
            use rustix::fs::{openat,Mode,OFlags};
            let parent=crate::candidate_artifact::open_directory(path.parent().context("progress parent absent")?)?;crate::candidate_artifact::require_private_root(&parent)?;
            let file=File::from(openat(&parent,path.file_name().context("progress leaf absent")?,OFlags::WRONLY|OFlags::CREATE|OFlags::EXCL|OFlags::NOFOLLOW|OFlags::CLOEXEC,Mode::RUSR|Mode::WUSR)?);parent.sync_all()?;
            Ok(Self {path:path.into(),parent,file:RefCell::new(file),written:std::cell::Cell::new(0),maximum})
        }
        #[cfg(not(unix))]{let _=(maximum,forbidden);anyhow::bail!("private progress log unavailable on this host")}
    }
    fn record(&self,event:&Event)->Result<()> {
        let mut bytes=serde_json::to_vec(event)?;bytes.push(b'\n');let total=self.written.get().checked_add(bytes.len() as u64).context("progress byte overflow")?;ensure!(total<=self.maximum,"progress file budget exhausted");
        let mut file=self.file.try_borrow_mut().context("progress sink reentry refused")?;
        #[cfg(unix)]{
            use rustix::fs::{fstat,openat,Mode,OFlags};
            let parent=crate::candidate_artifact::open_directory(self.path.parent().context("progress parent absent")?)?;let a=fstat(&parent)?;let b=fstat(&self.parent)?;ensure!(a.st_dev==b.st_dev&&a.st_ino==b.st_ino,"progress parent changed");
            let current=File::from(openat(&self.parent,self.path.file_name().context("progress leaf absent")?,OFlags::RDONLY|OFlags::NONBLOCK|OFlags::NOFOLLOW|OFlags::CLOEXEC,Mode::empty())?);ensure!(current.metadata()?.is_file(),"progress path is not a regular file");let a=fstat(&current)?;let b=fstat(&*file)?;
            ensure!(a.st_dev==b.st_dev&&a.st_ino==b.st_ino&&a.st_nlink==1&&a.st_size>=0&&a.st_size as u64==self.written.get(),"progress file identity/length changed");
        }
        file.write_all(&bytes)?;self.written.set(total);Ok(())
    }
    fn finish(&self)->Result<()> {self.file.try_borrow().context("progress sink busy")?.sync_data()?;Ok(())}
}
pub(super) fn execute<T>(arguments:&Mp4RequestArgs,forbidden:&[&Path],operation:impl FnOnce(&Observer<'_>)->Result<T>)->Result<(Result<T>,Statistics)> {
    let events=arguments.maximum_events.unwrap_or(mp4::progress::MAX_EVENTS);let bytes=arguments.maximum_event_bytes.unwrap_or(8_388_608);Observer::validate_limits(events,bytes)?;
    let file=arguments.event_log.as_ref().map(|path|EventFile::create(path,bytes,forbidden)).transpose()?;
    let callback=|event:&Event|match &file {Some(file)=>file.record(event),None=>Ok(())};
    let observer=if file.is_some(){Observer::new(&callback,events,bytes)?}else{Observer::silent()};
    let result=operation(&observer);if file.as_ref().is_some_and(|f|f.finish().is_err()){observer.sink_failed();}
    Ok((result,observer.statistics()))
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn explicit_log_refuses_source_overlap_and_existing_files(){let temporary=tempfile::tempdir().unwrap();let root=temporary.path().canonicalize().unwrap();let path=root.join("events 曲.jsonl");assert!(EventFile::create(&path,4096,&[root.as_path()]).is_err());assert_eq!(std::fs::read_dir(&root).unwrap().count(),0);std::fs::write(&path,b"retain exact prior bytes").unwrap();assert!(EventFile::create(&path,4096,&[]).is_err());assert_eq!(std::fs::read(&path).unwrap(),b"retain exact prior bytes");}
}
