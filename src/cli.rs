use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
}

/// Analyze media collections and produce evidence-backed plans.
#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about,
    propagate_version = true,
    after_long_help = "Stable exit codes:\n  0 success\n  1 internal failure\n  2 invalid input\n  3 partial success\n  4 required capability unavailable\n  5 stale or incompatible state\n  130 SIGINT\n  143 SIGTERM\n\nSee docs/cli-contract.md for the versioned outcome and stream contract."
)]
pub struct Cli {
    /// Override the persistent optiflow state directory.
    #[arg(long, global = true, value_name = "DIRECTORY")]
    pub state_directory: Option<PathBuf>,

    /// Select exactly one configuration file.
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Disable user, project, and explicitly selected configuration files.
    #[arg(long, global = true)]
    pub no_config: bool,

    /// Emit the primary command result as JSON.
    #[arg(long, global = true, conflicts_with = "output_format")]
    pub json: bool,

    /// Select the primary command-result format.
    #[arg(long, global = true, value_enum, value_name = "FORMAT")]
    pub output_format: Option<OutputFormat>,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn selected_output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            self.output_format.unwrap_or_default()
        }
    }

    pub fn command_name(&self) -> &'static str {
        match &self.command {
            Command::Doctor => "doctor",
            Command::Scan(_) => "scan",
            Command::Report(_) => "report",
            Command::Plan(_) => "plan",
            Command::Cache(_) => "cache",
            Command::Config(arguments) => match &arguments.command {
                ConfigCommand::Validate => "config validate",
                ConfigCommand::Show => "config show",
                ConfigCommand::Explain(_) => "config explain",
            },
        }
    }
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

    /// Validate and inspect the deterministic effective policy.
    Config(ConfigArgs),
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    /// One or more files or directories to inventory.
    #[arg(required = true, num_args = 1.., value_name = "INPUT")]
    pub inputs: Vec<PathBuf>,

    /// Follow symbolic links during traversal.
    #[arg(long)]
    pub follow_symlinks: bool,

    /// Explicitly keep symbolic-link traversal disabled.
    #[arg(long, conflicts_with = "follow_symlinks")]
    pub no_follow_symlinks: bool,

    /// Include hidden files and hidden directory trees.
    #[arg(long)]
    pub include_hidden: bool,

    /// Explicitly exclude hidden files and directories.
    #[arg(long, conflicts_with = "include_hidden")]
    pub exclude_hidden: bool,

    /// Allow traversal to cross filesystem boundaries.
    #[arg(long)]
    pub cross_filesystems: bool,

    /// Explicitly stay on each input's origin filesystem.
    #[arg(long, conflicts_with = "cross_filesystems")]
    pub stay_on_filesystem: bool,

    /// Skip optional ffprobe metadata extraction.
    #[arg(long)]
    pub no_probe: bool,

    /// Explicitly enable optional ffprobe media inspection.
    #[arg(long, conflicts_with = "no_probe")]
    pub probe: bool,
}

impl ScanArgs {
    pub fn follow_symlinks_override(&self) -> Option<bool> {
        self.follow_symlinks
            .then_some(true)
            .or_else(|| self.no_follow_symlinks.then_some(false))
    }

    pub fn include_hidden_override(&self) -> Option<bool> {
        self.include_hidden
            .then_some(true)
            .or_else(|| self.exclude_hidden.then_some(false))
    }

    pub fn cross_filesystems_override(&self) -> Option<bool> {
        self.cross_filesystems
            .then_some(true)
            .or_else(|| self.stay_on_filesystem.then_some(false))
    }

    pub fn probe_media_override(&self) -> Option<bool> {
        self.no_probe
            .then_some(false)
            .or_else(|| self.probe.then_some(true))
    }
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

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Validate all selected sources and the effective policy.
    Validate,

    /// Show the fully resolved policy, provenance, and fingerprints.
    Show,

    /// Explain one canonical setting path.
    Explain(ConfigExplainArgs),
}

#[derive(Debug, Args)]
pub struct ConfigExplainArgs {
    /// Canonical setting path, such as output.format.
    #[arg(value_name = "SETTING")]
    pub setting: String,
}
