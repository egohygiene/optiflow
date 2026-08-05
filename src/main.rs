use anyhow::Result;
use clap::Parser;

use optiflow::cli::Cli;

fn main() -> Result<()> {
    optiflow::run(Cli::parse())
}
