//! Safe, read-only media inventory and exact-duplicate planning.

pub mod adapters;
pub mod app;
pub mod cli;
pub mod discovery;
pub mod domain;
pub mod duplicates;
pub mod hashing;
pub mod inventory;
pub mod planning;
pub mod reports;
pub mod state;

use anyhow::Result;

use crate::cli::Cli;

/// Execute one parsed OptiFlow command.
pub fn run(cli: Cli) -> Result<()> {
    app::run(cli)
}
