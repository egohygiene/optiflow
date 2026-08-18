use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use directories::BaseDirs;
use serde::Serialize;
use uuid::Uuid;

use crate::adapters::ffprobe;
use crate::cli::{CacheCommand, Cli, Command, PlanCommand, ScanArgs};
use crate::contracts::{self, Contract};
use crate::discovery::{DiscoveryIssue, DiscoveryIssueKind, discover};
use crate::domain::{
    CachedAnalysis, DoctorReport, EvidenceValidity, FileObservation, HardLinkGroup, MediaKind,
    ObservationStatus, PLAN_SCHEMA_VERSION, PhysicalReclaimability, REPORT_SCHEMA_VERSION,
    REPORT_SCHEMA_VERSION_V1, REPORT_SCHEMA_VERSION_V2, RUN_SCHEMA_VERSION,
    ReclaimabilityReasonCode, ReclaimabilityStatus, ScanOptions, ScanReport, ScanRun, ScanSummary,
    SerializedPath, StorageAllocation, StorageSummary,
};
use crate::duplicates::exact_groups;
use crate::filesystem::metadata as fs_metadata;
use crate::hashing::{HASH_ALGORITHM, hash_with_stability};
use crate::outcome::{
    ArtifactReference, CommandResult, CoverageStatus, Diagnostic, DiagnosticClassification,
    DiagnosticCode, DiagnosticContext, DiagnosticImpact, DiagnosticSeverity,
};
use crate::planning::exact_duplicate_plan;
use crate::reports;
use crate::signals::{Interruption, SignalState};
use crate::state::StateStore;

struct Execution {
    coverage: Option<CoverageStatus>,
    artifacts: Vec<ArtifactReference>,
    diagnostics: Vec<Diagnostic>,
    result: Option<serde_json::Value>,
}

impl Execution {
    fn success<T: Serialize>(result: &T) -> Self {
        Self {
            coverage: None,
            artifacts: Vec::new(),
            diagnostics: Vec::new(),
            result: serde_json::to_value(result).ok(),
        }
    }

    fn failure(diagnostic: Diagnostic) -> Self {
        Self {
            coverage: None,
            artifacts: Vec::new(),
            diagnostics: vec![diagnostic],
            result: None,
        }
    }
}

pub fn run(cli: Cli, signals: &SignalState) -> CommandResult {
    let command_name = cli.command_name();
    let state_directory = cli.state_directory.unwrap_or_else(default_state_directory);
    let execution = match cli.command {
        Command::Doctor => run_doctor(&state_directory),
        Command::Scan(arguments) => run_scan(&state_directory, &arguments, signals),
        Command::Report(arguments) => run_report(&state_directory, &arguments.run),
        Command::Plan(arguments) => match arguments.command {
            PlanCommand::ExactDuplicates(arguments) => run_plan(
                &state_directory,
                &arguments.run,
                arguments.output.as_deref(),
            ),
        },
        Command::Cache(arguments) => match arguments.command {
            CacheCommand::Status => run_cache_status(&state_directory),
        },
    };
    CommandResult::resolve(
        command_name,
        execution.coverage,
        execution.artifacts,
        execution.diagnostics,
        execution.result,
    )
}

fn run_doctor(state_directory: &Path) -> Execution {
    let state_ready = StateStore::open(state_directory).is_ok();
    let report = DoctorReport {
        optiflow_version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        state_directory: state_directory.to_string_lossy().into_owned(),
        state_ready,
        tools: vec![
            ffprobe::status("ffprobe", "optional media stream inventory"),
            ffprobe::status("ffmpeg", "future transactional media validation"),
        ],
    };
    Execution::success(&report)
}

fn run_scan(state_directory: &Path, arguments: &ScanArgs, signals: &SignalState) -> Execution {
    let options = ScanOptions {
        follow_symlinks: arguments.follow_symlinks,
        include_hidden: arguments.include_hidden,
        cross_filesystems: arguments.cross_filesystems,
        probe_media: !arguments.no_probe,
    };
    let discovery = match discover(&arguments.inputs, &options, state_directory, signals) {
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

    let ffprobe_signature = options
        .probe_media
        .then(|| ffprobe::signature("ffprobe"))
        .flatten();
    let required_probe_signature = options.probe_media.then(|| {
        ffprobe_signature
            .as_deref()
            .unwrap_or("ffprobe-unavailable")
    });
    let mut observations = Vec::with_capacity(discovery.files.len());
    let mut cache_hits = 0_u64;

    for file in &discovery.files {
        if signals.is_cancelled() {
            return interrupt_active_scan(&store, &run_id, signals.current());
        }
        let cached = match store.lookup_cache(
            &file.path,
            file.size_bytes,
            file.modified_unix_ns,
            required_probe_signature,
        ) {
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
        let was_cache_hit = cached.is_some();
        if was_cache_hit {
            cache_hits = cache_hits.saturating_add(1);
        }
        let mut analysis = cached.unwrap_or_else(|| {
            crate::inventory::analyze(
                &file.path,
                options.probe_media,
                ffprobe_signature.as_deref(),
            )
        });

        let is_exact_candidate = size_frequency
            .get(&file.size_bytes)
            .is_some_and(|count| *count > 1);
        if is_exact_candidate && analysis.status != ObservationStatus::Unreadable {
            analysis.content_hash = None;
            let hash_result = hash_with_stability(&file.path, signals);
            if hash_result.interrupted {
                return interrupt_active_scan(&store, &run_id, signals.current());
            }
            if let Some(w) = hash_result.warning.clone() {
                analysis.warnings.push(w);
            }
            if hash_result.evidence_validity == EvidenceValidity::Current {
                analysis.content_hash = Some(hash_result.hash.clone());
            } else {
                analysis.status = ObservationStatus::Unreadable;
            }
            analysis.observation_stability = hash_result.stability;
            analysis.evidence_validity = hash_result.evidence_validity;
            analysis.attempt_count = hash_result.attempt_count;
        }

        // Only write to the cache when the observation is stable.  An unstable
        // result must not pollute the cache and be returned on a future scan.
        if analysis.evidence_validity == EvidenceValidity::Current {
            if let Err(error) = store.upsert_cache(
                &file.path,
                file.size_bytes,
                file.modified_unix_ns,
                &analysis,
            ) {
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

        // Collect filesystem identity and allocation metadata for this scan.
        // This is always refreshed (not cached) because link counts and
        // allocated sizes can change independently of content.
        let raw_fs_meta = fs_metadata::collect(&file.path, file.size_bytes);
        let (filesystem_identity, storage_allocation, fs_warnings) = {
            let warnings = raw_fs_meta.warnings.clone();
            let identity = raw_fs_meta.identity.clone();
            let allocation = if raw_fs_meta.allocated_size_bytes.is_some()
                || raw_fs_meta.allocation_source
                    != crate::filesystem::identity::AllocationSource::Unavailable
            {
                Some(StorageAllocation {
                    logical_size_bytes: raw_fs_meta.logical_size_bytes,
                    allocated_size_bytes: raw_fs_meta.allocated_size_bytes,
                    allocation_source: raw_fs_meta.allocation_source.clone(),
                    extent_sharing_status:
                        crate::filesystem::identity::ExtentSharingStatus::Unknown,
                })
            } else {
                None
            };
            (identity, allocation, warnings)
        };

        let mut obs = observation_from_analysis(&run_id, file, analysis, was_cache_hit);
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
        let mut paths: Vec<String> = indices
            .iter()
            .map(|&i| observations[i].path.clone())
            .collect();
        paths.sort();

        // Mark all but the first (lexicographic) path as aliases.
        for path in paths.iter().skip(1) {
            if let Some(obs) = observations.iter().find(|o| &o.path == path) {
                alias_observation_ids.insert(obs.observation_id.clone());
            }
        }

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
    let completed_at = Utc::now().to_rfc3339();
    let run = ScanRun {
        schema_version: RUN_SCHEMA_VERSION.to_owned(),
        run_id: run_id.clone(),
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

    let report = ScanReport {
        schema_version: REPORT_SCHEMA_VERSION.to_owned(),
        generated_at: Utc::now().to_rfc3339(),
        run: run.clone(),
        summary,
        duplicate_groups: duplicate_groups.clone(),
        observations: observations.clone(),
        hard_link_groups: hard_link_groups.clone(),
        storage: Some(storage),
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
    if let Err(error) = reports::write_json_atomic(&run_path, &run) {
        return fail_active_scan(
            &store,
            &run_id,
            DiagnosticCode::ArtifactCommitFailed,
            DiagnosticClassification::Artifact,
            "failed to commit the run artifact",
            &error,
        );
    }
    if let Err(error) = reports::write_json_atomic(&report_path, &report) {
        return fail_active_scan(
            &store,
            &run_id,
            DiagnosticCode::ArtifactCommitFailed,
            DiagnosticClassification::Artifact,
            "failed to commit the report artifact",
            &error,
        );
    }

    let artifacts = vec![
        artifact_reference("run", RUN_SCHEMA_VERSION, &run_id, &run_path),
        artifact_reference("report", REPORT_SCHEMA_VERSION, &run_id, &report_path),
    ];
    if signals.is_cancelled() {
        let mut execution = interrupt_active_scan(&store, &run_id, signals.current());
        execution.artifacts = artifacts;
        return execution;
    }
    if let Err(error) = store.finalize_scan(&run, &report, &observations, &duplicate_groups) {
        let mut execution = fail_active_scan(
            &store,
            &run_id,
            DiagnosticCode::StateTransactionFailed,
            DiagnosticClassification::Internal,
            "failed to finalize scan state",
            &error,
        );
        execution.artifacts = artifacts;
        return execution;
    }

    let mut diagnostics = discovery_diagnostics(&discovery.issues, true);
    if options.probe_media && ffprobe_signature.is_none() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::OptionalCapabilityUnavailable,
            DiagnosticSeverity::Warning,
            DiagnosticClassification::Capability,
            DiagnosticImpact::None,
            "ffprobe is unavailable; optional stream metadata was not collected",
        ));
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
    analysis: CachedAnalysis,
    cache_hit: bool,
) -> FileObservation {
    let hash_algorithm = analysis
        .content_hash
        .as_ref()
        .map(|_| HASH_ALGORITHM.to_owned());
    FileObservation {
        observation_id: Uuid::now_v7().to_string(),
        run_id: run_id.to_owned(),
        path: file.path.to_string_lossy().into_owned(),
        size_bytes: file.size_bytes,
        modified_unix_ns: file.modified_unix_ns,
        device_id: file.device_id,
        inode: file.inode,
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

struct LoadedReport {
    report: ScanReport,
    path: PathBuf,
}

fn run_report(state_directory: &Path, run_or_path: &str) -> Execution {
    let store = match StateStore::open(state_directory) {
        Ok(store) => store,
        Err(error) => {
            return internal_failure_with_code(
                DiagnosticCode::StateTransactionFailed,
                "failed to open persistent state",
                &error,
            );
        }
    };
    let loaded = match load_report(&store, run_or_path) {
        Ok(loaded) => loaded,
        Err(diagnostic) => return Execution::failure(*diagnostic),
    };
    let partial = report_is_partial(&loaded.report);
    let diagnostics = if partial {
        vec![source_run_partial_diagnostic(&loaded.report)]
    } else {
        Vec::new()
    };
    Execution {
        coverage: Some(if partial {
            CoverageStatus::Partial
        } else {
            CoverageStatus::Complete
        }),
        artifacts: vec![artifact_reference(
            "report",
            &loaded.report.schema_version,
            &loaded.report.run.run_id,
            &loaded.path,
        )],
        diagnostics,
        result: serde_json::to_value(&loaded.report).ok(),
    }
}

fn run_plan(state_directory: &Path, run_or_path: &str, output: Option<&Path>) -> Execution {
    let store = match StateStore::open(state_directory) {
        Ok(store) => store,
        Err(error) => {
            return internal_failure_with_code(
                DiagnosticCode::StateTransactionFailed,
                "failed to open persistent state",
                &error,
            );
        }
    };
    let loaded = match load_report(&store, run_or_path) {
        Ok(loaded) => loaded,
        Err(diagnostic) => return Execution::failure(*diagnostic),
    };
    let partial = report_is_partial(&loaded.report);
    let plan = exact_duplicate_plan(&loaded.report);
    let output_path = output.map_or_else(
        || Path::new(&loaded.report.run.artifact_directory).join("plan-exact-duplicates.json"),
        Path::to_path_buf,
    );
    if let Err(error) = contracts::validate(Contract::Plan, &plan) {
        return Execution::failure(Diagnostic::new(
            DiagnosticCode::ArtifactValidationFailed,
            DiagnosticSeverity::Fatal,
            DiagnosticClassification::Artifact,
            DiagnosticImpact::BlocksCommand,
            format!("the generated plan failed contract validation: {error}"),
        ));
    }
    if let Err(error) = reports::write_json_atomic(&output_path, &plan) {
        let mut diagnostic = Diagnostic::new(
            DiagnosticCode::OutputDestinationInvalid,
            DiagnosticSeverity::Error,
            DiagnosticClassification::Input,
            DiagnosticImpact::BlocksCommand,
            format!("the plan output destination could not be committed: {error}"),
        )
        .with_path(&output_path);
        diagnostic.retryable = Some(false);
        return Execution::failure(diagnostic);
    }
    Execution {
        coverage: Some(if partial {
            CoverageStatus::Partial
        } else {
            CoverageStatus::Complete
        }),
        artifacts: vec![artifact_reference(
            "plan",
            PLAN_SCHEMA_VERSION,
            &loaded.report.run.run_id,
            &output_path,
        )],
        diagnostics: if partial {
            vec![source_run_partial_diagnostic(&loaded.report)]
        } else {
            Vec::new()
        },
        result: serde_json::to_value(&plan).ok(),
    }
}

fn run_cache_status(state_directory: &Path) -> Execution {
    let store = match StateStore::open(state_directory) {
        Ok(store) => store,
        Err(error) => {
            return internal_failure_with_code(
                DiagnosticCode::StateTransactionFailed,
                "failed to open persistent state",
                &error,
            );
        }
    };
    match store.cache_status() {
        Ok(status) => Execution::success(&status),
        Err(error) => internal_failure_with_code(
            DiagnosticCode::StateTransactionFailed,
            "failed to inspect cache state",
            &error,
        ),
    }
}

fn load_report(store: &StateStore, run_or_path: &str) -> Result<LoadedReport, Box<Diagnostic>> {
    let candidate = Path::new(run_or_path);
    if candidate.exists() {
        let report_path = if candidate.is_dir() {
            candidate.join("report.json")
        } else {
            candidate.to_path_buf()
        };
        let bytes = fs::read(&report_path).map_err(|error| {
            Box::new(
                Diagnostic::new(
                    DiagnosticCode::StoredStateIncompatible,
                    DiagnosticSeverity::Error,
                    DiagnosticClassification::State,
                    DiagnosticImpact::BlocksCommand,
                    format!("the requested report could not be read: {error}"),
                )
                .with_path(&report_path),
            )
        })?;
        let report = decode_report(&bytes, &report_path)?;
        return Ok(LoadedReport {
            report,
            path: report_path,
        });
    }

    if Uuid::parse_str(run_or_path).is_err() {
        return Err(Box::new(Diagnostic::new(
            DiagnosticCode::InvalidCommandInput,
            DiagnosticSeverity::Error,
            DiagnosticClassification::Input,
            DiagnosticImpact::BlocksCommand,
            "the run reference is neither an existing report path nor a valid run identifier",
        )));
    }

    let report = store.load_report(run_or_path).map_err(|error| {
        Box::new(
            Diagnostic::new(
                DiagnosticCode::StoredStateIncompatible,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                format!("stored scan state could not be decoded: {error}"),
            )
            .with_run_id(run_or_path),
        )
    })?;
    if let Some(report) = report {
        return Ok(LoadedReport {
            path: Path::new(&report.run.artifact_directory).join("report.json"),
            report,
        });
    }

    let status = store.load_run_status(run_or_path).map_err(|error| {
        Box::new(
            Diagnostic::new(
                DiagnosticCode::StoredStateIncompatible,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                format!("stored scan status could not be inspected: {error}"),
            )
            .with_run_id(run_or_path),
        )
    })?;
    let (code, message) = if status.as_deref() == Some("interrupted") {
        (
            DiagnosticCode::SourceRunInterrupted,
            "the requested scan run was interrupted and has no complete report",
        )
    } else {
        (
            DiagnosticCode::StoredRunNotFound,
            "the requested completed scan run was not found",
        )
    };
    Err(Box::new(
        Diagnostic::new(
            code,
            DiagnosticSeverity::Error,
            DiagnosticClassification::State,
            DiagnosticImpact::BlocksCommand,
            message,
        )
        .with_run_id(run_or_path),
    ))
}

fn decode_report(bytes: &[u8], path: &Path) -> Result<ScanReport, Box<Diagnostic>> {
    let document: serde_json::Value = serde_json::from_slice(bytes).map_err(|error| {
        Box::new(
            Diagnostic::new(
                DiagnosticCode::InvalidCommandInput,
                DiagnosticSeverity::Error,
                DiagnosticClassification::Input,
                DiagnosticImpact::BlocksCommand,
                format!("the supplied report is not valid JSON: {error}"),
            )
            .with_path(path),
        )
    })?;
    let schema_version = document
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    if !matches!(
        schema_version.as_deref(),
        Some(REPORT_SCHEMA_VERSION | REPORT_SCHEMA_VERSION_V1 | REPORT_SCHEMA_VERSION_V2)
    ) {
        return Err(Box::new(
            Diagnostic::new(
                DiagnosticCode::StoredStateIncompatible,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                "the supplied report declares an unsupported schema",
            )
            .with_path(path),
        ));
    }
    let report: ScanReport = serde_json::from_value(document).map_err(|error| {
        Box::new(
            Diagnostic::new(
                DiagnosticCode::StoredStateIncompatible,
                DiagnosticSeverity::Error,
                DiagnosticClassification::State,
                DiagnosticImpact::BlocksCommand,
                format!("the supplied report is incompatible with its declared schema: {error}"),
            )
            .with_path(path),
        )
    })?;
    if schema_version.as_deref() == Some(REPORT_SCHEMA_VERSION) {
        contracts::validate(Contract::Report, &report).map_err(|error| {
            Box::new(
                Diagnostic::new(
                    DiagnosticCode::StoredStateIncompatible,
                    DiagnosticSeverity::Error,
                    DiagnosticClassification::State,
                    DiagnosticImpact::BlocksCommand,
                    format!("the supplied report failed contract validation: {error}"),
                )
                .with_path(path),
            )
        })?;
    }
    Ok(report)
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
                path: issue.path.as_deref().map(SerializedPath::from_path),
                os_error_kind: issue.os_error_kind.clone(),
                ..DiagnosticContext::default()
            };
            diagnostic
        })
        .collect()
}

fn report_is_partial(report: &ScanReport) -> bool {
    report.summary.unstable_observation_count > 0
        || report.summary.unreadable_files > 0
        || !report.run.warnings.is_empty()
}

fn source_run_partial_diagnostic(report: &ScanReport) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::SourceRunPartial,
        DiagnosticSeverity::Warning,
        DiagnosticClassification::Coverage,
        DiagnosticImpact::DegradesCoverage,
        "the source run has incomplete or excluded evidence",
    )
    .with_run_id(report.run.run_id.clone())
}

fn artifact_reference(kind: &str, schema: &str, run_id: &str, path: &Path) -> ArtifactReference {
    ArtifactReference {
        kind: kind.to_owned(),
        schema: schema.to_owned(),
        run_id: Some(run_id.to_owned()),
        path: SerializedPath::from_path(path),
    }
}

fn internal_failure(message: &str, error: &dyn std::fmt::Display) -> Execution {
    internal_failure_with_code(DiagnosticCode::InternalInvariantViolated, message, error)
}

fn internal_failure_with_code(
    code: DiagnosticCode,
    message: &str,
    error: &dyn std::fmt::Display,
) -> Execution {
    Execution::failure(Diagnostic::new(
        code,
        DiagnosticSeverity::Fatal,
        DiagnosticClassification::Internal,
        DiagnosticImpact::BlocksCommand,
        format!("{message}: {error}"),
    ))
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

fn interruption_execution(interruption: Option<Interruption>, run_id: Option<&str>) -> Execution {
    let (code, message) = match interruption {
        Some(Interruption::Terminate) => (
            DiagnosticCode::OperationTerminated,
            "the operation was terminated by SIGTERM",
        ),
        _ => (
            DiagnosticCode::OperationInterrupted,
            "the operation was interrupted by SIGINT",
        ),
    };
    let mut diagnostic = Diagnostic::new(
        code,
        DiagnosticSeverity::Error,
        DiagnosticClassification::Interruption,
        DiagnosticImpact::BlocksCommand,
        message,
    );
    diagnostic.context.run_id = run_id.map(str::to_owned);
    Execution::failure(diagnostic)
}

fn default_state_directory() -> PathBuf {
    if let Some(path) = std::env::var_os("OPTIFLOW_STATE_DIRECTORY") {
        return PathBuf::from(path);
    }

    let Some(base_directories) = BaseDirs::new() else {
        return PathBuf::from(".optiflow");
    };

    #[cfg(target_os = "macos")]
    {
        base_directories
            .home_dir()
            .join("Library")
            .join("Application Support")
            .join("optiflow")
    }

    #[cfg(not(target_os = "macos"))]
    {
        std::env::var_os("XDG_STATE_HOME").map_or_else(
            || base_directories.home_dir().join(".local/state/optiflow"),
            |path| PathBuf::from(path).join("optiflow"),
        )
    }
}
