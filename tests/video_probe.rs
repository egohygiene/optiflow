//! Synthetic parser cases authored October 28, 2026. No captured provider
//! output or real media is included, and these cases execute no provider.

use optiflow::video_probe::{MAX_REPORT_BYTES, ProbeRefusal, parse_ffv1_observation, probe_arguments};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/video/probe-ffv1-v1.json")).unwrap()
}

fn parse(value: &Value) -> Result<optiflow::video_probe::VideoProbeObservation, ProbeRefusal> {
    parse_ffv1_observation(&serde_json::to_vec(value).unwrap())
}

#[test]
fn accepted_report_retains_observations_without_complete_coverage_or_decode_claim() {
    let report = parse(&fixture()).unwrap();
    assert_eq!(report.media.container, "matroska");
    assert_eq!(report.media.frame_count, 2);
    assert_eq!(report.media.duration_ticks, 80);
    assert_eq!(report.frames[1].pts, 40);
    assert_eq!(report.frames[1].duration_ticks, 40);
    assert_eq!(report.decoded_sample_bytes, 320 * 180 * 3 / 2 * 2);
    assert_eq!(report.chroma_location, "unspecified");
    assert!(!report.media.feature_observation_complete);
    assert_eq!(report.coverage, "ffprobe_reported_features_only");
    assert!(report.limitations.contains(&"nonseekable_input_may_omit_container_metadata"));
    assert!(report.limitations.contains(&"ffprobe_success_is_not_independent_complete_decode_proof"));
    let bytes = serde_json::to_vec(&fixture()).unwrap();
    assert_eq!(report.raw_report_digest.value, blake3::hash(&bytes).to_hex().to_string());
    let mut whitespace_changed = bytes;
    whitespace_changed.push(b'\n');
    assert_ne!(parse_ffv1_observation(&whitespace_changed).unwrap().raw_report_digest,
               report.raw_report_digest);
}

#[test]
fn full_invocation_does_not_select_or_truncate_streams_or_frames() {
    let arguments = probe_arguments();
    for flag in ["-show_streams", "-show_format", "-show_frames", "-show_chapters",
                 "-show_programs", "-show_stream_groups", "-count_frames"] {
        assert!(arguments.iter().any(|argument| argument == flag));
    }
    assert!(!arguments.iter().any(|argument| ["-select_streams", "-read_intervals"].contains(&argument.as_str())));
    assert!(arguments.windows(2).any(|pair| pair[0] == "-show_optional_fields" && pair[1] == "always"));
    assert!(arguments.windows(2).any(|pair| pair[0] == "-protocol_whitelist" && pair[1] == "pipe"));
    assert_eq!(arguments.last().map(String::as_str), Some("pipe:0"));
}

#[test]
fn malformed_oversized_missing_or_unexpected_sections_are_refused() {
    assert_eq!(parse_ffv1_observation(b"{").err(), Some(ProbeRefusal::InvalidJson));
    assert_eq!(parse_ffv1_observation(&vec![b' '; MAX_REPORT_BYTES + 1]).err(), Some(ProbeRefusal::ReportTooLarge));
    for missing in ["streams", "frames", "format", "chapters", "programs", "stream_groups"] {
        let mut value = fixture();
        value.as_object_mut().unwrap().remove(missing);
        assert_eq!(parse(&value).err(), Some(ProbeRefusal::InvalidShape), "{missing}");
    }
    for pointer in ["", "/format", "/streams/0", "/frames/0"] {
        let mut value = fixture();
        value.pointer_mut(pointer).unwrap().as_object_mut().unwrap().insert("unknown_feature".to_owned(), json!(true));
        assert_eq!(parse(&value).err(), Some(ProbeRefusal::UnexpectedField), "{pointer}");
    }
    let mut value = fixture();
    value["error"] = json!({"code": -1, "string": "synthetic failure"});
    assert_eq!(parse(&value).err(), Some(ProbeRefusal::ProviderReportedError));
}

#[test]
fn unobserved_or_contradictory_frame_timing_is_never_inferred_from_duration_text() {
    for (pointer, replacement, reason) in [
        ("/frames/1/pts", json!(41), ProbeRefusal::InvalidTiming),
        ("/frames/1/best_effort_timestamp", json!(80), ProbeRefusal::InvalidTiming),
        ("/frames/1/duration", json!(0), ProbeRefusal::InvalidTiming),
        ("/frames/1/duration", json!("N/A"), ProbeRefusal::InvalidShape),
        ("/streams/0/start_pts", json!("N/A"), ProbeRefusal::InvalidShape),
        ("/streams/0/nb_read_frames", json!("3"), ProbeRefusal::InvalidTiming),
        ("/streams/0/avg_frame_rate", json!("24/1"), ProbeRefusal::InvalidTiming),
        ("/streams/0/time_base", json!("1/0"), ProbeRefusal::InvalidShape),
        ("/streams/0/duration_ts", json!(2000), ProbeRefusal::InvalidTiming),
    ] {
        let mut value = fixture();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert_eq!(parse(&value).err(), Some(reason), "{pointer}");
    }
    let mut value = fixture();
    value["format"]["duration"] = json!("N/A");
    assert_eq!(parse(&value).unwrap().media.duration_ticks, 80);
    value["frames"][0].as_object_mut().unwrap().remove("pts");
    assert_eq!(parse(&value).err(), Some(ProbeRefusal::InvalidShape));
}

#[test]
fn extra_streams_programs_tags_and_side_data_are_not_silently_dropped() {
    for (pointer, replacement) in [
        ("/programs", json!([{}])), ("/stream_groups", json!([{}])),
        ("/chapters", json!([{}])),
        ("/format/tags", json!({"title": "synthetic title"})),
        ("/streams/0/tags", json!({"rotate": "90"})),
        ("/frames/0/tags", json!({"note": "synthetic"})),
        ("/frames/0/side_data_list", json!([{"side_data_type": "Display Matrix"}])),
        ("/streams/0/side_data_list", json!([{"side_data_type": "Mastering display metadata"}])),
    ] {
        let mut value = fixture();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        value.pointer_mut(parent).unwrap().as_object_mut().unwrap().insert(key.to_owned(), replacement);
        assert_eq!(parse(&value).err(), Some(ProbeRefusal::UnsupportedFeature), "{pointer}");
    }
    let mut value = fixture();
    let duplicate = value["streams"][0].clone();
    value["streams"].as_array_mut().unwrap().push(duplicate);
    assert_eq!(parse(&value).err(), Some(ProbeRefusal::UnsupportedSource));
    value = fixture();
    value["format"].as_object_mut().unwrap().remove("tags");
    value["streams"][0].as_object_mut().unwrap().remove("tags");
    assert!(parse(&value).is_ok());
}

#[test]
fn per_frame_color_geometry_and_crop_changes_are_refused() {
    for (pointer, replacement) in [
        ("/frames/1/width", json!(318)), ("/frames/1/crop_bottom", json!(2)),
        ("/frames/1/interlaced_frame", json!(1)), ("/frames/1/repeat_pict", json!(1)),
        ("/frames/1/sample_aspect_ratio", json!("2:1")),
        ("/frames/1/color_transfer", json!("smpte2084")),
        ("/frames/1/chroma_location", json!("left")),
    ] {
        let mut value = fixture();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert_eq!(parse(&value).err(), Some(ProbeRefusal::UnsupportedFeature), "{pointer}");
    }
    let mut value = fixture();
    value["streams"][0]["width"] = json!(641);
    assert_eq!(parse(&value).err(), Some(ProbeRefusal::UnsupportedSource));
    value = fixture();
    value["streams"][0]["codec_name"] = json!("h264");
    assert_eq!(parse(&value).err(), Some(ProbeRefusal::UnsupportedSource));
}

#[test]
fn every_disposition_must_be_observed_and_only_default_is_permitted() {
    let mut value = fixture();
    value["streams"][0]["disposition"]["default"] = json!(0);
    assert_eq!(parse(&value).unwrap().media.disposition, "none");
    value["streams"][0]["disposition"]["forced"] = json!(1);
    assert_eq!(parse(&value).err(), Some(ProbeRefusal::UnsupportedFeature));
    value = fixture();
    value["streams"][0]["disposition"].as_object_mut().unwrap().remove("multilayer");
    assert_eq!(parse(&value).err(), Some(ProbeRefusal::InvalidShape));
    value = fixture();
    value["streams"][0]["disposition"]["new_flag"] = json!(0);
    assert_eq!(parse(&value).err(), Some(ProbeRefusal::UnexpectedField));
}

#[test]
fn frame_count_and_decoded_sample_budgets_bound_report_work() {
    let mut value = fixture();
    let frame = value["frames"][0].clone();
    value["frames"] = json!(vec![frame; 251]);
    assert_eq!(parse(&value).err(), Some(ProbeRefusal::LimitExceeded));
    value = fixture();
    value["streams"][0]["width"] = json!(640);
    value["streams"][0]["coded_width"] = json!(640);
    value["streams"][0]["height"] = json!(360);
    value["streams"][0]["coded_height"] = json!(360);
    value["streams"][0]["nb_read_frames"] = json!("250");
    let base = value["frames"][0].clone();
    value["frames"] = Value::Array((0..250).map(|index| {
        let mut frame = base.clone();
        frame["width"] = json!(640);
        frame["height"] = json!(360);
        frame["pts"] = json!(index * 40);
        frame["best_effort_timestamp"] = json!(index * 40);
        frame
    }).collect());
    assert_eq!(parse(&value).err(), Some(ProbeRefusal::LimitExceeded));
}
