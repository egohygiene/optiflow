//! Byte-bound video providers for read-only observation and untrusted capture.
//!
//! This boundary does not consume video_candidate::RequestedLimits and cannot
//! establish candidate acceptance. Pipe overflow aborts capture; one cooperative
//! deadline covers a provider session. RSS, provider scratch and escaped-process
//! containment remain unavailable.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::domain::{EvidenceDigest, MediaProviderEvidence, NativePath};
use crate::subprocess::{SubprocessCommand, SubprocessLimits, SubprocessOutput, SubprocessRunner};
use crate::video_candidate::{PROVIDER_VERSION, VideoOperation, invocation_recipe};
use crate::video_budget::OperationBudget;
use crate::video_io::{BoundedFileObservation, validate_private_directory_guarded};

const MAX_BINARY_BYTES: u64 = 128 * 1024 * 1024;
const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const VERSION_BYTES: usize = 64 * 1024;

/// Capture byte limits and a shared operation deadline, not provider memory quotas.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CaptureLimits {
    pub candidate_bytes: usize,
    pub probe_bytes: usize,
    pub stderr_bytes: usize,
    /// Shared by source observation, provider binding and all calls through this
    /// provider instance. Host IO checks are cooperative, not syscall preemption.
    pub elapsed_ms: u64,
}

impl CaptureLimits {
    pub(crate) fn check(self) -> Result<()> {
        ensure!((1..=16 * 1024 * 1024).contains(&self.candidate_bytes), "invalid candidate capture bound");
        ensure!((1..=4 * 1024 * 1024).contains(&self.probe_bytes), "invalid probe capture bound");
        ensure!((1..=256 * 1024).contains(&self.stderr_bytes), "invalid diagnostic capture bound");
        ensure!((1..=30_000).contains(&self.elapsed_ms), "invalid operation elapsed bound");
        Ok(())
    }

    fn runner(self, stdout_bytes: usize, remaining: Duration) -> Result<SubprocessRunner> {
        Ok(SubprocessRunner::new(SubprocessLimits {
            timeout: remaining,
            poll_interval: Duration::from_millis(10),
            max_stdout_bytes: stdout_bytes,
            max_stderr_bytes: self.stderr_bytes,
            max_concurrent_children: 1,
        })?)
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CaptureCapabilities {
    pub retained_stdout_and_stderr_bounded: bool,
    pub direct_child_elapsed_bounded: bool,
    pub output_cap_terminates_child_immediately: bool,
    pub output_cap_aborts_capture_on_detection: bool,
    pub stdin_requires_full_copy: bool,
    pub process_group_cleanup_before_reap: bool,
    pub shared_operation_deadline_checked: bool,
    pub host_file_io_preemptible: bool,
    pub provider_memory_enforced: bool,
    pub provider_scratch_enforced: bool,
    pub process_tree_contained: bool,
    pub source_or_candidate_validation: bool,
}

pub fn capture_capabilities() -> CaptureCapabilities {
    CaptureCapabilities {
        retained_stdout_and_stderr_bounded: true,
        direct_child_elapsed_bounded: true,
        output_cap_terminates_child_immediately: false,
        output_cap_aborts_capture_on_detection: crate::subprocess::bounded_pipe_capture_supported(),
        stdin_requires_full_copy: false,
        process_group_cleanup_before_reap: crate::subprocess::bounded_pipe_capture_supported(),
        shared_operation_deadline_checked: true,
        host_file_io_preemptible: false,
        provider_memory_enforced: false,
        provider_scratch_enforced: false,
        process_tree_contained: false,
        source_or_candidate_validation: false,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InvocationEvidence {
    pub provider: MediaProviderEvidence,
    pub build_fingerprint: EvidenceDigest,
    pub argv: Vec<String>,
    pub input_digest: EvidenceDigest,
    pub input_bytes: u64,
    pub stdout_digest: EvidenceDigest,
    pub captured_stdout_bytes: u64,
    pub captured_stderr_bytes: u64,
    /// Runner elapsed time; excludes source/provider hashing and parsing.
    pub elapsed_ms: u64,
    pub stdout_limit_bytes: usize,
    pub stderr_limit_bytes: usize,
    /// Rounded up for display; the exact child allocation is recorded below.
    pub child_elapsed_limit_ms: u64,
    pub child_elapsed_limit_ns: u64,
    pub operation_elapsed_limit_ms: u64,
    pub operation_elapsed_ms: u64,
    pub capture_capabilities: CaptureCapabilities,
}

/// Bytes from a successful process, not accepted media or authenticated facts.
#[derive(Debug)]
pub struct UntrustedCapture {
    pub bytes: Vec<u8>,
    pub evidence: InvocationEvidence,
}

struct LockedTool {
    name: &'static str,
    path: PathBuf,
    observation: BoundedFileObservation,
    build_fingerprint: EvidenceDigest,
}

/// Explicitly selected providers. No PATH discovery, install or fallback.
pub struct VideoProviders {
    producer: LockedTool,
    probe: LockedTool,
    workdir: PathBuf,
    limits: CaptureLimits,
    budget: OperationBudget,
    // Refuse concurrent captures through this instance rather than queueing
    // beyond a caller's cancellation/deadline expectations.
    invocation_gate: Mutex<()>,
}

impl VideoProviders {
    /// Observe selected executable bytes and version output. Keep installations
    /// and the private workdir stable: spawning still resolves executable/cwd
    /// paths, so before/after hashes are not an OS sandbox or atomic exec proof.
    pub fn lock(
        ffmpeg: &Path,
        ffprobe: &Path,
        workdir: &Path,
        limits: CaptureLimits,
        cancel: &impl Fn() -> bool,
    ) -> Result<Self> {
        let budget = OperationBudget::new(limits.elapsed_ms)?;
        Self::lock_with_budget(ffmpeg, ffprobe, workdir, limits, budget, cancel)
    }

    pub(crate) fn lock_with_budget(
        ffmpeg: &Path,
        ffprobe: &Path,
        workdir: &Path,
        limits: CaptureLimits,
        budget: OperationBudget,
        cancel: &impl Fn() -> bool,
    ) -> Result<Self> {
        limits.check()?;
        ensure!(budget.limit() == Duration::from_millis(limits.elapsed_ms),
            "operation budget differs from declared capture limits");
        ensure!(crate::subprocess::bounded_pipe_capture_supported(),
            "bounded video pipe capture requires Linux or macOS");
        let guard = || budget.check(cancel);
        guard()?;
        empty_workdir(workdir, &guard)?;
        let producer = bind_tool("ffmpeg", ffmpeg, workdir, limits, budget, cancel)?;
        let probe = bind_tool("ffprobe", ffprobe, workdir, limits, budget, cancel)?;
        let providers = Self {
            producer, probe, workdir: workdir.to_owned(), limits, budget,
            invocation_gate: Mutex::new(()),
        };
        providers.verify_current_with_cancel(cancel)?;
        Ok(providers)
    }

    pub fn verify_current(&self) -> Result<()> {
        self.verify_current_with_cancel(&|| false)
    }

    pub(crate) fn verify_current_with_cancel(&self, cancel: &impl Fn() -> bool) -> Result<()> {
        let guard = || self.budget.check(cancel);
        self.producer.observation.verify_current_guarded(&guard)?;
        self.probe.observation.verify_current_guarded(&guard)?;
        empty_workdir(&self.workdir, &guard)
    }

    pub fn producer_build_fingerprint(&self) -> &EvidenceDigest {
        &self.producer.build_fingerprint
    }

    pub fn probe_build_fingerprint(&self) -> &EvidenceDigest {
        &self.probe.build_fingerprint
    }

    pub fn producer_evidence(
        &self,
        operation: VideoOperation,
        disposition: &str,
    ) -> Result<MediaProviderEvidence> {
        let (arguments, _) = invocation_recipe(operation, disposition)?;
        self.evidence_for(&self.producer, &arguments, self.limits.candidate_bytes)
    }

    pub fn probe_evidence(&self) -> Result<MediaProviderEvidence> {
        self.evidence_for(&self.probe, &crate::video_probe::probe_arguments(), self.limits.probe_bytes)
    }

    pub fn probe_untrusted(
        &self,
        bytes: &[u8],
        cancel: &impl Fn() -> bool,
    ) -> Result<UntrustedCapture> {
        self.invoke(&self.probe, bytes, crate::video_probe::probe_arguments(), self.limits.probe_bytes, cancel)
    }

    /// Internal low-level capture only. The full requested memory/temp policy
    /// must be admitted by a future coordinator before exposing optimization.
    pub(crate) fn capture_untrusted(
        &self,
        bytes: &[u8],
        operation: VideoOperation,
        disposition: &str,
        cancel: &impl Fn() -> bool,
    ) -> Result<UntrustedCapture> {
        let (arguments, _) = invocation_recipe(operation, disposition)?;
        self.invoke(&self.producer, bytes, arguments, self.limits.candidate_bytes, cancel)
    }

    fn evidence_for(&self, tool: &LockedTool, arguments: &[String], stdout_bytes: usize) -> Result<MediaProviderEvidence> {
        let recipe = (
            tool.name, PROVIDER_VERSION, tool.observation.digest(), &tool.build_fingerprint,
            arguments, stdout_bytes, self.limits.stderr_bytes, self.limits.elapsed_ms,
            "bounded_host_owned_bytes_on_stdin", "untrusted_bounded_stdout_capture",
            "cleared_environment", NativePath::from_path(&self.workdir),
            "nonblocking_pipes_abort_on_overflow_group_cleanup_before_reap",
            "shared_cooperative_operation_deadline",
        );
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"optiflow.video-provider-invocation.v1\0");
        hasher.update(&serde_json::to_vec(&recipe)?);
        Ok(MediaProviderEvidence {
            name: tool.name.to_owned(), version: PROVIDER_VERSION.to_owned(),
            executable: NativePath::from_path(&tool.path),
            binary_digest: tool.observation.digest().clone(),
            invocation_fingerprint: EvidenceDigest {
                algorithm: "blake3-256".to_owned(), value: hasher.finalize().to_hex().to_string(),
            },
        })
    }

    fn invoke(
        &self,
        tool: &LockedTool,
        bytes: &[u8],
        arguments: Vec<String>,
        stdout_bytes: usize,
        cancel: &impl Fn() -> bool,
    ) -> Result<UntrustedCapture> {
        let _permit = self.invocation_gate.try_lock()
            .map_err(|_| anyhow::anyhow!("video provider capture already active or gate poisoned"))?;
        ensure!(!bytes.is_empty() && bytes.len() <= MAX_INPUT_BYTES, "video input exceeds capture bounds");
        let guard = || self.budget.check(cancel);
        guard()?;
        tool.observation.verify_current_guarded(&guard)?;
        empty_workdir(&self.workdir, &guard)?;
        let command = SubprocessCommand::new(tool.path.as_os_str().to_owned())
            .args(arguments.iter().map(String::as_str))
            .clear_environment().current_directory(&self.workdir);
        let remaining = self.budget.remaining(cancel)?;
        let result = self.limits.runner(stdout_bytes, remaining)?
            .run_with_bounded_pipes_and_cancel(&command, bytes, cancel);
        // On a runner error, do not start another potentially blocking host read.
        // No capture can be accepted; unexpected workdir entries stay untouched.
        let output = result.context("video provider capture failed")?;
        tool.observation.verify_current_guarded(&guard)?;
        empty_workdir(&self.workdir, &guard)?;
        guard()?;
        ensure!(output.stderr.is_empty(), "video provider emitted error diagnostics");
        ensure!(!output.stdout.is_empty(), "video provider returned empty output");
        let evidence = InvocationEvidence {
            provider: self.evidence_for(tool, &arguments, stdout_bytes)?,
            build_fingerprint: tool.build_fingerprint.clone(), argv: arguments,
            input_digest: digest(bytes), input_bytes: bytes.len() as u64,
            stdout_digest: digest(&output.stdout),
            captured_stdout_bytes: output.stdout.len() as u64,
            captured_stderr_bytes: output.stderr.len() as u64,
            elapsed_ms: output.elapsed.as_millis().try_into().unwrap_or(u64::MAX),
            stdout_limit_bytes: stdout_bytes, stderr_limit_bytes: self.limits.stderr_bytes,
            child_elapsed_limit_ms: remaining.as_nanos().div_ceil(1_000_000).try_into().unwrap_or(u64::MAX),
            child_elapsed_limit_ns: remaining.as_nanos().try_into().unwrap_or(u64::MAX),
            operation_elapsed_limit_ms: self.limits.elapsed_ms,
            operation_elapsed_ms: self.budget.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            capture_capabilities: capture_capabilities(),
        };
        guard()?;
        Ok(UntrustedCapture { bytes: output.stdout, evidence })
    }
}

fn bind_tool(
    name: &'static str,
    path: &Path,
    workdir: &Path,
    limits: CaptureLimits,
    budget: OperationBudget,
    cancel: &impl Fn() -> bool,
) -> Result<LockedTool> {
    let guard = || budget.check(cancel);
    guard()?;
    let observation = BoundedFileObservation::open_guarded(path, MAX_BINARY_BYTES, &guard)?;
    ensure!(observation.snapshot().mode & 0o111 != 0, "video provider has no executable permission bits");
    ensure!(observation.snapshot().mode & 0o6000 == 0,
        "setuid/setgid video providers are unsupported");
    empty_workdir(workdir, &guard)?;
    observation.verify_current_guarded(&guard)?;
    let command = SubprocessCommand::new(path.as_os_str().to_owned())
        .arg("-version").clear_environment().current_directory(workdir);
    let result = limits.runner(VERSION_BYTES, budget.remaining(cancel)?)?
        .run_with_bounded_pipes_and_cancel(&command, &[], cancel);
    let output = result.context("video provider version query failed")?;
    observation.verify_current_guarded(&guard)?;
    empty_workdir(workdir, &guard)?;
    version_output(name, &output)?;
    guard()?;
    Ok(LockedTool { name, path: path.to_owned(), observation, build_fingerprint: digest(&output.stdout) })
}

fn version_output(name: &str, output: &SubprocessOutput) -> Result<()> {
    ensure!(output.stderr.is_empty(), "video version query emitted diagnostics");
    let text = std::str::from_utf8(&output.stdout).context("invalid video version text")?;
    ensure!(!text.contains('\0'), "invalid NUL in video version text");
    let mut words = text.lines().next().unwrap_or_default().split_whitespace();
    ensure!(words.next() == Some(name) && words.next() == Some("version")
        && words.next() == Some(PROVIDER_VERSION), "video provider version is not exact {name} {PROVIDER_VERSION}");
    Ok(())
}

fn empty_workdir(path: &Path, guard: &impl Fn() -> Result<()>) -> Result<()> {
    guard()?;
    validate_private_directory_guarded(path, guard)?;
    ensure!(fs::read_dir(path)?.next().transpose()?.is_none(),
        "video workdir contains unowned entries; inspect and retain them");
    guard()
}

fn digest(bytes: &[u8]) -> EvidenceDigest {
    EvidenceDigest { algorithm: "blake3-256".to_owned(), value: blake3::hash(bytes).to_hex().to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_candidate::{H264Preset, PlanRefusal};

    fn output(text: &[u8]) -> SubprocessOutput {
        SubprocessOutput { stdout: text.to_vec(), stderr: Vec::new(), exit_code: Some(0), elapsed: Duration::ZERO }
    }

    #[test]
    fn exact_version_token_and_raw_build_bytes_are_distinct_requirements() {
        let first = output(b"ffmpeg version 8.0.1 Copyright\nconfiguration: one\n");
        let second = output(b"ffmpeg version 8.0.1 Copyright\nconfiguration: two\n");
        assert!(version_output("ffmpeg", &first).is_ok());
        assert!(version_output("ffmpeg", &second).is_ok());
        assert_ne!(digest(&first.stdout), digest(&second.stdout));
        for text in ["ffmpeg version 8.0.10\n", "ffmpeg version 8.0.1-extra\n", "ffprobe version 8.0.1\n", ""] {
            assert!(version_output("ffmpeg", &output(text.as_bytes())).is_err());
        }
        let mut noisy = first;
        noisy.stderr = b"warning".to_vec();
        assert!(version_output("ffmpeg", &noisy).is_err());
    }

    #[test]
    fn capture_bounds_never_claim_provider_resource_enforcement() {
        let good = CaptureLimits { candidate_bytes: 1024, probe_bytes: 1024, stderr_bytes: 1024, elapsed_ms: 1000 };
        assert!(good.check().is_ok());
        for bad in [CaptureLimits { candidate_bytes: 0, ..good }, CaptureLimits { probe_bytes: 4 * 1024 * 1024 + 1, ..good },
            CaptureLimits { stderr_bytes: 256 * 1024 + 1, ..good }, CaptureLimits { elapsed_ms: 30_001, ..good }] {
            assert!(bad.check().is_err());
        }
        let limits = capture_capabilities();
        assert!(!limits.provider_memory_enforced && !limits.provider_scratch_enforced);
        assert!(!limits.process_tree_contained && !limits.output_cap_terminates_child_immediately);
        assert_eq!(limits.output_cap_aborts_capture_on_detection, crate::subprocess::bounded_pipe_capture_supported());
        assert_eq!(limits.process_group_cleanup_before_reap, crate::subprocess::bounded_pipe_capture_supported());
        assert!(!limits.stdin_requires_full_copy && !limits.host_file_io_preemptible);
        assert!(limits.shared_operation_deadline_checked);
        assert!(!limits.source_or_candidate_validation);
    }

    #[test]
    fn child_runner_uses_remaining_time_without_resetting_operation_budget() {
        let limits = CaptureLimits { candidate_bytes: 1024, probe_bytes: 1024, stderr_bytes: 128, elapsed_ms: 30_000 };
        let remaining = Duration::from_nanos(123_456);
        let runner = limits.runner(512, remaining).unwrap();
        assert_eq!(runner.limits().timeout, remaining);
        assert_eq!(runner.limits().max_stdout_bytes, 512);
        assert!(limits.runner(512, Duration::ZERO).is_err());
    }

    #[test]
    fn internal_recipe_rejects_invalid_policy_without_schema_or_process_execution() {
        assert_eq!(invocation_recipe(VideoOperation::StreamCopy, "default -y").err(), Some(PlanRefusal::UnsupportedSource));
        for crf in [0, 52, u8::MAX] {
            assert_eq!(invocation_recipe(VideoOperation::H264Lossy { crf, preset: H264Preset::Medium }, "default").err(), Some(PlanRefusal::InvalidShape));
        }
        let (arguments, _) = invocation_recipe(VideoOperation::H264Lossy { crf: 23, preset: H264Preset::Slow }, "none").unwrap();
        assert!(arguments.windows(2).any(|pair| pair[0] == "-disposition:v:0" && pair[1] == "0"));
        assert!(arguments.windows(2).any(|pair| pair[0] == "-crf" && pair[1] == "23"));
        assert_eq!(arguments.last().map(String::as_str), Some("pipe:1"));
    }
}
