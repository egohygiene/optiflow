//! Offline, synthetic end-to-end proofs for bounded PNG candidate production.
//! The fake executable stands in for OxiPNG at the subprocess boundary. These
//! tests never launch a provider against user media and do not test OxiPNG's
//! own optimization algorithm.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use optiflow::candidate_artifact::{self, Status};
use optiflow::domain::EvidenceDigest;
use optiflow::png_production::{ProductionLimits, ProductionRefusal, produce_png};
use tempfile::TempDir;

struct SyntheticRun {
    _temp: TempDir,
    source: PathBuf,
    candidate: PathBuf,
    executable: PathBuf,
    root: PathBuf,
}

impl SyntheticRun {
    fn new(source: &str, candidate: &str) -> Self {
        // macOS commonly exposes its temporary directory through /var, which
        // is a symlink. The production no-follow walk requires the real path.
        let temp = tempfile::Builder::new()
            .prefix("optiflow-png-production-")
            .tempdir_in(std::env::temp_dir().canonicalize().unwrap())
            .unwrap();
        let source_path = temp.path().join("source.png");
        let candidate_path = temp.path().join("synthetic-provider-output.png");
        let executable = temp.path().join("synthetic-oxipng");
        let root = temp.path().join("candidate-sets");
        fs::write(&source_path, fixture(source)).unwrap();
        fs::write(&candidate_path, fixture(candidate)).unwrap();
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let run = Self {
            _temp: temp,
            source: source_path,
            candidate: candidate_path,
            executable,
            root,
        };
        run.script(&format!("/bin/cat {}", shell_quote(&run.candidate)));
        run
    }

    fn script(&self, body: &str) {
        self.versioned_script("oxipng 10.2.1", body);
    }

    fn versioned_script(&self, version: &str, body: &str) {
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf '%s\\n' {}\n  exit 0\nfi\n/bin/cat >/dev/null\n{}\n",
            shell_quote_str(version),
            body
        );
        fs::write(&self.executable, script).unwrap();
        fs::set_permissions(&self.executable, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn produce(
        &self,
        limits: ProductionLimits,
        cancelled: impl Fn() -> bool,
    ) -> Result<
        optiflow::png_production::ProductionReceipt,
        optiflow::png_production::ProductionFailure,
    > {
        produce_png(
            &self.source,
            &self.executable,
            &self.root,
            &policy(),
            limits,
            cancelled,
        )
    }

    fn assert_no_published_candidate(&self) {
        let entries: Vec<_> = fs::read_dir(&self.root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert!(
            entries.is_empty(),
            "failure left a candidate or uncleaned provider work: {entries:?}"
        );
    }
}

fn shell_quote(path: &Path) -> String {
    shell_quote_str(path.to_str().unwrap())
}

fn shell_quote_str(text: &str) -> String {
    format!(
        "\"{}\"",
        text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
            .replace('`', "\\`")
    )
}

fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/media/generated")
            .join(name),
    )
    .unwrap()
}

fn policy() -> EvidenceDigest {
    EvidenceDigest {
        algorithm: "blake3-256".to_owned(),
        value: "a".repeat(64),
    }
}

#[test]
fn rgb_and_rgba_candidates_publish_with_complete_evidence_and_leave_source_untouched() {
    for (source, candidate) in [
        ("rgb-stored.png", "rgb-fixed.png"),
        ("rgba-stored.png", "rgba-fixed.png"),
    ] {
        let run = SyntheticRun::new(source, candidate);
        let original = fs::read(&run.source).unwrap();
        let original_metadata = fs::metadata(&run.source).unwrap();
        let expected = fs::read(&run.candidate).unwrap();
        let receipt = run.produce(ProductionLimits::default(), || false).unwrap();
        assert_eq!(fs::read(&run.source).unwrap(), original);
        assert_eq!(
            fs::metadata(&run.source).unwrap().len(),
            original_metadata.len()
        );
        assert_eq!(
            fs::read(receipt.directory.join("candidate.png")).unwrap(),
            expected
        );
        assert_eq!(
            receipt.evidence.source.before,
            receipt.evidence.source.after
        );
        assert_eq!(
            receipt.evidence.source.content.logical_bytes,
            original.len() as u64
        );
        assert_eq!(
            receipt.evidence.source.content.digest.value,
            blake3::hash(&original).to_hex().to_string()
        );
        assert_eq!(
            receipt.evidence.candidate.content.logical_bytes,
            expected.len() as u64
        );
        assert_eq!(
            receipt.evidence.candidate.content.digest.value,
            blake3::hash(&expected).to_hex().to_string()
        );
        assert_eq!(receipt.evidence.producer.name, "oxipng");
        assert_eq!(receipt.evidence.producer.version, "10.2.1");
        assert_eq!(receipt.evidence.effective_policy_fingerprint, policy());
        assert_eq!(
            receipt.evidence.encoded_logical_reduction_bytes,
            (original.len() - expected.len()) as u64
        );
        assert_eq!(receipt.evidence.physical_savings_bytes, None);
        assert!(receipt.evidence.validation.independent_byte_validation);
        assert!(receipt.evidence.validation.exact_decoded_samples);
        assert!(
            receipt
                .evidence
                .validation
                .ordered_non_idat_chunks_preserved
        );
        assert!(receipt.evidence.validation.strict_encoded_reduction);
        assert!(receipt.evidence.cleanup.provider_work_directory_removed);
        assert!(!receipt.evidence.cleanup.source_mutation);
        assert_eq!(
            receipt.evidence.measurements.peak_provider_memory_bytes,
            None
        );
        assert_eq!(
            receipt.evidence.measurements.peak_provider_temporary_bytes,
            None
        );
        assert_eq!(
            candidate_artifact::inspect(&receipt.directory).status,
            Status::Committed
        );
        assert_eq!(
            candidate_artifact::recover(&run.root, receipt.set_id).status,
            Status::Committed
        );
        assert_eq!(fs::read_dir(&run.root).unwrap().count(), 1);
    }
}

#[test]
fn unsupported_and_corrupt_sources_refuse_before_provider_launch() {
    for (source, expected) in [
        (
            "rgb-source-iccp-stored.png",
            ProductionRefusal::UnsupportedPng,
        ),
        (
            "rgb-source-bad-crc-stored.png",
            ProductionRefusal::InvalidPng,
        ),
    ] {
        let run = SyntheticRun::new(source, "rgb-fixed.png");
        let marker = run._temp.path().join("version-was-called");
        fs::write(
            &run.executable,
            format!(
                "#!/bin/sh\n/bin/touch {}\nif [ \"$1\" = \"--version\" ]; then\n  printf '%s\\n' 'oxipng 10.2.1'\n  exit 0\nfi\nexit 0\n",
                shell_quote(&marker)
            ),
        )
        .unwrap();
        fs::set_permissions(&run.executable, fs::Permissions::from_mode(0o700)).unwrap();
        let failure = run
            .produce(ProductionLimits::default(), || false)
            .unwrap_err();
        assert_eq!(failure.reason, expected, "{source}: {failure}");
        assert!(!marker.exists());
        run.assert_no_published_candidate();
    }
}

#[test]
fn fake_provider_bytes_are_refused_if_equal_larger_changed_or_malformed() {
    for (source, candidate, expected) in [
        (
            "rgb-stored.png",
            "rgb-stored.png",
            ProductionRefusal::CandidateNotSmaller,
        ),
        (
            "rgb-fixed.png",
            "rgb-stored.png",
            ProductionRefusal::CandidateNotSmaller,
        ),
        (
            "rgba-stored.png",
            "rgba-hidden-rgb-change-fixed.png",
            ProductionRefusal::CandidateChanged,
        ),
        (
            "rgba-stored.png",
            "rgba-metadata-moved-fixed.png",
            ProductionRefusal::CandidateChanged,
        ),
        (
            "rgb-stored.png",
            "rgb-bad-crc-fixed.png",
            ProductionRefusal::CandidateChanged,
        ),
        (
            "rgb-stored.png",
            "rgb-indexed-fixed.png",
            ProductionRefusal::UnsupportedPng,
        ),
    ] {
        let run = SyntheticRun::new(source, candidate);
        let original = fs::read(&run.source).unwrap();
        let failure = run
            .produce(ProductionLimits::default(), || false)
            .unwrap_err();
        assert_eq!(
            failure.reason, expected,
            "{source} -> {candidate}: {failure}"
        );
        assert_eq!(fs::read(&run.source).unwrap(), original);
        run.assert_no_published_candidate();
    }
}

#[test]
fn bounded_output_and_provider_exit_leave_no_candidate_or_work_directory() {
    let run = SyntheticRun::new("rgb-stored.png", "rgb-fixed.png");
    let limits = ProductionLimits {
        candidate_bytes: fixture("rgb-fixed.png").len() - 1,
        ..ProductionLimits::default()
    };
    let failure = run.produce(limits, || false).unwrap_err();
    assert_eq!(failure.reason, ProductionRefusal::OutputBoundExceeded);
    run.assert_no_published_candidate();

    run.script("exit 27");
    let failure = run
        .produce(ProductionLimits::default(), || false)
        .unwrap_err();
    assert_eq!(failure.reason, ProductionRefusal::ProviderFailed);
    run.assert_no_published_candidate();
}

#[test]
fn timeout_and_cancellation_kill_provider_and_leave_no_candidate() {
    let run = SyntheticRun::new("rgb-stored.png", "rgb-fixed.png");
    run.script("/bin/sleep 2");
    let limits = ProductionLimits {
        elapsed_ms: 250,
        ..ProductionLimits::default()
    };
    let failure = run.produce(limits, || false).unwrap_err();
    assert_eq!(failure.reason, ProductionRefusal::ProviderTimedOut);
    run.assert_no_published_candidate();

    let cancelled = Arc::new(AtomicBool::new(false));
    let trigger = Arc::clone(&cancelled);
    let signal = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        trigger.store(true, Ordering::SeqCst);
    });
    let failure = run
        .produce(ProductionLimits::default(), || {
            cancelled.load(Ordering::SeqCst)
        })
        .unwrap_err();
    signal.join().unwrap();
    assert_eq!(failure.reason, ProductionRefusal::Cancelled);
    run.assert_no_published_candidate();
}

#[test]
fn source_changed_during_provider_execution_never_publishes() {
    let run = SyntheticRun::new("rgb-stored.png", "rgb-fixed.png");
    run.script(&format!(
        "/bin/cp {} {}\n/bin/cat {}",
        shell_quote(&run.candidate),
        shell_quote(&run.source),
        shell_quote(&run.candidate)
    ));
    let failure = run
        .produce(ProductionLimits::default(), || false)
        .unwrap_err();
    assert_eq!(failure.reason, ProductionRefusal::ChangedSource);
    run.assert_no_published_candidate();
}

#[test]
fn invalid_provider_identity_unsafe_paths_and_limits_refuse_without_publication() {
    let run = SyntheticRun::new("rgb-stored.png", "rgb-fixed.png");
    run.versioned_script("oxipng 9.0.0", "exit 0");
    assert_eq!(
        run.produce(ProductionLimits::default(), || false)
            .unwrap_err()
            .reason,
        ProductionRefusal::ProviderUnavailable
    );
    run.assert_no_published_candidate();

    run.script("exit 0");
    let source_link = run._temp.path().join("source-link.png");
    symlink(&run.source, &source_link).unwrap();
    assert_eq!(
        produce_png(
            &source_link,
            &run.executable,
            &run.root,
            &policy(),
            ProductionLimits::default(),
            || false
        )
        .unwrap_err()
        .reason,
        ProductionRefusal::InvalidInput
    );
    run.assert_no_published_candidate();

    let limits = ProductionLimits {
        source_bytes: fixture("rgb-stored.png").len() - 1,
        ..ProductionLimits::default()
    };
    assert_eq!(
        run.produce(limits, || false).unwrap_err().reason,
        ProductionRefusal::InvalidInput
    );
    run.assert_no_published_candidate();

    fs::set_permissions(&run.root, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        run.produce(ProductionLimits::default(), || false)
            .unwrap_err()
            .reason,
        ProductionRefusal::InvalidInput
    );
    run.assert_no_published_candidate();
}

#[test]
fn corrupting_committed_candidate_fails_readback_and_recovery_never_promotes_it() {
    let run = SyntheticRun::new("rgb-stored.png", "rgb-fixed.png");
    let receipt = run.produce(ProductionLimits::default(), || false).unwrap();
    fs::write(
        receipt.directory.join("candidate.png"),
        b"tampered candidate",
    )
    .unwrap();
    assert_eq!(
        candidate_artifact::inspect(&receipt.directory).status,
        Status::Incomplete
    );
    assert_eq!(
        candidate_artifact::recover(&run.root, receipt.set_id).status,
        Status::Incomplete
    );
}
