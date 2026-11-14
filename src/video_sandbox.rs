//! Explicit Linux user-manager launch policy for the narrow video candidate.
//! A safe internal exec guard checks actual cgroup/mount limits before any
//! media provider executes. Unsupported hosts refuse; there is no fallback.
use crate::domain::EvidenceDigest;
use crate::subprocess::{SubprocessCommand, SubprocessLimits, SubprocessOutput, SubprocessRunner};
use crate::video_io::BoundedFileObservation;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const GUARD: &str = "__optiflow-video-guard-v1";
const RECEIPT: &str = "OPTIFLOW_VIDEO_GUARD_V1 ";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedLauncher {
    pub path: PathBuf,
    pub digest: EvidenceDigest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinuxVideoSandbox {
    pub schema: String,
    pub systemd_run: PinnedLauncher,
    pub systemctl: PinnedLauncher,
    /// The optiflow CLI built with this internal guard, never a shell script.
    pub helper: PinnedLauncher,
    pub user_bus: PathBuf,
    pub provider_subtree_memory_bytes: u64,
    pub provider_scratch_bytes: u64,
    pub minimum_free_bytes: u64,
}
impl LinuxVideoSandbox {
    pub fn validate(&self) -> Result<()> {
        let delivery = self.schema == crate::mp4_delivery::planning::SANDBOX_SCHEMA;
        ensure!(
            self.schema == "optiflow.linux-video-sandbox.v1" || delivery,
            "unsupported video launch policy"
        );
        ensure!(
            (67_108_864..=2_147_483_648).contains(&self.provider_subtree_memory_bytes),
            "unsupported provider subtree memory limit"
        );
        if delivery {
            crate::mp4_delivery::planning::validate_sandbox(self)?;
        } else {
            ensure!(
                (1_048_576..=134_217_728).contains(&self.provider_scratch_bytes)
                    && self.provider_scratch_bytes % 4096 == 0,
                "unsupported aggregate scratch limit"
            );
        }
        for pin in [&self.systemd_run, &self.systemctl, &self.helper] {
            ensure!(
                pin.path.to_str().is_some_and(|v| v.starts_with('/')
                    && !v.contains('%')
                    && !v.chars().any(char::is_control))
                    && pin.digest.algorithm == "blake3-256"
                    && pin.digest.value.len() == 64
                    && pin
                        .digest
                        .value
                        .bytes()
                        .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_uppercase()),
                "launcher requires a complete pinned executable identity"
            );
        }
        ensure!(
            self.user_bus.is_absolute() && self.user_bus.file_name().is_some_and(|v| v == "bus"),
            "explicit user-manager bus socket required"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxReceipt {
    pub schema: String,
    pub memory_max: u64,
    pub memory_swap_max: u64,
    pub pids_max: u64,
    pub scratch_capacity_bytes: u64,
    pub writable_filesystems_confined_to_tmp: bool,
    pub no_new_privileges: bool,
    pub seccomp_filter_active: bool,
    pub capabilities_empty: bool,
}

pub(crate) struct SandboxSession {
    policy: LinuxVideoSandbox,
    bindings: Vec<BoundedFileObservation>,
    receipts: Mutex<Vec<SandboxReceipt>>,
}
impl SandboxSession {
    pub(crate) fn bind(policy: LinuxVideoSandbox, guard: &impl Fn() -> Result<()>) -> Result<Self> {
        ensure!(
            cfg!(target_os = "linux"),
            "video execution requires Linux cgroup v2 and a delegated user manager"
        );
        policy.validate()?;
        guard()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{FileTypeExt, MetadataExt};
            ensure!(
                policy.user_bus.canonicalize()? == policy.user_bus
                    && fs::symlink_metadata(&policy.user_bus)?
                        .file_type()
                        .is_socket(),
                "unsafe or missing user-manager bus socket"
            );
            let owner = fs::metadata(
                policy
                    .user_bus
                    .parent()
                    .context("missing user runtime directory")?,
            )?
            .uid();
            ensure!(
                policy.user_bus == PathBuf::from(format!("/run/user/{owner}/bus")),
                "only the explicit local user bus is supported"
            );
        }
        let mut bindings = Vec::new();
        for pin in [&policy.systemd_run, &policy.systemctl, &policy.helper] {
            let binding = BoundedFileObservation::open_guarded(&pin.path, 134_217_728, guard)?;
            ensure!(
                binding.digest() == &pin.digest
                    && binding.snapshot().mode & 0o111 != 0
                    && binding.snapshot().mode & 0o6000 == 0,
                "launcher changed or has unsafe execution mode"
            );
            bindings.push(binding);
        }
        Ok(Self {
            policy,
            bindings,
            receipts: Mutex::new(Vec::new()),
        })
    }
    pub(crate) fn verify(&self, guard: &impl Fn() -> Result<()>) -> Result<()> {
        for binding in &self.bindings {
            binding.verify_current_guarded(guard)?;
        }
        Ok(())
    }
    pub(crate) fn receipts(&self) -> Result<Vec<SandboxReceipt>> {
        Ok(self
            .receipts
            .lock()
            .map_err(|_| anyhow::anyhow!("video resource receipts poisoned"))?
            .clone())
    }
    pub(crate) fn policy(&self) -> &LinuxVideoSandbox {
        &self.policy
    }
    fn controller(
        &self,
        launcher: &PinnedLauncher,
        args: impl IntoIterator<Item = OsString>,
        cwd: &Path,
    ) -> SubprocessCommand {
        SubprocessCommand::new(launcher.path.as_os_str().to_owned())
            .args(args)
            .clear_environment()
            .current_directory(cwd)
            .env(
                "XDG_RUNTIME_DIR",
                self.policy
                    .user_bus
                    .parent()
                    .expect("validated runtime parent")
                    .as_os_str()
                    .to_owned(),
            )
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={}", self.policy.user_bus.display()),
            )
    }
    pub(crate) fn run(
        &self,
        tool: &Path,
        args: &[String],
        input: &[u8],
        runner: &SubprocessRunner,
        cwd: &Path,
        cancel: &impl Fn() -> bool,
    ) -> Result<SubprocessOutput> {
        ensure!(
            tool.to_str().is_some_and(|v| v.starts_with('/')
                && !v.contains('%')
                && !v.chars().any(char::is_control)),
            "video service tool locator cannot contain specifiers or controls"
        );
        let unit = format!("optiflow-video-{}.service", uuid::Uuid::now_v7());
        let launch = launch_arguments(&self.policy, &unit, tool, args, runner.limits().timeout)?;
        let command = self.controller(&self.policy.systemd_run, launch, cwd);
        let result = runner.run_with_bounded_pipes_and_cancel(&command, input, cancel);
        // The service is not the systemd-run process group. Always stop and
        // confirm it inactive, including cancellation/overflow/spawn failure.
        let cleanup = self.stop_and_confirm(&unit, cwd);
        cleanup.context("video service cleanup uncertain; candidate cannot be accepted")?;
        let mut output = result.context("resource-constrained video provider failed")?;
        let text =
            std::str::from_utf8(&output.stderr).context("video guard receipt is not UTF-8")?;
        let line = text
            .strip_prefix(RECEIPT)
            .and_then(|v| v.strip_suffix('\n'))
            .context("native provider did not return the exact resource guard receipt")?;
        let receipt: SandboxReceipt =
            serde_json::from_str(line).context("invalid resource guard receipt")?;
        check_receipt(
            &receipt,
            self.policy.provider_subtree_memory_bytes,
            self.policy.provider_scratch_bytes,
        )?;
        let mut receipts = self
            .receipts
            .lock()
            .map_err(|_| anyhow::anyhow!("video resource receipts poisoned"))?;
        ensure!(receipts.len() < 32, "too many video provider invocations");
        receipts.push(receipt);
        // Native stderr must have been empty: only the single guard record is
        // accepted. Provider errors/warnings cannot be hidden after that line.
        output.stderr.clear();
        Ok(output)
    }
    fn stop_and_confirm(&self, unit: &str, cwd: &Path) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let runner = |remaining| {
            SubprocessRunner::new(SubprocessLimits {
                timeout: remaining,
                poll_interval: Duration::from_millis(10),
                max_stdout_bytes: 4096,
                max_stderr_bytes: 4096,
                max_concurrent_children: 1,
            })
        };
        let stop = self.controller(
            &self.policy.systemctl,
            ["--user", "--no-pager", "stop", unit]
                .into_iter()
                .map(OsString::from),
            cwd,
        );
        let _stop_result =
            runner(Duration::from_secs(2))?.run_with_bounded_pipes_and_cancel(&stop, &[], || false);
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|v| !v.is_zero())
            .context("video cleanup deadline exhausted")?;
        let show = self.controller(
            &self.policy.systemctl,
            [
                "--user",
                "--no-pager",
                "list-units",
                "--all",
                "--plain",
                "--no-legend",
                "--full",
                "--output=json",
                unit,
            ]
            .into_iter()
            .map(OsString::from),
            cwd,
        );
        let output = runner(remaining)?
            .run_with_bounded_pipes_and_cancel(&show, &[], || false)
            .context("cannot confirm video service stopped")?;
        let units: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
            .context("invalid service cleanup observation")?;
        ensure!(
            units.len() <= 1
                && units
                    .iter()
                    .all(
                        |v| v.get("unit").and_then(serde_json::Value::as_str) == Some(unit)
                            && matches!(
                                v.get("active").and_then(serde_json::Value::as_str),
                                Some("inactive" | "failed")
                            )
                    ),
            "video service has not been confirmed inactive"
        );
        Ok(())
    }
}

fn launch_arguments(
    policy: &LinuxVideoSandbox,
    unit: &str,
    tool: &Path,
    args: &[String],
    remaining: Duration,
) -> Result<Vec<OsString>> {
    policy.validate()?;
    let maximum = if policy.schema == crate::mp4_delivery::planning::SANDBOX_SCHEMA {
        Duration::from_millis(crate::mp4_delivery::MAX_ELAPSED_MS)
    } else {
        Duration::from_secs(30)
    };
    ensure!(
        !remaining.is_zero() && remaining <= maximum,
        "invalid video child time allowance"
    );
    let mut values: Vec<OsString> = [
        "--user",
        "--quiet",
        "--wait",
        "--pipe",
        "--service-type=exec",
        "--expand-environment=no",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    values.push(format!("--unit={unit}").into());
    for property in [
        format!("MemoryMax={}", policy.provider_subtree_memory_bytes),
        "MemorySwapMax=0".to_owned(),
        "TasksMax=8".to_owned(),
        "ExitType=cgroup".to_owned(),
        "SendSIGKILL=yes".to_owned(),
        "KillMode=control-group".to_owned(),
        "TimeoutStopSec=1s".to_owned(),
        format!("RuntimeMaxSec={}ms", remaining.as_millis().max(1)),
        "NoNewPrivileges=yes".to_owned(),
        "ProtectSystem=strict".to_owned(),
        "ProtectHome=read-only".to_owned(),
        "ProtectControlGroups=yes".to_owned(),
        "ProtectKernelTunables=yes".to_owned(),
        "PrivateDevices=yes".to_owned(),
        "PrivateNetwork=yes".to_owned(),
        "RestrictNamespaces=yes".to_owned(),
        "RestrictSUIDSGID=yes".to_owned(),
        "CapabilityBoundingSet=".to_owned(),
        "SystemCallFilter=~@network-io @mount".to_owned(),
        "ReadOnlyPaths=/".to_owned(),
        "ReadWritePaths=/tmp".to_owned(),
        format!(
            "TemporaryFileSystem=/tmp:rw,size={},mode=0700",
            policy.provider_scratch_bytes
        ),
        "WorkingDirectory=/tmp".to_owned(),
        "UMask=0077".to_owned(),
    ] {
        values.push(format!("--property={property}").into());
    }
    values.extend([
        "--".into(),
        policy.helper.path.as_os_str().to_owned(),
        GUARD.into(),
        policy.provider_subtree_memory_bytes.to_string().into(),
        policy.provider_scratch_bytes.to_string().into(),
        tool.as_os_str().to_owned(),
    ]);
    values.extend(args.iter().map(OsString::from));
    Ok(values)
}

pub(crate) fn check_receipt(value: &SandboxReceipt, memory: u64, scratch: u64) -> Result<()> {
    ensure!(
        value.schema == "optiflow.video-resource-observation.v1"
            && value.memory_max > 0
            && value.memory_max <= memory
            && value.memory_swap_max == 0
            && value.pids_max > 0
            && value.pids_max <= 8
            && value.scratch_capacity_bytes > 0
            && value.scratch_capacity_bytes <= scratch
            && value.writable_filesystems_confined_to_tmp
            && value.no_new_privileges
            && value.seccomp_filter_active
            && value.capabilities_empty,
        "actual video resource boundaries do not satisfy the selected policy"
    );
    Ok(())
}

fn bounded_text(path: &Path) -> Result<String> {
    let mut text = String::new();
    File::open(path)?.take(65_537).read_to_string(&mut text)?;
    ensure!(
        text.len() <= 65_536,
        "kernel resource observation exceeds bound"
    );
    Ok(text)
}

#[cfg(target_os = "linux")]
pub(crate) fn observe_guard(memory: u64, scratch: u64) -> Result<SandboxReceipt> {
    let text = bounded_text(Path::new("/proc/self/cgroup"))?;
    let path = text
        .lines()
        .find_map(|v| v.strip_prefix("0::"))
        .context("unified cgroup membership is unavailable")?;
    ensure!(
        path.starts_with('/') && !path.split('/').any(|v| v == "." || v == ".."),
        "invalid unified cgroup membership"
    );
    let root = Path::new("/sys/fs/cgroup").join(path.trim_start_matches('/'));
    let number = |name| -> Result<u64> {
        bounded_text(&root.join(name))?
            .trim()
            .parse()
            .context("finite kernel cgroup limit is required")
    };
    let mounts = bounded_text(Path::new("/proc/self/mountinfo"))?;
    let mut tmp = false;
    for line in mounts.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        ensure!(fields.len() >= 10, "malformed mount resource observation");
        let separator = fields
            .iter()
            .position(|v| *v == "-")
            .context("missing mount type")?;
        let mountpoint = fields[4];
        let writable = fields[5].split(',').any(|v| v == "rw");
        if mountpoint == "/tmp" {
            ensure!(
                writable && fields.get(separator + 1) == Some(&"tmpfs"),
                "private bounded tmpfs is required"
            );
            tmp = true;
        } else {
            ensure!(
                !writable,
                "writable filesystem outside aggregate video scratch refused"
            );
        }
    }
    ensure!(tmp, "video scratch mount was not observed");
    let volume = rustix::fs::fstatvfs(File::open("/tmp")?)?;
    let capacity = volume
        .f_blocks
        .checked_mul(volume.f_frsize)
        .context("scratch capacity overflow")?;
    let status = bounded_text(Path::new("/proc/self/status"))?;
    let field = |name: &str| {
        status
            .lines()
            .find_map(|v| v.strip_prefix(name))
            .map(str::trim)
    };
    let value = SandboxReceipt {
        schema: "optiflow.video-resource-observation.v1".to_owned(),
        memory_max: number("memory.max")?,
        memory_swap_max: number("memory.swap.max")?,
        pids_max: number("pids.max")?,
        scratch_capacity_bytes: capacity,
        writable_filesystems_confined_to_tmp: true,
        no_new_privileges: field("NoNewPrivs:") == Some("1"),
        seccomp_filter_active: field("Seccomp:") == Some("2"),
        capabilities_empty: field("CapEff:")
            .is_some_and(|v| u64::from_str_radix(v, 16).ok() == Some(0))
            && field("CapBnd:").is_some_and(|v| u64::from_str_radix(v, 16).ok() == Some(0)),
    };
    check_receipt(&value, memory, scratch)?;
    Ok(value)
}

/// Internal CLI entry only. Returning Some means the guard was selected and
/// failed; success replaces this process with the selected provider via safe exec.
pub fn internal_exec_guard(arguments: &[OsString]) -> Option<std::process::ExitCode> {
    if arguments.get(1).is_none_or(|v| v != GUARD) {
        return None;
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        let operation = || -> Result<()> {
            ensure!(arguments.len() >= 5, "incomplete video guard arguments");
            let memory = arguments[2]
                .to_str()
                .context("invalid memory guard value")?
                .parse::<u64>()?;
            let scratch = arguments[3]
                .to_str()
                .context("invalid scratch guard value")?
                .parse::<u64>()?;
            let receipt = observe_guard(memory, scratch)?;
            let mut stderr = std::io::stderr().lock();
            writeln!(stderr, "{RECEIPT}{}", serde_json::to_string(&receipt)?)?;
            stderr.flush()?;
            drop(stderr);
            let error = std::process::Command::new(&arguments[4])
                .args(&arguments[5..])
                .env_clear()
                .current_dir("/tmp")
                .exec();
            Err(error).context("video guarded exec failed")
        };
        if operation().is_err() {
            let _ = writeln!(std::io::stderr(), "video resource guard refused");
        }
    }
    Some(std::process::ExitCode::from(125))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> LinuxVideoSandbox {
        let pin = |path: &str| PinnedLauncher {
            path: path.into(),
            digest: EvidenceDigest {
                algorithm: "blake3-256".to_owned(),
                value: "a".repeat(64),
            },
        };
        LinuxVideoSandbox {
            schema: "optiflow.linux-video-sandbox.v1".to_owned(),
            systemd_run: pin("/usr/bin/systemd-run"),
            systemctl: pin("/usr/bin/systemctl"),
            helper: pin("/synthetic/optiflow"),
            user_bus: "/run/user/1000/bus".into(),
            provider_subtree_memory_bytes: 134_217_728,
            provider_scratch_bytes: 8_388_608,
            minimum_free_bytes: 67_108_864,
        }
    }
    #[test]
    fn service_argv_sets_subtree_scratch_network_and_cleanup_policies_without_shell_expansion() {
        let mut selected = policy();
        selected.helper.path = "/synthetic/guard worker $(literal) 作品".into();
        let args = launch_arguments(
            &selected,
            "optiflow-video-synthetic.service",
            Path::new("/pinned/ffmpeg"),
            &["-version".to_owned()],
            Duration::from_millis(1234),
        )
        .unwrap();
        let args: Vec<_> = args.iter().map(|v| v.to_str().unwrap()).collect();
        for arg in [
            "--user",
            "--pipe",
            "--wait",
            "--expand-environment=no",
            "--property=MemoryMax=134217728",
            "--property=MemorySwapMax=0",
            "--property=TasksMax=8",
            "--property=ExitType=cgroup",
            "--property=KillMode=control-group",
            "--property=ReadOnlyPaths=/",
            "--property=ReadWritePaths=/tmp",
            "--property=TemporaryFileSystem=/tmp:rw,size=8388608,mode=0700",
            "--property=RuntimeMaxSec=1234ms",
            GUARD,
        ] {
            assert!(args.contains(&arg), "{arg}");
        }
        assert!(!args.contains(&"sh"));
        assert!(!args.contains(&"--scope"));
        let boundary = args.iter().position(|arg| *arg == "--").unwrap();
        assert_eq!(
            args[boundary + 1],
            "/synthetic/guard worker $(literal) 作品"
        );
        assert_eq!(args[boundary + 2], GUARD);
    }
    #[test]
    fn requested_limits_cannot_stand_in_for_observed_kernel_boundaries() {
        let receipt = SandboxReceipt {
            schema: "optiflow.video-resource-observation.v1".to_owned(),
            memory_max: 67_108_864,
            memory_swap_max: 0,
            pids_max: 8,
            scratch_capacity_bytes: 4_194_304,
            writable_filesystems_confined_to_tmp: true,
            no_new_privileges: true,
            seccomp_filter_active: true,
            capabilities_empty: true,
        };
        check_receipt(&receipt, 67_108_864, 4_194_304).unwrap();
        for value in [
            SandboxReceipt {
                memory_max: 67_108_865,
                ..receipt.clone()
            },
            SandboxReceipt {
                memory_swap_max: 1,
                ..receipt.clone()
            },
            SandboxReceipt {
                pids_max: 9,
                ..receipt.clone()
            },
            SandboxReceipt {
                scratch_capacity_bytes: 4_194_305,
                ..receipt.clone()
            },
            SandboxReceipt {
                writable_filesystems_confined_to_tmp: false,
                ..receipt.clone()
            },
            SandboxReceipt {
                seccomp_filter_active: false,
                ..receipt.clone()
            },
        ] {
            assert!(check_receipt(&value, 67_108_864, 4_194_304).is_err());
        }
    }
    #[test]
    fn internal_guard_is_not_an_ordinary_cli_action_and_policy_rejects_unbounded_values() {
        assert!(internal_exec_guard(&["optiflow".into(), "--help".into()]).is_none());
        let mut value = policy();
        value.provider_scratch_bytes = 8_388_609;
        assert!(value.validate().is_err());
        value = policy();
        value.provider_subtree_memory_bytes = u64::MAX;
        assert!(value.validate().is_err());
        assert!(
            launch_arguments(
                &policy(),
                "synthetic.service",
                Path::new("/pinned/ffmpeg"),
                &[],
                Duration::ZERO
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod mp4_scope_tests {
    use super::*;
    #[test]
    fn longer_delivery_allowance_requires_its_distinct_explicit_policy() {
        let pin = |path: &str| PinnedLauncher {
            path: path.into(),
            digest: EvidenceDigest {
                algorithm: "blake3-256".into(),
                value: "a".repeat(64),
            },
        };
        let mut p = LinuxVideoSandbox {
            schema: "optiflow.linux-video-sandbox.v1".into(),
            systemd_run: pin("/pinned/systemd-run"),
            systemctl: pin("/pinned/systemctl"),
            helper: pin("/pinned/optiflow"),
            user_bus: "/run/user/1/bus".into(),
            provider_subtree_memory_bytes: 1_073_741_824,
            provider_scratch_bytes: 8_388_608,
            minimum_free_bytes: 0,
        };
        assert!(
            launch_arguments(
                &p,
                "synthetic.service",
                Path::new("/pinned/ffmpeg"),
                &[],
                Duration::from_secs(45)
            )
            .is_err()
        );
        p.schema = crate::mp4_delivery::planning::SANDBOX_SCHEMA.into();
        p.provider_scratch_bytes = 536_870_912;
        assert!(
            launch_arguments(
                &p,
                "synthetic.service",
                Path::new("/pinned/ffmpeg"),
                &[],
                Duration::from_secs(45)
            )
            .is_ok()
        );
        p.schema = "optiflow.linux-video-sandbox.v1".into();
        assert!(p.validate().is_err());
    }
}
