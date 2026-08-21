use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result};

use crate::domain::{EvidenceValidity, ObservationStability};
use crate::filesystem::identity::FileStateSignature;
use crate::signals::SignalState;

pub const HASH_ALGORITHM: &str = "blake3-256";
const BUFFER_SIZE: usize = 1024 * 1024;

/// Maximum number of observation attempts before marking a file `retry_exhausted`.
pub const DEFAULT_MAX_OBSERVATION_ATTEMPTS: u32 = 2;

pub enum HandleHashOutcome {
    Complete(String),
    Interrupted,
}

/// Calculate a complete hash through an already-opened handle.
pub fn hash_file_handle(file: &mut File, signals: &SignalState) -> Result<HandleHashOutcome> {
    file.seek(SeekFrom::Start(0))
        .context("failed to rewind opened file before hashing")?;
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; BUFFER_SIZE];

    loop {
        if signals.is_cancelled() {
            return Ok(HandleHashOutcome::Interrupted);
        }
        let bytes_read = reader
            .read(&mut buffer)
            .context("failed while hashing opened file handle")?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    reader
        .seek(SeekFrom::Start(0))
        .context("failed to rewind opened file after hashing")?;
    Ok(HandleHashOutcome::Complete(
        hasher.finalize().to_hex().to_string(),
    ))
}

/// Result of a stable hash attempt.
pub struct StableHashResult {
    pub hash: String,
    pub stability: ObservationStability,
    pub evidence_validity: EvidenceValidity,
    pub attempt_count: u32,
    pub warning: Option<String>,
    pub interrupted: bool,
}

/// Hash a file with full before/after stability checks, retrying up to
/// `DEFAULT_MAX_OBSERVATION_ATTEMPTS` times if instability is detected.
pub fn hash_with_stability(path: &Path, signals: &SignalState) -> StableHashResult {
    let mut first_error: Option<String> = None;

    for attempt in 1..=DEFAULT_MAX_OBSERVATION_ATTEMPTS {
        match attempt_stable_hash(path, signals) {
            Ok(hash) => {
                return StableHashResult {
                    hash,
                    stability: ObservationStability::Stable,
                    evidence_validity: EvidenceValidity::Current,
                    attempt_count: attempt,
                    warning: if attempt > 1 {
                        Some(format!(
                            "file stabilized after {attempt} attempts: {}",
                            path.display()
                        ))
                    } else {
                        None
                    },
                    interrupted: false,
                };
            }
            Err(HashAttemptError::Interrupted) => {
                return StableHashResult {
                    hash: String::new(),
                    stability: ObservationStability::RetryExhausted,
                    evidence_validity: EvidenceValidity::Unavailable,
                    attempt_count: attempt,
                    warning: None,
                    interrupted: true,
                };
            }
            Err(HashAttemptError::Failed(err)) => {
                let msg = err.to_string();
                if first_error.is_none() {
                    first_error = Some(msg.clone());
                }
                if attempt < DEFAULT_MAX_OBSERVATION_ATTEMPTS {
                    continue;
                }
                // Final attempt failed.
                let first_msg = first_error.unwrap_or(msg.clone());
                let stability = classify_instability(&first_msg);
                return StableHashResult {
                    hash: String::new(),
                    stability: if attempt > 1 {
                        ObservationStability::RetryExhausted
                    } else {
                        stability
                    },
                    evidence_validity: EvidenceValidity::Stale,
                    attempt_count: attempt,
                    warning: Some(format!(
                        "first instability: {first_msg}; final error: {msg}"
                    )),
                    interrupted: false,
                };
            }
        }
    }
    // This point is unreachable: the final iteration always returns early.
    // The expression is retained to satisfy the Rust return-type checker.
    #[allow(unreachable_code)]
    {
        StableHashResult {
            hash: String::new(),
            stability: ObservationStability::RetryExhausted,
            evidence_validity: EvidenceValidity::Stale,
            attempt_count: DEFAULT_MAX_OBSERVATION_ATTEMPTS,
            warning: first_error
                .or_else(|| Some(format!("retry exhausted for {}", path.display()))),
            interrupted: false,
        }
    }
}

enum HashAttemptError {
    Interrupted,
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for HashAttemptError {
    fn from(error: anyhow::Error) -> Self {
        Self::Failed(error)
    }
}

fn attempt_stable_hash(path: &Path, signals: &SignalState) -> Result<String, HashAttemptError> {
    let pre_meta = std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to stat {}", path.display()))?;

    if pre_meta.file_type().is_symlink() {
        return Err(anyhow::anyhow!(
            "path became a symbolic link before hashing: {}",
            path.display()
        )
        .into());
    }

    let pre_sig = FileStateSignature::from_symlink_metadata(&pre_meta);

    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; BUFFER_SIZE];

    loop {
        if signals.is_cancelled() {
            return Err(HashAttemptError::Interrupted);
        }
        let bytes_read = reader
            .read(&mut buffer)
            .with_context(|| format!("failed while hashing {}", path.display()))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let post_meta = std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to re-stat after hashing {}", path.display()))?;

    if post_meta.file_type().is_symlink() {
        return Err(anyhow::anyhow!(
            "path became a symbolic link during hashing: {}",
            path.display()
        )
        .into());
    }

    let post_sig = FileStateSignature::from_symlink_metadata(&post_meta);

    if pre_sig != post_sig {
        return Err(
            anyhow::anyhow!("file changed while it was being hashed: {}", path.display()).into(),
        );
    }

    Ok(hasher.finalize().to_hex().to_string())
}

fn classify_instability(msg: &str) -> ObservationStability {
    if msg.contains("symbolic link") {
        ObservationStability::BecameSymlink
    } else if msg.contains("changed while") {
        ObservationStability::ChangedDuringHash
    } else if msg.contains("disappeared") || msg.contains("No such file") {
        ObservationStability::DisappearedDuringScan
    } else {
        ObservationStability::ChangedDuringHash
    }
}

/// Hash `path` unconditionally (for testing only).
pub fn complete_hash(path: &Path) -> Result<String> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; BUFFER_SIZE];

    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .with_context(|| format!("failed while hashing {}", path.display()))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

/// Kept for internal use; prefer `hash_with_stability` in production code.
pub fn complete_hash_stable(
    path: &Path,
    expected_size_bytes: u64,
    expected_modified_unix_ns: Option<i64>,
) -> Result<String> {
    verify_metadata(path, expected_size_bytes, expected_modified_unix_ns)?;
    let hash = complete_hash(path)?;
    verify_metadata(path, expected_size_bytes, expected_modified_unix_ns)?;
    Ok(hash)
}

fn verify_metadata(
    path: &Path,
    expected_size_bytes: u64,
    expected_modified_unix_ns: Option<i64>,
) -> Result<()> {
    let metadata =
        std::fs::metadata(path).with_context(|| format!("failed to re-stat {}", path.display()))?;
    let modified_unix_ns = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|duration| i64::try_from(duration.as_nanos()).ok());
    if metadata.len() != expected_size_bytes || modified_unix_ns != expected_modified_unix_ns {
        anyhow::bail!(
            "file changed while it was being analyzed: {}",
            path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn equal_files_have_equal_complete_hashes() {
        let directory = tempdir().expect("temporary directory");
        let first = directory.path().join("first.bin");
        let second = directory.path().join("second.bin");
        fs::write(&first, b"same bytes").expect("first fixture");
        fs::write(&second, b"same bytes").expect("second fixture");

        assert_eq!(
            complete_hash(&first).expect("first hash"),
            complete_hash(&second).expect("second hash")
        );
    }

    #[test]
    fn stable_hash_of_unchanged_file_is_current() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("stable.bin");
        fs::write(&path, b"stable content").expect("fixture");

        let result = hash_with_stability(&path, &SignalState::default());
        assert_eq!(result.stability, ObservationStability::Stable);
        assert_eq!(result.evidence_validity, EvidenceValidity::Current);
        assert_eq!(result.attempt_count, 1);
        assert!(result.warning.is_none());
        assert!(!result.interrupted);
        assert_eq!(result.hash, complete_hash(&path).expect("direct hash"));
    }

    #[test]
    fn stable_hash_of_empty_file_is_stable() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("empty.bin");
        fs::write(&path, b"").expect("empty fixture");

        let result = hash_with_stability(&path, &SignalState::default());
        assert_eq!(result.stability, ObservationStability::Stable);
        assert_eq!(result.evidence_validity, EvidenceValidity::Current);
    }

    #[test]
    fn interrupted_hash_stops_without_producing_evidence() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("interrupted.bin");
        fs::write(&path, b"content").expect("fixture");

        let signals = SignalState::interrupted(crate::signals::Interruption::Interrupt);
        let result = hash_with_stability(&path, &signals);

        assert!(result.interrupted);
        assert_eq!(result.evidence_validity, EvidenceValidity::Unavailable);
        assert!(result.hash.is_empty());
    }
}
