//! Deferred explicit folder trial. Reports/locators are private local evidence.
use std::{io::Read,path::PathBuf};
use anyhow::{Result,Context,ensure,bail};
fn main()->Result<()> {
    let args=std::env::args_os().skip(1).collect::<Vec<_>>();ensure!(args.len()==2,"usage: video_folder discover|run <request.json> OR video_folder recover <journal-directory>");
    if args[0]=="recover" {println!("{}",serde_json::to_string_pretty(&optiflow::video_folder::inspect_recovery(&PathBuf::from(&args[1]))?)?);return Ok(());}
    let mut bytes=Vec::new();std::fs::File::open(&args[1]).context("folder request unavailable")?.take(131_073).read_to_end(&mut bytes)?;ensure!(bytes.len()<=131_072,"folder request exceeds 128 KiB");
    let request:optiflow::video_folder::VideoFolderRequest=serde_json::from_slice(&bytes)?;
    let signals=optiflow::signals::SignalState::install()?;
    if args[0]=="discover" {println!("{}",serde_json::to_string_pretty(&optiflow::video_folder::discover(&request,||signals.is_cancelled())?)?);}
    else if args[0]=="run" {println!("{}",serde_json::to_string_pretty(&optiflow::video_folder::run(&request,||signals.is_cancelled())?)?);}
    else {bail!("action must be discover, run or recover");}Ok(())
}
