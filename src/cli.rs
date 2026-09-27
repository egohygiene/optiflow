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
            Command::Apply(_) => "apply",
            Command::Execution(arguments) => match &arguments.command {
                ExecutionCommand::Status(_) => "execution status",
                ExecutionCommand::Resume(_) => "execution resume",
                ExecutionCommand::Restore(_) => "execution restore",
                ExecutionCommand::Cleanup(_) => "execution cleanup",
            },
            Command::Cache(_) => "cache",
            Command::Config(arguments) => match &arguments.command {
                ConfigCommand::Validate => "config validate",
                ConfigCommand::Show => "config show",
                ConfigCommand::Explain(_) => "config explain",
            },
            Command::Extensions(arguments) => match &arguments.command {
                ExtensionsCommand::List(_) => "extensions list",
                ExtensionsCommand::Inspect(_) => "extensions inspect",
                ExtensionsCommand::Doctor(_) => "extensions doctor",
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

    /// Preview or execute explicitly approved exact-duplicate quarantine.
    Apply(ApplyArgs),

    /// Inspect and recover a bounded quarantine run.
    Execution(ExecutionArgs),

    /// Inspect the persistent analysis cache.
    Cache(CacheArgs),

    /// Validate and inspect the deterministic effective policy.
    Config(ConfigArgs),

    /// Inspect explicitly selected, operator-locked extension providers.
    Extensions(ExtensionsArgs),
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
    /// Select exact paths and capture a new immutable execution plan.
    Execution(ExecutionPlanArgs),
    /// Record explicit approval of a reviewed plan fingerprint.
    Approve(ApproveArgs),
}

#[derive(Debug, Args)]
pub struct ExecutionPlanArgs {
    #[arg(long, value_name = "FILE")]
    pub keep: PathBuf,
    #[arg(long, required = true, action = clap::ArgAction::Append, value_name = "FILE")]
    pub candidate: Vec<PathBuf>,
    #[arg(long, required = true, action = clap::ArgAction::Append, value_name = "DIRECTORY")]
    pub root: Vec<PathBuf>,
    /// Restrict selection to these subtrees; defaults to the declared roots.
    #[arg(long, action = clap::ArgAction::Append, value_name = "DIRECTORY")]
    pub subtree: Vec<PathBuf>,
    /// Existing quarantine directory outside all source roots.
    #[arg(long, value_name = "DIRECTORY")]
    pub quarantine: PathBuf,
    #[arg(long, default_value_t = 100)]
    pub max_actions: u64,
    #[arg(long, default_value_t = 1_073_741_824)]
    pub max_in_flight_bytes: u64,
    #[arg(long, default_value_t = 268_435_456)]
    pub reserve_bytes: u64,
    #[arg(long, value_name = "FILE")]
    pub output: PathBuf,
}

#[derive(Debug, Args)]
pub struct ApproveArgs {
    #[arg(long, value_name = "FILE")]
    pub plan: PathBuf,
    /// Copy the exact fingerprint from the plan you reviewed.
    #[arg(long)]
    pub fingerprint: String,
    /// Local audit label; this is not cryptographic authentication.
    #[arg(long)]
    pub approved_by: String,
    #[arg(long, value_name = "FILE")]
    pub output: PathBuf,
}

#[derive(Debug, Args)]
pub struct ApplyArgs {
    #[arg(long, value_name = "FILE")]
    pub plan: PathBuf,
    #[arg(long, value_name = "FILE")]
    pub approval: Option<PathBuf>,
    /// Preview only. Omit to execute the approved bounded quarantine transaction.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ExecutionArgs {
    #[command(subcommand)]
    pub command: ExecutionCommand,
}

#[derive(Debug, Subcommand)]
pub enum ExecutionCommand {
    /// Read-only journal report; never classifies or cleans ambiguous work.
    Status(ExecutionStatusArgs),
    /// Continue only verified, untouched actions from an interrupted run.
    Resume(ExecutionRecoveryArgs),
    /// Return one committed action to its original path without overwriting.
    Restore(ExecutionRestoreArgs),
    /// Remove an empty owned namespace after every action is restored.
    Cleanup(ExecutionRecoveryArgs),
}

#[derive(Debug, Args)]
pub struct ExecutionStatusArgs {
    #[arg(long, value_name = "UUID")]
    pub run: String,
}

#[derive(Debug, Args)]
pub struct ExecutionRecoveryArgs {
    #[arg(long, value_name = "FILE")]
    pub plan: PathBuf,
    #[arg(long, value_name = "FILE")]
    pub approval: PathBuf,
    #[arg(long, value_name = "UUID")]
    pub run: String,
}

#[derive(Debug, Args)]
pub struct ExecutionRestoreArgs {
    #[command(flatten)]
    pub recovery: ExecutionRecoveryArgs,
    #[arg(long, value_name = "ACTION-ID")]
    pub action: String,
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

#[derive(Debug, Args)]
pub struct ExtensionsArgs {
    #[command(subcommand)]
    pub command: ExtensionsCommand,
}

#[derive(Debug, Subcommand)]
pub enum ExtensionsCommand {
    /// List explicitly selected providers and their availability.
    List(ExtensionSourcesArgs),

    /// Inspect one explicitly selected provider, manifest, and operator lock.
    Inspect(ExtensionInspectArgs),

    /// Diagnose compatibility, authorization, and precedence resolution.
    Doctor(ExtensionSourcesArgs),
}

#[derive(Debug, Args)]
pub struct ExtensionSourcesArgs {
    /// Add one exact extension manifest file; no directory or PATH discovery occurs.
    #[arg(long = "manifest", required = true, action = clap::ArgAction::Append, value_name = "FILE")]
    pub manifests: Vec<PathBuf>,

    /// Add the matching operator-owned lock file.
    #[arg(long = "lock", required = true, action = clap::ArgAction::Append, value_name = "FILE")]
    pub locks: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ExtensionInspectArgs {
    /// Qualified extension identifier to inspect.
    #[arg(value_name = "EXTENSION_ID")]
    pub extension_id: String,

    #[command(flatten)]
    pub sources: ExtensionSourcesArgs,
}
