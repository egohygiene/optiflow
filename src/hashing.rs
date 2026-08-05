use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result};

pub const HASH_ALGORITHM: &str = "blake3-256";
const BUFFER_SIZE: usize = 1024 * 1024;

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
}
