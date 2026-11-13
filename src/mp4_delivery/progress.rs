//! Caller-owned operational observation. Event loss/sink errors never establish
//! or rewrite authoritative work. No background threads, globals or exporters.
use super::*;
use std::{cell::Cell,time::Instant};
pub const MAX_EVENTS:u64=16_384;
pub const MAX_BYTES:u64=16_777_216;
#[derive(Debug,Clone,Copy,Serialize)]
#[serde(rename_all="snake_case")]
pub enum Phase {Operation,Planning,RuntimePreparation,SourceDecode,Encoding,CandidateValidation,ArtifactCommit,FolderDiscovery,FolderReconciliation,Entry,FolderReport}
#[derive(Debug,Clone,Copy,Serialize)]
#[serde(rename_all="snake_case")]
pub enum Transition {Started,Finished,Stopped,Failed}
#[derive(Debug,Serialize)]
pub struct Event {
    pub schema:&'static str,pub profile:&'static str,pub operation_id:String,
    pub sequence:u64,pub phase:Phase,pub transition:Transition,pub entry_index:Option<usize>,
    pub completed_entries:Option<u64>,pub total_entries:Option<u64>,
    pub elapsed_ms:u64,pub authoritative:bool,
}
#[derive(Debug,Clone,Copy,Serialize)]
pub struct Statistics {pub delivered:u64,pub dropped:u64,pub sink_failures:u64,pub admitted_bytes:u64,pub authoritative:bool}
impl Statistics {pub fn degraded(&self)->bool {self.dropped>0||self.sink_failures>0}}
pub struct Observer<'a> {
    callback:Option<&'a dyn Fn(&Event)->Result<()>>,id:String,started:Instant,
    maximum_events:u64,maximum_bytes:u64,attempted:Cell<u64>,admitted:Cell<u64>,
    delivered:Cell<u64>,dropped:Cell<u64>,sink_failures:Cell<u64>,
}
impl<'a> Observer<'a> {
    pub fn validate_limits(events:u64,bytes:u64)->Result<()> {ensure!((1..=MAX_EVENTS).contains(&events)&&(1..=MAX_BYTES).contains(&bytes),"bounded explicit MP4 observer event/byte limits required");Ok(())}
    pub fn new(callback:&'a dyn Fn(&Event)->Result<()>,events:u64,bytes:u64)->Result<Self> {
        Self::validate_limits(events,bytes)?;Ok(Self {callback:Some(callback),id:uuid::Uuid::now_v7().to_string(),maximum_events:events,maximum_bytes:bytes,..Self::silent()})
    }
    pub fn silent()->Self {Self {callback:None,id:String::new(),started:Instant::now(),maximum_events:0,maximum_bytes:0,attempted:Cell::new(0),admitted:Cell::new(0),delivered:Cell::new(0),dropped:Cell::new(0),sink_failures:Cell::new(0)}}
    pub fn statistics(&self)->Statistics {Statistics {delivered:self.delivered.get(),dropped:self.dropped.get(),sink_failures:self.sink_failures.get(),admitted_bytes:self.admitted.get(),authoritative:false}}
    pub(crate) fn sink_failed(&self){self.sink_failures.set(self.sink_failures.get().saturating_add(1));}
    pub(crate) fn phase(&self,phase:Phase,transition:Transition,entry:Option<usize>){self.emit(phase,transition,entry,None,None);}
    pub(crate) fn entries(&self,transition:Transition,index:usize,completed:u64,total:u64){self.emit(Phase::Entry,transition,Some(index),Some(completed),Some(total));}
    fn emit(&self,phase:Phase,transition:Transition,entry_index:Option<usize>,completed_entries:Option<u64>,total_entries:Option<u64>){
        let Some(callback)=self.callback else{return;};let sequence=self.attempted.get();self.attempted.set(sequence.saturating_add(1));
        if sequence>=self.maximum_events {self.dropped.set(self.dropped.get().saturating_add(1));return;}
        let event=Event {schema:"optiflow.mp4-progress.v1",profile:PROFILE,operation_id:self.id.clone(),sequence,phase,transition,entry_index,completed_entries,total_entries,elapsed_ms:self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,authoritative:false};
        let size=match serde_json::to_vec(&event){Ok(bytes)=>bytes.len() as u64+1,Err(_)=>{self.dropped.set(self.dropped.get().saturating_add(1));return;}};
        let Some(total)=self.admitted.get().checked_add(size).filter(|n|*n<=self.maximum_bytes) else{self.dropped.set(self.dropped.get().saturating_add(1));return;};self.admitted.set(total);
        if callback(&event).is_ok(){self.delivered.set(self.delivered.get().saturating_add(1));}else{self.sink_failed();}
    }
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn sink_failure_and_event_budget_do_not_become_work_or_replay_authority(){let captured=std::cell::RefCell::new(Vec::new());let callback=|event:&Event|->Result<()>{captured.borrow_mut().push(serde_json::to_value(event)?);anyhow::bail!("synthetic sink failure")};let observer=Observer::new(&callback,1,4096).unwrap();observer.phase(Phase::Encoding,Transition::Started,None);observer.phase(Phase::Encoding,Transition::Finished,None);let stats=observer.statistics();assert_eq!(stats.sink_failures,1);assert_eq!(stats.dropped,1);assert_eq!(stats.delivered,0);let values=captured.borrow();let value=&values[0];assert_eq!(value["authoritative"],false);assert!(value["completed_entries"].is_null()&&value["total_entries"].is_null());assert!(value.get("source").is_none()&&value.get("argv").is_none()&&value.get("content").is_none());}
}
