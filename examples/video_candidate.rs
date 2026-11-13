//! Explicit development trial entry; no installs, discovery or replacement.
use anyhow::{Context, Result, ensure};
use optiflow::video_production::{VideoProductionRequest, produce};
use std::io::Read;
fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .context("usage: video_candidate <request.json>")?;
    ensure!(
        args.next().is_none(),
        "usage: video_candidate <request.json>"
    );
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(131_073)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 131_072,
        "video production request exceeds 128 KiB"
    );
    let request: VideoProductionRequest = serde_json::from_slice(&bytes)?;
    let signals = optiflow::signals::SignalState::install()?;
    let outcome = produce(&request, || signals.is_cancelled())?;
    println!("{}", serde_json::to_string_pretty(&outcome)?);
    Ok(())
}
