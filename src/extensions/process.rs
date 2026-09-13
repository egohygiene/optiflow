use std::error::Error;
use std::fmt;
use std::fs;
use std::time::Duration;

use super::catalog::{LoadedExtension, digest_regular_file, validate_invocation, validate_result};
use super::model::{ExecutionModeDeclaration, ExtensionInvocationV1, ExtensionResultV1, TrustMode};
use crate::subprocess::{SubprocessCommand, SubprocessError, SubprocessLimits, SubprocessRunner};

#[derive(Debug)]
pub enum ProcessExtensionError {
    Unavailable {
        message: String,
    },
    InvocationRejected {
        message: String,
    },
    InputTooLarge {
        limit_bytes: u64,
        observed_bytes: usize,
    },
    ExecutableChanged {
        message: String,
    },
    Process(SubprocessError),
    ResultRejected {
        message: String,
    },
}

impl fmt::Display for ProcessExtensionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { message } => {
                write!(formatter, "process extension is unavailable: {message}")
            }
            Self::InvocationRejected { message } => {
                write!(
                    formatter,
                    "process extension invocation was rejected: {message}"
                )
            }
            Self::InputTooLarge {
                limit_bytes,
                observed_bytes,
            } => write!(
                formatter,
                "process extension invocation has {observed_bytes} bytes; limit is {limit_bytes}"
            ),
            Self::ExecutableChanged { message } => {
                write!(formatter, "locked process executable changed: {message}")
            }
            Self::Process(error) => write!(formatter, "process extension failed: {error}"),
            Self::ResultRejected { message } => {
                write!(
                    formatter,
                    "process extension result was rejected: {message}"
                )
            }
        }
    }
}

impl Error for ProcessExtensionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Process(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SubprocessError> for ProcessExtensionError {
    fn from(error: SubprocessError) -> Self {
        Self::Process(error)
    }
}

/// Client for one explicitly locked, bounded process provider.
#[derive(Debug, Clone)]
pub struct ProcessExtensionClient {
    extension: LoadedExtension,
    runner: SubprocessRunner,
}

impl ProcessExtensionClient {
    pub fn new(extension: LoadedExtension) -> Result<Self, ProcessExtensionError> {
        if extension.lock.trust != TrustMode::TrustedProcess {
            return Err(ProcessExtensionError::Unavailable {
                message: "the operator lock does not grant trusted process execution".to_owned(),
            });
        }
        if !extension.availability_reasons.is_empty() {
            return Err(ProcessExtensionError::Unavailable {
                message: extension.availability_reasons.join("; "),
            });
        }
        let Some(ExecutionModeDeclaration::Process { limits, .. }) =
            extension.selected_execution_mode()
        else {
            return Err(ProcessExtensionError::Unavailable {
                message: "the selected execution mode is not a process contract".to_owned(),
            });
        };
        let runner = SubprocessRunner::new(SubprocessLimits {
            timeout: Duration::from_millis(limits.timeout_ms),
            poll_interval: Duration::from_millis(5),
            max_stdout_bytes: to_usize(limits.max_stdout_bytes, "max_stdout_bytes")?,
            max_stderr_bytes: to_usize(limits.max_stderr_bytes, "max_stderr_bytes")?,
            max_concurrent_children: 1,
        })?;
        Ok(Self { extension, runner })
    }

    pub fn extension(&self) -> &LoadedExtension {
        &self.extension
    }

    pub fn invoke<F>(
        &self,
        invocation: &ExtensionInvocationV1,
        is_cancelled: F,
    ) -> Result<ExtensionResultV1, ProcessExtensionError>
    where
        F: Fn() -> bool,
    {
        validate_invocation(&self.extension, invocation)
            .map_err(|message| ProcessExtensionError::InvocationRejected { message })?;

        let Some(ExecutionModeDeclaration::Process {
            arguments, limits, ..
        }) = self.extension.selected_execution_mode()
        else {
            return Err(ProcessExtensionError::Unavailable {
                message: "the selected process mode disappeared".to_owned(),
            });
        };
        if invocation.limits != *limits {
            return Err(ProcessExtensionError::InvocationRejected {
                message: "invocation limits do not match the pinned manifest".to_owned(),
            });
        }
        let process = self.extension.lock.process.as_ref().ok_or_else(|| {
            ProcessExtensionError::Unavailable {
                message: "the operator lock has no process identity".to_owned(),
            }
        })?;
        let executable = process.executable.to_path_buf();
        let working_directory = process.working_directory.to_path_buf();
        validate_working_directory(&working_directory)?;
        verify_executable(&executable, &process.executable_digest)?;

        let input = serde_json::to_vec(invocation).map_err(|error| {
            ProcessExtensionError::InvocationRejected {
                message: format!("invocation could not be serialized: {error}"),
            }
        })?;
        if input.len() as u64 > limits.max_stdin_bytes {
            return Err(ProcessExtensionError::InputTooLarge {
                limit_bytes: limits.max_stdin_bytes,
                observed_bytes: input.len(),
            });
        }
        let command = SubprocessCommand::new(&executable)
            .args(arguments)
            .clear_environment()
            .current_directory(&working_directory);
        let execution = self
            .runner
            .run_json_with_input_and_cancel::<serde_json::Value, _>(&command, &input, is_cancelled);

        verify_executable(&executable, &process.executable_digest)?;
        validate_working_directory(&working_directory)?;
        let document = execution?;
        crate::contracts::validate(crate::contracts::Contract::ExtensionResult, &document)
            .map_err(|error| ProcessExtensionError::ResultRejected {
                message: format!("result failed the closed JSON contract: {error}"),
            })?;
        let result: ExtensionResultV1 = serde_json::from_value(document).map_err(|error| {
            ProcessExtensionError::ResultRejected {
                message: format!("result could not be decoded: {error}"),
            }
        })?;
        validate_result(&self.extension, invocation, &result)
            .map_err(|message| ProcessExtensionError::ResultRejected { message })?;
        Ok(result)
    }
}

fn verify_executable(
    path: &std::path::Path,
    expected: &super::model::Digest,
) -> Result<(), ProcessExtensionError> {
    if !has_executable_permission(path) {
        return Err(ProcessExtensionError::ExecutableChanged {
            message: "the locked file is not executable".to_owned(),
        });
    }
    let actual = digest_regular_file(path)
        .map_err(|message| ProcessExtensionError::ExecutableChanged { message })?;
    if &actual != expected {
        return Err(ProcessExtensionError::ExecutableChanged {
            message: "the stable BLAKE3 digest does not match the operator lock".to_owned(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn has_executable_permission(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn has_executable_permission(path: &std::path::Path) -> bool {
    path.is_file()
}

fn validate_working_directory(path: &std::path::Path) -> Result<(), ProcessExtensionError> {
    if !path.is_absolute() {
        return Err(ProcessExtensionError::Unavailable {
            message: "working directory is not absolute".to_owned(),
        });
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|error| ProcessExtensionError::Unavailable {
            message: format!("working directory cannot be inspected: {error}"),
        })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(ProcessExtensionError::Unavailable {
            message: "working directory is not a non-symlink directory".to_owned(),
        });
    }
    Ok(())
}

fn to_usize(value: u64, name: &str) -> Result<usize, ProcessExtensionError> {
    usize::try_from(value).map_err(|_| ProcessExtensionError::Unavailable {
        message: format!("{name} does not fit this platform"),
    })
}
