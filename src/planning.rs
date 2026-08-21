use chrono::Utc;
use uuid::Uuid;

use crate::domain::{
    FileObservation, FilePrecondition, NativePath, PLAN_SCHEMA_VERSION, Plan, PlanAction,
    PlanSafety, PlanSummary, ScanReport,
};

pub fn exact_duplicate_plan(report: &ScanReport) -> Plan {
    let observations: std::collections::HashMap<String, &FileObservation> = report
        .observations
        .iter()
        .map(|observation| (observation.path.sqlite_key().into_owned(), observation))
        .collect();
    let mut actions = Vec::new();

    for group in &report.duplicate_groups {
        // Collect all paths: primary + aliases for every member, sorted.
        let mut all_member_paths: Vec<(NativePath, Vec<NativePath>)> = group
            .members
            .iter()
            .map(|member| (member.path.clone(), member.alias_paths.clone()))
            .collect();
        all_member_paths.sort_by(|(a, _), (b, _)| a.cmp(b));

        let Some((keep_path, keep_aliases)) = all_member_paths.first().cloned() else {
            continue;
        };

        // Candidate paths = all paths from non-keeper members.
        let candidate_paths: Vec<NativePath> = all_member_paths
            .iter()
            .skip(1)
            .flat_map(|(primary, aliases)| {
                let mut paths = vec![primary.clone()];
                paths.extend_from_slice(aliases);
                paths
            })
            .collect();

        // Preconditions for all observed paths (primary + aliases of all members).
        let preconditions = group
            .members
            .iter()
            .flat_map(|member| {
                let mut paths = vec![member.path.clone()];
                paths.extend_from_slice(&member.alias_paths);
                paths
            })
            .filter_map(|path| observations.get(path.sqlite_key().as_ref()))
            .map(|observation| FilePrecondition {
                path: observation.path.clone(),
                expected_size_bytes: observation.size_bytes,
                expected_modified_unix_ns: observation.modified_unix_ns,
                expected_complete_content_hash: group.evidence.complete_content_hash.clone(),
                required_apply_behavior: "re-stat, recalculate the complete BLAKE3 hash, and perform byte-for-byte confirmation before any mutation".to_owned(),
            })
            .collect();

        actions.push(PlanAction {
            action_id: Uuid::now_v7().to_string(),
            classification: "exact".to_owned(),
            proposed_operation: "review_and_select".to_owned(),
            keep_path,
            keep_alias_paths: keep_aliases,
            candidate_paths,
            potential_reclaimable_bytes: group.reclaimable_bytes,
            physical_reclaimability: group.physical_reclaimability.clone(),
            reason: "members have identical byte length and complete BLAKE3 content hashes; the lexicographically first path is only a deterministic review default, not a quality judgment".to_owned(),
            evidence: group.evidence.clone(),
            preconditions,
        });
    }

    let candidate_file_count = actions
        .iter()
        .map(|action| u64::try_from(action.candidate_paths.len()).unwrap_or(u64::MAX))
        .sum();
    let potential_reclaimable_bytes = actions
        .iter()
        .map(|action| action.potential_reclaimable_bytes)
        .sum();

    Plan {
        schema_version: PLAN_SCHEMA_VERSION.to_owned(),
        plan_id: Uuid::now_v7().to_string(),
        source_run_id: report.run.run_id.clone(),
        source_artifact_set_id: report.run.artifact_set_id.clone(),
        created_at: Utc::now().to_rfc3339(),
        mode: "exact_duplicates_review".to_owned(),
        safety: PlanSafety {
            mutates_files: false,
            requires_explicit_apply: true,
            description: "This v0.1 plan is evidence for review only. optiflow v0.1 has no apply or deletion command.".to_owned(),
        },
        summary: PlanSummary {
            action_count: u64::try_from(actions.len()).unwrap_or(u64::MAX),
            candidate_file_count,
            potential_reclaimable_bytes,
        },
        actions,
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        DuplicateGroup, DuplicateMember, ExactDuplicateEvidence, MediaKind, NativePath,
        ObservationStatus, PhysicalReclaimability, REPORT_SCHEMA_VERSION, ReclaimabilityReasonCode,
        ReclaimabilityStatus, ScanOptions, ScanRun, ScanSummary,
    };

    use super::*;

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
            observation_stability: crate::domain::ObservationStability::Stable,
            evidence_validity: crate::domain::EvidenceValidity::Current,
            attempt_count: 1,
        }
    }

    fn make_report(observations: Vec<FileObservation>, groups: Vec<DuplicateGroup>) -> ScanReport {
        ScanReport {
            schema_version: REPORT_SCHEMA_VERSION.to_owned(),
            generated_at: "now".to_owned(),
            run: ScanRun {
                schema_version: "optiflow.run.v3".to_owned(),
                run_id: "run".to_owned(),
                artifact_set_id: None,
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
                discovered_files: 2,
                analyzed_files: 2,
                cache_hits: 0,
                total_bytes: 200,
                warnings: Vec::new(),
            },
            summary: ScanSummary {
                file_count: 2,
                total_bytes: 200,
                media_files: 0,
                unsupported_files: 2,
                unreadable_files: 0,
                exact_duplicate_groups: 1,
                exact_duplicate_files: 2,
                reclaimable_bytes: 100,
                cache_hits: 0,
                unique_object_count: 2,
                hard_link_alias_path_count: 0,
                unstable_observation_count: 0,
            },
            duplicate_groups: groups,
            observations,
            hard_link_groups: Vec::new(),
            storage: None,
        }
    }

    #[test]
    fn plan_is_read_only_and_deterministic_about_default_keep_path() {
        let observations = vec![observation("/z"), observation("/a")];
        let report = make_report(
            observations,
            vec![DuplicateGroup {
                group_id: "group".to_owned(),
                classification: "exact".to_owned(),
                evidence: ExactDuplicateEvidence {
                    algorithm: "blake3-256".to_owned(),
                    complete_content_hash: "hash".to_owned(),
                    identical_size_bytes: 100,
                    member_count: 2,
                    observed_path_count: 2,
                },
                members: vec![
                    DuplicateMember {
                        path: NativePath::Utf8 {
                            value: "/z".to_owned(),
                        },
                        observation_id: "/z".to_owned(),
                        alias_paths: Vec::new(),
                    },
                    DuplicateMember {
                        path: NativePath::Utf8 {
                            value: "/a".to_owned(),
                        },
                        observation_id: "/a".to_owned(),
                        alias_paths: Vec::new(),
                    },
                ],
                reclaimable_bytes: 100,
                physical_reclaimability: PhysicalReclaimability {
                    status: ReclaimabilityStatus::Unknown,
                    reason_codes: vec![ReclaimabilityReasonCode::ExtentSharingUnknown],
                },
            }],
        );

        let plan = exact_duplicate_plan(&report);
        assert!(!plan.safety.mutates_files);
        assert_eq!(
            plan.actions[0].keep_path,
            NativePath::Utf8 {
                value: "/a".to_owned()
            }
        );
        assert_eq!(
            plan.actions[0].candidate_paths,
            vec![NativePath::Utf8 {
                value: "/z".to_owned()
            }]
        );
    }
}
