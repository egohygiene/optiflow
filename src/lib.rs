//! Safe, read-only media inventory and exact-duplicate planning.

pub mod adapters;
pub mod app;
pub mod cli;
pub mod configuration;
pub mod contracts;
pub mod discovery;
pub mod domain;
pub mod duplicates;
pub mod filesystem;
pub mod hashing;
pub mod inventory;
pub mod outcome;
pub mod planning;
pub mod render;
pub mod reports;
pub mod signals;
pub mod state;

use crate::cli::Cli;
use crate::signals::SignalState;

/// Execute one parsed optiflow command.
pub fn run(
    cli: Cli,
    signals: &SignalState,
) -> (crate::outcome::CommandResult, crate::cli::OutputFormat) {
    app::run(cli, signals)
}
