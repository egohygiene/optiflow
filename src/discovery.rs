use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::{DirEntry, WalkDir};

use crate::domain::ScanOptions;
use crate::filesystem::identity::FileStateSignature;
use crate::signals::SignalState;

#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified_unix_ns: Option<i64>,
    pub signature: FileStateSignature,
}

#[derive(Debug, Default)]
pub struct DiscoveryResult {
    pub files: Vec<DiscoveredFile>,
    pub issues: Vec<DiscoveryIssue>,
    pub accepted_input_count: usize,
    pub interrupted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryIssueKind {
    InputUnavailable,
    InputExcludedByPolicy,
    InvalidInputType,
    TraversalIncomplete,
    PathChanged,
}

#[derive(Debug, Clone)]
pub struct DiscoveryIssue {
    pub kind: DiscoveryIssueKind,
    pub path: Option<PathBuf>,
    pub message: String,
    pub os_error_kind: Option<String>,
}

pub fn discover(
    inputs: &[PathBuf],
    options: &ScanOptions,
    state_directory: &Path,
    signals: &SignalState,
) -> Result<DiscoveryResult> {
    let state_directory = state_directory.canonicalize().ok();
    let mut unique_paths = BTreeSet::new();
    let mut issues = Vec::new();
    let mut accepted_input_count = 0;
    let mut interrupted = false;

    for input in inputs {
        if signals.is_cancelled() {
            interrupted = true;
            break;
        }
        let absolute_input = absolute_path(input)?;
        let link_metadata = match fs::symlink_metadata(&absolute_input) {
            Ok(metadata) => metadata,
            Err(error) => {
                issues.push(DiscoveryIssue {
                    kind: DiscoveryIssueKind::InputUnavailable,
                    path: Some(absolute_input.clone()),
                    message: format!("could not inspect {}: {error}", absolute_input.display()),
                    os_error_kind: Some(format!("{:?}", error.kind()).to_lowercase()),
                });
                continue;
            }
        };

        if link_metadata.file_type().is_symlink() && !options.follow_symlinks {
            issues.push(DiscoveryIssue {
                kind: DiscoveryIssueKind::InputExcludedByPolicy,
                path: Some(absolute_input.clone()),
                message: format!(
                    "skipped symbolic-link input {} because link following is disabled",
                    absolute_input.display()
                ),
                os_error_kind: None,
            });
            continue;
        }

        if link_metadata.is_file() {
            accepted_input_count += 1;
            unique_paths.insert(absolute_input);
            continue;
        }

        if !link_metadata.is_dir() {
            issues.push(DiscoveryIssue {
                kind: DiscoveryIssueKind::InvalidInputType,
                path: Some(absolute_input.clone()),
                message: format!(
                    "skipped non-file, non-directory input {}",
                    absolute_input.display()
                ),
                os_error_kind: None,
            });
            continue;
        }
        accepted_input_count += 1;

        let root_device = device_id(&link_metadata);
        let root = absolute_input.clone();
        let state_directory_for_filter = state_directory.clone();

        let walker = WalkDir::new(&absolute_input)
            .follow_links(options.follow_symlinks)
            .into_iter()
            .filter_entry(move |entry| {
                should_descend(
                    entry,
                    &root,
                    options,
                    root_device,
                    state_directory_for_filter.as_deref(),
                )
            });

        for entry in walker {
            if signals.is_cancelled() {
                interrupted = true;
                break;
            }
            match entry {
                Ok(entry) if entry.file_type().is_file() => {
                    unique_paths.insert(entry.into_path());
                }
                Ok(_) => {}
                Err(error) => issues.push(DiscoveryIssue {
                    kind: DiscoveryIssueKind::TraversalIncomplete,
                    path: error.path().map(Path::to_path_buf),
                    message: format!("filesystem traversal warning: {error}"),
                    os_error_kind: error
                        .io_error()
                        .map(|error| format!("{:?}", error.kind()).to_lowercase()),
                }),
            }
        }
        if interrupted {
            break;
        }
    }

    let mut files = Vec::with_capacity(unique_paths.len());
    for path in unique_paths {
        if signals.is_cancelled() {
            interrupted = true;
            break;
        }
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                let signature = FileStateSignature::from_symlink_metadata(&metadata);
                files.push(DiscoveredFile {
                    path,
                    size_bytes: signature.logical_size_bytes,
                    modified_unix_ns: signature.modified_unix_ns,
                    signature,
                });
            }
            Ok(_) => issues.push(DiscoveryIssue {
                kind: DiscoveryIssueKind::PathChanged,
                path: Some(path.clone()),
                message: format!("path stopped being a file: {}", path.display()),
                os_error_kind: None,
            }),
            Err(error) => issues.push(DiscoveryIssue {
                kind: DiscoveryIssueKind::PathChanged,
                path: Some(path.clone()),
                message: format!("could not read {}: {error}", path.display()),
                os_error_kind: Some(format!("{:?}", error.kind()).to_lowercase()),
            }),
        }
    }

    Ok(DiscoveryResult {
        files,
        issues,
        accepted_input_count,
        interrupted,
    })
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .context("failed to determine the current directory")?
            .join(path))
    }
}

fn should_descend(
    entry: &DirEntry,
    root: &Path,
    options: &ScanOptions,
    root_device: Option<u64>,
    state_directory: Option<&Path>,
) -> bool {
    if entry.depth() == 0 {
        return true;
    }

    if state_directory.is_some_and(|state| entry.path().starts_with(state)) {
        return false;
    }

    if !options.include_hidden && is_hidden(entry.path(), root) {
        return false;
    }

    if !options.cross_filesystems && entry.file_type().is_dir() {
        if let (Some(root_device), Ok(metadata)) = (root_device, entry.metadata()) {
            if device_id(&metadata).is_some_and(|candidate| candidate != root_device) {
                return false;
            }
        }
    }

    true
}

fn is_hidden(path: &Path, root: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        relative.components().any(|component| match component {
            Component::Normal(name) => name.to_string_lossy().starts_with('.'),
            _ => false,
        })
    })
}

#[cfg(unix)]
fn device_id(metadata: &fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.dev())
}

#[cfg(not(unix))]
fn device_id(_metadata: &fs::Metadata) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn hidden_files_are_excluded_by_default() {
        let directory = tempdir().expect("temporary directory");
        fs::write(directory.path().join("visible.txt"), "visible").expect("visible fixture");
        fs::write(directory.path().join(".hidden.txt"), "hidden").expect("hidden fixture");

        let result = discover(
            &[directory.path().to_path_buf()],
            &ScanOptions {
                follow_symlinks: false,
                include_hidden: false,
                cross_filesystems: false,
                probe_media: false,
            },
            &directory.path().join("state"),
            &SignalState::default(),
        )
        .expect("discovery succeeds");

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].path.ends_with("visible.txt"));
    }
}
