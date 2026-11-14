//! Closed worker inside the existing observed cgroup/tmpfs guard. Two passes
//! share scratch; no arbitrary argv or original host source path is accepted.
use super::*;
use crate::subprocess::{SubprocessCommand, SubprocessLimits, SubprocessRunner};
use crate::video_sandbox::PinnedLauncher;
use anyhow::Context;
use planning::{Allocation, INPUT_FILE, OUTPUT_FILE, encode_argv};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Write},
    time::{Duration, Instant},
};
pub(crate) const FLAG: &str = "__optiflow-mp4-worker-v1";
pub(crate) const MAGIC: &[u8; 8] = b"OPMP4V1\n";
pub(crate) const REPORT_BYTES: usize = 131_072;
pub(crate) const TRACE_BYTES: usize = 12_582_912;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Operation {
    Probe,
    Encode,
    DecodeVideo,
    DecodeAudio,
}
impl Operation {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Probe => "probe",
            Self::Encode => "encode",
            Self::DecodeVideo => "decode_video",
            Self::DecodeAudio => "decode_audio",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Job {
    pub schema: String,
    pub operation: Operation,
    pub input_digest: EvidenceDigest,
    pub input_bytes: u64,
    pub ffmpeg: PinnedLauncher,
    pub ffprobe: PinnedLauncher,
    pub media: container::Summary,
    pub control: RateControl,
    pub allocation: Allocation,
    pub elapsed_ms: u64,
    pub provider_memory_bytes: u64,
    pub scratch_bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Child {
    pub tool: String,
    pub argv: Vec<String>,
    pub stdout_digest: EvidenceDigest,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub exit_code: i32,
    pub elapsed_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Report {
    pub schema: String,
    pub job_digest: EvidenceDigest,
    pub output_digest: EvidenceDigest,
    pub output_bytes: u64,
    pub children: Vec<Child>,
    pub pass_log_digests: BTreeMap<String, EvidenceDigest>,
    pub runtime_qualified: bool,
    pub source_mutation: bool,
}
impl Job {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.schema == "optiflow.mp4-worker-job.v1"
                && (1..=INPUT_BYTES).contains(&self.input_bytes)
                && (1..=MAX_ELAPSED_MS).contains(&self.elapsed_ms)
                && (1..=OUTPUT_BYTES).contains(&self.allocation.maximum_candidate_bytes),
            "bounded explicit MP4 worker job required"
        );
        planning::valid_digest(&self.input_digest)?;
        ensure!(
            (134_217_728..=2_147_483_648).contains(&self.provider_memory_bytes)
                && (16_777_216..=2_147_483_648).contains(&self.scratch_bytes)
                && self.scratch_bytes % 4096 == 0
                && self
                    .scratch_bytes
                    .checked_add(67_108_864)
                    .is_some_and(|n| n <= self.provider_memory_bytes),
            "worker resource policy exceeds MP4 scope"
        );
        for p in [&self.ffmpeg, &self.ffprobe] {
            normalized(&p.path)?;
            planning::valid_digest(&p.digest)?;
        }
        ensure!(
            self.ffmpeg.path != self.ffprobe.path,
            "distinct worker tools required"
        );
        Ok(())
    }
}
fn decoder_args(audio: bool, scale: u32) -> Vec<String> {
    let mut args = [
        "-hide_banner",
        "-nostdin",
        "-loglevel",
        "error",
        "-xerror",
        "-err_detect",
        "explode",
        "-copyts",
        "-i",
        INPUT_FILE,
        "-map",
        if audio { "0:a:0" } else { "0:v:0" },
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    args.extend(
        if audio {
            vec!["-vn", "-c:a", "pcm_f32le"]
        } else {
            vec![
                "-an",
                "-c:v",
                "rawvideo",
                "-pix_fmt",
                "+yuv420p",
                "-fps_mode:v",
                "passthrough",
            ]
        }
        .into_iter()
        .map(str::to_owned),
    );
    if !audio {
        args.extend(["-enc_time_base:v".into(), format!("1/{scale}")]);
    }
    args.extend(
        ["-f", "framehash", "-hash", "sha256", "-"]
            .into_iter()
            .map(str::to_owned),
    );
    args
}
pub(crate) fn expected_argv(job: &Job) -> Result<Vec<Vec<String>>> {
    match job.operation {
        Operation::Probe => Ok(vec![
            [
                "-v",
                "error",
                "-show_streams",
                "-show_format",
                "-of",
                "json",
                INPUT_FILE,
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ]),
        Operation::DecodeVideo => Ok(vec![decoder_args(false, job.media.video.timescale)]),
        Operation::DecodeAudio => Ok(vec![decoder_args(true, job.media.video.timescale)]),
        Operation::Encode => encode_argv(
            &job.control,
            &job.allocation,
            job.media.video.timescale,
            job.media.movie_timescale,
        ),
    }
}
fn operate(job: &Job) -> Result<()> {
    job.validate()?;
    #[cfg(target_os = "linux")]
    {
        crate::video_sandbox::observe_guard(job.provider_memory_bytes, job.scratch_bytes)?;
    }
    #[cfg(not(target_os = "linux"))]
    {
        anyhow::bail!("MP4 internal worker requires the observed Linux guard");
    }
    let started = Instant::now();
    let limit = Duration::from_millis(job.elapsed_ms);
    let guard = || {
        ensure!(started.elapsed() < limit, "MP4 worker deadline exhausted");
        Ok(())
    };
    let mut pins = vec![];
    for pin in [&job.ffmpeg, &job.ffprobe] {
        let f = BoundedFileObservation::open_guarded(&pin.path, 134_217_728, &guard)?;
        ensure!(
            f.digest() == &pin.digest
                && f.snapshot().mode & 0o111 != 0
                && f.snapshot().mode & 0o6000 == 0,
            "worker tool differs from selected safe pin"
        );
        pins.push(f);
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(INPUT_FILE)?;
    let mut input = std::io::stdin().lock().take(
        job.input_bytes
            .checked_add(1)
            .context("worker input bound overflow")?,
    );
    let mut total = 0u64;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 65536];
    loop {
        guard()?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        ensure!(
            total <= job.input_bytes,
            "worker input exceeds exact selected length"
        );
        hasher.update(&buffer[..count]);
        file.write_all(&buffer[..count])?;
    }
    file.sync_all()?;
    drop(file);
    ensure!(
        total == job.input_bytes && hasher.finalize().to_hex().as_str() == job.input_digest.value,
        "worker input content differs"
    );
    let actual = super::inspect(Path::new(INPUT_FILE), job.input_bytes, || {
        started.elapsed() >= limit
    })?;
    ensure!(
        actual.media == job.media,
        "worker container/source clock declaration differs"
    );
    let mut children = vec![];
    let mut invoke = |tool: &Path, args: Vec<String>, maximum: usize| -> Result<Vec<u8>> {
        guard()?;
        for pin in &pins {
            pin.verify_current_guarded(&guard)?;
        }
        let remaining = limit
            .checked_sub(started.elapsed())
            .filter(|d| !d.is_zero())
            .context("MP4 child allowance exhausted")?;
        let runner = SubprocessRunner::new(SubprocessLimits {
            timeout: remaining,
            poll_interval: Duration::from_millis(10),
            max_stdout_bytes: maximum,
            max_stderr_bytes: 65536,
            max_concurrent_children: 1,
        })?;
        let output = runner.run_with_bounded_pipes_and_cancel(
            &SubprocessCommand::new(tool.as_os_str().to_owned())
                .args(&args)
                .clear_environment()
                .current_directory("/tmp"),
            &[],
            || false,
        )?;
        ensure!(
            output.exit_code == Some(0) && output.stderr.is_empty(),
            "MP4 child refused or emitted diagnostics"
        );
        guard()?;
        children.push(Child {
            tool: tool.to_string_lossy().into(),
            argv: args,
            stdout_digest: hash(&output.stdout),
            stdout_bytes: output.stdout.len() as u64,
            stderr_bytes: 0,
            exit_code: 0,
            elapsed_ms: output.elapsed.as_millis().try_into()?,
        });
        Ok(output.stdout)
    };
    let mut logs = BTreeMap::new();
    let bytes = match job.operation {
        Operation::Probe => invoke(
            &job.ffprobe.path,
            [
                "-v",
                "error",
                "-show_streams",
                "-show_format",
                "-of",
                "json",
                INPUT_FILE,
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            2_097_152,
        )?,
        Operation::DecodeVideo => invoke(
            &job.ffmpeg.path,
            decoder_args(false, job.media.video.timescale),
            TRACE_BYTES,
        )?,
        Operation::DecodeAudio => invoke(
            &job.ffmpeg.path,
            decoder_args(true, job.media.video.timescale),
            TRACE_BYTES,
        )?,
        Operation::Encode => {
            for args in encode_argv(
                &job.control,
                &job.allocation,
                job.media.video.timescale,
                job.media.movie_timescale,
            )? {
                ensure!(
                    invoke(&job.ffmpeg.path, args, 4096)?.is_empty(),
                    "encoder emitted unexpected stdout"
                );
            }
            let mut used = 0u64;
            for e in fs::read_dir("/tmp")? {
                let e = e?;
                let name = e
                    .file_name()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("unexpected worker member encoding"))?;
                if name.starts_with("delivery-pass") {
                    ensure!(
                        ["delivery-pass-0.log", "delivery-pass-0.log.mbtree"]
                            .contains(&name.as_str())
                            && e.file_type()?.is_file(),
                        "unknown pass-log member refused"
                    );
                    let f = BoundedFileObservation::open_guarded(&e.path(), 33_554_432, &guard)?;
                    used = used
                        .checked_add(f.snapshot().logical_bytes)
                        .context("pass-log size overflow")?;
                    ensure!(used <= 67_108_864, "pass-log byte allowance exceeded");
                    logs.insert(name, f.digest().clone());
                }
            }
            if matches!(job.control, RateControl::TargetSize { .. }) {
                ensure!(
                    logs.contains_key("delivery-pass-0.log"),
                    "two-pass statistics absent"
                );
            }
            let candidate = BoundedFileObservation::open_mp4(
                Path::new(OUTPUT_FILE),
                job.allocation.maximum_candidate_bytes,
                &guard,
            )?;
            candidate.read_mp4_bytes(&guard)?
        }
    };
    for pin in &pins {
        pin.verify_current_guarded(&guard)?;
    }
    guard()?;
    let report = Report {
        schema: "optiflow.mp4-worker-report.v1".into(),
        job_digest: digest(job)?,
        output_digest: hash(&bytes),
        output_bytes: bytes.len() as u64,
        children,
        pass_log_digests: logs,
        runtime_qualified: false,
        source_mutation: false,
    };
    let header = serde_json::to_vec(&report)?;
    ensure!(header.len() <= REPORT_BYTES, "worker report exceeds bound");
    let mut output = std::io::stdout().lock();
    output.write_all(MAGIC)?;
    output.write_all(&(header.len() as u32).to_be_bytes())?;
    output.write_all(&header)?;
    output.write_all(&bytes)?;
    output.flush()?;
    Ok(())
}
pub fn internal_worker(args: &[OsString]) -> Option<std::process::ExitCode> {
    if args.get(1).is_none_or(|v| v != FLAG) {
        return None;
    }
    let result = (|| -> Result<()> {
        ensure!(args.len() == 3, "one closed worker job required");
        let text = args[2].to_str().context("worker job UTF8 required")?;
        ensure!(text.len() <= REPORT_BYTES, "worker job exceeds bound");
        operate(&serde_json::from_str::<Job>(text)?)
    })();
    Some(if result.is_ok() {
        std::process::ExitCode::SUCCESS
    } else {
        let _ = writeln!(
            std::io::stderr(),
            "MP4 worker refused; retain parent attempt"
        );
        std::process::ExitCode::FAILURE
    })
}
pub(crate) fn unpack(mut bytes: Vec<u8>, job: &Job) -> Result<(Report, Vec<u8>)> {
    ensure!(
        bytes.len() >= 12 && bytes.get(..8) == Some(MAGIC.as_slice()),
        "worker frame missing"
    );
    let size = u32::from_be_bytes(bytes[8..12].try_into()?) as usize;
    ensure!(
        size <= REPORT_BYTES && bytes.len() >= 12 + size,
        "bounded complete worker frame required"
    );
    let report: Report = serde_json::from_slice(&bytes[12..12 + size])?;
    let data = bytes.split_off(12 + size);
    ensure!(
        report.schema == "optiflow.mp4-worker-report.v1"
            && report.job_digest == digest(job)?
            && report.output_bytes == data.len() as u64
            && report.output_digest == hash(&data)
            && !report.runtime_qualified
            && !report.source_mutation,
        "worker output identity/authority differs"
    );
    let expected = if job.operation == Operation::Encode
        && matches!(job.control, RateControl::TargetSize { .. })
    {
        2
    } else {
        1
    };
    ensure!(
        report.children.len() == expected
            && report
                .children
                .iter()
                .all(|c| c.exit_code == 0 && c.stderr_bytes == 0),
        "worker child acceptance missing"
    );
    Ok((report, data))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_is_not_a_normal_cli_action_and_decoder_never_trims_or_resamples() {
        assert!(internal_worker(&["optiflow".into(), "--help".into()]).is_none());
        for audio in [false, true] {
            let a = decoder_args(audio, 30000);
            for forbidden in ["-t", "-shortest", "-ar", "-ac", "-vf", "-af"] {
                assert!(!a.iter().any(|v| v == forbidden));
            }
        }
    }
}
