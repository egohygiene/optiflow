//! Opt-in nonblocking capture for the initially supported Unix targets.
//!
//! Keep the direct child waitable until its process group has been signalled.
//! `Child::try_wait` would reap it and make a later group signal unsafe under
//! PID reuse. No background worker retains input bytes or pipe handles.

use std::io::{self, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::Instant;

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::process::{Pid, Signal, WaitId, WaitIdOptions, kill_process_group, waitid};

use super::{OutputStream, SubprocessCommand, SubprocessError, SubprocessLimits, SubprocessOutput};

const IO_CHUNK: usize = 8192;
// Keep continuously readable/writable pipes from starving cancellation checks.
const IO_TURN: usize = 64 * 1024;

struct OwnedChild {
    child: Child,
    pid: Pid,
    wait_owned: bool,
    exit_observed: bool,
    group_signalled: bool,
}

impl OwnedChild {
    fn new(child: Child) -> Self {
        Self {
            pid: Pid::from_child(&child),
            child,
            wait_owned: true,
            exit_observed: false,
            group_signalled: false,
        }
    }

    fn signal_group(&mut self) -> io::Result<()> {
        if self.group_signalled || !self.wait_owned {
            return Ok(());
        }
        // This PID belongs to an unreaped child created with process_group(0).
        // Never allow the special kill(-1) meaning, even on an unexpected host.
        if self.pid.is_init() {
            return Err(io::Error::other("refusing process-group signal for PID 1"));
        }
        match kill_process_group(self.pid, Signal::Kill) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {
                self.group_signalled = true;
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    fn poll_exited(&mut self) -> io::Result<bool> {
        if self.exit_observed {
            return Ok(true);
        }
        match waitid(
            WaitId::Pid(self.pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
        ) {
            Ok(Some(_)) => {
                self.exit_observed = true;
                // Clean up same-group descendants even when the leader exited
                // successfully. Its exit status remains available for wait().
                self.signal_group()?;
                Ok(true)
            }
            Ok(None) | Err(rustix::io::Errno::INTR) => Ok(false),
            Err(error) => {
                // A competing reaper or nonstandard SIGCHLD policy invalidates
                // our identity ownership. Do not signal a potentially reused ID.
                if error == rustix::io::Errno::CHILD {
                    self.wait_owned = false;
                }
                Err(error.into())
            }
        }
    }

    fn finish(&mut self) -> io::Result<ExitStatus> {
        if !self.wait_owned {
            return Err(io::Error::other("subprocess wait ownership was lost"));
        }
        let signal_result = self.signal_group();
        if !self.exit_observed {
            // Direct-child fallback if the group signal failed. No reaping has
            // occurred yet. This does not promise descendant containment.
            let _ = self.child.kill();
        }
        let status = self.child.wait();
        if status.is_ok()
            || status.as_ref().err().is_some_and(|error| {
                error.raw_os_error() == Some(rustix::io::Errno::CHILD.raw_os_error())
            })
        {
            self.wait_owned = false;
        }
        // Reap first, but retain any cleanup failure instead of reporting a
        // successful capture whose process-group cleanup was unsuccessful.
        signal_result?;
        status
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.wait_owned {
            let _ = self.finish();
        }
    }
}

pub(super) fn run<F>(
    command: &SubprocessCommand,
    input: &[u8],
    limits: SubprocessLimits,
    started: Instant,
    deadline: Instant,
    cancelled: &F,
) -> Result<SubprocessOutput, SubprocessError>
where
    F: Fn() -> bool,
{
    let program = command.display_program();
    check_progress(&program, limits, deadline, cancelled)?;
    let mut process = Command::new(command.program());
    process
        .args(command.arguments())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    if command.clear_environment {
        process.env_clear();
    }
    if let Some(directory) = &command.current_directory {
        process.current_dir(directory);
    }
    let child = process.spawn().map_err(|error| SubprocessError::Spawn {
        program: program.clone(),
        message: error.to_string(),
    })?;
    let mut child = OwnedChild::new(child);
    let result = pump(&mut child, input, &program, limits, deadline, cancelled);
    // pump owns the IO handles, so they are closed before kill/reap on failure.
    // Kernel scheduling/spawn/reaping are not a hard real-time guarantee.
    let status = child.finish().map_err(|error| SubprocessError::Wait {
        program: program.clone(),
        message: match &result {
            Ok(_) => format!("subprocess cleanup failed: {error}"),
            Err(cause) => format!("{cause}; subprocess cleanup failed: {error}"),
        },
    })?;
    let (stdout, stderr) = result?;
    check_progress(&program, limits, deadline, cancelled)?;
    if !status.success() {
        return Err(SubprocessError::Exit {
            program,
            code: status.code(),
            stderr: String::from_utf8_lossy(&stderr).trim().to_owned(),
        });
    }
    Ok(SubprocessOutput { stdout, stderr, exit_code: status.code(), elapsed: started.elapsed() })
}

fn nonblocking(fd: &impl AsFd) -> io::Result<()> {
    let flags = fcntl_getfl(fd)?;
    fcntl_setfl(fd, flags | OFlags::NONBLOCK)?;
    Ok(())
}

fn check_progress<F>(
    program: &str,
    limits: SubprocessLimits,
    deadline: Instant,
    cancelled: &F,
) -> Result<(), SubprocessError>
where
    F: Fn() -> bool,
{
    if cancelled() {
        return Err(SubprocessError::Cancelled { program: program.to_owned() });
    }
    if Instant::now() >= deadline {
        return Err(SubprocessError::Timeout { program: program.to_owned(), timeout: limits.timeout });
    }
    Ok(())
}

fn pump<F>(
    child: &mut OwnedChild,
    input: &[u8],
    program: &str,
    limits: SubprocessLimits,
    deadline: Instant,
    cancelled: &F,
) -> Result<(Vec<u8>, Vec<u8>), SubprocessError>
where
    F: Fn() -> bool,
{
    let missing = || SubprocessError::Internal {
        message: format!("{program} capture pipe was not created"),
    };
    let mut stdin = Some(child.child.stdin.take().ok_or_else(missing)?);
    let mut stdout = Some(child.child.stdout.take().ok_or_else(missing)?);
    let mut stderr = Some(child.child.stderr.take().ok_or_else(missing)?);
    for result in [
        nonblocking(stdin.as_ref().ok_or_else(missing)?),
        nonblocking(stdout.as_ref().ok_or_else(missing)?),
        nonblocking(stderr.as_ref().ok_or_else(missing)?),
    ] {
        result.map_err(|error| SubprocessError::Spawn {
            program: program.to_owned(),
            message: format!("cannot configure nonblocking capture: {error}"),
        })?;
    }
    let mut position = 0;
    let mut captured_stdout = Vec::new();
    let mut captured_stderr = Vec::new();
    if input.is_empty() {
        stdin = None;
    }

    loop {
        check_progress(program, limits, deadline, cancelled)?;
        let mut progressed = drain(
            &mut stdout, &mut captured_stdout, limits.max_stdout_bytes,
            OutputStream::Stdout, program,
        )?;
        check_progress(program, limits, deadline, cancelled)?;
        progressed |= drain(
            &mut stderr, &mut captured_stderr, limits.max_stderr_bytes,
            OutputStream::Stderr, program,
        )?;
        check_progress(program, limits, deadline, cancelled)?;
        let exited = child.poll_exited().map_err(|error| SubprocessError::Wait {
            program: program.to_owned(), message: error.to_string(),
        })?;
        if exited {
            stdin = None;
            if stdout.is_none() && stderr.is_none() {
                if position != input.len() {
                    return Err(SubprocessError::InputWrite {
                        program: program.to_owned(),
                        message: "child exited before all input bytes were written".to_owned(),
                    });
                }
                return Ok((captured_stdout, captured_stderr));
            }
        } else if let Some(writer) = stdin.as_mut() {
            let end = input.len().min(position.saturating_add(IO_TURN));
            match writer.write(&input[position..end]) {
                Ok(0) => return Err(SubprocessError::InputWrite {
                    program: program.to_owned(), message: "stdin write made no progress".to_owned(),
                }),
                Ok(count) => {
                    position += count;
                    progressed = true;
                    if position == input.len() {
                        stdin = None;
                    }
                }
                Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => {}
                Err(error) => return Err(SubprocessError::InputWrite {
                    program: program.to_owned(), message: error.to_string(),
                }),
            }
        }
        check_progress(program, limits, deadline, cancelled)?;
        if !progressed {
            thread::sleep(limits.poll_interval.min(deadline.saturating_duration_since(Instant::now())));
        }
    }
}

fn drain<R: Read>(
    reader: &mut Option<R>,
    captured: &mut Vec<u8>,
    limit: usize,
    stream: OutputStream,
    program: &str,
) -> Result<bool, SubprocessError> {
    let Some(pipe) = reader.as_mut() else { return Ok(false); };
    let mut buffer = [0u8; IO_CHUNK];
    let mut total = 0;
    loop {
        // Read at most one byte past the cap; never retain that excess byte.
        let count_limit = IO_CHUNK.min(limit.saturating_sub(captured.len()).saturating_add(1));
        match pipe.read(&mut buffer[..count_limit]) {
            Ok(0) => {
                *reader = None;
                return Ok(true);
            }
            Ok(count) => {
                let observed = captured.len().saturating_add(count);
                if observed > limit {
                    return Err(SubprocessError::Truncated {
                        program: program.to_owned(), stream, limit_bytes: limit, observed_bytes: observed,
                    });
                }
                if observed > captured.capacity() {
                    // Geometric growth avoids repeatedly copying a large
                    // capture, while the requested capacity never exceeds its
                    // byte cap. Allocator overhead is not an RSS guarantee.
                    let target = captured.capacity().saturating_mul(2)
                        .max(observed).min(limit);
                    captured.try_reserve_exact(target - captured.len())
                        .map_err(|error| SubprocessError::OutputRead {
                            program: program.to_owned(), stream,
                            message: format!("capture allocation failed: {error}"),
                        })?;
                }
                captured.extend_from_slice(&buffer[..count]);
                total += count;
                if total >= IO_TURN {
                    return Ok(true);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(total != 0),
            // Return to the outer loop so repeated interrupts cannot starve
            // cancellation/deadline checks.
            Err(error) if error.kind() == io::ErrorKind::Interrupted => return Ok(total != 0),
            Err(error) => return Err(SubprocessError::OutputRead {
                program: program.to_owned(), stream, message: error.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subprocess::SubprocessRunner;
    use std::time::Duration;

    fn runner(stdout: usize, stderr: usize) -> SubprocessRunner {
        SubprocessRunner::new(SubprocessLimits {
            timeout: Duration::from_secs(2), poll_interval: Duration::from_millis(2),
            max_stdout_bytes: stdout, max_stderr_bytes: stderr, max_concurrent_children: 1,
        }).unwrap()
    }

    fn shell(script: &str) -> SubprocessCommand {
        SubprocessCommand::new("sh").args(["-c", script])
    }

    #[test]
    fn bidirectional_input_larger_than_pipe_capacity_has_no_deadlock() {
        let input = vec![b'x'; 1024 * 1024];
        let output = runner(input.len(), 0)
            .run_with_bounded_pipes_and_cancel(&shell("cat"), &input, || false).unwrap();
        assert_eq!(output.stdout, input);
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn stdout_overflow_terminates_without_waiting_for_child_exit() {
        let error = runner(17, 0).run_with_bounded_pipes_and_cancel(
            &shell("while :; do printf abcdefgh; done"), &[], || false,
        ).unwrap_err();
        assert!(matches!(error, SubprocessError::Truncated {
            stream: OutputStream::Stdout, limit_bytes: 17, observed_bytes: 18, ..
        }));
    }

    #[test]
    fn stderr_overflow_terminates_while_stdin_is_blocked() {
        let input = vec![0; 1024 * 1024];
        let error = runner(0, 17).run_with_bounded_pipes_and_cancel(
            &shell("while :; do printf abcdefgh >&2; done"), &input, || false,
        ).unwrap_err();
        assert!(matches!(error, SubprocessError::Truncated {
            stream: OutputStream::Stderr, limit_bytes: 17, observed_bytes: 18, ..
        }));
    }

    #[test]
    fn exact_and_zero_output_limits_are_distinct_from_excess() {
        assert_eq!(runner(3, 0).run_with_bounded_pipes_and_cancel(
            &shell("printf abc"), &[], || false,
        ).unwrap().stdout, b"abc");
        assert!(runner(0, 0).run_with_bounded_pipes_and_cancel(
            &shell("exit 0"), &[], || false,
        ).is_ok());
        assert!(matches!(runner(0, 0).run_with_bounded_pipes_and_cancel(
            &shell("printf x"), &[], || false,
        ), Err(SubprocessError::Truncated { limit_bytes: 0, observed_bytes: 1, .. })));
    }

    #[test]
    fn cancellation_and_deadline_stop_nonreading_children() {
        let started = Instant::now();
        assert!(matches!(runner(0, 0).run_with_bounded_pipes_and_cancel(
            &shell("while :; do :; done"), &[0; 32], || started.elapsed() >= Duration::from_millis(25),
        ), Err(SubprocessError::Cancelled { .. })));
        let mut limits = runner(0, 0).limits();
        limits.timeout = Duration::from_millis(25);
        assert!(matches!(SubprocessRunner::new(limits).unwrap().run_with_bounded_pipes_and_cancel(
            &shell("while :; do :; done"), &[0; 32], || false,
        ), Err(SubprocessError::Timeout { .. })));
    }

    #[test]
    fn exited_leader_cleans_same_group_pipe_holders_before_reaping() {
        let output = runner(4, 0).run_with_bounded_pipes_and_cancel(
            &shell("sleep 10 & printf done; exit 0"), &[], || false,
        ).unwrap();
        assert_eq!(output.stdout, b"done");
    }

    #[test]
    fn nonzero_exit_retains_bounded_diagnostics() {
        let error = runner(0, 32).run_with_bounded_pipes_and_cancel(
            &shell("printf denied >&2; exit 7"), &[], || false,
        ).unwrap_err();
        assert!(matches!(error, SubprocessError::Exit { code: Some(7), ref stderr, .. } if stderr == "denied"));
    }

    #[test]
    fn pre_cancelled_request_does_not_spawn_missing_executable() {
        let error = runner(0, 0).run_with_bounded_pipes_and_cancel(
            &SubprocessCommand::new("/synthetic/not-installed/provider"), &[], || true,
        ).unwrap_err();
        assert!(matches!(error, SubprocessError::Cancelled { .. }));
    }
}
