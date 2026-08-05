use std::collections::HashMap;

use chrono::Utc;
use uuid::Uuid;

use crate::domain::{
    FileObservation, FilePrecondition, PLAN_SCHEMA_VERSION, Plan, PlanAction, PlanSafety,
    PlanSummary, ScanReport,
};

pub fn exact_duplicate_plan(report: &ScanReport) -> Plan {
    let observations: HashMap<&str, &FileObservation> = report
        .observations
        .iter()
        .map(|observation| (observation.path.as_str(), observation))
        .collect();
    let mut actions = Vec::new();

    for group in &report.duplicate_groups {
        let mut member_paths: Vec<String> = group
            .members
            .iter()
            .map(|member| member.path.clone())
            .collect();
        member_paths.sort();
        let Some(keep_path) = member_paths.first().cloned() else {
            continue;
        };
        let candidate_paths = member_paths.into_iter().skip(1).collect::<Vec<_>>();
        let preconditions = group
            .members
            .iter()
            .filter_map(|member| observations.get(member.path.as_str()))
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
            candidate_paths,
            potential_reclaimable_bytes: group.reclaimable_bytes,
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
        created_at: Utc::now().to_rfc3339(),
        mode: "exact_duplicates_review".to_owned(),
        safety: PlanSafety {
            mutates_files: false,
            requires_explicit_apply: true,
            description: "This v0.1 plan is evidence for review only. OptiFlow v0.1 has no apply or deletion command.".to_owned(),
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
        DuplicateGroup, DuplicateMember, ExactDuplicateEvidence, MediaKind, ObservationStatus,
        REPORT_SCHEMA_VERSION, ScanOptions, ScanRun, ScanSummary,
    };

    use super::*;

    #[test]
    fn plan_is_read_only_and_deterministic_about_default_keep_path() {
        let observation = |path: &str| FileObservation {
            observation_id: path.to_owned(),
            run_id: "run".to_owned(),
            path: path.to_owned(),
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
        };
        let observations = vec![observation("/z"), observation("/a")];
        let report = ScanReport {
            schema_version: REPORT_SCHEMA_VERSION.to_owned(),
            generated_at: "now".to_owned(),
            run: ScanRun {
                schema_version: "optiflow.run.v1".to_owned(),
                run_id: "run".to_owned(),
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
            },
            duplicate_groups: vec![DuplicateGroup {
                group_id: "group".to_owned(),
                classification: "exact".to_owned(),
                evidence: ExactDuplicateEvidence {
                    algorithm: "blake3-256".to_owned(),
                    complete_content_hash: "hash".to_owned(),
                    identical_size_bytes: 100,
                    member_count: 2,
                },
                members: vec![
                    DuplicateMember {
                        path: "/z".to_owned(),
                        observation_id: "/z".to_owned(),
                    },
                    DuplicateMember {
                        path: "/a".to_owned(),
                        observation_id: "/a".to_owned(),
                    },
                ],
                reclaimable_bytes: 100,
            }],
            observations,
        };

        let plan = exact_duplicate_plan(&report);
        assert!(!plan.safety.mutates_files);
        assert_eq!(plan.actions[0].keep_path, "/a");
        assert_eq!(plan.actions[0].candidate_paths, vec!["/z"]);
    }
}
