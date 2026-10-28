//! Supplied synthetic declarations only; no media, provider or filesystem IO.

use optiflow::video_candidate::{
    PlanRefusal, PreparedForReview, parse_request, prepare_candidate, request_fingerprint,
};
use serde_json::{Value, json};

fn example() -> Value {
    serde_json::from_str(include_str!("../examples/video-candidate-request-v1.json"))
        .expect("synthetic request fixture")
}

fn prepare(value: &Value) -> Result<PreparedForReview, PlanRefusal> {
    prepare_candidate(&parse_request(value)?)
}

fn has_pair(plan: &PreparedForReview, flag: &str, value: &str) -> bool {
    plan.arguments.windows(2).any(|pair| pair[0] == flag && pair[1] == value)
}

#[test]
fn supplied_declarations_produce_only_a_review_recipe() {
    let value = example();
    let request = parse_request(&value).unwrap();
    assert_eq!(serde_json::to_value(&request).unwrap(), value);
    let plan = prepare_candidate(&request).unwrap();
    assert_eq!(plan.status, "prepared_for_review_only");
    assert_eq!(plan.request_fingerprint, request_fingerprint(&request));
    assert_ne!(plan.preparation_fingerprint, plan.request_fingerprint);
    assert!(plan.preservation_claim.contains("not_verified"));
    assert!(plan.outstanding_validation.contains(&"elementary_payload_and_timestamp_preservation"));
    assert!(plan.outstanding_validation.contains(&"host_enforcement_of_requested_resource_limits"));
    assert!(plan.outstanding_validation.contains(&"versioned_video_artifact_publication_and_recovery"));
    assert!(!plan.arguments.iter().any(|arg| arg.contains("/synthetic/")));
    assert!(has_pair(&plan, "-protocol_whitelist", "pipe"));
    assert!(has_pair(&plan, "-i", "pipe:0"));
    assert!(has_pair(&plan, "-map", "0:0"));
    assert!(has_pair(&plan, "-c:v", "copy"));
    assert!(has_pair(&plan, "-disposition:v:0", "default"));
    assert!(has_pair(&plan, "-avoid_negative_ts", "disabled"));
    assert_eq!(plan.arguments.last().map(String::as_str), Some("pipe:1"));
    assert!(!plan.arguments.iter().any(|arg| ["-an", "-sn", "-dn", "-y"].contains(&arg.as_str())));
    // The sample's invented provider paths need not exist: this API does no IO.
    assert_eq!(prepare(&value).unwrap().preparation_fingerprint, plan.preparation_fingerprint);
}

#[test]
fn lossy_plan_requires_both_settings_and_never_claims_exactness() {
    let mut value = example();
    value["operation"] = json!({"kind": "h264_lossy", "crf": 23, "preset": "medium"});
    let plan = prepare(&value).unwrap();
    assert!(has_pair(&plan, "-c:v", "libx264"));
    assert!(has_pair(&plan, "-crf", "23"));
    assert!(has_pair(&plan, "-preset", "medium"));
    assert!(has_pair(&plan, "-pix_fmt", "+yuv420p"));
    assert!(has_pair(&plan, "-fps_mode:v", "passthrough"));
    assert!(plan.preservation_claim.contains("no_exact_equivalence"));
    assert!(plan.outstanding_validation.contains(&"declared_lossy_evidence_and_operator_quality_review"));
    for operation in [
        json!({"kind": "h264_lossy"}),
        json!({"kind": "h264_lossy", "crf": 23}),
        json!({"kind": "h264_lossy", "preset": "medium"}),
        json!({"kind": "h264_lossy", "crf": 0, "preset": "medium"}),
        json!({"kind": "h264_lossy", "crf": 52, "preset": "medium"}),
        json!({"kind": "h264_lossy", "crf": 23, "preset": "medium -y"}),
        json!({"kind": "stream_copy", "crf": 23}),
    ] {
        value["operation"] = operation;
        assert_eq!(prepare(&value).err(), Some(PlanRefusal::InvalidShape));
    }
}

#[test]
fn missing_unknown_and_malformed_nested_fields_cannot_be_normalized_away() {
    for pointer in ["", "/source", "/source/media", "/policy", "/producer",
        "/producer/executable", "/producer/binary_digest", "/source/media/frame_rate",
        "/operation", "/limits"] {
        let mut value = example();
        value.pointer_mut(pointer).unwrap().as_object_mut().unwrap()
            .insert("unexpected".to_owned(), json!(true));
        assert_eq!(prepare(&value).err(), Some(PlanRefusal::InvalidShape), "{pointer}");
    }
    for (pointer, key) in [
        ("", "operation"), ("", "probe_build_fingerprint"),
        ("/source/media", "feature_observation_complete"),
        ("/source/media", "disposition"), ("/policy", "metadata"),
    ] {
        let mut value = example();
        value.pointer_mut(pointer).unwrap().as_object_mut().unwrap().remove(key);
        assert_eq!(prepare(&value).err(), Some(PlanRefusal::InvalidShape));
    }
    for (pointer, replacement) in [
        ("/source/media/frame_rate/denominator", json!(0)),
        ("/source/media/color_transfer", Value::Null),
        ("/source/content_digest/value", json!("not-a-content-digest")),
        ("/policy/source_preserving", json!(false)),
        ("/policy/require_strictly_smaller", json!(false)),
        ("/policy/metadata", json!("strip_all")),
    ] {
        let mut value = example();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert_eq!(prepare(&value).err(), Some(PlanRefusal::InvalidShape), "{pointer}");
    }
}

#[test]
fn absent_or_unsupported_media_features_refuse_before_a_recipe_exists() {
    let mut value = example();
    value["source"]["media"]["feature_observation_complete"] = json!(false);
    assert_eq!(prepare(&value).err(), Some(PlanRefusal::IncompleteObservation));
    for (field, replacement) in [
        ("container", json!("mp4")), ("codec", json!("h264")),
        ("total_streams", json!(2)), ("stream_index", json!(1)),
        ("width", json!(319)), ("pixel_format", json!("yuv420p10le")),
        ("field_order", json!("tt")), ("color_transfer", json!("smpte2084")),
        ("rotation_degrees", json!(90)), ("chapters", json!(1)),
        ("side_data_entries", json!(1)), ("descriptive_metadata_entries", json!(1)),
        ("unknown_metadata_entries", json!(1)), ("disposition", json!("attached_pic")),
    ] {
        let mut value = example();
        value["source"]["media"][field] = replacement;
        assert_eq!(prepare(&value).err(), Some(PlanRefusal::UnsupportedSource), "{field}");
    }
}

#[test]
fn exact_declared_timing_and_disposition_are_preserved_in_intent() {
    for (field, replacement) in [
        ("constant_frame_rate", json!(false)), ("start_pts", json!(1)),
        ("duration_ticks", json!(1999)),
        ("frame_rate", json!({"numerator": 24, "denominator": 1})),
        ("time_base", json!({"numerator": 1, "denominator": 90000})),
    ] {
        let mut value = example();
        value["source"]["media"][field] = replacement;
        assert_eq!(prepare(&value).err(), Some(PlanRefusal::InvalidTiming), "{field}");
    }
    let mut value = example();
    value["source"]["media"]["disposition"] = json!("none");
    assert!(has_pair(&prepare(&value).unwrap(), "-disposition:v:0", "0"));
}

#[test]
fn provider_declarations_require_exact_names_version_and_normalized_absolute_paths() {
    for (pointer, replacement) in [
        ("/producer/name", json!("wrapper")), ("/probe/name", json!("ffmpeg")),
        ("/producer/version", json!("8.0.2")), ("/probe/version", json!("8.0")),
        ("/producer/executable/value", json!("ffmpeg")),
        ("/producer/executable/value", json!("/tools/../ffmpeg")),
        ("/probe/executable/value", json!("/tools//ffprobe")),
        ("/probe/executable/value", json!("/tools/ffprobe\u{0}")),
    ] {
        let mut value = example();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert_eq!(prepare(&value).err(), Some(PlanRefusal::ProviderMismatch), "{pointer}");
    }
}

#[test]
fn requested_limits_are_consistent_but_are_not_claimed_enforced() {
    for (field, replacement) in [
        ("input_bytes", json!(10)), ("decoded_bytes", json!(100)),
        ("temporary_bytes", json!(100)), ("memory_bytes", json!(100)),
    ] {
        let mut value = example();
        value["limits"][field] = replacement;
        assert_eq!(prepare(&value).err(), Some(PlanRefusal::LimitExceeded), "{field}");
    }
    let mut value = example();
    value["limits"]["elapsed_ms"] = json!(30001);
    assert_eq!(prepare(&value).err(), Some(PlanRefusal::InvalidShape));
    // Peak streaming memory need not equal all decoded frames added together.
    value = example();
    value["limits"]["memory_bytes"] = json!(1048576);
    assert!(prepare(&value).is_ok());
}

#[test]
fn fingerprints_bind_source_policy_provider_build_and_recipe_choices() {
    let original = prepare(&example()).unwrap();
    for (pointer, replacement) in [
        ("/source/observation_id", json!("another-synthetic-observation")),
        ("/source/content_digest/value", json!("b".repeat(64))),
        ("/source/probe_observation_fingerprint/value", json!("b".repeat(64))),
        ("/policy/effective_policy_fingerprint/value", json!("b".repeat(64))),
        ("/producer/binary_digest/value", json!("b".repeat(64))),
        ("/producer_build_fingerprint/value", json!("b".repeat(64))),
        ("/probe/invocation_fingerprint/value", json!("b".repeat(64))),
        ("/probe_build_fingerprint/value", json!("b".repeat(64))),
        ("/limits/elapsed_ms", json!(20000)),
        ("/operation", json!({"kind": "h264_lossy", "crf": 23, "preset": "slow"})),
    ] {
        let mut value = example();
        *value.pointer_mut(pointer).unwrap() = replacement;
        let changed = prepare(&value).unwrap();
        assert_ne!(changed.request_fingerprint, original.request_fingerprint, "{pointer}");
        assert_ne!(changed.preparation_fingerprint, original.preparation_fingerprint, "{pointer}");
    }
}
