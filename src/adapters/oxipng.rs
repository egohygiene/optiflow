//! A pinned, source-preserving OxiPNG invocation boundary.
//!
//! This adapter captures provider bytes from stdout. It does not validate PNG
//! semantics, publish candidate media, or grant authority over the source.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::domain::{EvidenceDigest, MediaProviderEvidence, NativePath};
use crate::subprocess::{SubprocessCommand, SubprocessOutput, SubprocessRunner};

const VERSION: &str = "10.2.1";
const VERSION_OUTPUT: &str = "oxipng 10.2.1";
const INVOCATION_DOMAIN: &[u8] = b"optiflow.oxipng-invocation.v1\0";

/// One byte-observed provider and its immutable invocation recipe.
#[derive(Debug, Clone)]
pub struct OxipngAdapter {
    executable: PathBuf,
    identity: BinaryIdentity,
    evidence: MediaProviderEvidence,
    arguments: Vec<OsString>,
    max_binary_bytes: u64,
    max_raw_bytes: u64,
    runner: SubprocessRunner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BinaryIdentity {
    digest: EvidenceDigest,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
    length: u64,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    modified: (i64, i64),
    #[cfg(unix)]
    changed: (i64, i64),
}

#[derive(Serialize)]
struct InvocationRecipe<'a> {
    provider: &'static str,
    version: &'static str,
    binary_digest: &'a EvidenceDigest,
    arguments: &'a [String],
    input_binding: &'static str,
    output_binding: &'static str,
    environment: &'static str,
    working_directory: &'static str,
}

impl OxipngAdapter {
    /// Bind one explicitly configured, canonical, regular executable.
    ///
    /// The provider is separately installed; this does not download or bundle
    /// it. A path swap between the final hash and the OS exec remains a host
    /// race, so callers should use a private, stable installation location.
    pub fn lock(
        executable: &Path,
        runner: SubprocessRunner,
        max_binary_bytes: u64,
        max_raw_bytes: u64,
    ) -> Result<Self> {
        if max_binary_bytes == 0 || max_raw_bytes == 0 || usize::try_from(max_raw_bytes).is_err() {
            bail!("invalid OxiPNG binary or raw-byte ceiling");
        }
        let canonical = canonical_executable(executable)?;
        let identity = observe_binary(&canonical, max_binary_bytes)?;

        // Query the same absolute path through the bounded runner, with no
        // inherited environment and an otherwise empty private directory.
        let version_directory = VersionDirectory::new()?;
        let command = SubprocessCommand::new(canonical.as_os_str().to_owned())
            .arg("--version")
            .clear_environment()
            .current_directory(&version_directory.0);
        let version = runner
            .run(&command)
            .context("failed to query pinned OxiPNG version")?;
        if !version.stderr.is_empty() || version.stdout != format!("{VERSION_OUTPUT}\n").as_bytes()
        {
            bail!("provider did not identify exact OxiPNG v{VERSION}");
        }
        if observe_binary(&canonical, max_binary_bytes)? != identity {
            bail!("OxiPNG identity changed during version query");
        }

        let arguments = arguments(max_raw_bytes);
        let argument_strings: Vec<String> = arguments
            .iter()
            .map(|value| value.to_str().expect("fixed ASCII argv").to_owned())
            .collect();
        let recipe = InvocationRecipe {
            provider: "oxipng",
            version: VERSION,
            binary_digest: &identity.digest,
            arguments: &argument_strings,
            input_binding: "bounded_host_observed_bytes_on_stdin",
            output_binding: "bounded_stdout_to_host_owned_candidate_staging",
            environment: "cleared",
            working_directory: "private",
        };
        let mut hasher = blake3::Hasher::new();
        hasher.update(INVOCATION_DOMAIN);
        hasher.update(&serde_json::to_vec(&recipe).context("failed to fingerprint OxiPNG argv")?);
        let evidence = MediaProviderEvidence {
            name: "oxipng".to_owned(),
            version: VERSION.to_owned(),
            executable: NativePath::from_path(&canonical),
            binary_digest: identity.digest.clone(),
            invocation_fingerprint: EvidenceDigest {
                algorithm: "blake3-256".to_owned(),
                value: hasher.finalize().to_hex().to_string(),
            },
        };
        Ok(Self {
            executable: canonical,
            identity,
            evidence,
            arguments,
            max_binary_bytes,
            max_raw_bytes,
            runner,
        })
    }

    pub fn evidence(&self) -> &MediaProviderEvidence {
        &self.evidence
    }

    /// The exact argv, including the decimal raw-byte cap and stdin marker.
    pub fn invocation_arguments(&self) -> Vec<OsString> {
        self.arguments.clone()
    }

    /// Reopen and rehash the path; identity is never inferred from --version.
    pub fn verify_identity(&self) -> Result<()> {
        if canonical_executable(&self.executable)? != self.executable
            || observe_binary(&self.executable, self.max_binary_bytes)? != self.identity
        {
            bail!("OxiPNG executable identity changed");
        }
        Ok(())
    }

    /// Produce untrusted candidate bytes. The caller must independently verify
    /// complete source stability and the candidate's PNG bytes before commit.
    pub fn optimize(
        &self,
        source: &[u8],
        workdir: &Path,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<SubprocessOutput> {
        // OxiPNG applies --max-raw-size to decompressed scanlines, but for stdin
        // it does not also check the encoded input length. Bound both here.
        if source.is_empty() || u64::try_from(source.len()).unwrap_or(u64::MAX) > self.max_raw_bytes
        {
            bail!("OxiPNG input exceeds its configured byte ceiling");
        }
        let workdir = private_workdir(workdir)?;
        self.verify_identity()
            .context("OxiPNG identity changed before invocation")?;
        let command = SubprocessCommand::new(self.executable.as_os_str().to_owned())
            .args(self.arguments.clone())
            .clear_environment()
            .current_directory(workdir);
        let result = self
            .runner
            .run_with_input_and_cancel(&command, source, is_cancelled);
        self.verify_identity()
            .context("OxiPNG identity changed during invocation")?;
        let output = result.context("bounded OxiPNG invocation failed")?;
        if !output.stderr.is_empty() {
            bail!("OxiPNG emitted unexpected stderr on a successful exit");
        }
        if output.stdout.is_empty() {
            bail!("OxiPNG produced no candidate bytes");
        }
        Ok(output)
    }
}

fn arguments(max_raw_bytes: u64) -> Vec<OsString> {
    [
        "--opt".to_owned(),
        "2".to_owned(),
        "--nx".to_owned(),
        "--interlace".to_owned(),
        "keep".to_owned(),
        "--threads".to_owned(),
        "1".to_owned(),
        "--max-raw-size".to_owned(),
        max_raw_bytes.to_string(),
        "--quiet".to_owned(),
        "--stdout".to_owned(),
        "-".to_owned(),
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn canonical_executable(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("OxiPNG executable path must be absolute");
    }
    let input_metadata =
        fs::symlink_metadata(path).context("failed to inspect OxiPNG executable path")?;
    if !input_metadata.is_file() || input_metadata.file_type().is_symlink() {
        bail!("OxiPNG executable must be a regular non-symlink file");
    }
    let actual = path
        .canonicalize()
        .context("failed to resolve OxiPNG executable")?;
    let metadata = fs::symlink_metadata(&actual).context("failed to inspect OxiPNG executable")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        bail!("OxiPNG executable must be a regular non-symlink file");
    }
    Ok(actual)
}

fn observe_binary(path: &Path, max_binary_bytes: u64) -> Result<BinaryIdentity> {
    let before_path = fs::symlink_metadata(path).context("failed to stat OxiPNG path")?;
    if !before_path.is_file() || before_path.file_type().is_symlink() {
        bail!("OxiPNG path is not a regular executable file");
    }
    let mut file = File::open(path).context("failed to open OxiPNG binary")?;
    let before = file
        .metadata()
        .context("failed to stat opened OxiPNG binary")?;
    if !before.is_file() || before.len() == 0 || before.len() > max_binary_bytes {
        bail!("OxiPNG binary exceeds the configured byte ceiling");
    }
    if snapshot(&before_path) != snapshot(&before) {
        bail!("OxiPNG path and opened binary identities disagree");
    }
    let mut hasher = blake3::Hasher::new();
    let mut count = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .context("failed to hash OxiPNG binary")?;
        if read == 0 {
            break;
        }
        count = count
            .checked_add(read as u64)
            .context("OxiPNG binary length overflowed")?;
        if count > max_binary_bytes {
            bail!("OxiPNG binary exceeds the configured byte ceiling");
        }
        hasher.update(&buffer[..read]);
    }
    let after = file
        .metadata()
        .context("failed to restat opened OxiPNG binary")?;
    let after_path = fs::symlink_metadata(path).context("failed to restat OxiPNG path")?;
    if count != before.len()
        || snapshot(&before) != snapshot(&after)
        || snapshot(&after) != snapshot(&after_path)
        || !after_path.is_file()
        || after_path.file_type().is_symlink()
    {
        bail!("OxiPNG binary changed while being observed");
    }
    let mut identity = snapshot(&after);
    identity.digest = EvidenceDigest {
        algorithm: "blake3-256".to_owned(),
        value: hasher.finalize().to_hex().to_string(),
    };
    Ok(identity)
}

fn snapshot(metadata: &fs::Metadata) -> BinaryIdentity {
    BinaryIdentity {
        digest: EvidenceDigest {
            algorithm: "blake3-256".to_owned(),
            value: String::new(),
        },
        #[cfg(unix)]
        dev: metadata.dev(),
        #[cfg(unix)]
        ino: metadata.ino(),
        length: metadata.len(),
        #[cfg(unix)]
        mode: metadata.mode(),
        #[cfg(unix)]
        modified: (metadata.mtime(), metadata.mtime_nsec()),
        #[cfg(unix)]
        changed: (metadata.ctime(), metadata.ctime_nsec()),
    }
}

#[cfg(unix)]
fn private_workdir(workdir: &Path) -> Result<PathBuf> {
    if !workdir.is_absolute() {
        bail!("OxiPNG working directory must be absolute");
    }
    let metadata = fs::symlink_metadata(workdir).context("failed to inspect OxiPNG workdir")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || metadata.mode() & 0o077 != 0 {
        bail!("OxiPNG working directory must be a private non-symlink directory");
    }
    let canonical = workdir
        .canonicalize()
        .context("failed to resolve OxiPNG working directory")?;
    Ok(canonical)
}

#[cfg(not(unix))]
fn private_workdir(_workdir: &Path) -> Result<PathBuf> {
    bail!("OxiPNG execution currently requires Unix workdir checks");
}

struct VersionDirectory(PathBuf);

impl VersionDirectory {
    fn new() -> Result<Self> {
        #[cfg(unix)]
        {
            let parent = std::env::temp_dir()
                .canonicalize()
                .context("failed to resolve OxiPNG temporary directory")?;
            let path = parent.join(format!("optiflow-oxipng-version-{}", uuid::Uuid::now_v7()));
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .context("failed to create private OxiPNG version workdir")?;
            Ok(Self(private_workdir(&path)?))
        }
        #[cfg(not(unix))]
        {
            bail!("OxiPNG execution currently requires Unix workdir checks");
        }
    }
}

impl Drop for VersionDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::time::Duration;

    use tempfile::tempdir;

    use super::*;
    use crate::subprocess::{OutputStream, SubprocessError, SubprocessLimits};

    fn runner(timeout: Duration) -> SubprocessRunner {
        SubprocessRunner::new(SubprocessLimits {
            timeout,
            poll_interval: Duration::from_millis(5),
            max_stdout_bytes: 128,
            max_stderr_bytes: 128,
            max_concurrent_children: 1,
        })
        .unwrap()
    }

    fn provider(directory: &Path, version: &str, body: &str) -> PathBuf {
        let path = directory.join("oxipng-fixture");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nif [ \"${{1:-}}\" = '--version' ]; then printf '%s\\n' '{version}'; exit 0; fi\n{body}\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    #[test]
    fn locks_only_exact_version_and_stable_binary() {
        let directory = tempdir().unwrap();
        let executable = provider(directory.path(), VERSION_OUTPUT, "cat");
        let adapter =
            OxipngAdapter::lock(&executable, runner(Duration::from_secs(1)), 4096, 64).unwrap();
        assert_eq!(adapter.evidence().version, VERSION);
        assert_eq!(adapter.evidence().name, "oxipng");
        assert_eq!(adapter.evidence().binary_digest.algorithm, "blake3-256");
        assert_eq!(adapter.invocation_arguments().len(), 12);
        adapter.verify_identity().unwrap();
        fs::write(&executable, b"#!/bin/sh\nprintf replaced\n").unwrap();
        assert!(adapter.verify_identity().is_err());

        let incorrect = provider(directory.path(), "oxipng 10.2.0", "cat");
        assert!(OxipngAdapter::lock(&incorrect, runner(Duration::from_secs(1)), 4096, 64).is_err());
    }

    #[test]
    fn rejects_symlink_and_oversized_binary() {
        let directory = tempdir().unwrap();
        let executable = provider(directory.path(), VERSION_OUTPUT, "cat");
        let link = directory.path().join("link");
        symlink(&executable, &link).unwrap();
        assert!(OxipngAdapter::lock(&link, runner(Duration::from_secs(1)), 4096, 64).is_err());
        assert!(OxipngAdapter::lock(&executable, runner(Duration::from_secs(1)), 10, 64).is_err());
    }

    #[test]
    fn uses_exact_argv_private_workdir_and_bounded_stdin() {
        let directory = tempdir().unwrap();
        let executable = provider(
            directory.path(),
            VERSION_OUTPUT,
            "test -z \"${HOME+x}\" || exit 6\n\
             test \"$#\" -eq 12 || exit 7\n\
             test \"$1\" = --opt && test \"$2\" = 2 && test \"$3\" = --nx || exit 8\n\
             test \"$4\" = --interlace && test \"$5\" = keep || exit 9\n\
             test \"$6\" = --threads && test \"$7\" = 1 || exit 10\n\
             test \"$8\" = --max-raw-size && test \"$9\" = 64 || exit 11\n\
             shift 9\n\
             test \"$1\" = --quiet && test \"$2\" = --stdout && test \"$3\" = - || exit 12\n\
             cat",
        );
        let adapter =
            OxipngAdapter::lock(&executable, runner(Duration::from_secs(1)), 4096, 64).unwrap();
        let workdir = directory.path().join("private");
        fs::create_dir(&workdir).unwrap();
        fs::set_permissions(&workdir, fs::Permissions::from_mode(0o700)).unwrap();
        let output = adapter.optimize(b"synthetic", &workdir, || false).unwrap();
        assert_eq!(output.stdout, b"synthetic");
        assert!(adapter.optimize(&[0_u8; 65], &workdir, || false).is_err());
        fs::set_permissions(&workdir, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(adapter.optimize(b"synthetic", &workdir, || false).is_err());
    }

    #[test]
    fn propagates_bounded_runner_refusals() {
        let directory = tempdir().unwrap();
        let workdir = directory.path().join("private");
        fs::create_dir(&workdir).unwrap();
        fs::set_permissions(&workdir, fs::Permissions::from_mode(0o700)).unwrap();
        let executable = provider(directory.path(), VERSION_OUTPUT, "printf '%04096d' 0");
        let adapter =
            OxipngAdapter::lock(&executable, runner(Duration::from_secs(1)), 4096, 64).unwrap();
        let error = adapter
            .optimize(b"fixture", &workdir, || false)
            .unwrap_err();
        assert!(matches!(
            error.root_cause().downcast_ref::<SubprocessError>(),
            Some(SubprocessError::Truncated {
                stream: OutputStream::Stdout,
                ..
            })
        ));
        provider(directory.path(), VERSION_OUTPUT, "sleep 1");
        let timed =
            OxipngAdapter::lock(&executable, runner(Duration::from_millis(150)), 4096, 64).unwrap();
        let error = timed.optimize(b"fixture", &workdir, || false).unwrap_err();
        assert!(matches!(
            error.root_cause().downcast_ref::<SubprocessError>(),
            Some(SubprocessError::Timeout { .. })
        ));
    }
}
