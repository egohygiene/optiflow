use optiflow::artifact_set::{
    ArtifactDigest, ArtifactSetCommitState, ArtifactSetKind, ArtifactSetManifest, ArtifactSetMember,
};
use optiflow::domain::{
    DuplicateGroup, DuplicateMember, ExactDuplicateEvidence, FileObservation, MediaKind,
    NativePath, ObservationStatus, PhysicalReclaimability, REPORT_SCHEMA_VERSION,
    ReclaimabilityReasonCode, ScanOptions, ScanReport, ScanRun, ScanSummary,
};
use optiflow::planning::exact_duplicate_plan;
use proptest::prelude::*;
use serde_json::Value;

fn native_path_strategy() -> impl Strategy<Value = NativePath> {
    prop_oneof![
        any::<String>().prop_map(|value| NativePath::Utf8 { value }),
        any::<String>().prop_map(|base64| NativePath::UnixBytes { base64 }),
    ]
}

fn manifest_strategy() -> impl Strategy<Value = ArtifactSetManifest> {
    (
        any::<String>(),
        any::<String>(),
        prop_oneof![Just(ArtifactSetKind::Scan), Just(ArtifactSetKind::Plan)],
        any::<String>(),
        any::<String>(),
        proptest::option::of(any::<String>()),
        prop::collection::vec(
            (
                any::<String>(),
                any::<String>(),
                native_path_strategy(),
                any::<u64>(),
                any::<String>(),
                any::<String>(),
            ),
            0..8,
        ),
    )
        .prop_map(
            |(schema, set_id, set_kind, run_id, created_at, source_set_id, members)| {
                ArtifactSetManifest {
                    schema,
                    set_id,
                    set_kind,
                    run_id,
                    created_at,
                    state: ArtifactSetCommitState::Committed,
                    source_set_id,
                    members: members
                        .into_iter()
                        .map(|(kind, schema, path, size_bytes, algorithm, value)| {
                            ArtifactSetMember {
                                kind,
                                schema,
                                path,
                                size_bytes,
                                digest: ArtifactDigest { algorithm, value },
                            }
                        })
                        .collect(),
                }
            },
        )
}

fn observation(path: &str) -> FileObservation {
    FileObservation {
        observation_id: path.to_owned(),
        run_id: "run".to_owned(),
        path: NativePath::Utf8 {
            value: path.to_owned(),
        },
        size_bytes: 100,
        modified_unix_ns: Some(1),
        device_id: None,
        inode: None,
        content_type: None,
        media_kind: MediaKind::Unknown,
        content_hash: Some("hash".to_owned()),
        hash_algorithm: Some("blake3-256".to_owned()),
        media: None,
        status: ObservationStatus::Unsupported,
        cache_hit: false,
        warnings: Vec::new(),
        filesystem_identity: None,
        storage_allocation: None,
        observation_stability: optiflow::domain::ObservationStability::Stable,
        evidence_validity: optiflow::domain::EvidenceValidity::Current,
        attempt_count: 1,
    }
}

fn report(paths: &[String], reversed: bool) -> ScanReport {
    let ordered: Vec<&String> = if reversed {
        paths.iter().rev().collect()
    } else {
        paths.iter().collect()
    };
    let observations = ordered
        .iter()
        .flat_map(|path| {
            [
                (*path).clone(),
                format!("{path}.alias-a"),
                format!("{path}.alias-z"),
            ]
        })
        .map(|path| observation(&path))
        .collect();
    let members = ordered
        .iter()
        .map(|path| DuplicateMember {
            path: NativePath::Utf8 {
                value: (*path).clone(),
            },
            observation_id: (*path).clone(),
            alias_paths: if reversed {
                vec![
                    NativePath::Utf8 {
                        value: format!("{path}.alias-z"),
                    },
                    NativePath::Utf8 {
                        value: format!("{path}.alias-a"),
                    },
                ]
            } else {
                vec![
                    NativePath::Utf8 {
                        value: format!("{path}.alias-a"),
                    },
                    NativePath::Utf8 {
                        value: format!("{path}.alias-z"),
                    },
                ]
            },
        })
        .collect();
    let member_count = u64::try_from(paths.len()).expect("bounded property input");
    let observed_path_count = member_count * 3;

    ScanReport {
        schema_version: REPORT_SCHEMA_VERSION.to_owned(),
        generated_at: "now".to_owned(),
        run: ScanRun {
            schema_version: "optiflow.run.v5".to_owned(),
            run_id: "run".to_owned(),
            artifact_set_id: Some("set".to_owned()),
            created_at: "now".to_owned(),
            completed_at: "now".to_owned(),
            inputs: Vec::new(),
            options: ScanOptions {
                follow_symlinks: false,
                include_hidden: false,
                cross_filesystems: false,
                probe_media: false,
            },
            artifact_directory: "/tmp".to_owned(),
            discovered_files: observed_path_count,
            analyzed_files: observed_path_count,
            cache_hits: 0,
            total_bytes: observed_path_count * 100,
            warnings: Vec::new(),
        },
        summary: ScanSummary {
            file_count: observed_path_count,
            total_bytes: observed_path_count * 100,
            media_files: 0,
            unsupported_files: observed_path_count,
            unreadable_files: 0,
            exact_duplicate_groups: 1,
            exact_duplicate_files: member_count,
            reclaimable_bytes: member_count.saturating_sub(1) * 100,
            cache_hits: 0,
            unique_object_count: member_count,
            hard_link_alias_path_count: observed_path_count - member_count,
            unstable_observation_count: 0,
        },
        duplicate_groups: vec![DuplicateGroup {
            group_id: "group".to_owned(),
            classification: "exact".to_owned(),
            evidence: ExactDuplicateEvidence {
                algorithm: "blake3-256".to_owned(),
                complete_content_hash: "hash".to_owned(),
                identical_size_bytes: 100,
                member_count,
                observed_path_count,
            },
            members,
            reclaimable_bytes: member_count.saturating_sub(1) * 100,
            physical_reclaimability: PhysicalReclaimability::unknown(vec![
                ReclaimabilityReasonCode::ExtentSharingUnknown,
            ]),
        }],
        observations,
        hard_link_groups: Vec::new(),
        storage: None,
        media_profile_evidence: Vec::new(),
    }
}

fn stable_plan_projection(report: &ScanReport) -> Value {
    let mut value = serde_json::to_value(exact_duplicate_plan(report)).expect("serialize plan");
    let object = value.as_object_mut().expect("plan object");
    object.remove("plan_id");
    object.remove("created_at");
    for action in object
        .get_mut("actions")
        .and_then(Value::as_array_mut)
        .expect("plan actions")
    {
        action
            .as_object_mut()
            .expect("plan action object")
            .remove("action_id");
    }
    value
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn native_path_json_round_trip_is_lossless(path in native_path_strategy()) {
        let encoded = serde_json::to_vec(&path).expect("serialize native path");
        let decoded: NativePath = serde_json::from_slice(&encoded).expect("deserialize native path");
        prop_assert_eq!(decoded, path);
    }

    #[test]
    fn hostile_utf8_path_display_contains_no_control_characters(value in any::<String>()) {
        let path = NativePath::Utf8 { value };
        prop_assert!(path.display().chars().all(|character| !character.is_control()));
    }

    #[test]
    fn artifact_manifest_json_round_trip_is_lossless(manifest in manifest_strategy()) {
        let encoded = serde_json::to_vec(&manifest).expect("serialize manifest");
        let decoded: ArtifactSetManifest = serde_json::from_slice(&encoded)
            .expect("deserialize manifest");
        prop_assert_eq!(decoded, manifest);
    }

    #[test]
    fn plan_decisions_ignore_report_member_and_alias_order(
        names in prop::collection::btree_set("[a-zA-Z0-9_]{1,12}", 2..9)
    ) {
        let paths: Vec<String> = names
            .into_iter()
            .map(|name| format!("/media/{name}"))
            .collect();
        let forward = report(&paths, false);
        let reverse = report(&paths, true);

        prop_assert_eq!(stable_plan_projection(&forward), stable_plan_projection(&reverse));
    }
}

#[cfg(unix)]
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn unix_path_bytes_round_trip_through_json(bytes in prop::collection::vec(any::<u8>(), 0..256)) {
        use std::ffi::OsString;
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let original = OsString::from_vec(bytes.clone());
        let path = NativePath::from_path(std::path::Path::new(&original));
        let encoded = serde_json::to_vec(&path).expect("serialize native path");
        let decoded: NativePath = serde_json::from_slice(&encoded).expect("deserialize native path");

        let round_trip = decoded.to_path_buf();
        prop_assert_eq!(round_trip.as_os_str().as_bytes(), bytes.as_slice());
    }
}
