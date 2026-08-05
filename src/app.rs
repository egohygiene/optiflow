use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use directories::BaseDirs;
use uuid::Uuid;

use crate::adapters::ffprobe;
use crate::cli::{CacheCommand, Cli, Command, PlanCommand, ScanArgs};
use crate::discovery::discover;
use crate::domain::{
    CacheStatus, CachedAnalysis, DoctorReport, FileObservation, MediaKind, ObservationStatus,
    REPORT_SCHEMA_VERSION, RUN_SCHEMA_VERSION, ScanOptions, ScanReport, ScanRun, ScanSummary,
};
use crate::duplicates::exact_groups;
use crate::hashing::{HASH_ALGORITHM, complete_hash_stable};
use crate::planning::exact_duplicate_plan;
use crate::reports;
use crate::state::StateStore;

pub fn run(cli: Cli) -> Result<()> {
    let state_directory = cli.state_directory.unwrap_or_else(default_state_directory);
    match cli.command {
        Command::Doctor => run_doctor(&state_directory, cli.json),
        Command::Scan(arguments) => run_scan(&state_directory, cli.json, &arguments),
        Command::Report(arguments) => {
            let store = StateStore::open(&state_directory)?;
            let report = load_report(&store, &arguments.run)?;
            if cli.json {
                reports::print_json(&report)
            } else {
                reports::print_scan_report(&report);
                Ok(())
            }
        }
        Command::Plan(arguments) => match arguments.command {
            PlanCommand::ExactDuplicates(arguments) => {
                let store = StateStore::open(&state_directory)?;
                let report = load_report(&store, &arguments.run)?;
                let plan = exact_duplicate_plan(&report);
                let output_path = arguments.output.unwrap_or_else(|| {
                    Path::new(&report.run.artifact_directory).join("plan-exact-duplicates.json")
                });
                reports::write_json_atomic(&output_path, &plan)?;
                if cli.json {
                    reports::print_json(&plan)
                } else {
                    reports::print_plan(&plan, &output_path);
                    Ok(())
                }
            }
        },
        Command::Cache(arguments) => match arguments.command {
            CacheCommand::Status => {
                let store = StateStore::open(&state_directory)?;
                let status = store.cache_status()?;
                output_cache_status(&status, cli.json)
            }
        },
    }
}

fn run_doctor(state_directory: &Path, json: bool) -> Result<()> {
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
    if json {
        reports::print_json(&report)
    } else {
        reports::print_doctor(&report);
        Ok(())
    }
}

fn run_scan(state_directory: &Path, json: bool, arguments: &ScanArgs) -> Result<()> {
    let mut store = StateStore::open(state_directory)?;
    let run_id = Uuid::now_v7().to_string();
    let created_at = Utc::now().to_rfc3339();
    store.begin_scan(&run_id, &created_at)?;

    let options = ScanOptions {
        follow_symlinks: arguments.follow_symlinks,
        include_hidden: arguments.include_hidden,
        cross_filesystems: arguments.cross_filesystems,
        probe_media: !arguments.no_probe,
    };
    let discovery = discover(&arguments.inputs, &options, state_directory)?;
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
        let cached = store.lookup_cache(
            &file.path,
            file.size_bytes,
            file.modified_unix_ns,
            required_probe_signature,
        )?;
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
            match complete_hash_stable(&file.path, file.size_bytes, file.modified_unix_ns) {
                Ok(hash) => analysis.content_hash = Some(hash),
                Err(error) => {
                    analysis.status = ObservationStatus::Unreadable;
                    analysis.warnings.push(error.to_string());
                }
            }
        }

        store.upsert_cache(
            &file.path,
            file.size_bytes,
            file.modified_unix_ns,
            &analysis,
        )?;
        observations.push(observation_from_analysis(
            &run_id,
            file,
            analysis,
            was_cache_hit,
        ));
    }

    let duplicate_groups = exact_groups(&observations);
    let total_bytes = observations
        .iter()
        .map(|observation| observation.size_bytes)
        .sum();
    let media_files = observations
        .iter()
        .filter(|observation| {
            matches!(
                observation.media_kind,
                MediaKind::Image | MediaKind::Video | MediaKind::Audio
            )
        })
        .count();
    let unsupported_files = observations
        .iter()
        .filter(|observation| observation.status == ObservationStatus::Unsupported)
        .count();
    let unreadable_files = observations
        .iter()
        .filter(|observation| observation.status == ObservationStatus::Unreadable)
        .count();
    let exact_duplicate_files = duplicate_groups
        .iter()
        .map(|group| group.evidence.member_count)
        .sum();
    let reclaimable_bytes = duplicate_groups
        .iter()
        .map(|group| group.reclaimable_bytes)
        .sum();

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
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        options,
        artifact_directory: artifact_directory.to_string_lossy().into_owned(),
        discovered_files: u64::try_from(discovery.files.len()).unwrap_or(u64::MAX),
        analyzed_files: u64::try_from(observations.len()).unwrap_or(u64::MAX),
        cache_hits,
        total_bytes,
        warnings: discovery.warnings,
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
    };
    let report = ScanReport {
        schema_version: REPORT_SCHEMA_VERSION.to_owned(),
        generated_at: completed_at,
        run: run.clone(),
        summary,
        duplicate_groups: duplicate_groups.clone(),
        observations: observations.clone(),
    };

    reports::write_json_atomic(&artifact_directory.join("run.json"), &run)?;
    reports::write_json_atomic(&artifact_directory.join("report.json"), &report)?;
    store.finalize_scan(&run, &report, &observations, &duplicate_groups)?;

    if json {
        reports::print_json(&report)
    } else {
        reports::print_scan_report(&report);
        Ok(())
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
    }
}

fn load_report(store: &StateStore, run_or_path: &str) -> Result<ScanReport> {
    let candidate = Path::new(run_or_path);
    if candidate.exists() {
        let report_path = if candidate.is_dir() {
            candidate.join("report.json")
        } else {
            candidate.to_path_buf()
        };
        let bytes = fs::read(&report_path)
            .with_context(|| format!("failed to read {}", report_path.display()))?;
        return serde_json::from_slice(&bytes)
            .with_context(|| format!("invalid report JSON in {}", report_path.display()));
    }

    store
        .load_report(run_or_path)?
        .with_context(|| format!("scan run not found: {run_or_path}"))
}

fn output_cache_status(status: &CacheStatus, json: bool) -> Result<()> {
    if json {
        reports::print_json(status)
    } else {
        reports::print_cache_status(status);
        Ok(())
    }
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
        return base_directories
            .home_dir()
            .join("Library")
            .join("Application Support")
            .join("optiflow");
    }

    #[cfg(not(target_os = "macos"))]
    {
        std::env::var_os("XDG_STATE_HOME").map_or_else(
            || base_directories.home_dir().join(".local/state/optiflow"),
            |path| PathBuf::from(path).join("optiflow"),
        )
    }
}
