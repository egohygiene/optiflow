use std::collections::HashMap;
use std::path::Path;

use chrono::Utc;
use uuid::Uuid;

use crate::adapters::ffprobe;
use crate::artifact_set::{
    self, ARTIFACT_SET_SCHEMA, ArtifactPayload, ArtifactSetManifest, ArtifactSetStatus,
};
use crate::cli::ScanArgs;
use crate::configuration::EffectivePolicyV1;
use crate::contracts::{self, Contract};
use crate::discovery::{DiscoveryIssue, DiscoveryIssueKind, discover};
use crate::domain::{
    CachedAnalysis, EvidenceValidity, FileObservation, HardLinkGroup, MediaKind,
    MediaProfileCoverageStatus, NativePath, ObservationStatus, PhysicalReclaimability,
    REPORT_SCHEMA_VERSION, RUN_SCHEMA_VERSION, ReclaimabilityReasonCode, ReclaimabilityStatus,
    ScanOptions, ScanReport, ScanRun, ScanSummary, StorageAllocation, StorageSummary,
};
use crate::duplicates::exact_groups;
use crate::hashing::HASH_ALGORITHM;
use crate::media_profiles;
use crate::observation;
use crate::outcome::{
    ArtifactReference, CoverageStatus, Diagnostic, DiagnosticClassification, DiagnosticCode,
    DiagnosticContext, DiagnosticImpact, DiagnosticSeverity,
};
use crate::signals::{Interruption, SignalState};
use crate::state::StateStore;

use super::{
    Execution, artifact_reference, internal_failure, internal_failure_with_code,
    interruption_execution,
};

pub(super) fn run(
    state_directory: &Path,
    arguments: &ScanArgs,
    options: &ScanOptions,
    effective_policy: &EffectivePolicyV1,
    signals: &SignalState,
) -> Execution {
    let discovery = match discover(&arguments.inputs, options, state_directory, signals) {
        Ok(discovery) => discovery,
        Err(error) => return internal_failure("filesystem discovery failed", &error),
    };
    if discovery.interrupted || signals.is_cancelled() {
        return interruption_execution(signals.current(), None);
    }
    if discovery.accepted_input_count == 0 {
        let mut diagnostics = discovery_diagnostics(&discovery.issues, false);
        if diagnostics.is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidCommandInput,
                DiagnosticSeverity::Error,
                DiagnosticClassification::Input,
                DiagnosticImpact::BlocksCommand,
                "no valid scan input was available",
            ));
        }
        return Execution {
            coverage: None,
            artifacts: Vec::new(),
            diagnostics,
            result: None,
        };
    }

    let mut store = match StateStore::open(state_directory) {
        Ok(store) => store,
        Err(error) => {
            return internal_failure_with_code(
                DiagnosticCode::StateTransactionFailed,
                "failed to open persistent state",
                &error,
            );
        }
    };
    let run_id = Uuid::now_v7().to_string();
    let created_at = Utc::now().to_rfc3339();
    if let Err(error) = store.begin_scan(&run_id, &created_at) {
        return internal_failure_with_code(
            DiagnosticCode::StateTransactionFailed,
            "failed to begin the scan transaction",
            &error,
        );
    }
    let mut size_frequency = HashMap::<u64, usize>::new();
    for file in &discovery.files {
        *size_frequency.entry(file.size_bytes).or_default() += 1;
    }

    let ffprobe_adapter = options
        .probe_media
        .then(ffprobe::FfprobeAdapter::discover)
        .and_then(Result::ok);
    let required_probe_signature = options.probe_media.then(|| {
        ffprobe_adapter
            .as_ref()
            .map(ffprobe::FfprobeAdapter::cache_signature)
            .unwrap_or("ffprobe-unavailable")
    });
    let mut observations = Vec::with_capacity(discovery.files.len());
    let mut cache_hits = 0_u64;

    for file in &discovery.files {
        if signals.is_cancelled() {
            return interrupt_active_scan(&store, &run_id, signals.current());
        }
        let cached = match store.lookup_cache(&file.path, &file.signature, required_probe_signature)
        {
            Ok(cached) => cached,
            Err(error) => {
                return fail_active_scan(
                    &store,
                    &run_id,
                    DiagnosticCode::StateTransactionFailed,
                    DiagnosticClassification::Internal,
                    "failed to read the analysis cache",
                    &error,
                );
            }
        };
        let is_exact_candidate = size_frequency
            .get(&file.size_bytes)
            .is_some_and(|count| *count > 1);
        let observed = observation::observe(
            file,
            cached,
            is_exact_candidate,
            options.probe_media,
            ffprobe_adapter.as_ref(),
            signals,
        );
        if observed.interrupted {
            return interrupt_active_scan(&store, &run_id, signals.current());
        }
        if observed.cache_hit {
            cache_hits = cache_hits.saturating_add(1);
        }
        let analysis = observed.analysis;

        // Only write to the cache when the observation is stable.  An unstable
        // result must not pollute the cache and be returned on a future scan.
        if analysis.evidence_validity == EvidenceValidity::Current {
            if let Some(signature) = observed.signature.as_ref() {
                if let Err(error) = store.upsert_cache(&file.path, signature, &analysis) {
                    return fail_active_scan(
                        &store,
                        &run_id,
                        DiagnosticCode::StateTransactionFailed,
                        DiagnosticClassification::Internal,
                        "failed to commit analysis cache state",
                        &error,
                    );
                }
            }
        }

        // Identity and allocation evidence come from the same opened handle as
        // content evidence. An unstable attempt publishes neither.
        let (filesystem_identity, storage_allocation, fs_warnings) = observed
            .filesystem_metadata
            .map(|raw| {
                let allocation = if raw.allocated_size_bytes.is_some()
                    || raw.allocation_source
                        != crate::filesystem::identity::AllocationSource::Unavailable
                {
                    Some(StorageAllocation {
                        logical_size_bytes: raw.logical_size_bytes,
                        allocated_size_bytes: raw.allocated_size_bytes,
                        allocation_source: raw.allocation_source.clone(),
                        extent_sharing_status:
                            crate::filesystem::identity::ExtentSharingStatus::Unknown,
                    })
                } else {
                    None
                };
                (raw.identity, allocation, raw.warnings)
            })
            .unwrap_or_else(|| (None, None, Vec::new()));

        let mut obs = observation_from_analysis(
            &run_id,
            file,
            observed.signature.as_ref(),
            analysis,
            observed.cache_hit,
        );
        obs.filesystem_identity = filesystem_identity;
        obs.storage_allocation = storage_allocation;
        obs.warnings.extend(fs_warnings);
        observations.push(obs);
    }

    // --- Build hard-link groups -------------------------------------------
    // Map (filesystem_id, file_id) → list of observation indices.
    let mut identity_map: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (idx, obs) in observations.iter().enumerate() {
        if let Some(id) = &obs.filesystem_identity {
            identity_map
                .entry((id.filesystem_id.clone(), id.file_id.clone()))
                .or_default()
                .push(idx);
        }
    }

    let mut hard_link_groups: Vec<HardLinkGroup> = Vec::new();
    let mut alias_observation_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    for ((fs_id, file_id), indices) in &identity_map {
        if indices.len() < 2 {
            continue; // single path – not an alias group
        }
        let first = &observations[indices[0]];
        let identity = first.filesystem_identity.clone().expect("identity present");
        let mut members: Vec<(&NativePath, &String)> = indices
            .iter()
            .map(|&index| {
                (
                    &observations[index].path,
                    &observations[index].observation_id,
                )
            })
            .collect();
        members.sort_by(|left, right| left.0.cmp(right.0));

        // Mark all but the first (lexicographic) path as aliases.
        for (_, observation_id) in members.iter().skip(1) {
            alias_observation_ids.insert((*observation_id).clone());
        }
        let paths = members
            .into_iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();

        let reported_link_count = identity.link_count;
        let observed_path_count = u64::try_from(paths.len()).unwrap_or(u64::MAX);
        let unobserved_link_count =
            reported_link_count.and_then(|lc| lc.checked_sub(observed_path_count));

        let logical_size_bytes = first.size_bytes;
        let allocated_size_bytes = first
            .storage_allocation
            .as_ref()
            .and_then(|a| a.allocated_size_bytes);

        let group_seed = format!("{fs_id}:{file_id}");
        let group_hash = blake3::hash(group_seed.as_bytes()).to_hex().to_string();
        let group_id = format!("hl-{}", &group_hash[..16]);

        hard_link_groups.push(HardLinkGroup {
            group_id,
            identity,
            observed_paths: paths,
            observed_path_count,
            reported_link_count,
            unobserved_link_count,
            logical_size_bytes,
            allocated_size_bytes,
            warnings: Vec::new(),
        });
    }
    hard_link_groups.sort_by(|a, b| a.group_id.cmp(&b.group_id));

    let duplicate_groups = exact_groups(&observations);

    // --- Storage accounting -----------------------------------------------
    let path_logical_bytes: u64 = observations
        .iter()
        .map(|o| o.size_bytes)
        .fold(0u64, u64::saturating_add);

    // Unique objects: de-duplicate by (filesystem_id, file_id); fall back
    // to treating each observation as independent when identity is unavailable.
    let mut seen_identities: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    let mut unique_object_logical_bytes: u64 = 0;
    let mut known_allocated_bytes: u64 = 0;
    let mut unknown_allocation_object_count: u64 = 0;

    for obs in &observations {
        let is_new_object = match &obs.filesystem_identity {
            Some(id) => seen_identities.insert((id.filesystem_id.clone(), id.file_id.clone())),
            None => true,
        };
        if is_new_object {
            unique_object_logical_bytes =
                unique_object_logical_bytes.saturating_add(obs.size_bytes);
            match obs
                .storage_allocation
                .as_ref()
                .and_then(|a| a.allocated_size_bytes)
            {
                Some(alloc) => {
                    known_allocated_bytes = known_allocated_bytes.saturating_add(alloc);
                }
                None => {
                    unknown_allocation_object_count =
                        unknown_allocation_object_count.saturating_add(1);
                }
            }
        }
    }

    let hard_link_alias_logical_bytes =
        path_logical_bytes.saturating_sub(unique_object_logical_bytes);

    let duplicate_logical_bytes: u64 = duplicate_groups
        .iter()
        .map(|g| g.reclaimable_bytes)
        .fold(0u64, u64::saturating_add);

    // Aggregate physical reclaimability across duplicate groups.
    let overall_reclaimability = aggregate_reclaimability(&duplicate_groups);

    let storage = StorageSummary {
        path_logical_bytes,
        unique_object_logical_bytes,
        known_allocated_bytes,
        unknown_allocation_object_count,
        hard_link_alias_logical_bytes,
        duplicate_logical_bytes,
        estimated_reclaimable_allocated_bytes: None,
        physical_reclaimability: overall_reclaimability,
    };

    // --- Summary metrics --------------------------------------------------
    let total_bytes = path_logical_bytes;
    let media_files = observations
        .iter()
        .filter(|o| {
            matches!(
                o.media_kind,
                MediaKind::Image | MediaKind::Video | MediaKind::Audio
            )
        })
        .count();
    let unsupported_files = observations
        .iter()
        .filter(|o| o.status == ObservationStatus::Unsupported)
        .count();
    let unreadable_files = observations
        .iter()
        .filter(|o| o.status == ObservationStatus::Unreadable)
        .count();
    let exact_duplicate_files: u64 = duplicate_groups
        .iter()
        .map(|group| group.evidence.member_count)
        .sum();
    let reclaimable_bytes = duplicate_logical_bytes;

    let unique_object_count = u64::try_from(seen_identities.len())
        .unwrap_or(u64::MAX)
        .saturating_add(
            // Add back observations without identity (each is its own object).
            observations
                .iter()
                .filter(|o| o.filesystem_identity.is_none())
                .count()
                .try_into()
                .unwrap_or(u64::MAX),
        );
    let hard_link_alias_path_count = u64::try_from(alias_observation_ids.len()).unwrap_or(u64::MAX);
    let unstable_observation_count = u64::try_from(
        observations
            .iter()
            .filter(|o| o.evidence_validity != EvidenceValidity::Current)
            .count(),
    )
    .unwrap_or(u64::MAX);

    let artifact_directory = state_directory.join("runs").join(&run_id);
    let artifact_set_id = Uuid::now_v7().to_string();
    let completed_at = Utc::now().to_rfc3339();
    let run = ScanRun {
        schema_version: RUN_SCHEMA_VERSION.to_owned(),
        run_id: run_id.clone(),
        artifact_set_id: Some(artifact_set_id.clone()),
        created_at,
        completed_at: completed_at.clone(),
        inputs: arguments
            .inputs
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        options: options.clone(),
        artifact_directory: artifact_directory.to_string_lossy().into_owned(),
        discovered_files: u64::try_from(discovery.files.len()).unwrap_or(u64::MAX),
        analyzed_files: u64::try_from(observations.len()).unwrap_or(u64::MAX),
        cache_hits,
        total_bytes,
        warnings: discovery
            .issues
            .iter()
            .map(|issue| issue.message.clone())
            .collect(),
    };

    let summary = ScanSummary {
        file_count: u64::try_from(observations.len()).unwrap_or(u64::MAX),
        total_bytes,
        media_files: u64::try_from(media_files).unwrap_or(u64::MAX),
        unsupported_files: u64::try_from(unsupported_files).unwrap_or(u64::MAX),
        unreadable_files: u64::try_from(unreadable_files).unwrap_or(u64::MAX),
        exact_duplicate_groups: u64::try_from(duplicate_groups.len()).unwrap_or(u64::MAX),
        exact_duplicate_files,
        reclaimable_bytes,
        cache_hits,
        unique_object_count,
        hard_link_alias_path_count,
        unstable_observation_count,
    };

    let media_profile_evidence = match media_profiles::lossless_png_evidence(
        &run_id,
        &observations,
        options.probe_media,
        ffprobe_adapter
            .as_ref()
            .map(ffprobe::FfprobeAdapter::evidence),
        &effective_policy.fingerprints.evidence_policy,
    ) {
        Ok(evidence) => evidence,
        Err(error) => {
            return fail_active_scan(
                &store,
                &run_id,
                DiagnosticCode::ArtifactValidationFailed,
                DiagnosticClassification::Artifact,
                "failed to derive media-profile evidence",
                &error,
            );
        }
    };
    if let Err(error) = contracts::validate(Contract::MediaProfileEvidence, &media_profile_evidence)
    {
        return fail_active_scan(
            &store,
            &run_id,
            DiagnosticCode::ArtifactValidationFailed,
            DiagnosticClassification::Artifact,
            "the generated media-profile evidence failed contract validation",
            &error,
        );
    }

    let report = ScanReport {
        schema_version: REPORT_SCHEMA_VERSION.to_owned(),
        generated_at: Utc::now().to_rfc3339(),
        run: run.clone(),
        summary,
        duplicate_groups: duplicate_groups.clone(),
        observations: observations.clone(),
        hard_link_groups: hard_link_groups.clone(),
        storage: Some(storage),
        media_profile_evidence: vec![media_profile_evidence.clone()],
    };

    if let Err(error) = contracts::validate(Contract::Run, &run) {
        return fail_active_scan(
            &store,
            &run_id,
            DiagnosticCode::ArtifactValidationFailed,
            DiagnosticClassification::Artifact,
            "the generated run artifact failed contract validation",
            &error,
        );
    }
    if let Err(error) = contracts::validate(Contract::Report, &report) {
        return fail_active_scan(
            &store,
            &run_id,
            DiagnosticCode::ArtifactValidationFailed,
            DiagnosticClassification::Artifact,
            "the generated report artifact failed contract validation",
            &error,
        );
    }

    if signals.is_cancelled() {
        return interrupt_active_scan(&store, &run_id, signals.current());
    }

    let run_path = artifact_directory.join("run.json");
    let report_path = artifact_directory.join("report.json");
    let policy_path = artifact_directory.join("effective-policy.json");
    let committed_set = (|| -> anyhow::Result<ArtifactSetManifest> {
        let payloads = vec![
            ArtifactPayload::json(
                "effective_policy",
                &effective_policy.schema,
                "effective-policy.json",
                effective_policy,
            )?,
            ArtifactPayload::json("run", RUN_SCHEMA_VERSION, "run.json", &run)?,
            ArtifactPayload::json("report", REPORT_SCHEMA_VERSION, "report.json", &report)?,
        ];
        artifact_set::commit_scan_set(&artifact_directory, &run_id, &artifact_set_id, payloads)
    })();
    if let Err(error) = committed_set {
        if artifact_set::inspect_scan_set(&artifact_directory).status
            == ArtifactSetStatus::Committed
        {
            return recoverable_active_scan_failure(
                &run_id,
                DiagnosticCode::ArtifactCommitFailed,
                DiagnosticClassification::Artifact,
                "the scan artifact set is complete but its final durability sync failed",
                &error,
                vec![artifact_reference(
                    "artifact_set",
                    ARTIFACT_SET_SCHEMA,
                    &run_id,
                    &artifact_directory.join(artifact_set::SCAN_MARKER_FILE_NAME),
                )],
            );
        }
        return fail_active_scan(
            &store,
            &run_id,
            DiagnosticCode::ArtifactCommitFailed,
            DiagnosticClassification::Artifact,
            "failed to commit the scan artifact set",
            &error,
        );
    }

    let marker_path = artifact_directory.join(artifact_set::SCAN_MARKER_FILE_NAME);
    let artifacts = vec![
        artifact_reference(
            "effective_policy",
            &effective_policy.schema,
            &run_id,
            &policy_path,
        ),
        artifact_reference("run", RUN_SCHEMA_VERSION, &run_id, &run_path),
        artifact_reference("report", REPORT_SCHEMA_VERSION, &run_id, &report_path),
        artifact_reference("artifact_set", ARTIFACT_SET_SCHEMA, &run_id, &marker_path),
    ];
    if let Err(error) = store.finalize_scan(&run, &report, &observations, &duplicate_groups) {
        return recoverable_active_scan_failure(
            &run_id,
            DiagnosticCode::StateTransactionFailed,
            DiagnosticClassification::Internal,
            "the artifact set is committed but scan-state finalization requires recovery",
            &error,
            artifacts,
        );
    }

    let mut diagnostics = discovery_diagnostics(&discovery.issues, true);
    if options.probe_media && ffprobe_adapter.is_none() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::OptionalCapabilityUnavailable,
            DiagnosticSeverity::Warning,
            DiagnosticClassification::Capability,
            DiagnosticImpact::None,
            "ffprobe is unavailable; optional stream metadata was not collected",
        ));
    }
    if matches!(
        media_profile_evidence.coverage.status,
        MediaProfileCoverageStatus::Partial | MediaProfileCoverageStatus::Unavailable
    ) {
        diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::ResultProducedWithExclusions,
                DiagnosticSeverity::Warning,
                DiagnosticClassification::Coverage,
                DiagnosticImpact::DegradesCoverage,
                "lossless PNG profile evidence was incomplete; affected inputs produced no optimization opportunity",
            )
            .with_count(media_profile_evidence.coverage.limited_evidence_count),
        );
    }
    if unstable_observation_count > 0 {
        let mut diagnostic = Diagnostic::new(
            DiagnosticCode::ResultProducedWithExclusions,
            DiagnosticSeverity::Warning,
            DiagnosticClassification::Observation,
            DiagnosticImpact::DegradesCoverage,
            "unstable or unavailable observations were excluded from exact conclusions",
        );
        diagnostic.context.count = Some(unstable_observation_count);
        diagnostics.push(diagnostic);
    }
    let coverage = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.impact == DiagnosticImpact::DegradesCoverage)
    {
        CoverageStatus::Partial
    } else {
        CoverageStatus::Complete
    };

    Execution {
        coverage: Some(coverage),
        artifacts,
        diagnostics,
        result: serde_json::to_value(&report).ok(),
    }
}

/// Aggregate physical reclaimability across all duplicate groups.
fn aggregate_reclaimability(groups: &[crate::domain::DuplicateGroup]) -> PhysicalReclaimability {
    if groups.is_empty() {
        return PhysicalReclaimability::unknown(vec![]);
    }
    let mut all_estimated = true;
    let mut reason_codes: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for group in groups {
        if group.physical_reclaimability.status != ReclaimabilityStatus::Estimated {
            all_estimated = false;
        }
        for code in &group.physical_reclaimability.reason_codes {
            reason_codes.insert(format!("{code:?}"));
        }
    }

    if all_estimated {
        PhysicalReclaimability::estimated(vec![])
    } else {
        PhysicalReclaimability::unknown(vec![ReclaimabilityReasonCode::ExtentSharingUnknown])
    }
}

fn observation_from_analysis(
    run_id: &str,
    file: &crate::discovery::DiscoveredFile,
    accepted_signature: Option<&crate::filesystem::identity::FileStateSignature>,
    analysis: CachedAnalysis,
    cache_hit: bool,
) -> FileObservation {
    let signature = accepted_signature.unwrap_or(&file.signature);
    let identity = signature.identity.as_ref();
    let hash_algorithm = analysis
        .content_hash
        .as_ref()
        .map(|_| HASH_ALGORITHM.to_owned());
    FileObservation {
        observation_id: Uuid::now_v7().to_string(),
        run_id: run_id.to_owned(),
        path: NativePath::from_path(&file.path),
        size_bytes: signature.logical_size_bytes,
        modified_unix_ns: signature.modified_unix_ns,
        device_id: identity.and_then(|identity| identity.filesystem_id.parse().ok()),
        inode: identity.and_then(|identity| identity.file_id.parse().ok()),
        content_type: analysis.content_type,
        media_kind: analysis.media_kind,
        content_hash: analysis.content_hash,
        hash_algorithm,
        media: analysis.media,
        status: analysis.status,
        cache_hit,
        warnings: analysis.warnings,
        filesystem_identity: None, // populated after construction
        storage_allocation: None,  // populated after construction
        observation_stability: analysis.observation_stability,
        evidence_validity: analysis.evidence_validity,
        attempt_count: analysis.attempt_count,
    }
}

fn discovery_diagnostics(issues: &[DiscoveryIssue], accepted_input: bool) -> Vec<Diagnostic> {
    issues
        .iter()
        .map(|issue| {
            let (code, severity, classification, impact) = if accepted_input {
                (
                    DiagnosticCode::PartialInventory,
                    DiagnosticSeverity::Warning,
                    DiagnosticClassification::Coverage,
                    DiagnosticImpact::DegradesCoverage,
                )
            } else {
                (
                    DiagnosticCode::InvalidCommandInput,
                    DiagnosticSeverity::Error,
                    DiagnosticClassification::Input,
                    DiagnosticImpact::BlocksCommand,
                )
            };
            let classification = match issue.kind {
                DiscoveryIssueKind::PathChanged if accepted_input => {
                    DiagnosticClassification::Observation
                }
                _ => classification,
            };
            let mut diagnostic = Diagnostic::new(
                code,
                severity,
                classification,
                impact,
                issue.message.clone(),
            );
            diagnostic.context = DiagnosticContext {
                path: issue.path.as_deref().map(NativePath::from_path),
                os_error_kind: issue.os_error_kind.clone(),
                ..DiagnosticContext::default()
            };
            diagnostic
        })
        .collect()
}

fn fail_active_scan(
    store: &StateStore,
    run_id: &str,
    code: DiagnosticCode,
    classification: DiagnosticClassification,
    message: &str,
    error: &dyn std::fmt::Display,
) -> Execution {
    let completed_at = Utc::now().to_rfc3339();
    let _ = store.mark_scan_failed(run_id, &completed_at);
    let mut execution = Execution::failure(Diagnostic::new(
        code,
        DiagnosticSeverity::Fatal,
        classification,
        DiagnosticImpact::BlocksCommand,
        format!("{message}: {error}"),
    ));
    execution.diagnostics[0].context.run_id = Some(run_id.to_owned());
    execution
}

fn recoverable_active_scan_failure(
    run_id: &str,
    code: DiagnosticCode,
    classification: DiagnosticClassification,
    message: &str,
    error: &dyn std::fmt::Display,
    artifacts: Vec<ArtifactReference>,
) -> Execution {
    Execution {
        coverage: None,
        artifacts,
        diagnostics: vec![
            Diagnostic::new(
                code,
                DiagnosticSeverity::Fatal,
                classification,
                DiagnosticImpact::BlocksCommand,
                format!("{message}: {error}"),
            )
            .with_run_id(run_id.to_owned()),
        ],
        result: None,
    }
}

fn interrupt_active_scan(
    store: &StateStore,
    run_id: &str,
    interruption: Option<Interruption>,
) -> Execution {
    let completed_at = Utc::now().to_rfc3339();
    if let Err(error) = store.mark_scan_interrupted(run_id, &completed_at) {
        return internal_failure("failed to record interrupted scan state", &error);
    }
    interruption_execution(interruption, Some(run_id))
}
