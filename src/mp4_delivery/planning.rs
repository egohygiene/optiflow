//! Pure rate allocation plus read-only source/tool binding. No provider runs.
use super::*;
use anyhow::Context;
use crate::video_sandbox::{LinuxVideoSandbox,PinnedLauncher};

pub const REQUEST_SCHEMA:&str="optiflow.mp4-delivery-request.v1";
pub const PLAN_SCHEMA:&str="optiflow.mp4-delivery-plan.v1";
pub const SANDBOX_SCHEMA:&str="optiflow.linux-mp4-delivery-sandbox.v1";
pub(crate) const INPUT_FILE:&str="/tmp/delivery-source.mp4";
pub(crate) const OUTPUT_FILE:&str="/tmp/delivery-candidate.mp4";
pub(crate) const PASS_LOG:&str="/tmp/delivery-pass";
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Preset {Fast,Medium,Slow,VerySlow}
impl Preset {pub(crate) fn argument(self)->&'static str{match self {Self::Fast=>"fast",Self::Medium=>"medium",Self::Slow=>"slow",Self::VerySlow=>"veryslow"}}}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum RateControl {Quality {crf:u8,preset:Preset},TargetSize {maximum_bytes:u64,mux_reserve_bytes:u64,preset:Preset}}
impl RateControl {
    pub(crate) fn maximum(&self,limit:u64)->u64 {match self{Self::TargetSize {maximum_bytes,..}=>*maximum_bytes,Self::Quality {..}=>limit}}
    fn validate(&self,limit:u64)->Result<()>{match self {
        Self::Quality {crf,..}=>ensure!((1..=51).contains(crf),"explicit bounded H264 CRF required"),
        Self::TargetSize {maximum_bytes,mux_reserve_bytes,..}=>ensure!((1_048_576..=limit).contains(maximum_bytes)&&*mux_reserve_bytes>=1_048_576&&mux_reserve_bytes<maximum_bytes,"explicit target byte cap and positive mux headroom required"),
    }Ok(())}
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {pub input_bytes:u64,pub candidate_bytes:u64,pub elapsed_ms:u64,pub memory_admission_bytes:u64,pub working_bytes:u64,pub minimum_free_bytes:u64}
impl Limits {pub(crate) fn validate(&self)->Result<()>{ensure!((1..=INPUT_BYTES).contains(&self.input_bytes)&&(1..=OUTPUT_BYTES).contains(&self.candidate_bytes)&&(1..=MAX_ELAPSED_MS).contains(&self.elapsed_ms)&&(134_217_728..=8_589_934_592).contains(&self.memory_admission_bytes)&&(1..=4_294_967_296).contains(&self.working_bytes)&&self.minimum_free_bytes<=1_099_511_627_776,"explicit bounded MP4 source/candidate/time/memory/storage settings required");Ok(())}}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema:String,pub profile:String,pub source:PathBuf,pub ffmpeg:PinnedLauncher,pub ffprobe:PinnedLauncher,
    pub sandbox:LinuxVideoSandbox,pub private_workdir:PathBuf,pub artifact_root:PathBuf,
    pub control:RateControl,pub audio:String,pub metadata:String,pub limits:Limits,
}
impl Request {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{ensure!(bytes.len()<=131_072,"MP4 request exceeds 128 KiB");let value:Self=serde_json::from_slice(bytes)?;value.validate()?;Ok(value)}
    pub fn validate(&self)->Result<()> {
        self.limits.validate()?;self.control.validate(self.limits.candidate_bytes)?;
        ensure!(self.schema==REQUEST_SCHEMA&&self.profile==PROFILE&&self.audio=="copy"&&self.metadata=="preserve_supported_allow_encoder_change","explicit MP4 profile/audio-copy/metadata policy required");
        validate_sandbox(&self.sandbox)?;
        for path in [&self.source,&self.ffmpeg.path,&self.ffprobe.path,&self.private_workdir,&self.artifact_root,&self.sandbox.systemd_run.path,&self.sandbox.systemctl.path,&self.sandbox.helper.path,&self.sandbox.user_bus] {normalized(path)?;ensure!(path.to_str().is_some_and(|s|s.len()<=4096&&!s.contains('%')&&!s.chars().any(char::is_control)),"bounded UTF8 path without service specifiers/control characters required");}
        ensure!(self.ffmpeg.path!=self.ffprobe.path,"distinct explicit ffmpeg/ffprobe required");
        for output in [&self.private_workdir,&self.artifact_root] {for source in [&self.source,&self.ffmpeg.path,&self.ffprobe.path,&self.sandbox.systemd_run.path,&self.sandbox.systemctl.path,&self.sandbox.helper.path] {ensure!(!output.starts_with(source)&&!source.starts_with(output),"MP4 output overlaps selected source/tool");}}
        ensure!(!self.private_workdir.starts_with(&self.artifact_root)&&!self.artifact_root.starts_with(&self.private_workdir),"private MP4 work and publication namespaces must be disjoint");
        for pin in [&self.ffmpeg,&self.ffprobe,&self.sandbox.systemd_run,&self.sandbox.systemctl,&self.sandbox.helper] {valid_digest(&pin.digest)?;}
        Ok(())
    }
}
pub(crate) fn valid_digest(d:&EvidenceDigest)->Result<()> {ensure!(d.algorithm=="blake3-256"&&d.value.len()==64&&d.value.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)),"complete lowercase Blake3 digest required");Ok(())}
pub(crate) fn validate_sandbox(s:&LinuxVideoSandbox)->Result<()> {
    ensure!(s.schema==SANDBOX_SCHEMA&&(134_217_728..=2_147_483_648).contains(&s.provider_subtree_memory_bytes)&&(16_777_216..=2_147_483_648).contains(&s.provider_scratch_bytes)&&s.provider_scratch_bytes%4096==0&&s.minimum_free_bytes<=1_099_511_627_776,"separate bounded MP4 Linux resource policy required");
    ensure!(s.provider_scratch_bytes.checked_add(67_108_864).is_some_and(|n|n<=s.provider_subtree_memory_bytes),"provider memory admission must cover aggregate tmpfs scratch and decoder allowance");
    normalized(&s.user_bus)?;ensure!(s.user_bus.file_name().is_some_and(|n|n=="bus"),"explicit local user-manager bus required");Ok(())
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {pub name:String,pub path:PathBuf,pub content_digest:EvidenceDigest,pub snapshot:FileSnapshot}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Allocation {pub maximum_candidate_bytes:u64,pub copied_audio_bytes:u64,pub reserved_mux_bytes:u64,pub video_bitrate_bps:Option<u64>,pub guarantee:String}
pub(crate) fn target_rate(maximum:u64,reserve:u64,audio:u64,duration:container::Time)->Result<u64>{
    ensure!(duration.numerator>0&&duration.denominator>0,"positive complete video duration required");
    let available=maximum.checked_sub(reserve).and_then(|n|n.checked_sub(audio)).context("audio and mux allowance consume selected target; no audio reduction or resizing is implicit")?;
    let rate=u128::from(available).checked_mul(8).and_then(|n|n.checked_mul(u128::from(duration.denominator))).context("target rate overflow")?/duration.numerator as u128;
    let rate=u64::try_from(rate)?;ensure!((10_000..=120_000_000).contains(&rate),"target video bitrate outside bounded profile; select another explicit target/policy");Ok(rate)
}
fn allocation(control:&RateControl,media:&container::Summary,limit:u64)->Result<Allocation>{
    control.validate(limit)?;let audio=media.audio.payload_bytes;
    let (reserve,bitrate)=match control {
        RateControl::Quality {..}=>(0,None),
        RateControl::TargetSize {maximum_bytes,mux_reserve_bytes,..}=>{
            let minimum=u64::from(media.video.sample_count).checked_mul(64).and_then(|n|n.checked_add(u64::from(media.audio.sample_count)*32)).and_then(|n|n.checked_add(1_048_576)).context("mux allowance overflow")?;
            ensure!(*mux_reserve_bytes>=minimum,"mux allowance is below explicit source sample-table budget");
            let period=media.video.frame_period.context("CFR period missing")?;let duration=container::Time::new(i128::from(period.numerator)*i128::from(media.video.sample_count),period.denominator)?;
            (*mux_reserve_bytes,Some(target_rate(*maximum_bytes,*mux_reserve_bytes,audio,duration)?))
        },
    };
    Ok(Allocation {maximum_candidate_bytes:control.maximum(limit),copied_audio_bytes:audio,reserved_mux_bytes:reserve,video_bitrate_bps:bitrate,guarantee:"planning_estimate_only; final_actual_bytes_must_pass_cap_and_strictly_smaller_validation".into()})
}
pub(crate) fn encode_argv(control:&RateControl,allocation:&Allocation,video_scale:u32,movie_scale:u32)->Result<Vec<Vec<String>>>{
    control.validate(allocation.maximum_candidate_bytes)?;ensure!(video_scale>0&&movie_scale>0,"explicit original track/movie clocks required");
    let preset=match control{RateControl::Quality {preset,..}|RateControl::TargetSize {preset,..}=>preset.argument()};
    let base=||["-hide_banner","-nostdin","-loglevel","error","-xerror","-err_detect","explode","-copyts","-i",INPUT_FILE,"-map","0:v:0","-c:v","libx264","-preset",preset,"-pix_fmt","+yuv420p","-fps_mode:v","passthrough","-color_range","tv","-colorspace","bt709","-color_trc","bt709","-color_primaries","bt709"].into_iter().map(str::to_owned).collect::<Vec<_>>();
    let mut common=base();common.extend(["-enc_time_base:v".into(),format!("1/{video_scale}")]);
    match control{RateControl::Quality {crf,..}=>common.extend(["-crf".into(),crf.to_string()]),RateControl::TargetSize {..}=>common.extend(["-b:v".into(),allocation.video_bitrate_bps.context("target video rate missing")?.to_string(),"-passlogfile".into(),PASS_LOG.into()])};
    let mut invocations=vec![];
    if matches!(control,RateControl::TargetSize {..}) {let mut first=common.clone();first.extend(["-pass","1","-an","-f","null","-"].into_iter().map(str::to_owned));invocations.push(first);}
    let mut final_pass=common;if matches!(control,RateControl::TargetSize {..}){final_pass.extend(["-pass".into(),"2".into()]);}
    final_pass.extend(["-map","0:a:0","-c:a","copy","-map_metadata","0","-map_metadata:s:v:0","0:s:v:0","-map_metadata:s:a:0","0:s:a:0","-map_chapters","-1","-avoid_negative_ts","disabled","-movflags","+faststart","-use_editlist","1","-video_track_timescale"].into_iter().map(str::to_owned));
    final_pass.extend([video_scale.to_string(),"-movie_timescale".into(),movie_scale.to_string(),"-n".into(),OUTPUT_FILE.into()]);invocations.push(final_pass);Ok(invocations)
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {pub request:Request,pub source:Source,pub tools:Vec<Tool>,pub allocation:Allocation,pub encoder_argv:Vec<Vec<String>>,pub projected_host_payload_bytes:u64,pub projected_scratch_bytes:u64,pub runtime_qualified:bool,pub quality_review:String,pub source_mutation:bool}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {pub schema:String,pub payload:Payload,pub plan_digest:EvidenceDigest}
pub(crate) fn bind_tools(request:&Request,guard:&impl Fn()->Result<()>)->Result<Vec<Tool>>{
    let mut tools=vec![];
    for (name,pin) in [("ffmpeg",&request.ffmpeg),("ffprobe",&request.ffprobe),("systemd_run",&request.sandbox.systemd_run),("systemctl",&request.sandbox.systemctl),("worker",&request.sandbox.helper)] {
        let observed=BoundedFileObservation::open_guarded(&pin.path,134_217_728,guard)?;
        ensure!(observed.digest()==&pin.digest&&observed.snapshot().mode&0o111!=0&&observed.snapshot().mode&0o6000==0,"selected MP4 executable differs from exact safe pin");tools.push(Tool {name:name.into(),path:pin.path.clone(),content_digest:pin.digest.clone(),snapshot:observed.snapshot().clone()});
    }Ok(tools)
}
pub(crate) fn retained(plan:&Plan)->Result<()> {
    let p=&plan.payload;let r=&p.request;r.validate()?;
    ensure!(plan.schema==PLAN_SCHEMA&&plan.plan_digest==digest(p)?&&p.source.path==r.source&&p.source.snapshot.logical_bytes>0&&p.source.snapshot.logical_bytes<=r.limits.input_bytes&&!p.runtime_qualified&&!p.source_mutation&&p.quality_review=="required; lossy video has no exact visual equivalence claim","retained MP4 plan scope differs");
    ensure!(p.allocation==allocation(&r.control,&p.source.media,r.limits.candidate_bytes)?&&p.encoder_argv==encode_argv(&r.control,&p.allocation,p.source.media.video.timescale,p.source.media.movie_timescale)?,"retained MP4 target budget or encoder intent differs");
    let pins=[("ffmpeg",&r.ffmpeg),("ffprobe",&r.ffprobe),("systemd_run",&r.sandbox.systemd_run),("systemctl",&r.sandbox.systemctl),("worker",&r.sandbox.helper)];ensure!(p.tools.len()==pins.len(),"retained tool membership differs");
    for (tool,(name,pin)) in p.tools.iter().zip(pins){ensure!(tool.name==name&&tool.path==pin.path&&tool.content_digest==pin.digest&&tool.snapshot.logical_bytes>0&&tool.snapshot.logical_bytes<=134_217_728&&tool.snapshot.mode&0o111!=0&&tool.snapshot.mode&0o6000==0,"retained MP4 tool binding differs");}Ok(())
}
pub fn plan(request:&Request,cancel:impl Fn()->bool)->Result<Plan>{
    request.validate()?;let guard=||{ensure!(!cancel(),"MP4 planning cancelled");Ok(())};
    let source=super::inspect(&request.source,request.limits.input_bytes,&cancel)?;
    let tools=bind_tools(request,&guard)?;
    let allocation=allocation(&request.control,&source.media,request.limits.candidate_bytes)?;
    let out=allocation.maximum_candidate_bytes;
    let projected_scratch_bytes=request.limits.input_bytes.checked_add(out.checked_mul(2).context("faststart scratch overflow")?).and_then(|n|n.checked_add(83_886_080)).context("MP4 whole scratch admission overflow")?;
    ensure!(projected_scratch_bytes<=request.sandbox.provider_scratch_bytes&&projected_scratch_bytes.checked_add(out.checked_mul(2).context("host staging overflow")?).and_then(|n|n.checked_add(134_217_728)).is_some_and(|n|n<=request.limits.working_bytes),"whole input/pass-log/faststart/private-trace/publication scratch exceeds selected admission");
    let projected_host_payload_bytes=request.limits.input_bytes.checked_add(out.checked_mul(3).context("MP4 candidate buffer admission overflow")?).and_then(|n|n.checked_add(134_217_728)).context("MP4 host buffer admission overflow")?;
    ensure!(projected_host_payload_bytes.checked_add(request.sandbox.provider_subtree_memory_bytes).is_some_and(|n|n<=request.limits.memory_admission_bytes),"host payload buffers plus provider quota exceed selected memory admission; total host RSS is not guaranteed");
    let encoder_argv=encode_argv(&request.control,&allocation,source.media.video.timescale,source.media.movie_timescale)?;
    let payload=Payload {request:request.clone(),source,tools,allocation,encoder_argv,projected_host_payload_bytes,projected_scratch_bytes,runtime_qualified:false,quality_review:"required; lossy video has no exact visual equivalence claim".into(),source_mutation:false};
    let plan=Plan {schema:PLAN_SCHEMA.into(),plan_digest:digest(&payload)?,payload};ensure!(serde_json::to_vec(&plan)?.len()<=METADATA_BYTES,"MP4 plan exceeds bound");guard()?;Ok(plan)
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn size_budget_keeps_copied_audio_and_mux_headroom_without_float_rounding(){let duration=container::Time::new(240,1).unwrap();assert_eq!(target_rate(100_000_000,2_000_000,8_000_000,duration).unwrap(),3_000_000);assert!(target_rate(8_000_000,1_000_000,8_000_000,duration).is_err());}
    #[test]fn two_pass_argv_cannot_silently_trim_resize_or_reencode_audio(){let control=RateControl::TargetSize {maximum_bytes:100_000_000,mux_reserve_bytes:2_000_000,preset:Preset::VerySlow};let allocation=Allocation {maximum_candidate_bytes:100_000_000,copied_audio_bytes:8_000_000,reserved_mux_bytes:2_000_000,video_bitrate_bps:Some(3_000_000),guarantee:"fixture".into()};let args=encode_argv(&control,&allocation,30000,1000).unwrap();assert_eq!(args.len(),2);assert!(args[0].windows(2).any(|p|p==["-pass","1"]));assert!(args[1].windows(2).any(|p|p==["-c:a","copy"]));for a in args{for forbidden in ["-t","-shortest","-r","-vf","-af","-fs"]{assert!(!a.iter().any(|v|v==forbidden));}}}
}
