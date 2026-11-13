//! Existing bounded direct supervision and Linux guard, specialized closed jobs.
use super::*;
use std::{fs::File,time::{Duration,Instant}};
use anyhow::Context;
use crate::subprocess::{SubprocessLimits,SubprocessRunner};
use crate::video_sandbox::{SandboxSession,SandboxReceipt};
use super::worker::{self,Job,Operation};

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invocation {pub job_digest:EvidenceDigest,pub operation:String,pub job:Option<serde_json::Value>,pub version_text:Option<String>,pub report:Option<serde_json::Value>,pub resource:SandboxReceipt,pub elapsed_ms:u64,pub output_digest:EvidenceDigest,pub output_bytes:u64}
pub(crate) struct Session<'a,F:Fn()->bool>{pub plan:Plan,request:&'a Request,cancel:&'a F,started:Instant,source:BoundedFileObservation,tools:Vec<BoundedFileObservation>,sandbox:SandboxSession,pub invocations:Vec<Invocation>}
impl<'a,F:Fn()->bool> Session<'a,F> {
    pub fn new(request:&'a Request,cancel:&'a F)->Result<Self>{
        Self::new_at(request,cancel,Instant::now())
    }
    pub fn new_at(request:&'a Request,cancel:&'a F,started:Instant)->Result<Self>{
        ensure!(cfg!(target_os="linux"),"MP4 production requires the selected observed Linux guard; other hosts remain unqualified/unavailable");let stop=||cancel()||started.elapsed()>=Duration::from_millis(request.limits.elapsed_ms);
        let plan=planning::plan(request,&stop)?;
        crate::candidate_artifact::validate_private_root(&request.private_workdir)?;crate::candidate_artifact::validate_private_root(&request.artifact_root)?;
        let guard=||{ensure!(!stop(),"MP4 session cancelled or deadline exhausted");Ok(())};
        let source=BoundedFileObservation::open_mp4(&request.source,request.limits.input_bytes,&guard)?;ensure!(source.digest()==&plan.payload.source.content_digest&&source.snapshot()==&plan.payload.source.snapshot,"source changed after planning");
        let mut tools=vec![];for tool in &plan.payload.tools{let current=BoundedFileObservation::open_guarded(&tool.path,134_217_728,&guard)?;ensure!(current.digest()==&tool.content_digest&&current.snapshot()==&tool.snapshot,"tool changed after planning");tools.push(current);}
        let sandbox=SandboxSession::bind(request.sandbox.clone(),&guard)?;
        let mut session=Self {plan,request,cancel,started,source,tools,sandbox,invocations:vec![]};session.recheck()?;
        for (name,path) in [("ffmpeg",&request.ffmpeg.path),("ffprobe",&request.ffprobe.path)]{
            let output=session.raw(path,&["-version".into()],&[],65536)?;let text=std::str::from_utf8(&output.0)?;let words=text.lines().next().unwrap_or("").split_whitespace().collect::<Vec<_>>();
            ensure!(words.len()>=3&&words[0]==name&&words[1]=="version"&&words[2]=="8.0.1","selected MP4 codec tool is not exact version 8.0.1");
            session.invocations.push(Invocation {job_digest:digest(&(name,"-version"))?,operation:format!("{name}_version"),job:None,version_text:Some(text.into()),report:None,resource:output.1,elapsed_ms:output.2,output_digest:hash(&output.0),output_bytes:output.0.len() as u64});
        }Ok(session)
    }
    pub fn check(&self)->Result<()> {ensure!(!(self.cancel)()&&self.started.elapsed()<Duration::from_millis(self.request.limits.elapsed_ms),"MP4 session cancelled or deadline exhausted");Ok(())}
    pub fn recheck(&self)->Result<()> {self.check()?;let guard=||self.check();self.source.verify_current_guarded(&guard)?;for tool in &self.tools{tool.verify_current_guarded(&guard)?;}self.sandbox.verify(&guard)?;
        let reserve=self.request.limits.minimum_free_bytes.max(self.request.sandbox.minimum_free_bytes).checked_add(self.plan.payload.allocation.maximum_candidate_bytes.checked_mul(2).context("MP4 host reserve overflow")?).and_then(|n|n.checked_add(134_217_728)).context("MP4 host reserve overflow")?;
        free(&self.request.artifact_root,reserve)?;free(&self.request.private_workdir,reserve)?;self.check()
    }
    fn raw(&self,tool:&Path,args:&[String],input:&[u8],maximum:usize)->Result<(Vec<u8>,SandboxReceipt,u64)>{
        self.recheck()?;let remaining=Duration::from_millis(self.request.limits.elapsed_ms).checked_sub(self.started.elapsed()).filter(|d|!d.is_zero()).context("MP4 invocation allowance exhausted")?;
        let runner=SubprocessRunner::new(SubprocessLimits {timeout:remaining,poll_interval:Duration::from_millis(10),max_stdout_bytes:maximum,max_stderr_bytes:65536,max_concurrent_children:1})?;
        let output=self.sandbox.run(tool,args,input,&runner,&self.request.private_workdir,self.cancel)?;ensure!(output.exit_code==Some(0)&&output.stderr.is_empty(),"guarded MP4 invocation refused");self.recheck()?;
        Ok((output.stdout,self.sandbox.receipts()?.last().context("MP4 resource receipt absent")?.clone(),output.elapsed.as_millis().try_into()?))
    }
    pub fn source_bytes(&self)->Result<Vec<u8>>{self.source.read_mp4_bytes(&||self.check())}
    pub fn call(&mut self,operation:Operation,input:&[u8],media:&container::Summary)->Result<Vec<u8>>{
        self.check()?;let remaining=self.request.limits.elapsed_ms.checked_sub(self.started.elapsed().as_millis().try_into()?).filter(|n|*n>0).context("MP4 worker allowance exhausted")?;
        let job=Job {schema:"optiflow.mp4-worker-job.v1".into(),operation,input_digest:hash(input),input_bytes:input.len() as u64,ffmpeg:self.request.ffmpeg.clone(),ffprobe:self.request.ffprobe.clone(),media:media.clone(),control:self.request.control.clone(),allocation:self.plan.payload.allocation.clone(),elapsed_ms:remaining,provider_memory_bytes:self.request.sandbox.provider_subtree_memory_bytes,scratch_bytes:self.request.sandbox.provider_scratch_bytes};
        let maximum=if operation==Operation::Encode{self.plan.payload.allocation.maximum_candidate_bytes as usize}else{worker::TRACE_BYTES};
        let text=serde_json::to_string(&job)?;ensure!(text.len()<=worker::REPORT_BYTES,"MP4 worker job exceeds bound");
        let output=self.raw(&self.request.sandbox.helper.path,&[worker::FLAG.into(),text],input,maximum+worker::REPORT_BYTES+12)?;
        let (report,bytes)=worker::unpack(output.0,&job)?;ensure!(bytes.len()<=maximum,"MP4 worker payload exceeds cap");
        self.invocations.push(Invocation {job_digest:digest(&job)?,operation:operation.name().into(),job:Some(serde_json::to_value(&job)?),version_text:None,report:Some(serde_json::to_value(report)?),resource:output.1,elapsed_ms:output.2,output_digest:hash(&bytes),output_bytes:bytes.len() as u64});Ok(bytes)
    }
}
pub(crate) fn free(root:&Path,required:u64)->Result<()> {
    #[cfg(unix)]{let root=crate::candidate_artifact::open_directory(root)?;let volume=rustix::fs::fstatvfs(&root)?;ensure!(volume.f_frsize>0&&volume.f_bavail.checked_mul(volume.f_frsize).is_some_and(|n|n>=required),"MP4 observed free-space reserve unavailable");Ok(())}
    #[cfg(not(unix))]{let _=(root,required);anyhow::bail!("MP4 storage admission unavailable")}
}
pub(crate) fn fresh_directory(root:&Path,label:&str)->Result<(PathBuf,File)>{
    crate::candidate_artifact::validate_private_root(root)?;ensure!(label.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'),"fixed private namespace label required");let name=format!("{label}-{}",uuid::Uuid::now_v7());
    #[cfg(unix)]{use rustix::fs::{mkdirat,openat,Mode,OFlags};let parent=crate::candidate_artifact::open_directory(root)?;mkdirat(&parent,name.as_str(),Mode::RUSR|Mode::WUSR|Mode::XUSR)?;let child=File::from(openat(&parent,name.as_str(),OFlags::RDONLY|OFlags::DIRECTORY|OFlags::NOFOLLOW|OFlags::CLOEXEC,Mode::empty())?);child.sync_all()?;parent.sync_all()?;let path=root.join(&name);let observed=crate::candidate_artifact::open_directory(&path)?;let a=rustix::fs::fstat(&child)?;let b=rustix::fs::fstat(&observed)?;ensure!(a.st_dev==b.st_dev&&a.st_ino==b.st_ino,"private attempt namespace changed");Ok((path,child))}
    #[cfg(not(unix))]{let _=name;anyhow::bail!("MP4 private attempt unavailable")}
}
pub(crate) fn write_member(directory:&File,name:&str,bytes:&[u8])->Result<()> {
    ensure!(!name.is_empty()&&name!="."&&name!=".."&&name.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_.".contains(&b))&&bytes.len()<=OUTPUT_BYTES as usize,"bounded owned MP4 member required");
    #[cfg(unix)]{crate::candidate_artifact::write_new(directory,name,bytes)}
    #[cfg(not(unix))]{let _=(directory,name,bytes);anyhow::bail!("MP4 private member writing unavailable")}
}
#[derive(Debug,Serialize)]
pub struct UnacceptedCandidate {pub schema:String,pub attempt_directory:PathBuf,pub candidate:PathBuf,pub content_digest:EvidenceDigest,pub byte_count:u64,pub plan:Plan,pub invocations:Vec<Invocation>,pub state:String,pub source_mutation:bool}
pub(crate) fn require_probe(bytes:&[u8],media:&container::Summary)->Result<serde_json::Value>{
    let value:serde_json::Value=serde_json::from_slice(bytes)?;let streams=value["streams"].as_array().filter(|s|s.len()==2).context("exactly two observed native streams required")?;
    let video=streams.iter().filter(|s|s["codec_type"]=="video").collect::<Vec<_>>();let audio=streams.iter().filter(|s|s["codec_type"]=="audio").collect::<Vec<_>>();ensure!(video.len()==1&&audio.len()==1,"native video/audio selection differs");let v=video[0];let a=audio[0];
    ensure!(v["codec_name"]=="h264"&&v["pix_fmt"]=="yuv420p"&&v["field_order"]=="progressive"&&v["width"].as_u64()==Some(u64::from(media.video.width))&&v["height"].as_u64()==Some(u64::from(media.video.height))
        &&v["sample_aspect_ratio"]=="1:1"&&v["color_range"]=="tv"&&v["color_space"]=="bt709"&&v["color_transfer"]=="bt709"&&v["color_primaries"]=="bt709"
        &&a["codec_name"]=="aac"&&a["profile"]=="LC"&&a["channels"].as_u64()==Some(u64::from(media.audio.channels))&&a["sample_rate"].as_str()==Some(media.audio.sample_rate.to_string().as_str()),"native codec/pixel/color/channel scope differs from supported MP4 declaration");
    Ok(value)
}
/// This explicit operation only captures an unaccepted private candidate.
pub fn capture_unaccepted(request:&Request,cancel:impl Fn()->bool)->Result<UnacceptedCandidate>{
    let mut session=Session::new(request,&cancel)?;let (root,directory)=fresh_directory(&request.private_workdir,"mp4-attempt")?;let intent=serde_json::to_vec(&session.plan)?;write_member(&directory,"intent.json",&intent)?;
    let input=session.source_bytes()?;let media=session.plan.payload.source.media.clone();require_probe(&session.call(Operation::Probe,&input,&media)?,&media)?;let bytes=session.call(Operation::Encode,&input,&media)?;session.recheck()?;
    write_member(&directory,"candidate.mp4",&bytes)?;directory.sync_all()?;
    Ok(UnacceptedCandidate {schema:"optiflow.mp4-unaccepted-candidate.v1".into(),attempt_directory:root.clone(),candidate:root.join("candidate.mp4"),content_digest:hash(&bytes),byte_count:bytes.len() as u64,plan:session.plan,invocations:session.invocations,state:"untrusted_candidate_requires_independent_validation".into(),source_mutation:false})
}
