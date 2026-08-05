use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::{DirEntry, WalkDir};

use crate::domain::ScanOptions;

#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified_unix_ns: Option<i64>,
    pub device_id: Option<u64>,
    pub inode: Option<u64>,
}

#[derive(Debug, Default)]
pub struct DiscoveryResult {
    pub files: Vec<DiscoveredFile>,
    pub warnings: Vec<String>,
}

pub fn discover(
    inputs: &[PathBuf],
    options: &ScanOptions,
    state_directory: &Path,
) -> Result<DiscoveryResult> {
    let state_directory = state_directory.canonicalize().ok();
    let mut unique_paths = BTreeSet::new();
    let mut warnings = Vec::new();

    for input in inputs {
        let absolute_input = absolute_path(input)?;
        let link_metadata = match fs::symlink_metadata(&absolute_input) {
            Ok(metadata) => metadata,
            Err(error) => {
                warnings.push(format!(
                    "could not inspect {}: {error}",
                    absolute_input.display()
                ));
                continue;
            }
        };

        if link_metadata.file_type().is_symlink() && !options.follow_symlinks {
            warnings.push(format!(
                "skipped symbolic-link input {} because link following is disabled",
                absolute_input.display()
            ));
            continue;
        }

        if link_metadata.is_file() {
            unique_paths.insert(absolute_input);
            continue;
        }

        if !link_metadata.is_dir() {
            warnings.push(format!(
                "skipped non-file, non-directory input {}",
                absolute_input.display()
            ));
            continue;
        }

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
            match entry {
                Ok(entry) if entry.file_type().is_file() => {
                    unique_paths.insert(entry.into_path());
                }
                Ok(_) => {}
                Err(error) => warnings.push(format!("filesystem traversal warning: {error}")),
            }
        }
    }

    let mut files = Vec::with_capacity(unique_paths.len());
    for path in unique_paths {
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => files.push(DiscoveredFile {
                path,
                size_bytes: metadata.len(),
                modified_unix_ns: modified_unix_ns(&metadata),
                device_id: device_id(&metadata),
                inode: inode(&metadata),
            }),
            Ok(_) => warnings.push(format!("path stopped being a file: {}", path.display())),
            Err(error) => warnings.push(format!("could not read {}: {error}", path.display())),
        }
    }

    Ok(DiscoveryResult { files, warnings })
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

fn modified_unix_ns(metadata: &fs::Metadata) -> Option<i64> {
    let duration = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    i64::try_from(duration.as_nanos()).ok()
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

#[cfg(unix)]
fn inode(metadata: &fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.ino())
}

#[cfg(not(unix))]
fn inode(_metadata: &fs::Metadata) -> Option<u64> {
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
        )
        .expect("discovery succeeds");

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].path.ends_with("visible.txt"));
    }
}
