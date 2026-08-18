use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::domain::{CacheStatus, DoctorReport, Plan, ReclaimabilityStatus, ScanReport};

pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .context("artifact path does not have a parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create artifact directory {}", parent.display()))?;

    let file_name = path
        .file_name()
        .context("artifact path does not have a file name")?
        .to_string_lossy();
    let temporary_path = parent.join(format!(".{file_name}.tmp"));
    let file = File::create(&temporary_path)
        .with_context(|| format!("failed to create {}", temporary_path.display()))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    fs::rename(&temporary_path, path).with_context(|| {
        format!(
            "failed to commit artifact {} to {}",
            temporary_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    serde_json::to_writer_pretty(&mut writer, value)?;
    writer.write_all(b"\n")?;
    Ok(())
}

pub fn print_scan_report(report: &ScanReport) {
    println!("optiflow scan {}", report.run.run_id);
    println!("  Paths scanned:            {}", report.summary.file_count);
    println!(
        "  Unique filesystem objects: {}",
        report.summary.unique_object_count
    );
    println!(
        "  Hard-link alias paths:    {}",
        report.summary.hard_link_alias_path_count
    );
    println!("  Path logical bytes:       {}", report.summary.total_bytes);

    if let Some(storage) = &report.storage {
        println!(
            "  Unique-object logical bytes: {}",
            storage.unique_object_logical_bytes
        );
        println!(
            "  Logical duplicate bytes:  {}",
            storage.duplicate_logical_bytes
        );

        match storage.physical_reclaimability.status {
            ReclaimabilityStatus::Estimated => {
                println!(
                    "  Est. reclaimable allocated bytes: {}",
                    storage
                        .estimated_reclaimable_allocated_bytes
                        .map_or("unknown".to_owned(), |b| b.to_string())
                );
            }
            ReclaimabilityStatus::Unknown => {
                let codes: Vec<&str> = storage
                    .physical_reclaimability
                    .reason_codes
                    .iter()
                    .map(|c| reason_code_label(c))
                    .collect();
                println!(
                    "  Est. reclaimable allocated bytes: unknown ({})",
                    if codes.is_empty() {
                        "reason unspecified".to_owned()
                    } else {
                        codes.join(", ")
                    }
                );
            }
        }
    } else {
        println!(
            "  Reclaimable bytes (logical): {}",
            report.summary.reclaimable_bytes
        );
    }

    println!(
        "  Exact duplicate groups:   {}",
        report.summary.exact_duplicate_groups
    );
    println!("  Cache hits:               {}", report.summary.cache_hits);
    if report.summary.unstable_observation_count > 0 {
        println!(
            "  Unstable observations:    {} (excluded from duplicate groups)",
            report.summary.unstable_observation_count
        );
    }
    println!("  Artifacts: {}", report.run.artifact_directory);

    // Surface hard-link group warnings.
    for group in &report.hard_link_groups {
        if let Some(unobserved) = group.unobserved_link_count {
            if unobserved > 0 {
                println!(
                    "  Warning: filesystem object {} has {unobserved} link(s) outside the scanned inputs",
                    group.identity.file_id
                );
            }
        }
        for warning in &group.warnings {
            println!("  Warning: {warning}");
        }
    }

    for warning in &report.run.warnings {
        println!("  Warning: {warning}");
    }
}

fn reason_code_label(code: &crate::domain::ReclaimabilityReasonCode) -> &'static str {
    use crate::domain::ReclaimabilityReasonCode::*;
    match code {
        FilesystemIdentityUnavailable => "filesystem_identity_unavailable",
        AllocationMetadataUnavailable => "allocation_metadata_unavailable",
        UnobservedHardLinks => "unobserved_hard_links",
        ExtentSharingUnknown => "extent_sharing_unknown",
        ArithmeticOverflow => "arithmetic_overflow",
        PlatformMetadataUnsupported => "platform_metadata_unsupported",
    }
}

pub fn print_plan(plan: &Plan, path: &Path) {
    println!("optiflow exact-duplicate review plan {}", plan.plan_id);
    println!("  Actions: {}", plan.summary.action_count);
    println!("  Candidate files: {}", plan.summary.candidate_file_count);
    println!(
        "  Potentially reclaimable bytes: {}",
        plan.summary.potential_reclaimable_bytes
    );
    println!("  Mutates files: {}", plan.safety.mutates_files);
    println!("  Plan: {}", path.display());
}

pub fn print_doctor(report: &DoctorReport) {
    println!("optiflow {}", report.optiflow_version);
    println!("  Platform: {}", report.platform);
    println!("  State directory: {}", report.state_directory);
    println!("  State ready: {}", report.state_ready);
    for tool in &report.tools {
        let status = if tool.available {
            "available"
        } else {
            "not found"
        };
        println!("  {}: {} ({})", tool.name, status, tool.required_for);
    }
}

pub fn print_cache_status(status: &CacheStatus) {
    println!("optiflow cache");
    println!("  Database: {}", status.database_path);
    println!("  Database bytes: {}", status.database_size_bytes);
    println!("  Cached files: {}", status.cached_file_count);
    println!("  Stored runs: {}", status.stored_run_count);
}
