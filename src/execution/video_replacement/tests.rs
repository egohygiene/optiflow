use super::*;
use crate::filesystem::identity::FilesystemIdentity;
fn dir(path: &str) -> DirectoryBinding {
    DirectoryBinding {
        path: NativePath::from_path(Path::new(path)),
        identity: FilesystemIdentity {
            platform: "linux".into(),
            filesystem_id: "1".into(),
            file_id: "1".into(),
            link_count: None,
        },
        mode: 0o40700,
        uid: 1000,
        gid: 1000,
    }
}
fn file(path: &str, n: u64) -> FileBinding {
    FileBinding {
        directory: dir("/media"),
        path: NativePath::from_path(Path::new(path)),
        identity: FilesystemIdentity {
            platform: "linux".into(),
            filesystem_id: "1".into(),
            file_id: n.to_string(),
            link_count: Some(1),
        },
        size_bytes: n,
        modified_unix_ns: 0,
        changed_unix_ns: 0,
        mode: 0o100600,
        uid: 1000,
        gid: 1000,
        blake3: "a".repeat(64),
    }
}
fn review() -> StagedVideoReview {
    let mut r = StagedVideoReview {
        schema: "optiflow.staged-video-replacement-review.v1".into(),
        fingerprint: String::new(),
        operation: "stage_video_replacement.v1".into(),
        artifact_schema: "optiflow.video-candidate-artifact-set.v2".into(),
        source: file("/media/a.mkv", 100),
        candidate: file("/state/set/candidate.mkv", 80),
        candidate_set: NativePath::from_path(Path::new("/state/set")),
        marker_blake3: "a".repeat(64),
        evidence_blake3: "b".repeat(64),
        policy_fingerprint: "c".repeat(64),
        state: dir("/state"),
        retention: dir("/retained"),
        encoded_logical_reduction_bytes: 20,
        current_source_content_bound: true,
        independent_container_comparison: true,
        live_decode_observed: false,
        native_properties_preserved: false,
        replacement_qualified: false,
        apply_authority: false,
        original_disposal_authority: false,
    };
    r.fingerprint = hash(&r).unwrap();
    r
}
#[test]
fn current_stream_copy_bytes_do_not_create_live_decode_or_disposal_authority() {
    let r = review();
    r.validate().unwrap();
    assert!(admit_to_transaction(&r).is_err());
    for field in [
        "live_decode_observed",
        "native_properties_preserved",
        "apply_authority",
        "original_disposal_authority",
    ] {
        let mut raw = serde_json::to_value(&r).unwrap();
        raw[field] = serde_json::json!(true);
        assert!(StagedVideoReview::from_json_slice(&serde_json::to_vec(&raw).unwrap()).is_err());
    }
}
#[test]
fn video_review_cannot_be_loaded_as_a_png_replacement_plan() {
    let r = review();
    assert!(
        super::super::replacement::ReplacementPlan::from_json_slice(
            &serde_json::to_vec(&r).unwrap()
        )
        .is_err()
    );
}
#[test]
fn unknown_artifact_profile_does_not_upgrade_staged_review() {
    let mut r = review();
    r.artifact_schema = "optiflow.video-candidate-artifact-set.v3".into();
    r.fingerprint.clear();
    r.fingerprint = hash(&r).unwrap();
    assert!(r.validate().is_err());
}
#[test]
fn changed_retention_volume_invalidates_video_review() {
    let mut r = review();
    r.retention.identity.filesystem_id = "2".into();
    r.fingerprint.clear();
    r.fingerprint = hash(&r).unwrap();
    assert!(r.validate().is_err());
}

fn live_request() -> live::Request {
    let mut r = review();
    let mut p: crate::video_production::VideoProductionRequest = serde_json::from_str(
        include_str!("../../../examples/video-production-request-v2.json"),
    )
    .unwrap();
    p.source = r.source.path.to_path_buf();
    p.artifact_root = r
        .candidate_set
        .to_path_buf()
        .parent()
        .unwrap()
        .to_path_buf();
    r.policy_fingerprint = blake3::hash(
        &serde_json::to_vec(&(
            &p.sandbox,
            p.limits,
            crate::video_candidate::PROFILE_V2,
            "closed_ebml_stream_copy_only",
        ))
        .unwrap(),
    )
    .to_hex()
    .to_string();
    r.fingerprint.clear();
    r.fingerprint = hash(&r).unwrap();
    live::Request {
        schema: "optiflow.video-replacement-validation-request.v1".into(),
        review: r,
        production: p,
    }
}
#[test]
fn live_validation_refuses_another_source_before_opening_it() {
    let mut r = live_request();
    r.validate().unwrap();
    r.production.source = "/another/source.mkv".into();
    assert!(r.validate().is_err());
}
#[test]
fn live_validation_limits_cannot_widen_the_selected_video_profile() {
    let mut r = live_request();
    r.production.limits.input_bytes = 134_217_729;
    assert!(r.validate().is_err());
    r = live_request();
    r.production.schema = "optiflow.video-production-request.v1".into();
    assert!(r.validate().is_err());
}
#[test]
fn saved_staged_review_cannot_manufacture_a_live_validation_request() {
    let r = review();
    assert!(live::Request::from_json_slice(&serde_json::to_vec(&r).unwrap()).is_err());
}
#[test]
fn cancelled_live_validation_stops_before_subject_or_provider_io() {
    let r = live_request();
    let signals = SignalState::interrupted(crate::signals::Interruption::Interrupt);
    assert!(live::validate(&r, &signals).is_err());
}
#[test]
fn validation_cli_requires_explicit_request_and_output_and_has_no_apply_flag() {
    use clap::Parser;
    assert!(
        crate::cli::Cli::try_parse_from([
            "optiflow",
            "replacement",
            "validate-video",
            "--request",
            "/records/r.json"
        ])
        .is_err()
    );
    assert!(
        crate::cli::Cli::try_parse_from([
            "optiflow",
            "replacement",
            "validate-video",
            "--request",
            "/records/r.json",
            "--output",
            "/records/v.json",
            "--apply"
        ])
        .is_err()
    );
}

#[test]
fn video_plan_and_approval_are_separate_from_png_and_disposal() {
    let p = transaction::prepare(live_request(), 1024, 65536).unwrap();
    let mut a = transaction::approve(&p, "operator".into()).unwrap();
    assert!(
        super::super::replacement::ReplacementPlan::from_json_slice(
            &serde_json::to_vec(&p).unwrap()
        )
        .is_err()
    );
    a.authority = "discard_original".into();
    a.authorization_id.clear();
    a.authorization_id = hash(&a).unwrap();
    assert!(a.validate(&p).is_err());
}
#[test]
fn video_approval_cannot_follow_a_changed_profile_or_candidate() {
    let p = transaction::prepare(live_request(), 1024, 65536).unwrap();
    let a = transaction::approve(&p, "operator".into()).unwrap();
    let mut changed = p;
    changed.body.free_space_reserve_bytes += 1;
    changed.fingerprint = hash(&changed.body).unwrap();
    assert!(a.validate(&changed).is_err());
}
fn video_event() -> super::super::replacement::transaction::ReplacementEvent {
    super::super::replacement::transaction::ReplacementEvent {
        schema: "optiflow.replacement-event.v2".into(),
        plan_fingerprint: "a".repeat(64),
        authorization_id: "b".repeat(64),
        sequence: 0,
        phase: "planned".into(),
        original_disposal_authorization: None,
        video_action: Some(super::super::replacement::transaction::VideoAction {
            operation: "replace_validated_video_stream_copy.v1".into(),
            review_fingerprint: "c".repeat(64),
            validation_request_fingerprint: "d".repeat(64),
            current_validation_fingerprint: "e".repeat(64),
        }),
        media_validation: None,
        staged: None,
        retained: None,
        published: None,
    }
}
#[test]
fn video_journal_requires_current_validation_in_its_initial_event() {
    assert!(video_event().validate_after(&[]).is_err());
}
#[test]
fn legacy_png_event_cannot_smuggle_a_video_action() {
    let mut e = video_event();
    e.schema = "optiflow.replacement-event.v1".into();
    assert!(e.validate_after(&[]).is_err());
}
#[test]
fn video_mutation_retains_native_qualification_gate_before_provider_io() {
    let p = transaction::prepare(live_request(), 1024, 65536).unwrap();
    let a = transaction::approve(&p, "operator".into()).unwrap();
    assert!(
        transaction::apply(&p, &a, &SignalState::default())
            .unwrap_err()
            .to_string()
            .contains("qualification")
    );
}
#[test]
fn video_apply_cli_has_no_unqualified_bypass() {
    use clap::Parser;
    assert!(
        crate::cli::Cli::try_parse_from([
            "optiflow",
            "replacement",
            "apply-video",
            "--plan",
            "/p.json",
            "--approval",
            "/a.json",
            "--allow-unqualified"
        ])
        .is_err()
    );
}
