//! Handle-bound file observation.
//!
//! One accepted attempt opens a file once, binds identity to that handle,
//! derives content evidence through it, and verifies both the handle and path
//! again before publishing evidence. Failed attempts are discarded wholesale.

use std::fs::File;
use std::path::Path;

use crate::discovery::DiscoveredFile;
use crate::domain::{
    CachedAnalysis, EvidenceValidity, MediaKind, ObservationStability, ObservationStatus,
};
use crate::filesystem::identity::FileStateSignature;
use crate::filesystem::metadata::{RawFilesystemMetadata, collect_from_handle_metadata};
use crate::hashing::{DEFAULT_MAX_OBSERVATION_ATTEMPTS, HandleHashOutcome, hash_file_handle};
use crate::signals::SignalState;

pub struct ObservationResult {
    pub analysis: CachedAnalysis,
    pub filesystem_metadata: Option<RawFilesystemMetadata>,
    pub signature: Option<FileStateSignature>,
    pub cache_hit: bool,
    pub interrupted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObservationStage {
    AfterOpen,
    AfterEvidence,
}

struct AttemptSuccess {
    analysis: CachedAnalysis,
    filesystem_metadata: RawFilesystemMetadata,
    signature: FileStateSignature,
    cache_hit: bool,
}

struct AttemptFailure {
    stability: ObservationStability,
    validity: EvidenceValidity,
    message: String,
    interrupted: bool,
}

/// Observe one discovered path with bounded retries.
pub fn observe(
    discovered: &DiscoveredFile,
    cached: Option<CachedAnalysis>,
    needs_hash: bool,
    probe_media: bool,
    ffprobe_signature: Option<&str>,
    signals: &SignalState,
) -> ObservationResult {
    observe_with_hook(
        discovered,
        cached,
        needs_hash,
        probe_media,
        ffprobe_signature,
        signals,
        &mut |_, _| {},
    )
}

fn observe_with_hook<F>(
    discovered: &DiscoveredFile,
    cached: Option<CachedAnalysis>,
    needs_hash: bool,
    probe_media: bool,
    ffprobe_signature: Option<&str>,
    signals: &SignalState,
    hook: &mut F,
) -> ObservationResult
where
    F: FnMut(ObservationStage, &Path),
{
    let mut first_failure: Option<String> = None;
    let mut final_failure: Option<AttemptFailure> = None;

    for attempt in 1..=DEFAULT_MAX_OBSERVATION_ATTEMPTS {
        match attempt_once(
            discovered,
            cached.clone(),
            needs_hash,
            probe_media,
            ffprobe_signature,
            signals,
            hook,
        ) {
            Ok(mut success) => {
                success.analysis.observation_stability = ObservationStability::Stable;
                success.analysis.evidence_validity = EvidenceValidity::Current;
                success.analysis.attempt_count = attempt;
                if attempt > 1 {
                    success.analysis.warnings.push(format!(
                        "handle-bound observation stabilized after {attempt} attempts: {}",
                        discovered.path.display()
                    ));
                }
                return ObservationResult {
                    analysis: success.analysis,
                    filesystem_metadata: Some(success.filesystem_metadata),
                    signature: Some(success.signature),
                    cache_hit: success.cache_hit,
                    interrupted: false,
                };
            }
            Err(failure) if failure.interrupted => {
                return ObservationResult {
                    analysis: failed_analysis(
                        ObservationStability::RetryExhausted,
                        EvidenceValidity::Unavailable,
                        attempt,
                        None,
                    ),
                    filesystem_metadata: None,
                    signature: None,
                    cache_hit: false,
                    interrupted: true,
                };
            }
            Err(failure) => {
                if first_failure.is_none() {
                    first_failure = Some(failure.message.clone());
                }
                final_failure = Some(failure);
            }
        }
    }

    let failure = final_failure.expect("the bounded attempt loop always records a failure");
    let warning = format!(
        "handle-bound observation rejected after {} attempts; first failure: {}; final failure: {}",
        DEFAULT_MAX_OBSERVATION_ATTEMPTS,
        first_failure.unwrap_or_else(|| failure.message.clone()),
        failure.message
    );
    ObservationResult {
        analysis: failed_analysis(
            failure.stability,
            failure.validity,
            DEFAULT_MAX_OBSERVATION_ATTEMPTS,
            Some(warning),
        ),
        filesystem_metadata: None,
        signature: None,
        cache_hit: false,
        interrupted: false,
    }
}

fn attempt_once<F>(
    discovered: &DiscoveredFile,
    cached: Option<CachedAnalysis>,
    needs_hash: bool,
    probe_media: bool,
    ffprobe_signature: Option<&str>,
    signals: &SignalState,
    hook: &mut F,
) -> Result<AttemptSuccess, AttemptFailure>
where
    F: FnMut(ObservationStage, &Path),
{
    if signals.is_cancelled() {
        return Err(interrupted());
    }

    let pre_metadata =
        std::fs::symlink_metadata(&discovered.path).map_err(|error| AttemptFailure {
            stability: ObservationStability::DisappearedDuringScan,
            validity: EvidenceValidity::Unavailable,
            message: format!("failed to inspect discovered path: {error}"),
            interrupted: false,
        })?;
    if !pre_metadata.is_file() || pre_metadata.file_type().is_symlink() {
        return Err(AttemptFailure {
            stability: if pre_metadata.file_type().is_symlink() {
                ObservationStability::BecameSymlink
            } else {
                ObservationStability::ReplacedDuringScan
            },
            validity: EvidenceValidity::Unavailable,
            message: "discovered path was no longer a regular file before open".to_owned(),
            interrupted: false,
        });
    }
    let pre_signature = FileStateSignature::from_symlink_metadata(&pre_metadata);
    require_regular_identity(&pre_signature, "before open")?;
    require_same_signature(
        &discovered.signature,
        &pre_signature,
        ObservationStability::ChangedDuringHash,
        EvidenceValidity::Unavailable,
        "path changed after discovery",
    )?;

    let mut file = File::open(&discovered.path).map_err(|error| AttemptFailure {
        stability: ObservationStability::Unreadable,
        validity: EvidenceValidity::Unavailable,
        message: format!("failed to open discovered file: {error}"),
        interrupted: false,
    })?;
    let opened_metadata = file.metadata().map_err(|error| AttemptFailure {
        stability: ObservationStability::MetadataUnavailable,
        validity: EvidenceValidity::Unavailable,
        message: format!("failed to inspect opened file handle: {error}"),
        interrupted: false,
    })?;
    if !opened_metadata.is_file() {
        return Err(AttemptFailure {
            stability: ObservationStability::ReplacedDuringScan,
            validity: EvidenceValidity::Unavailable,
            message: "opened handle did not refer to a regular file".to_owned(),
            interrupted: false,
        });
    }
    let opened_signature = FileStateSignature::from_file_metadata(&opened_metadata);
    require_regular_identity(&opened_signature, "opened handle")?;
    require_same_signature(
        &pre_signature,
        &opened_signature,
        ObservationStability::ReplacedDuringScan,
        EvidenceValidity::Unavailable,
        "opened handle did not match the inspected path",
    )?;

    hook(ObservationStage::AfterOpen, &discovered.path);

    let cache_hit = cached.is_some();
    let mut analysis = match cached {
        Some(analysis) => analysis,
        None => crate::inventory::analyze_file(
            &mut file,
            &discovered.path,
            probe_media,
            ffprobe_signature,
        )
        .map_err(|error| AttemptFailure {
            stability: ObservationStability::Unreadable,
            validity: EvidenceValidity::Unavailable,
            message: format!("opened-handle content analysis failed: {error:#}"),
            interrupted: false,
        })?,
    };

    if needs_hash && analysis.status != ObservationStatus::Unreadable {
        analysis.content_hash = None;
        match hash_file_handle(&mut file, signals).map_err(|error| AttemptFailure {
            stability: ObservationStability::Unreadable,
            validity: EvidenceValidity::Unavailable,
            message: format!("opened-handle hashing failed: {error:#}"),
            interrupted: false,
        })? {
            HandleHashOutcome::Complete(hash) => analysis.content_hash = Some(hash),
            HandleHashOutcome::Interrupted => return Err(interrupted()),
        }
    }

    hook(ObservationStage::AfterEvidence, &discovered.path);

    let final_handle_metadata = file.metadata().map_err(|error| AttemptFailure {
        stability: ObservationStability::MetadataUnavailable,
        validity: EvidenceValidity::Stale,
        message: format!("failed to re-inspect opened file handle: {error}"),
        interrupted: false,
    })?;
    if !final_handle_metadata.is_file() {
        return Err(AttemptFailure {
            stability: ObservationStability::ReplacedDuringScan,
            validity: EvidenceValidity::Stale,
            message: "opened handle stopped reporting a regular file".to_owned(),
            interrupted: false,
        });
    }
    let final_handle_signature = FileStateSignature::from_file_metadata(&final_handle_metadata);
    let evidence_change = if probe_media {
        ObservationStability::ChangedDuringProbe
    } else {
        ObservationStability::ChangedDuringHash
    };
    require_same_signature(
        &opened_signature,
        &final_handle_signature,
        evidence_change,
        EvidenceValidity::Stale,
        "opened filesystem object changed during evidence collection",
    )?;

    let post_metadata =
        std::fs::symlink_metadata(&discovered.path).map_err(|error| AttemptFailure {
            stability: ObservationStability::DisappearedDuringScan,
            validity: EvidenceValidity::Stale,
            message: format!("failed to re-inspect path after evidence collection: {error}"),
            interrupted: false,
        })?;
    if !post_metadata.is_file() || post_metadata.file_type().is_symlink() {
        return Err(AttemptFailure {
            stability: if post_metadata.file_type().is_symlink() {
                ObservationStability::BecameSymlink
            } else {
                ObservationStability::ReplacedDuringScan
            },
            validity: EvidenceValidity::Stale,
            message: "path was no longer a regular file after evidence collection".to_owned(),
            interrupted: false,
        });
    }
    let post_signature = FileStateSignature::from_symlink_metadata(&post_metadata);
    require_regular_identity(&post_signature, "after evidence collection")?;
    require_same_signature(
        &final_handle_signature,
        &post_signature,
        ObservationStability::ReplacedDuringScan,
        EvidenceValidity::Stale,
        "path no longer referenced the opened filesystem object",
    )?;

    Ok(AttemptSuccess {
        analysis,
        filesystem_metadata: collect_from_handle_metadata(&final_handle_metadata),
        signature: final_handle_signature,
        cache_hit,
    })
}

fn require_regular_identity(
    signature: &FileStateSignature,
    stage: &str,
) -> Result<(), AttemptFailure> {
    if signature.is_symlink {
        return Err(AttemptFailure {
            stability: ObservationStability::BecameSymlink,
            validity: EvidenceValidity::Unavailable,
            message: format!("path was a symbolic link {stage}"),
            interrupted: false,
        });
    }
    if signature.identity.is_none() || signature.changed_unix_ns.is_none() {
        return Err(AttemptFailure {
            stability: ObservationStability::MetadataUnavailable,
            validity: EvidenceValidity::Unavailable,
            message: format!("required handle-bound identity metadata was unavailable {stage}"),
            interrupted: false,
        });
    }
    Ok(())
}

fn require_same_signature(
    expected: &FileStateSignature,
    actual: &FileStateSignature,
    content_change: ObservationStability,
    validity: EvidenceValidity,
    context: &str,
) -> Result<(), AttemptFailure> {
    if expected == actual {
        return Ok(());
    }
    let stability = classify_signature_change(expected, actual, content_change);
    Err(AttemptFailure {
        stability,
        validity,
        message: context.to_owned(),
        interrupted: false,
    })
}

fn classify_signature_change(
    expected: &FileStateSignature,
    actual: &FileStateSignature,
    content_change: ObservationStability,
) -> ObservationStability {
    if actual.is_symlink {
        return ObservationStability::BecameSymlink;
    }
    match (&expected.identity, &actual.identity) {
        (Some(expected), Some(actual)) if expected.filesystem_id != actual.filesystem_id => {
            ObservationStability::FilesystemBoundaryChanged
        }
        (Some(expected), Some(actual)) if expected.file_id != actual.file_id => {
            ObservationStability::ReplacedDuringScan
        }
        (None, _) | (_, None) => ObservationStability::MetadataUnavailable,
        _ if expected.logical_size_bytes != actual.logical_size_bytes
            || expected.modified_unix_ns != actual.modified_unix_ns
            || expected.changed_unix_ns != actual.changed_unix_ns
            || expected.identity != actual.identity =>
        {
            content_change
        }
        _ => content_change,
    }
}

fn interrupted() -> AttemptFailure {
    AttemptFailure {
        stability: ObservationStability::RetryExhausted,
        validity: EvidenceValidity::Unavailable,
        message: "observation interrupted".to_owned(),
        interrupted: true,
    }
}

fn failed_analysis(
    stability: ObservationStability,
    validity: EvidenceValidity,
    attempt_count: u32,
    warning: Option<String>,
) -> CachedAnalysis {
    CachedAnalysis {
        content_type: None,
        media_kind: MediaKind::Unknown,
        content_hash: None,
        media: None,
        probe_signature: None,
        status: ObservationStatus::Unreadable,
        warnings: warning.into_iter().collect(),
        observation_stability: stability,
        evidence_validity: validity,
        attempt_count,
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};

    use tempfile::tempdir;

    use super::*;

    fn discovered(path: &Path) -> DiscoveredFile {
        let metadata = fs::symlink_metadata(path).unwrap();
        let signature = FileStateSignature::from_symlink_metadata(&metadata);
        DiscoveredFile {
            path: path.to_path_buf(),
            size_bytes: signature.logical_size_bytes,
            modified_unix_ns: signature.modified_unix_ns,
            signature,
        }
    }

    #[test]
    fn stable_attempt_derives_hash_and_identity_from_one_handle() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("stable.bin");
        fs::write(&path, b"stable bytes").unwrap();
        let discovered = discovered(&path);

        let result = observe(
            &discovered,
            None,
            true,
            false,
            None,
            &SignalState::default(),
        );

        assert_eq!(result.analysis.evidence_validity, EvidenceValidity::Current);
        assert_eq!(
            result.analysis.observation_stability,
            ObservationStability::Stable
        );
        assert!(result.analysis.content_hash.is_some());
        assert!(
            result
                .filesystem_metadata
                .and_then(|metadata| metadata.identity)
                .is_some()
        );
    }

    #[test]
    fn rename_replacement_is_rejected_after_open() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("observed.bin");
        let displaced = directory.path().join("displaced.bin");
        fs::write(&path, b"original bytes").unwrap();
        let discovered = discovered(&path);
        let mut replaced = false;

        let result = observe_with_hook(
            &discovered,
            None,
            true,
            false,
            None,
            &SignalState::default(),
            &mut |stage, observed_path| {
                if stage == ObservationStage::AfterOpen && !replaced {
                    fs::rename(observed_path, &displaced).unwrap();
                    fs::write(observed_path, b"replacement!!").unwrap();
                    replaced = true;
                }
            },
        );

        assert_eq!(
            result.analysis.observation_stability,
            ObservationStability::ReplacedDuringScan
        );
        assert_ne!(result.analysis.evidence_validity, EvidenceValidity::Current);
        assert!(result.analysis.content_hash.is_none());
    }

    #[test]
    fn truncation_and_growth_are_rejected() {
        for replacement in [b"x".as_slice(), b"substantially longer bytes".as_slice()] {
            let directory = tempdir().unwrap();
            let path = directory.path().join("changing.bin");
            fs::write(&path, b"original").unwrap();
            let discovered = discovered(&path);
            let mut changed = false;

            let result = observe_with_hook(
                &discovered,
                None,
                true,
                false,
                None,
                &SignalState::default(),
                &mut |stage, observed_path| {
                    if stage == ObservationStage::AfterOpen && !changed {
                        fs::write(observed_path, replacement).unwrap();
                        changed = true;
                    }
                },
            );

            assert_eq!(
                result.analysis.observation_stability,
                ObservationStability::ChangedDuringHash
            );
            assert_ne!(result.analysis.evidence_validity, EvidenceValidity::Current);
        }
    }

    #[cfg(unix)]
    #[test]
    fn metadata_race_is_rejected() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempdir().unwrap();
        let path = directory.path().join("metadata.bin");
        fs::write(&path, b"content").unwrap();
        let discovered = discovered(&path);
        let mut changed = false;

        let result = observe_with_hook(
            &discovered,
            None,
            true,
            false,
            None,
            &SignalState::default(),
            &mut |stage, observed_path| {
                if stage == ObservationStage::AfterEvidence && !changed {
                    let mut permissions = fs::metadata(observed_path).unwrap().permissions();
                    permissions.set_mode(0o600);
                    fs::set_permissions(observed_path, permissions).unwrap();
                    changed = true;
                }
            },
        );

        assert_ne!(result.analysis.evidence_validity, EvidenceValidity::Current);
    }

    #[test]
    fn inode_reuse_like_signature_change_is_not_accepted() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("inode.bin");
        fs::write(&path, b"content").unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let expected = FileStateSignature::from_file_metadata(&metadata);
        let mut reused = expected.clone();
        reused.changed_unix_ns = reused.changed_unix_ns.map(|value| value.saturating_add(1));

        assert_eq!(
            classify_signature_change(&expected, &reused, ObservationStability::ChangedDuringHash,),
            ObservationStability::ChangedDuringHash
        );
    }

    #[test]
    fn interrupted_attempt_publishes_no_evidence() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("interrupt.bin");
        fs::write(&path, b"content").unwrap();
        let discovered = discovered(&path);
        let signals = SignalState::interrupted(crate::signals::Interruption::Interrupt);

        let result = observe(&discovered, None, true, false, None, &signals);

        assert!(result.interrupted);
        assert_eq!(
            result.analysis.evidence_validity,
            EvidenceValidity::Unavailable
        );
        assert!(result.analysis.content_hash.is_none());
    }

    #[test]
    fn cache_hit_is_accepted_only_inside_stable_handle_window() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("cache.bin");
        fs::write(&path, b"content").unwrap();
        let discovered = discovered(&path);
        let cached = CachedAnalysis {
            content_type: Some("application/octet-stream".to_owned()),
            media_kind: MediaKind::Other,
            content_hash: None,
            media: None,
            probe_signature: None,
            status: ObservationStatus::Unsupported,
            warnings: Vec::new(),
            observation_stability: ObservationStability::Stable,
            evidence_validity: EvidenceValidity::Current,
            attempt_count: 1,
        };

        let result = observe(
            &discovered,
            Some(cached),
            false,
            false,
            None,
            &SignalState::default(),
        );

        assert!(result.cache_hit);
        assert_eq!(result.analysis.evidence_validity, EvidenceValidity::Current);
    }

    #[test]
    fn opened_handle_remains_read_only() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("readonly.bin");
        fs::write(&path, b"content").unwrap();
        let discovered = discovered(&path);
        let before = fs::read(&path).unwrap();

        let result = observe(
            &discovered,
            None,
            true,
            false,
            None,
            &SignalState::default(),
        );
        let mut opened = OpenOptions::new().read(true).open(&path).unwrap();
        let mut buffer = Vec::new();
        use std::io::Read;
        opened.read_to_end(&mut buffer).unwrap();

        assert_eq!(result.analysis.evidence_validity, EvidenceValidity::Current);
        assert_eq!(before, fs::read(&path).unwrap());
        assert_eq!(buffer, before);
    }
}
