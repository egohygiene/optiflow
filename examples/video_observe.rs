//! Development observation example; generates no optimized media.

use std::path::PathBuf;

use anyhow::{Result, bail};
use optiflow::adapters::video::CaptureLimits;
use optiflow::signals::SignalState;
use optiflow::video_observation::observe_video_source;

fn main() -> Result<()> {
    let arguments: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if arguments.len() != 4 {
        bail!("usage: video_observe <absolute-source> <absolute-ffmpeg> <absolute-ffprobe> <empty-private-directory>");
    }
    let limits = CaptureLimits {
        candidate_bytes: 16 * 1024 * 1024,
        probe_bytes: 4 * 1024 * 1024,
        stderr_bytes: 256 * 1024,
        elapsed_ms: 30_000,
    };
    let signals = SignalState::install()?;
    let observation = observe_video_source(
        &arguments[0], &arguments[1], &arguments[2], &arguments[3],
        limits, || signals.is_cancelled(),
    )?;
    println!("{}", serde_json::to_string_pretty(&observation)?);
    Ok(())
}
