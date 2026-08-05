use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::domain::{CacheStatus, DoctorReport, Plan, ScanReport};

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
    println!("OptiFlow scan {}", report.run.run_id);
    println!("  Files: {}", report.summary.file_count);
    println!("  Bytes: {}", report.summary.total_bytes);
    println!(
        "  Exact duplicate groups: {}",
        report.summary.exact_duplicate_groups
    );
    println!(
        "  Potentially reclaimable bytes: {}",
        report.summary.reclaimable_bytes
    );
    println!("  Cache hits: {}", report.summary.cache_hits);
    println!("  Artifacts: {}", report.run.artifact_directory);
}

pub fn print_plan(plan: &Plan, path: &Path) {
    println!("OptiFlow exact-duplicate review plan {}", plan.plan_id);
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
    println!("OptiFlow {}", report.optiflow_version);
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
    println!("OptiFlow cache");
    println!("  Database: {}", status.database_path);
    println!("  Database bytes: {}", status.database_size_bytes);
    println!("  Cached files: {}", status.cached_file_count);
    println!("  Stored runs: {}", status.stored_run_count);
}
