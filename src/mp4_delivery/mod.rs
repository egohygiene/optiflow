//! Source-preserving, explicitly lossy MP4 delivery candidates. Native execution,
//! structural acceptance, quality review and publication remain distinct.
use std::path::{Path,PathBuf};
use anyhow::{Result,ensure};
use serde::{Deserialize,Serialize};
use crate::domain::EvidenceDigest;
use crate::video_io::{BoundedFileObservation,FileSnapshot};
pub mod container;

pub const PROFILE:&str="optiflow.mp4-h264-aac-delivery.v1";
pub const INPUT_BYTES:u64=536_870_912;
pub const OUTPUT_BYTES:u64=268_435_456;
pub const METADATA_BYTES:usize=16_777_216;
pub const MAX_FRAMES:usize=36_000;
pub const MAX_AUDIO_PACKETS:usize=50_000;
pub const MAX_ELAPSED_MS:u64=7_200_000;
pub(crate) fn digest<T:Serialize>(v:&T)->Result<EvidenceDigest>{let value=serde_json::to_value(v)?;Ok(hash(&serde_json::to_vec(&value)?))}
pub(crate) fn hash(bytes:&[u8])->EvidenceDigest{EvidenceDigest {algorithm:"blake3-256".into(),value:blake3::hash(bytes).to_hex().to_string()}}
pub(crate) fn normalized(path:&Path)->Result<()> {ensure!(path.is_absolute()&&!path.components().any(|c|matches!(c,std::path::Component::CurDir|std::path::Component::ParentDir)),"normalized absolute MP4 path required");Ok(())}

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {pub path:PathBuf,pub snapshot:FileSnapshot,pub content_digest:EvidenceDigest,pub media:container::Summary}
/// Full streaming identity plus bounded descriptor range inspection, no provider.
pub fn inspect(path:&Path,maximum:u64,cancel:impl Fn()->bool)->Result<Source>{
    normalized(path)?;ensure!((1..=INPUT_BYTES).contains(&maximum),"explicit MP4 input ceiling required");
    let guard=||{ensure!(!cancel(),"MP4 inspection cancelled");Ok(())};
    let file=BoundedFileObservation::open_mp4(path,maximum,&guard)?;
    ensure!(file.snapshot().link_count==1,"MP4 source must be singly linked");
    let parsed=container::observe(&file,&guard)?;file.verify_current_guarded(&guard)?;
    Ok(Source {path:path.into(),snapshot:file.snapshot().clone(),content_digest:file.digest().clone(),media:parsed.summary})
}
pub fn profiles()->serde_json::Value{serde_json::json!({"schema":"optiflow.mp4-delivery-profile-catalog.v1","profile":PROFILE,"maximum_input_bytes":INPUT_BYTES,"maximum_candidate_bytes":OUTPUT_BYTES,"maximum_width":4096,"maximum_height":2160,"maximum_frames":MAX_FRAMES,"maximum_duration_seconds":900,"subset":"unfragmented self-contained progressive SDR H264/yuv420p CFR plus one mono/stereo AAC-LC track at 44100/48000 Hz; explicit edits; no chapters, subtitles, HDR or rotation","source_mutation":false,"native_qualified":false,"perceptual_quality":"human_review_required","provider_invoked":false})}
