use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Analyze media collections and produce evidence-backed plans.
#[derive(Debug, Parser)]
#[command(author, version, about, propagate_version = true)]
pub struct Cli {
    /// Override the persistent optiflow state directory.
    #[arg(long, global = true, value_name = "DIRECTORY")]
    pub state_directory: Option<PathBuf>,

    /// Emit the primary command result as JSON.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect optiflow's runtime and optional media tools.
    Doctor,

    /// Inventory files and prove exact duplicate groups without modifying inputs.
    Scan(ScanArgs),

    /// Render a stored scan report.
    Report(ReportArgs),

    /// Generate an immutable, review-only action plan.
    Plan(PlanArgs),

    /// Inspect the persistent analysis cache.
    Cache(CacheArgs),
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    /// One or more files or directories to inventory.
    #[arg(required = true, num_args = 1.., value_name = "INPUT")]
    pub inputs: Vec<PathBuf>,

    /// Follow symbolic links during traversal.
    #[arg(long)]
    pub follow_symlinks: bool,

    /// Include hidden files and hidden directory trees.
    #[arg(long)]
    pub include_hidden: bool,

    /// Allow traversal to cross filesystem boundaries.
    #[arg(long)]
    pub cross_filesystems: bool,

    /// Skip optional ffprobe metadata extraction.
    #[arg(long)]
    pub no_probe: bool,
}

#[derive(Debug, Args)]
pub struct ReportArgs {
    /// Scan run identifier or path to a report JSON file.
    #[arg(value_name = "RUN")]
    pub run: String,
}

#[derive(Debug, Args)]
pub struct PlanArgs {
    #[command(subcommand)]
    pub command: PlanCommand,
}

#[derive(Debug, Subcommand)]
pub enum PlanCommand {
    /// Propose review actions for byte-identical duplicate groups.
    ExactDuplicates(ExactDuplicatePlanArgs),
}

#[derive(Debug, Args)]
pub struct ExactDuplicatePlanArgs {
    /// Scan run identifier or path to a report JSON file.
    #[arg(long, value_name = "RUN")]
    pub run: String,

    /// Write the plan to this path instead of the run artifact directory.
    #[arg(long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct CacheArgs {
    #[command(subcommand)]
    pub command: CacheCommand,
}

#[derive(Debug, Subcommand)]
pub enum CacheCommand {
    /// Show cache location, entry count, and database size.
    Status,
}
