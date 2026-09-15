use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::domain::{
    EvidenceDigest, MediaDescriptor, MediaProviderEvidence, MediaStream, NativePath, ToolStatus,
};
use crate::subprocess::{SubprocessCommand, SubprocessRunner};

const PROBE_ARGUMENTS: &[&str] = &[
    "-v",
    "error",
    "-show_entries",
    "format=format_name,duration,bit_rate:stream=index,codec_type,codec_name,width,height,sample_rate,channels",
    "-of",
    "json",
    "/dev/fd/0",
];

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

/// One exact, evidence-bearing ffprobe executable selected for a scan.
#[derive(Debug, Clone)]
pub struct FfprobeAdapter {
    executable: PathBuf,
    evidence: MediaProviderEvidence,
    cache_signature: String,
    runner: SubprocessRunner,
}

impl FfprobeAdapter {
    /// Resolve ffprobe once and bind later invocations to the exact canonical
    /// executable path and binary digest observed here.
    pub fn discover() -> Result<Self> {
        let executable = which::which("ffprobe").context("ffprobe is unavailable")?;
        Self::from_executable(executable, shared_runner().clone())
    }

    fn from_executable(executable: PathBuf, runner: SubprocessRunner) -> Result<Self> {
        let executable = executable
            .canonicalize()
            .with_context(|| format!("failed to resolve ffprobe path {}", executable.display()))?;
        let binary_digest = stable_binary_digest(&executable)?;
        let version = exact_tool_version(&executable, &runner)?;
        if !version.starts_with("ffprobe version ") {
            bail!("ffprobe version output did not identify ffprobe");
        }
        verify_binary(&executable, &binary_digest)?;
        let invocation_fingerprint = digest(&InvocationIdentity {
            provider: "ffprobe",
            binary_digest: &binary_digest,
            arguments: PROBE_ARGUMENTS,
            input_binding: "opened_read_only_file_descriptor",
        })?;
        let evidence = MediaProviderEvidence {
            name: "ffprobe".to_owned(),
            version,
            executable: NativePath::from_path(&executable),
            binary_digest,
            invocation_fingerprint,
        };
        let cache_signature = format!("ffprobe-v1:{}", digest(&evidence)?.value);
        Ok(Self {
            executable,
            evidence,
            cache_signature,
            runner,
        })
    }

    pub fn evidence(&self) -> &MediaProviderEvidence {
        &self.evidence
    }

    pub fn cache_signature(&self) -> &str {
        &self.cache_signature
    }

    /// Inspect the exact source object represented by an opened file handle.
    /// The provider binary is checked before and after the bounded invocation.
    #[cfg(unix)]
    pub fn inspect_file(&self, file: &File, display_path: &Path) -> Result<MediaDescriptor> {
        verify_binary(&self.executable, &self.evidence.binary_digest)
            .context("ffprobe binary identity changed before inspection")?;
        let command = SubprocessCommand::new(self.executable.as_os_str().to_owned())
            .args(PROBE_ARGUMENTS.iter().copied());
        let output = self.runner.run_with_file(&command, file).with_context(|| {
            format!(
                "ffprobe handle inspection failed for {}",
                display_path.display()
            )
        })?;
        verify_binary(&self.executable, &self.evidence.binary_digest)
            .context("ffprobe binary identity changed during inspection")?;
        if !output.stderr.is_empty() {
            bail!(
                "ffprobe reported error output for {}: {}",
                display_path.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let parsed: ProbeOutput = serde_json::from_slice(&output.stdout)
            .context("ffprobe returned invalid JSON after a successful exit")?;
        descriptor_from_probe(parsed).with_context(|| {
            format!(
                "ffprobe returned invalid evidence for {}",
                display_path.display()
            )
        })
    }

    #[cfg(not(unix))]
    pub fn inspect_file(&self, _file: &File, display_path: &Path) -> Result<MediaDescriptor> {
        anyhow::bail!(
            "ffprobe handle inspection is unavailable for {} on this platform",
            display_path.display()
        )
    }
}

#[derive(Serialize)]
struct InvocationIdentity<'a> {
    provider: &'a str,
    binary_digest: &'a EvidenceDigest,
    arguments: &'a [&'a str],
    input_binding: &'a str,
}

fn shared_runner() -> &'static SubprocessRunner {
    static RUNNER: OnceLock<SubprocessRunner> = OnceLock::new();
    RUNNER.get_or_init(SubprocessRunner::default)
}

pub fn inspect(path: &Path) -> Result<MediaDescriptor> {
    let adapter = FfprobeAdapter::discover()?;
    let file = File::open(path)
        .with_context(|| format!("failed to open media for inspection: {}", path.display()))?;
    adapter.inspect_file(&file, path)
}

/// Inspect one already opened source handle with a freshly bound provider.
/// Scan coordination should prefer one [`FfprobeAdapter`] for the whole run.
pub fn inspect_file(file: &File, display_path: &Path) -> Result<MediaDescriptor> {
    FfprobeAdapter::discover()?.inspect_file(file, display_path)
}

fn descriptor_from_probe(parsed: ProbeOutput) -> Result<MediaDescriptor> {
    let format = parsed
        .format
        .context("ffprobe response omitted the format object")?;
    let format_name = format
        .format_name
        .filter(|name| !name.trim().is_empty())
        .context("ffprobe response omitted a non-empty format name")?;
    if parsed.streams.is_empty() {
        bail!("ffprobe response did not contain any media streams");
    }
    let mut stream_indices = BTreeSet::new();
    let mut streams = Vec::with_capacity(parsed.streams.len());
    for stream in parsed.streams {
        if !stream_indices.insert(stream.index) {
            bail!("ffprobe response contained a duplicate stream index");
        }
        if stream.width == Some(0) || stream.height == Some(0) || stream.channels == Some(0) {
            bail!("ffprobe response contained a zero-valued stream observation");
        }
        streams.push(MediaStream {
            index: stream.index,
            codec_type: non_empty(stream.codec_type),
            codec_name: non_empty(stream.codec_name),
            width: stream.width,
            height: stream.height,
            sample_rate: parse_optional_u32(stream.sample_rate.as_deref(), "sample rate")?,
            channels: stream.channels,
        });
    }

    Ok(MediaDescriptor {
        format_name: Some(format_name),
        duration_seconds: parse_optional_f64(format.duration.as_deref(), "duration")?,
        bit_rate: parse_optional_u64(format.bit_rate.as_deref(), "bit rate")?,
        streams,
    })
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn parse_optional_u32(value: Option<&str>, label: &str) -> Result<Option<u32>> {
    parse_optional(value, label)
}

fn parse_optional_u64(value: Option<&str>, label: &str) -> Result<Option<u64>> {
    parse_optional(value, label)
}

fn parse_optional<T>(value: Option<&str>, label: &str) -> Result<Option<T>>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match value {
        None | Some("N/A") => Ok(None),
        Some(value) => value
            .parse()
            .map(Some)
            .map_err(|error| anyhow::anyhow!("ffprobe returned an invalid {label}: {error}")),
    }
}

fn parse_optional_f64(value: Option<&str>, label: &str) -> Result<Option<f64>> {
    let parsed = parse_optional::<f64>(value, label)?;
    if parsed.is_some_and(|value| !value.is_finite() || value < 0.0) {
        bail!("ffprobe returned an invalid {label}");
    }
    Ok(parsed)
}

fn exact_tool_version(executable: &Path, runner: &SubprocessRunner) -> Result<String> {
    let command = SubprocessCommand::new(executable.as_os_str().to_owned()).arg("-version");
    let output = runner
        .run(&command)
        .context("failed to query the exact ffprobe version")?;
    if !output.stderr.is_empty() {
        bail!("ffprobe version query produced error output");
    }
    let text = String::from_utf8(output.stdout).context("ffprobe version output was not UTF-8")?;
    text.lines()
        .next()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .context("ffprobe version output was empty")
}

fn stable_binary_digest(path: &Path) -> Result<EvidenceDigest> {
    let mut file = File::open(path)
        .with_context(|| format!("failed to open provider binary {}", path.display()))?;
    let before = file
        .metadata()
        .context("failed to inspect provider binary")?;
    if !before.is_file() {
        bail!("provider binary is not a regular file");
    }
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .context("failed to read provider binary")?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let after = file
        .metadata()
        .context("failed to re-inspect provider binary")?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        bail!("provider binary changed while its identity was observed");
    }
    Ok(EvidenceDigest {
        algorithm: "blake3-256".to_owned(),
        value: hasher.finalize().to_hex().to_string(),
    })
}

fn verify_binary(path: &Path, expected: &EvidenceDigest) -> Result<()> {
    let actual = stable_binary_digest(path)?;
    if &actual != expected {
        bail!("provider binary digest no longer matches the accepted identity");
    }
    Ok(())
}

fn digest<T: Serialize>(value: &T) -> Result<EvidenceDigest> {
    let bytes = serde_json::to_vec(value).context("failed to fingerprint provider evidence")?;
    Ok(EvidenceDigest {
        algorithm: "blake3-256".to_owned(),
        value: blake3::hash(&bytes).to_hex().to_string(),
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

/// Return the legacy path-and-version discovery signature.
///
/// Scan cache identity uses [`FfprobeAdapter::cache_signature`] so it also
/// binds executable bytes and invocation semantics.
pub fn signature(name: &str) -> Option<String> {
    let executable = which::which(name).ok()?;
    let version = tool_version(Some(&executable)).unwrap_or_else(|| "unknown-version".to_owned());
    Some(format!("{}|{version}", executable.to_string_lossy()))
}

fn tool_version(executable: Option<&Path>) -> Option<String> {
    let command = SubprocessCommand::new(executable?.as_os_str().to_owned()).arg("-version");
    let output = shared_runner().run(&command).ok()?;
    String::from_utf8(output.stdout)
        .ok()
        .and_then(|text| text.lines().next().map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use tempfile::tempdir;

    use super::*;
    use crate::subprocess::SubprocessLimits;

    #[test]
    fn semantic_validation_rejects_empty_and_duplicate_stream_evidence() {
        let empty: ProbeOutput =
            serde_json::from_str(r#"{"streams":[],"format":{"format_name":"png_pipe"}}"#)
                .expect("empty fixture");
        assert!(descriptor_from_probe(empty).is_err());

        let duplicate: ProbeOutput = serde_json::from_str(
            r#"{
                "streams":[
                    {"index":0,"codec_type":"video","codec_name":"png","width":1,"height":1},
                    {"index":0,"codec_type":"video","codec_name":"png","width":1,"height":1}
                ],
                "format":{"format_name":"png_pipe"}
            }"#,
        )
        .expect("duplicate fixture");
        assert!(descriptor_from_probe(duplicate).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn successful_exit_with_error_output_is_not_accepted_as_evidence() {
        let directory = tempdir().expect("temporary directory");
        let executable = write_provider(
            directory.path(),
            "stderr-provider",
            r#"printf '%s\n' '{"streams":[{"index":0,"codec_type":"video","codec_name":"png","width":1,"height":1}],"format":{"format_name":"png_pipe"}}'
printf '%s\n' 'decode error' >&2"#,
        );
        let adapter =
            FfprobeAdapter::from_executable(executable, test_runner(Duration::from_secs(1)))
                .expect("adapter identity");
        let input_path = directory.path().join("input.png");
        fs::write(&input_path, b"fixture").expect("input fixture");
        let input = File::open(&input_path).expect("opened input");

        let error = adapter
            .inspect_file(&input, &input_path)
            .expect_err("stderr must invalidate evidence");
        assert!(error.to_string().contains("reported error output"));
    }

    #[cfg(unix)]
    #[test]
    fn successful_exit_with_invalid_semantics_is_not_accepted_as_evidence() {
        let directory = tempdir().expect("temporary directory");
        let executable = write_provider(
            directory.path(),
            "invalid-provider",
            "printf '%s\\n' '{\"streams\":[],\"format\":{\"format_name\":\"png_pipe\"}}'",
        );
        let adapter =
            FfprobeAdapter::from_executable(executable, test_runner(Duration::from_secs(1)))
                .expect("adapter identity");
        let input_path = directory.path().join("input.png");
        fs::write(&input_path, b"fixture").expect("input fixture");
        let input = File::open(&input_path).expect("opened input");

        let error = adapter
            .inspect_file(&input, &input_path)
            .expect_err("empty streams must invalidate evidence");
        assert!(error.to_string().contains("invalid evidence"));
    }

    #[cfg(unix)]
    #[test]
    fn provider_timeout_remains_typed_through_the_adapter() {
        let directory = tempdir().expect("temporary directory");
        let executable = write_provider(directory.path(), "timeout-provider", "sleep 1");
        let adapter =
            FfprobeAdapter::from_executable(executable, test_runner(Duration::from_millis(30)))
                .expect("adapter identity");
        let input_path = directory.path().join("input.png");
        fs::write(&input_path, b"fixture").expect("input fixture");
        let input = File::open(&input_path).expect("opened input");

        let error = adapter
            .inspect_file(&input, &input_path)
            .expect_err("provider must time out");
        let text = format!("{error:#}");
        assert!(text.contains("exceeded timeout"), "{text}");
    }

    #[cfg(unix)]
    #[test]
    fn replaced_provider_binary_is_rejected_before_inspection() {
        let directory = tempdir().expect("temporary directory");
        let executable = write_provider(
            directory.path(),
            "replaceable-provider",
            r#"printf '%s\n' '{"streams":[{"index":0,"codec_type":"video","codec_name":"png","width":1,"height":1}],"format":{"format_name":"png_pipe"}}'"#,
        );
        let adapter = FfprobeAdapter::from_executable(
            executable.clone(),
            test_runner(Duration::from_secs(1)),
        )
        .expect("adapter identity");
        write_provider(
            directory.path(),
            "replaceable-provider",
            r#"printf '%s\n' '{"streams":[{"index":0,"codec_type":"video","codec_name":"mjpeg","width":1,"height":1}],"format":{"format_name":"jpeg_pipe"}}'"#,
        );
        let input_path = directory.path().join("input.png");
        fs::write(&input_path, b"fixture").expect("input fixture");
        let input = File::open(&input_path).expect("opened input");

        let error = adapter
            .inspect_file(&input, &input_path)
            .expect_err("replaced provider must fail closed");
        assert!(
            error
                .to_string()
                .contains("identity changed before inspection")
        );
    }

    #[cfg(unix)]
    fn write_provider(directory: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = directory.join(name);
        let script = format!(
            "#!/bin/sh\nif [ \"${{1:-}}\" = \"-version\" ]; then\n  printf '%s\\n' 'ffprobe version fixture'\n  exit 0\nfi\n{body}\n"
        );
        fs::write(&path, script).expect("provider fixture");
        let mut permissions = fs::metadata(&path)
            .expect("provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("provider permissions");
        path
    }

    #[cfg(unix)]
    fn test_runner(timeout: Duration) -> SubprocessRunner {
        SubprocessRunner::new(SubprocessLimits {
            timeout,
            poll_interval: Duration::from_millis(5),
            max_stdout_bytes: 16 * 1024,
            max_stderr_bytes: 16 * 1024,
            max_concurrent_children: 1,
        })
        .expect("test runner")
    }
}
