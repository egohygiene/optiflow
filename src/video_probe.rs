//! Bounded parsing of the pinned provider's complete, reported FFV1 view.
//!
//! This module does no IO and authenticates neither its input nor a provider.
//! Even a complete ffprobe response is not complete container-feature coverage
//! or proof of successful full decode. The coordinator owns byte/provider
//! identity, invocation completion and source stability.

use serde::Serialize;
use serde_json::{Map, Value};

use crate::domain::EvidenceDigest;
use crate::video_candidate::{Rational, VideoDeclaration};

pub const MAX_REPORT_BYTES: usize = 4 * 1024 * 1024;
const MAX_FRAMES: usize = 250;
const MAX_DECODED_BYTES: u64 = 64 * 1024 * 1024;

/// Full display is material: without it, omitted tags/side-data cannot mean
/// the pinned provider reported none. Never add selection or read intervals.
pub fn probe_arguments() -> Vec<String> {
    [
        "-v", "error", "-protocol_whitelist", "pipe", "-threads", "1",
        "-f", "matroska", "-show_error", "-show_optional_fields", "always",
        "-count_frames", "-show_streams", "-show_format", "-show_programs",
        "-show_stream_groups", "-show_chapters", "-show_frames", "-of", "json",
        "-i", "pipe:0",
    ].into_iter().map(str::to_owned).collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct FrameTiming {
    pub pts: i64,
    pub best_effort_timestamp: i64,
    pub duration_ticks: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoProbeObservation {
    pub media: VideoDeclaration,
    pub chroma_location: String,
    pub raw_report_digest: EvidenceDigest,
    pub frames: Vec<FrameTiming>,
    pub decoded_sample_bytes: u64,
    pub coverage: &'static str,
    pub limitations: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeRefusal {
    ReportTooLarge,
    InvalidJson,
    InvalidShape,
    UnexpectedField,
    ProviderReportedError,
    UnsupportedSource,
    UnsupportedFeature,
    InvalidTiming,
    LimitExceeded,
}

impl std::fmt::Display for ProbeRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "video probe refused: {self:?}")
    }
}
impl std::error::Error for ProbeRefusal {}

type Object = Map<String, Value>;
type Result<T> = std::result::Result<T, ProbeRefusal>;

/// Parse only the FFmpeg 8.0.1 reported view of the first FFV1 subset.
/// Unknown source features are not inferred absent outside that view.
pub fn parse_ffv1_observation(bytes: &[u8]) -> Result<VideoProbeObservation> {
    if bytes.len() > MAX_REPORT_BYTES {
        return Err(ProbeRefusal::ReportTooLarge);
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| ProbeRefusal::InvalidJson)?;
    let root = object(&value)?;
    keys(root, &["streams", "format", "frames", "programs", "stream_groups", "chapters", "error"])?;
    if root.contains_key("error") {
        return Err(ProbeRefusal::ProviderReportedError);
    }
    for section in ["programs", "stream_groups", "chapters"] {
        if !array(root, section)?.is_empty() {
            return Err(ProbeRefusal::UnsupportedFeature);
        }
    }
    let streams = array(root, "streams")?;
    let frames = array(root, "frames")?;
    if streams.len() != 1 || frames.is_empty() {
        return Err(ProbeRefusal::UnsupportedSource);
    }
    if frames.len() > MAX_FRAMES {
        return Err(ProbeRefusal::LimitExceeded);
    }
    let format = object(required(root, "format")?)?;
    keys(format, &[
        "filename", "nb_streams", "nb_programs", "nb_stream_groups", "format_name",
        "format_long_name", "start_time", "duration", "size", "bit_rate", "probe_score", "tags",
    ])?;
    if text(format, "format_name")? != "matroska,webm"
        || unsigned(format, "nb_streams")? != 1
        || unsigned(format, "nb_programs")? != 0
        || unsigned(format, "nb_stream_groups")? != 0
    {
        return Err(ProbeRefusal::UnsupportedSource);
    }
    technical_tags(format)?;

    let stream = object(&streams[0])?;
    keys(stream, &[
        "index", "codec_name", "codec_long_name", "profile", "codec_type", "codec_tag_string",
        "codec_tag", "width", "height", "coded_width", "coded_height", "closed_captions",
        "film_grain", "has_b_frames", "sample_aspect_ratio", "display_aspect_ratio", "pix_fmt",
        "level", "color_range", "color_space", "color_transfer", "color_primaries", "chroma_location",
        "field_order", "refs", "id", "r_frame_rate", "avg_frame_rate", "time_base", "start_pts",
        "start_time", "duration_ts", "duration", "bit_rate", "max_bit_rate", "bits_per_raw_sample",
        "nb_frames", "nb_read_frames", "nb_read_packets", "extradata_size", "extradata_hash",
        "disposition", "tags", "side_data_list",
    ])?;
    if text(stream, "codec_name")? != "ffv1" || text(stream, "codec_type")? != "video"
        || unsigned(stream, "index")? != 0 || text(stream, "pix_fmt")? != "yuv420p"
        || text(stream, "field_order")? != "progressive" || unsigned(stream, "has_b_frames")? != 0
    {
        return Err(ProbeRefusal::UnsupportedSource);
    }
    let width = dimension(stream, "width", 640)?;
    let height = dimension(stream, "height", 360)?;
    if unsigned(stream, "coded_width")? != u64::from(width)
        || unsigned(stream, "coded_height")? != u64::from(height)
        || rational(stream, "sample_aspect_ratio", ':')? != (Rational { numerator: 1, denominator: 1 })
    {
        return Err(ProbeRefusal::UnsupportedFeature);
    }
    colors(stream)?;
    let chroma_location = text(stream, "chroma_location")?.to_owned();
    if !["unspecified", "left", "center", "topleft", "top", "bottomleft", "bottom"]
        .contains(&chroma_location.as_str())
    {
        return Err(ProbeRefusal::UnsupportedFeature);
    }
    technical_tags(stream)?;
    no_side_data(stream)?;
    for field in ["closed_captions", "film_grain"] {
        if stream.contains_key(field) && unsigned(stream, field)? != 0 {
            return Err(ProbeRefusal::UnsupportedFeature);
        }
    }
    let disposition = disposition(stream)?;
    let frame_rate = Rational { numerator: 25, denominator: 1 };
    let time_base = Rational { numerator: 1, denominator: 1000 };
    if rational(stream, "r_frame_rate", '/')? != frame_rate
        || rational(stream, "avg_frame_rate", '/')? != frame_rate
        || rational(stream, "time_base", '/')? != time_base
        || signed(stream, "start_pts")? != 0
        || decimal(text(stream, "nb_read_frames")?)? != frames.len() as u64
    {
        return Err(ProbeRefusal::InvalidTiming);
    }
    // Matroska often has no nb_frames/duration_ts; those are not a substitute
    // for decoded-frame observations. If supplied, contradictions refuse.
    optional_count(stream, "nb_frames", frames.len() as u64)?;
    optional_ticks(stream, "duration_ts", frames.len() as u64 * 40)?;

    let mut timings = Vec::with_capacity(frames.len());
    for (index, frame) in frames.iter().enumerate() {
        let frame = object(frame)?;
        keys(frame, &[
            "media_type", "stream_index", "key_frame", "pts", "pts_time", "pkt_dts", "pkt_dts_time",
            "best_effort_timestamp", "best_effort_timestamp_time", "duration", "duration_time",
            "pkt_pos", "pkt_size", "width", "height", "crop_top", "crop_bottom", "crop_left", "crop_right",
            "pix_fmt", "sample_aspect_ratio", "pict_type", "interlaced_frame", "top_field_first",
            "lossless", "repeat_pict", "color_range", "color_space", "color_primaries", "color_transfer",
            "chroma_location", "tags", "side_data_list",
        ])?;
        if text(frame, "media_type")? != "video" || unsigned(frame, "stream_index")? != 0
            || unsigned(frame, "width")? != u64::from(width)
            || unsigned(frame, "height")? != u64::from(height)
            || text(frame, "pix_fmt")? != "yuv420p"
            || rational(frame, "sample_aspect_ratio", ':')? != (Rational { numerator: 1, denominator: 1 })
            || text(frame, "chroma_location")? != chroma_location
        {
            return Err(ProbeRefusal::UnsupportedFeature);
        }
        colors(frame)?;
        for zero in ["crop_top", "crop_bottom", "crop_left", "crop_right", "interlaced_frame", "top_field_first", "repeat_pict"] {
            if unsigned(frame, zero)? != 0 {
                return Err(ProbeRefusal::UnsupportedFeature);
            }
        }
        if let Some(tags) = frame.get("tags") {
            if !object(tags)?.is_empty() {
                return Err(ProbeRefusal::UnsupportedFeature);
            }
        }
        no_side_data(frame)?;
        let pts = signed(frame, "pts")?;
        let best_effort_timestamp = signed(frame, "best_effort_timestamp")?;
        let duration_ticks = unsigned(frame, "duration")?;
        if pts != index as i64 * 40 || best_effort_timestamp != pts || duration_ticks != 40 {
            return Err(ProbeRefusal::InvalidTiming);
        }
        timings.push(FrameTiming { pts, best_effort_timestamp, duration_ticks });
    }
    let decoded_sample_bytes = u64::from(width) * u64::from(height) * 3 / 2 * frames.len() as u64;
    if decoded_sample_bytes > MAX_DECODED_BYTES {
        return Err(ProbeRefusal::LimitExceeded);
    }
    Ok(VideoProbeObservation {
        media: VideoDeclaration {
            container: "matroska".to_owned(), codec: "ffv1".to_owned(), stream_index: 0,
            total_streams: 1, disposition, width, height, pixel_format: "yuv420p".to_owned(),
            field_order: "progressive".to_owned(), color_range: "tv".to_owned(),
            color_space: "bt709".to_owned(), color_transfer: "bt709".to_owned(),
            color_primaries: "bt709".to_owned(), sample_aspect_ratio: Rational { numerator: 1, denominator: 1 },
            frame_rate, time_base, frame_count: frames.len() as u64, start_pts: 0,
            duration_ticks: frames.len() as u64 * 40, constant_frame_rate: true,
            rotation_degrees: 0, feature_observation_complete: false, chapters: 0,
            side_data_entries: 0, descriptive_metadata_entries: 0, unknown_metadata_entries: 0,
        },
        chroma_location,
        raw_report_digest: EvidenceDigest {
            algorithm: "blake3-256".to_owned(),
            value: blake3::hash(bytes).to_hex().to_string(),
        },
        frames: timings,
        decoded_sample_bytes,
        coverage: "ffprobe_reported_features_only",
        limitations: vec![
            "parser_does_not_authenticate_provider_source_or_invocation_completion",
            "nonseekable_input_may_omit_container_metadata",
            "ffprobe_report_is_not_complete_container_feature_coverage",
            "ffprobe_success_is_not_independent_complete_decode_proof",
            "no_candidate_validation_publication_or_replacement_authority",
        ],
    })
}

fn object(value: &Value) -> Result<&Object> { value.as_object().ok_or(ProbeRefusal::InvalidShape) }
fn required<'a>(value: &'a Object, key: &str) -> Result<&'a Value> {
    value.get(key).ok_or(ProbeRefusal::InvalidShape)
}
fn text<'a>(value: &'a Object, key: &str) -> Result<&'a str> {
    required(value, key)?.as_str().ok_or(ProbeRefusal::InvalidShape)
}
fn unsigned(value: &Object, key: &str) -> Result<u64> {
    required(value, key)?.as_u64().ok_or(ProbeRefusal::InvalidShape)
}
fn signed(value: &Object, key: &str) -> Result<i64> {
    required(value, key)?.as_i64().ok_or(ProbeRefusal::InvalidShape)
}
fn array<'a>(value: &'a Object, key: &str) -> Result<&'a Vec<Value>> {
    required(value, key)?.as_array().ok_or(ProbeRefusal::InvalidShape)
}
fn keys(value: &Object, allowed: &[&str]) -> Result<()> {
    if value.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(ProbeRefusal::UnexpectedField);
    }
    Ok(())
}
fn decimal(value: &str) -> Result<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ProbeRefusal::InvalidShape);
    }
    value.parse().map_err(|_| ProbeRefusal::InvalidShape)
}
fn rational(value: &Object, key: &str, separator: char) -> Result<Rational> {
    let (numerator, denominator) = text(value, key)?.split_once(separator).ok_or(ProbeRefusal::InvalidShape)?;
    let numerator = u32::try_from(decimal(numerator)?).map_err(|_| ProbeRefusal::InvalidShape)?;
    let denominator = u32::try_from(decimal(denominator)?).map_err(|_| ProbeRefusal::InvalidShape)?;
    if numerator == 0 || denominator == 0 { return Err(ProbeRefusal::InvalidShape); }
    Ok(Rational { numerator, denominator })
}
fn dimension(value: &Object, key: &str, maximum: u64) -> Result<u32> {
    let value = unsigned(value, key)?;
    if value < 2 || value > maximum || value % 2 != 0 { return Err(ProbeRefusal::UnsupportedSource); }
    Ok(value as u32)
}
fn colors(value: &Object) -> Result<()> {
    for (key, expected) in [("color_range", "tv"), ("color_space", "bt709"), ("color_transfer", "bt709"), ("color_primaries", "bt709")] {
        if text(value, key)? != expected { return Err(ProbeRefusal::UnsupportedFeature); }
    }
    Ok(())
}
fn technical_tags(value: &Object) -> Result<()> {
    // In pinned ffprobe show_tags(), no dictionary means no tags section.
    if let Some(tags) = value.get("tags") {
        let tags = object(tags)?;
        let mut seen = std::collections::BTreeSet::new();
        for (key, value) in tags {
            let key = key.to_ascii_lowercase();
            if !["encoder", "duration"].contains(&key.as_str()) || !seen.insert(key) {
                return Err(ProbeRefusal::UnsupportedFeature);
            }
            if value.as_str().is_none_or(|text| text.len() > 4096) {
                return Err(ProbeRefusal::InvalidShape);
            }
        }
    }
    Ok(())
}
fn no_side_data(value: &Object) -> Result<()> {
    // Pinned show_frame/show_stream emit the array only for nonzero side data.
    if value.contains_key("side_data_list") && !array(value, "side_data_list")?.is_empty() {
        return Err(ProbeRefusal::UnsupportedFeature);
    }
    Ok(())
}
fn optional_count(value: &Object, key: &str, expected: u64) -> Result<()> {
    if let Some(found) = value.get(key) {
        let found = found.as_str().ok_or(ProbeRefusal::InvalidShape)?;
        if found != "N/A" && decimal(found)? != expected { return Err(ProbeRefusal::InvalidTiming); }
    }
    Ok(())
}
fn optional_ticks(value: &Object, key: &str, expected: u64) -> Result<()> {
    if let Some(found) = value.get(key) {
        if found.as_str() != Some("N/A") && found.as_u64() != Some(expected) { return Err(ProbeRefusal::InvalidTiming); }
    }
    Ok(())
}
fn disposition(value: &Object) -> Result<String> {
    const FLAGS: &[&str] = &[
        "default", "dub", "original", "comment", "lyrics", "karaoke", "forced",
        "hearing_impaired", "visual_impaired", "clean_effects", "attached_pic", "timed_thumbnails",
        "non_diegetic", "captions", "descriptions", "metadata", "dependent", "still_image", "multilayer",
    ];
    let values = object(required(value, "disposition")?)?;
    keys(values, FLAGS)?;
    for flag in FLAGS {
        let state = unsigned(values, flag)?;
        if state > 1 || (*flag != "default" && state != 0) { return Err(ProbeRefusal::UnsupportedFeature); }
    }
    Ok(if unsigned(values, "default")? == 1 { "default" } else { "none" }.to_owned())
}
