use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::domain::{MediaDescriptor, MediaStream, ToolStatus};
use crate::subprocess::{SubprocessCommand, SubprocessRunner};

#[derive(Debug, Deserialize)]
struct ProbeOutput {
    #[serde(default)]
    streams: Vec<ProbeStream>,
    format: Option<ProbeFormat>,
}

#[derive(Debug, Deserialize)]
struct ProbeFormat {
    format_name: Option<String>,
    duration: Option<String>,
    bit_rate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProbeStream {
    index: u32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    sample_rate: Option<String>,
    channels: Option<u32>,
}

fn runner() -> &'static SubprocessRunner {
    static RUNNER: OnceLock<SubprocessRunner> = OnceLock::new();
    RUNNER.get_or_init(SubprocessRunner::default)
}

pub fn inspect(path: &Path) -> Result<MediaDescriptor> {
    let command = SubprocessCommand::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=format_name,duration,bit_rate:stream=index,codec_type,codec_name,width,height,sample_rate,channels",
            "-of",
            "json",
        ])
        .arg(path.as_os_str().to_owned());

    let parsed: ProbeOutput = runner()
        .run_json(&command)
        .with_context(|| format!("ffprobe inspection failed for {}", path.display()))?;

    let format = parsed.format;
    Ok(MediaDescriptor {
        format_name: format.as_ref().and_then(|value| value.format_name.clone()),
        duration_seconds: format
            .as_ref()
            .and_then(|value| value.duration.as_deref())
            .and_then(|value| value.parse().ok()),
        bit_rate: format
            .as_ref()
            .and_then(|value| value.bit_rate.as_deref())
            .and_then(|value| value.parse().ok()),
        streams: parsed
            .streams
            .into_iter()
            .map(|stream| MediaStream {
                index: stream.index,
                codec_type: stream.codec_type,
                codec_name: stream.codec_name,
                width: stream.width,
                height: stream.height,
                sample_rate: stream.sample_rate.and_then(|value| value.parse().ok()),
                channels: stream.channels,
            })
            .collect(),
    })
}

pub fn status(name: &str, required_for: &str) -> ToolStatus {
    let executable = which::which(name).ok();
    let version = tool_version(executable.as_deref());

    ToolStatus {
        name: name.to_owned(),
        required_for: required_for.to_owned(),
        available: executable.is_some(),
        executable: executable.map(|path| path.to_string_lossy().into_owned()),
        version,
    }
}

pub fn signature(name: &str) -> Option<String> {
    let executable = which::which(name).ok()?;
    let version = tool_version(Some(&executable)).unwrap_or_else(|| "unknown-version".to_owned());
    Some(format!("{}|{version}", executable.to_string_lossy()))
}

fn tool_version(executable: Option<&Path>) -> Option<String> {
    let command = SubprocessCommand::new(executable?.as_os_str().to_owned()).arg("-version");
    let output = runner().run(&command).ok()?;
    String::from_utf8(output.stdout)
        .ok()
        .and_then(|text| text.lines().next().map(str::to_owned))
}
