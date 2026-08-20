//! Bounded, typed execution for external tools.
//!
//! External adapters must use argv execution through this module instead of
//! invoking a shell or collecting unbounded process output.

use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const DEFAULT_MAX_STDOUT_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_MAX_STDERR_BYTES: usize = 256 * 1024;
const DEFAULT_MAX_CONCURRENT_CHILDREN: usize = 2;

/// Limits applied to every subprocess started by one runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubprocessLimits {
    pub timeout: Duration,
    pub poll_interval: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_concurrent_children: usize,
}

impl Default for SubprocessLimits {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            poll_interval: DEFAULT_POLL_INTERVAL,
            max_stdout_bytes: DEFAULT_MAX_STDOUT_BYTES,
            max_stderr_bytes: DEFAULT_MAX_STDERR_BYTES,
            max_concurrent_children: DEFAULT_MAX_CONCURRENT_CHILDREN,
        }
    }
}

/// An argv-only subprocess request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessCommand {
    program: OsString,
    arguments: Vec<OsString>,
}

impl SubprocessCommand {
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            arguments: Vec::new(),
        }
    }

    pub fn arg(mut self, argument: impl Into<OsString>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    pub fn args<I, S>(mut self, arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.arguments.extend(arguments.into_iter().map(Into::into));
        self
    }

    pub fn program(&self) -> &OsStr {
        &self.program
    }

    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    fn display_program(&self) -> String {
        self.program.to_string_lossy().into_owned()
    }
}

/// Successful bounded process output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
    pub elapsed: Duration,
}

/// Which stream exceeded its configured in-memory capture limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

impl fmt::Display for OutputStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stdout => formatter.write_str("stdout"),
            Self::Stderr => formatter.write_str("stderr"),
        }
    }
}

/// Typed failures exposed to adapters and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubprocessError {
    InvalidConfiguration {
        message: String,
    },
    Spawn {
        program: String,
        message: String,
    },
    Wait {
        program: String,
        message: String,
    },
    Timeout {
        program: String,
        timeout: Duration,
    },
    Cancelled {
        program: String,
    },
    Exit {
        program: String,
        code: Option<i32>,
        stderr: String,
    },
    Truncated {
        program: String,
        stream: OutputStream,
        limit_bytes: usize,
        observed_bytes: usize,
    },
    Parse {
        program: String,
        message: String,
    },
    OutputRead {
        program: String,
        stream: OutputStream,
        message: String,
    },
    Internal {
        message: String,
    },
}

impl fmt::Display for SubprocessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration { message } => {
                write!(formatter, "invalid subprocess configuration: {message}")
            }
            Self::Spawn { program, message } => {
                write!(formatter, "failed to spawn {program}: {message}")
            }
            Self::Wait { program, message } => {
                write!(formatter, "failed while waiting for {program}: {message}")
            }
            Self::Timeout { program, timeout } => {
                write!(formatter, "{program} exceeded timeout of {timeout:?}")
            }
            Self::Cancelled { program } => write!(formatter, "{program} was cancelled"),
            Self::Exit {
                program,
                code,
                stderr,
            } => write!(
                formatter,
                "{program} exited unsuccessfully with code {code:?}: {stderr}"
            ),
            Self::Truncated {
                program,
                stream,
                limit_bytes,
                observed_bytes,
            } => write!(
                formatter,
                "{program} {stream} exceeded {limit_bytes} byte capture limit ({observed_bytes} bytes observed)"
            ),
            Self::Parse { program, message } => {
                write!(formatter, "failed to parse {program} output: {message}")
            }
            Self::OutputRead {
                program,
                stream,
                message,
            } => write!(formatter, "failed reading {program} {stream}: {message}"),
            Self::Internal { message } => write!(formatter, "subprocess runner failure: {message}"),
        }
    }
}

impl Error for SubprocessError {}

#[derive(Debug)]
struct PermitPool {
    maximum: usize,
    active: Mutex<usize>,
    changed: Condvar,
}

impl PermitPool {
    fn new(maximum: usize) -> Self {
        Self {
            maximum,
            active: Mutex::new(0),
            changed: Condvar::new(),
        }
    }

    fn acquire<F>(
        &self,
        command: &SubprocessCommand,
        deadline: Instant,
        timeout: Duration,
        poll_interval: Duration,
        is_cancelled: &F,
    ) -> Result<Permit<'_>, SubprocessError>
    where
        F: Fn() -> bool,
    {
        let program = command.display_program();
        let mut active = self.active.lock().map_err(|error| SubprocessError::Internal {
            message: format!("subprocess permit lock is poisoned: {error}"),
        })?;

        loop {
            if is_cancelled() {
                return Err(SubprocessError::Cancelled { program });
            }
            if *active < self.maximum {
                *active += 1;
                return Ok(Permit { pool: self });
            }

            let now = Instant::now();
            if now >= deadline {
                return Err(SubprocessError::Timeout { program, timeout });
            }
            let remaining = deadline.saturating_duration_since(now);
            let wait_for = poll_interval.min(remaining);
            let (next_active, _) = self
                .changed
                .wait_timeout(active, wait_for)
                .map_err(|error| SubprocessError::Internal {
                    message: format!("subprocess permit wait is poisoned: {error}"),
                })?;
            active = next_active;
        }
    }

    fn release(&self) {
        if let Ok(mut active) = self.active.lock() {
            *active = active.saturating_sub(1);
            self.changed.notify_one();
        }
    }
}

struct Permit<'a> {
    pool: &'a PermitPool,
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.pool.release();
    }
}

#[derive(Debug)]
struct BoundedRead {
    captured: Vec<u8>,
    observed_bytes: usize,
    truncated: bool,
}

fn read_bounded(mut reader: impl Read, limit: usize) -> io::Result<BoundedRead> {
    let mut captured = Vec::with_capacity(limit.min(64 * 1024));
    let mut observed_bytes = 0usize;
    let mut buffer = [0u8; 8192];

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        observed_bytes = observed_bytes.saturating_add(count);
        if captured.len() < limit {
            let remaining = limit - captured.len();
            let keep = count.min(remaining);
            captured.extend_from_slice(&buffer[..keep]);
        }
    }

    Ok(BoundedRead {
        captured,
        observed_bytes,
        truncated: observed_bytes > limit,
    })
}

fn join_reader(
    program: &str,
    stream: OutputStream,
    handle: thread::JoinHandle<io::Result<BoundedRead>>,
) -> Result<BoundedRead, SubprocessError> {
    let result = handle.join().map_err(|_| SubprocessError::OutputRead {
        program: program.to_owned(),
        stream,
        message: "reader thread panicked".to_owned(),
    })?;
    result.map_err(|error| SubprocessError::OutputRead {
        program: program.to_owned(),
        stream,
        message: error.to_string(),
    })
}

/// Reusable bounded runner. Clones share the same concurrency permit pool.
#[derive(Debug, Clone)]
pub struct SubprocessRunner {
    limits: SubprocessLimits,
    permits: Arc<PermitPool>,
}

impl Default for SubprocessRunner {
    fn default() -> Self {
        Self::new(SubprocessLimits::default()).expect("default subprocess limits are valid")
    }
}

impl SubprocessRunner {
    pub fn new(limits: SubprocessLimits) -> Result<Self, SubprocessError> {
        if limits.timeout.is_zero() {
            return Err(SubprocessError::InvalidConfiguration {
                message: "timeout must be greater than zero".to_owned(),
            });
        }
        if limits.poll_interval.is_zero() {
            return Err(SubprocessError::InvalidConfiguration {
                message: "poll interval must be greater than zero".to_owned(),
            });
        }
        if limits.max_concurrent_children == 0 {
            return Err(SubprocessError::InvalidConfiguration {
                message: "max_concurrent_children must be greater than zero".to_owned(),
            });
        }

        Ok(Self {
            limits,
            permits: Arc::new(PermitPool::new(limits.max_concurrent_children)),
        })
    }

    pub fn limits(&self) -> SubprocessLimits {
        self.limits
    }

    pub fn run(&self, command: &SubprocessCommand) -> Result<SubprocessOutput, SubprocessError> {
        self.run_with_cancel(command, || false)
    }

    pub fn run_json<T>(&self, command: &SubprocessCommand) -> Result<T, SubprocessError>
    where
        T: DeserializeOwned,
    {
        let output = self.run(command)?;
        serde_json::from_slice(&output.stdout).map_err(|error| SubprocessError::Parse {
            program: command.display_program(),
            message: error.to_string(),
        })
    }

    pub fn run_json_with_cancel<T, F>(
        &self,
        command: &SubprocessCommand,
        is_cancelled: F,
    ) -> Result<T, SubprocessError>
    where
        T: DeserializeOwned,
        F: Fn() -> bool,
    {
        let output = self.run_with_cancel(command, is_cancelled)?;
        serde_json::from_slice(&output.stdout).map_err(|error| SubprocessError::Parse {
            program: command.display_program(),
            message: error.to_string(),
        })
    }

    pub fn run_with_cancel<F>(
        &self,
        command: &SubprocessCommand,
        is_cancelled: F,
    ) -> Result<SubprocessOutput, SubprocessError>
    where
        F: Fn() -> bool,
    {
        let started = Instant::now();
        let deadline = started + self.limits.timeout;
        let program = command.display_program();
        let _permit = self.permits.acquire(
            command,
            deadline,
            self.limits.timeout,
            self.limits.poll_interval,
            &is_cancelled,
        )?;

        if is_cancelled() {
            return Err(SubprocessError::Cancelled { program });
        }

        let mut child = Command::new(command.program())
            .args(command.arguments())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| SubprocessError::Spawn {
                program: program.clone(),
                message: error.to_string(),
            })?;

        let stdout = child.stdout.take().ok_or_else(|| SubprocessError::Internal {
            message: format!("{program} stdout pipe was not created"),
        })?;
        let stderr = child.stderr.take().ok_or_else(|| SubprocessError::Internal {
            message: format!("{program} stderr pipe was not created"),
        })?;
        let stdout_limit = self.limits.max_stdout_bytes;
        let stderr_limit = self.limits.max_stderr_bytes;
        let stdout_reader = thread::spawn(move || read_bounded(stdout, stdout_limit));
        let stderr_reader = thread::spawn(move || read_bounded(stderr, stderr_limit));

        let exit_status = loop {
            if is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(SubprocessError::Cancelled { program });
            }

            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(SubprocessError::Wait {
                        program,
                        message: error.to_string(),
                    });
                }
            }

            let now = Instant::now();
            if now >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(SubprocessError::Timeout {
                    program,
                    timeout: self.limits.timeout,
                });
            }
            thread::sleep(self.limits.poll_interval.min(deadline - now));
        };

        let stdout = join_reader(&program, OutputStream::Stdout, stdout_reader)?;
        let stderr = join_reader(&program, OutputStream::Stderr, stderr_reader)?;

        if stdout.truncated {
            return Err(SubprocessError::Truncated {
                program,
                stream: OutputStream::Stdout,
                limit_bytes: self.limits.max_stdout_bytes,
                observed_bytes: stdout.observed_bytes,
            });
        }
        if stderr.truncated {
            return Err(SubprocessError::Truncated {
                program,
                stream: OutputStream::Stderr,
                limit_bytes: self.limits.max_stderr_bytes,
                observed_bytes: stderr.observed_bytes,
            });
        }
        if !exit_status.success() {
            return Err(SubprocessError::Exit {
                program,
                code: exit_status.code(),
                stderr: String::from_utf8_lossy(&stderr.captured).trim().to_owned(),
            });
        }

        Ok(SubprocessOutput {
            stdout: stdout.captured,
            stderr: stderr.captured,
            exit_code: exit_status.code(),
            elapsed: started.elapsed(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use serde_json::Value;

    use super::*;

    fn test_limits() -> SubprocessLimits {
        SubprocessLimits {
            timeout: Duration::from_secs(2),
            poll_interval: Duration::from_millis(5),
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
            max_concurrent_children: 2,
        }
    }

    fn shell(script: &str) -> SubprocessCommand {
        SubprocessCommand::new("sh").args(["-c", script])
    }

    #[test]
    fn captures_successful_output() {
        let runner = SubprocessRunner::new(test_limits()).unwrap();
        let output = runner.run(&shell("printf hello")).unwrap();
        assert_eq!(output.stdout, b"hello");
        assert_eq!(output.exit_code, Some(0));
    }

    #[test]
    fn returns_typed_exit_error() {
        let runner = SubprocessRunner::new(test_limits()).unwrap();
        let error = runner
            .run(&shell("printf denied >&2; exit 7"))
            .unwrap_err();
        assert!(matches!(
            error,
            SubprocessError::Exit {
                code: Some(7),
                ref stderr,
                ..
            } if stderr == "denied"
        ));
    }

    #[test]
    fn rejects_stdout_truncation_after_draining_child_output() {
        let mut limits = test_limits();
        limits.max_stdout_bytes = 64;
        let runner = SubprocessRunner::new(limits).unwrap();
        let error = runner.run(&shell("printf '%04096d' 0")).unwrap_err();
        assert!(matches!(
            error,
            SubprocessError::Truncated {
                stream: OutputStream::Stdout,
                limit_bytes: 64,
                observed_bytes: 4096,
                ..
            }
        ));
    }

    #[test]
    fn times_out_and_terminates_child() {
        let mut limits = test_limits();
        limits.timeout = Duration::from_millis(50);
        let runner = SubprocessRunner::new(limits).unwrap();
        let error = runner.run(&shell("sleep 1")).unwrap_err();
        assert!(matches!(error, SubprocessError::Timeout { .. }));
    }

    #[test]
    fn cancellation_terminates_running_child() {
        let runner = SubprocessRunner::new(test_limits()).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let writer = Arc::clone(&cancelled);
        let setter = thread::spawn(move || {
            thread::sleep(Duration::from_millis(25));
            writer.store(true, Ordering::Relaxed);
        });
        let error = runner
            .run_with_cancel(&shell("sleep 1"), || cancelled.load(Ordering::Relaxed))
            .unwrap_err();
        setter.join().unwrap();
        assert!(matches!(error, SubprocessError::Cancelled { .. }));
    }

    #[test]
    fn returns_typed_json_parse_error() {
        let runner = SubprocessRunner::new(test_limits()).unwrap();
        let error = runner
            .run_json::<Value>(&shell("printf not-json"))
            .unwrap_err();
        assert!(matches!(error, SubprocessError::Parse { .. }));
    }

    #[test]
    fn rejects_invalid_limits() {
        let mut limits = test_limits();
        limits.max_concurrent_children = 0;
        let error = SubprocessRunner::new(limits).unwrap_err();
        assert!(matches!(error, SubprocessError::InvalidConfiguration { .. }));
    }
}
