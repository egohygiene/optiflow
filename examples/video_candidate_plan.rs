//! Print a review-only video recipe from supplied declarations; execute no tool.

use std::fs::File;
use std::io::Read;

use anyhow::{Context, Result, bail};
use optiflow::video_candidate::{parse_request, prepare_candidate};

fn main() -> Result<()> {
    let mut arguments = std::env::args_os().skip(1);
    let Some(path) = arguments.next() else {
        bail!("usage: video_candidate_plan <request.json>");
    };
    if arguments.next().is_some() {
        bail!("usage: video_candidate_plan <request.json>");
    }
    const MAX_REQUEST_BYTES: u64 = 128 * 1024;
    let mut bytes = Vec::new();
    File::open(path)
        .context("cannot open review request")?
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("cannot read review request")?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        bail!("review request exceeds 128 KiB");
    }
    let value = serde_json::from_slice(&bytes).context("invalid request JSON")?;
    let request = parse_request(&value)
        .map_err(|reason| anyhow::anyhow!("request refused: {reason:?}"))?;
    let review = prepare_candidate(&request)
        .map_err(|reason| anyhow::anyhow!("preparation refused: {reason:?}"))?;
    println!("{}", serde_json::to_string_pretty(&review)?);
    Ok(())
}
