//! Comparison of reconciled retained technical outcomes, never whole-volume
//! reclamation, human quality approval or downstream original selection.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Complete,
    Cancelled,
    ElapsedWindow,
    CumulativeActionLimit,
    CumulativePublicationLimit,
    ReconciliationRequired,
    NotRecorded,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WindowReport {
    pub schema: String,
    pub record: Report,
    pub stop_reason: StopReason,
    pub window_elapsed_ms: u64,
}
pub(super) struct Historical {
    pub record: Report,
    pub stop_reason: StopReason,
    pub window_elapsed_ms: Option<u64>,
}
pub(super) fn read_report(bound: &File, name: &str) -> Result<Historical> {
    let bytes = crate::candidate_artifact::read_member(bound, name, JOURNAL_BYTES as usize)?;
    let raw: serde_json::Value = serde_json::from_slice(&bytes)?;
    match raw["schema"].as_str() {
        Some("optiflow.mp4-folder-report.v1") => Ok(Historical {
            record: serde_json::from_slice(&bytes)?,
            stop_reason: StopReason::NotRecorded,
            window_elapsed_ms: None,
        }),
        Some("optiflow.mp4-folder-report.v2") => {
            let value: WindowReport = serde_json::from_slice(&bytes)?;
            ensure!(
                value.schema == "optiflow.mp4-folder-report.v2"
                    && value.stop_reason != StopReason::NotRecorded,
                "new folder report must record its actual stop cause"
            );
            Ok(Historical {
                record: value.record,
                stop_reason: value.stop_reason,
                window_elapsed_ms: Some(value.window_elapsed_ms),
            })
        }
        _ => anyhow::bail!("unsupported MP4 folder report schema"),
    }
}
pub(super) fn validate_stop(
    value: &Historical,
    request: &Request,
    inventory: &FolderInventory,
) -> Result<()> {
    let r = &value.record;
    let next = inventory.entries.get(r.result_count);
    let eligible = next.is_some_and(|e| e.disposition == EntryDisposition::Eligible);
    let valid = match value.stop_reason {
        StopReason::NotRecorded => value.window_elapsed_ms.is_none(),
        StopReason::Complete => r.state == "retained_complete" && next.is_none(),
        StopReason::Cancelled => r.state == "never_started_work_remains" && next.is_some(),
        StopReason::ElapsedWindow => {
            r.state == "never_started_work_remains"
                && next.is_some()
                && value
                    .window_elapsed_ms
                    .is_some_and(|elapsed| elapsed >= request.batch.elapsed_ms)
        }
        StopReason::ReconciliationRequired => r.state == "reconciliation_required",
        StopReason::CumulativeActionLimit => {
            eligible
                && r.state == "never_started_work_remains"
                && r.actions_started >= request.batch.maximum_actions
        }
        StopReason::CumulativePublicationLimit => {
            eligible
                && r.state == "never_started_work_remains"
                && r.actions_started < request.batch.maximum_actions
                && admission(
                    request,
                    r.actions_started
                        .checked_add(1)
                        .context("report action overflow")?,
                )? > request.batch.maximum_publication_bytes
        }
    };
    ensure!(
        valid,
        "MP4 report stop cause disagrees with validated prefix/budget witness"
    );
    Ok(())
}
#[derive(Debug, Default, Serialize)]
pub struct Counts {
    pub discovered_entries: u64,
    pub files_seen: u64,
    pub inspected_files: u64,
    pub eligible_files: u64,
    pub optimized_files: u64,
    pub unchanged_files: u64,
    pub unsupported_entries: u64,
    pub skipped_entries: u64,
    pub failed_files: u64,
    pub recorded_results: u64,
    pub pending_entries: u64,
    pub uncertain_attempts: u64,
}
fn counts(inventory: &FolderInventory, results: &[Record], unfinished: &[usize]) -> Counts {
    let mut value = Counts {
        discovered_entries: inventory.entries_seen,
        files_seen: inventory.files_seen,
        inspected_files: inventory
            .entries
            .iter()
            .filter(|e| e.source.is_some())
            .count() as u64,
        eligible_files: inventory
            .entries
            .iter()
            .filter(|e| e.disposition == EntryDisposition::Eligible)
            .count() as u64,
        recorded_results: results.len() as u64,
        pending_entries: inventory.entries.len().saturating_sub(results.len()) as u64,
        uncertain_attempts: unfinished.len() as u64,
        ..Default::default()
    };
    for r in results {
        match r.status.as_str() {
            "accepted" => value.optimized_files += 1,
            "unchanged" => value.unchanged_files += 1,
            "unsupported" => value.unsupported_entries += 1,
            "skipped" => value.skipped_entries += 1,
            "failed" => value.failed_files += 1,
            _ => {}
        }
    }
    value
}
#[derive(Debug, Serialize)]
pub struct Entry {
    pub index: usize,
    pub relative_path: crate::domain::NativePath,
    pub status: String,
    pub reason: String,
    pub observed_source_bytes: Option<u64>,
    pub candidate_bytes: Option<u64>,
    pub encoded_bytes_saved: u64,
    pub candidate_scope: String,
    pub artifact: Option<artifact::Pin>,
}
#[derive(Debug, Serialize)]
pub struct Reduction {
    pub saved_bytes: u64,
    pub accepted_source_bytes: u64,
}
#[derive(Debug, Serialize)]
pub struct LastReport {
    pub sequence: usize,
    pub result_count: usize,
    pub stop_reason: StopReason,
    pub window_elapsed_ms: Option<u64>,
}
#[derive(Debug, Serialize)]
pub struct Comparison {
    pub schema: String,
    pub request_digest: EvidenceDigest,
    pub inventory_digest: EvidenceDigest,
    pub state: String,
    pub counts: Counts,
    pub entries: Vec<Entry>,
    pub accepted_source_bytes: u64,
    pub accepted_candidate_bytes: u64,
    pub encoded_bytes_saved: u64,
    pub reduction: Option<Reduction>,
    pub last_report: Option<LastReport>,
    pub continuation: String,
    pub source_freshness: String,
    pub savings_scope: String,
    pub quality_review: String,
    pub native_qualified: bool,
    pub physical_space_reclaimed: bool,
    pub replacement_authority: bool,
    pub upload_authority: bool,
}
pub(super) fn continuation(b: &Basis) -> Result<&'static str> {
    let current = summary(b)?;
    if current.state == "reconciliation_required" {
        return Ok("manual_reconciliation_required");
    }
    if current.state == "retained_complete" {
        return Ok("complete");
    }
    if b.reports.len() >= 64 {
        return Ok("report_history_limit");
    }
    if b.inventory
        .entries
        .get(b.results.len())
        .is_some_and(|e| e.disposition == EntryDisposition::Eligible)
    {
        if current.actions_started >= b.request.batch.maximum_actions {
            return Ok("cumulative_action_limit");
        }
        if admission(&b.request, current.actions_started + 1)?
            > b.request.batch.maximum_publication_bytes
        {
            return Ok("cumulative_publication_limit");
        }
    }
    Ok("explicit_resume_of_never_started_work")
}
fn project(b: &Basis) -> Result<Comparison> {
    let current = summary(b)?;
    let mut entries = Vec::with_capacity(b.inventory.entries.len());
    let mut source_total = 0u64;
    let mut candidate_total = 0u64;
    for (index, entry) in b.inventory.entries.iter().enumerate() {
        let record = b.results.get(index);
        let started = b.starts.iter().any(|s| s.index == index);
        let mut value = Entry {
            index,
            relative_path: entry.relative_path.clone(),
            status: record.map(|r| r.status.clone()).unwrap_or_else(|| {
                if started {
                    "uncertain_started"
                } else {
                    "not_started"
                }
                .into()
            }),
            reason: record.map(|r| r.reason.clone()).unwrap_or_else(|| {
                if started {
                    "retained start has no confirmed result; no replay"
                } else {
                    "no action/result recorded"
                }
                .into()
            }),
            observed_source_bytes: entry.source.as_ref().map(|s| s.snapshot.logical_bytes),
            candidate_bytes: None,
            encoded_bytes_saved: 0,
            candidate_scope: "not_observed_by_this_record".into(),
            artifact: None,
        };
        if let Some(outcome) = record.and_then(|r| r.outcome.as_ref()) {
            match outcome {
                Outcome::Accepted {
                    artifact,
                    source_bytes,
                    candidate_bytes,
                    encoded_bytes_saved,
                    ..
                } => {
                    source_total = source_total
                        .checked_add(*source_bytes)
                        .context("comparison source sum overflow")?;
                    candidate_total = candidate_total
                        .checked_add(*candidate_bytes)
                        .context("comparison candidate sum overflow")?;
                    value.candidate_bytes = Some(*candidate_bytes);
                    value.encoded_bytes_saved = *encoded_bytes_saved;
                    value.candidate_scope =
                        "current_pinned_private_artifact_with_retained_technical_validation".into();
                    value.artifact = Some(artifact.clone());
                }
                Outcome::Unchanged {
                    reason,
                    candidate_bytes,
                    ..
                } => {
                    value.reason = reason.clone();
                    value.candidate_bytes = *candidate_bytes;
                    value.candidate_scope=if candidate_bytes.is_some(){"retained_nonshrinking_candidate_measurement; generated_duplicate_may_be_removed"}else{"encoding_not_performed"}.into();
                }
            }
        }
        entries.push(value);
    }
    ensure!(
        source_total.checked_sub(candidate_total) == Some(current.encoded_bytes_saved),
        "accepted subset byte comparison differs from reconciled savings"
    );
    let result=Comparison {schema:"optiflow.mp4-folder-comparison.v1".into(),request_digest:current.request_digest,inventory_digest:current.inventory_digest,state:current.state,counts:counts(&b.inventory,&b.results,&b.unfinished),entries,accepted_source_bytes:source_total,accepted_candidate_bytes:candidate_total,encoded_bytes_saved:current.encoded_bytes_saved,reduction:if source_total>0{Some(Reduction {saved_bytes:current.encoded_bytes_saved,accepted_source_bytes:source_total})}else{None},last_report:b.reports.last().map(|r|LastReport {sequence:r.record.sequence,result_count:r.record.result_count,stop_reason:r.stop_reason,window_elapsed_ms:r.window_elapsed_ms}),continuation:continuation(b)?.into(),source_freshness:current.source_freshness,savings_scope:"logical encoded bytes across accepted technical candidates only; originals remain; no whole-collection, allocated/shared-storage or free-space reduction claim".into(),quality_review:current.quality_review,native_qualified:false,physical_space_reclaimed:false,replacement_authority:false,upload_authority:false};
    ensure!(
        serde_json::to_vec(&result)?.len() <= JOURNAL_BYTES as usize,
        "MP4 comparison view exceeds bound"
    );
    Ok(result)
}
pub fn comparison(root: &Path) -> Result<Comparison> {
    let bound = locked(root)?;
    project(&reconcile(root, &bound)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_outcome_counts_keep_unsupported_unchanged_and_pending_distinct() {
        let entry = |name: &str, disposition| crate::video_folder::FolderEntry {
            relative_path: crate::domain::NativePath::from_path(Path::new(name)),
            disposition,
            reason: "synthetic fixture".into(),
            source: None,
        };
        let inventory = FolderInventory {
            schema: "optiflow.mp4-folder-inventory.v1".into(),
            components: None,
            request_fingerprint: hash(b"fixture"),
            entries_seen: 3,
            files_seen: 3,
            hashed_input_bytes: 0,
            scope: "synthetic".into(),
            entries: vec![
                entry("first.mp4", EntryDisposition::Eligible),
                entry("keep.txt", EntryDisposition::Unsupported),
                entry("last.mp4", EntryDisposition::Eligible),
            ],
        };
        let records = vec![
            Record {
                schema: "optiflow.mp4-folder-result.v1".into(),
                index: 0,
                status: "unchanged".into(),
                reason: "already fits".into(),
                outcome: Some(Outcome::Unchanged {
                    reason: "already fits".into(),
                    source_bytes: 10,
                    candidate_bytes: None,
                    candidate_digest: None,
                    attempt_directory: None,
                }),
            },
            Record {
                schema: "optiflow.mp4-folder-result.v1".into(),
                index: 1,
                status: "unsupported".into(),
                reason: "unsupported".into(),
                outcome: None,
            },
        ];
        let value = counts(&inventory, &records, &[]);
        assert_eq!(value.optimized_files, 0);
        assert_eq!(value.unchanged_files, 1);
        assert_eq!(value.unsupported_entries, 1);
        assert_eq!(value.pending_entries, 1);
        assert_eq!(value.recorded_results, 2);
        assert_eq!(value.eligible_files, 2);
    }
    #[cfg(unix)]
    #[test]
    fn legacy_report_stop_causes_are_not_inferred_or_relabelled_as_new_evidence() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let bound = crate::candidate_artifact::open_directory(&root).unwrap();
        let record=Report {schema:"optiflow.mp4-folder-report.v1".into(),sequence:0,request_digest:hash(b"request"),inventory_digest:hash(b"inventory"),results_digest:hash(b"results"),result_count:0,actions_started:0,encoded_bytes_saved:0,publication_admission_bytes:0,state:"never_started_work_remains".into(),elapsed_policy:"new_explicit_bounded_window_for_never_started_work; cumulative_actions_and_publication_caps".into(),physical_space_reclaimed:false,replacement_authority:false};
        runtime::write_member(&bound, "legacy.json", &serde_json::to_vec(&record).unwrap())
            .unwrap();
        let legacy = read_report(&bound, "legacy.json").unwrap();
        assert_eq!(legacy.stop_reason, StopReason::NotRecorded);
        assert!(legacy.window_elapsed_ms.is_none());
        let forged = WindowReport {
            schema: "optiflow.mp4-folder-report.v2".into(),
            record,
            stop_reason: StopReason::NotRecorded,
            window_elapsed_ms: 0,
        };
        runtime::write_member(&bound, "new.json", &serde_json::to_vec(&forged).unwrap()).unwrap();
        assert!(read_report(&bound, "new.json").is_err());
    }
}
