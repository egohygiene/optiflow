//! Synthetic Linux/macOS corpus. No providers, user paths, mounts, or write authority.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use clap::Parser;
use optiflow::artifact_set::{ArtifactSetStatus, inspect_plan_set, inspect_scan_set};
use optiflow::cli::Cli;
use optiflow::domain::{NativePath, ScanReport};
use optiflow::signals::SignalState;
use serde_json::{Value, json};

const BYTES: &[u8] = b"optiflow filesystem corpus v1\n";

struct Fixture {
    directory: tempfile::TempDir,
    input: PathBuf,
    state: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("volume");
        let state = directory.path().join("state");
        fs::create_dir(&input).unwrap();
        Self {
            directory,
            input,
            state,
        }
    }

    fn command(&self, args: &[OsString]) -> Value {
        let mut argv = vec![
            OsString::from("optiflow"),
            OsString::from("--no-config"),
            OsString::from("--state-directory"),
            self.state.clone().into_os_string(),
            OsString::from("--json"),
        ];
        argv.extend_from_slice(args);
        let (result, _) =
            optiflow::run(Cli::try_parse_from(argv).unwrap(), &SignalState::default());
        result.validate().unwrap();
        serde_json::to_value(result).unwrap()
    }

    fn scan(&self, paths: &[PathBuf]) -> Value {
        let mut args = vec![OsString::from("scan"), OsString::from("--no-probe")];
        args.extend(paths.iter().map(|p| p.as_os_str().to_owned()));
        self.command(&args)
    }

    fn finish(self, before: &BTreeMap<PathBuf, Value>) {
        assert_eq!(
            &snapshot(&self.input),
            before,
            "source content, identity, links and modes"
        );
        let path = self.directory.path().to_owned();
        self.directory.close().unwrap();
        assert!(!path.exists(), "explicit corpus cleanup");
    }
}

// atime is intentionally excluded: reads may update it on the host filesystem.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Value> {
    walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .map(|entry| {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = fs::symlink_metadata(path).unwrap();
            let digest = metadata
                .is_file()
                .then(|| blake3::hash(&fs::read(path).unwrap()).to_hex().to_string());
            let link = metadata
                .file_type()
                .is_symlink()
                .then(|| NativePath::from_path(&fs::read_link(path).unwrap()));
            (
                path.strip_prefix(root).unwrap().to_owned(),
                json!({
                    "digest": digest, "link": link, "mode": metadata.mode(),
                    "device": metadata.dev(), "inode": metadata.ino(), "links": metadata.nlink(),
                    "mtime": [metadata.mtime(), metadata.mtime_nsec()],
                    "ctime": [metadata.ctime(), metadata.ctime_nsec()]
                }),
            )
        })
        .collect()
}

fn expect(id: &str, actual: Value) {
    let catalog: Value =
        serde_json::from_str(include_str!("fixtures/filesystem/cases.json")).unwrap();
    let case = catalog["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap();
    assert_eq!(actual, case["expected"], "fixture {id}");
}

#[test]
fn independent_duplicates_and_hard_links() {
    let fixture = Fixture::new();
    let first = fixture.input.join("first.bin");
    fs::write(&first, BYTES).unwrap();
    fs::copy(&first, fixture.input.join("copy.bin")).unwrap();
    fs::hard_link(&first, fixture.input.join("alias.bin")).unwrap();
    let before = snapshot(&fixture.input);
    let document = fixture.scan(std::slice::from_ref(&fixture.input));
    let report = &document["result"];
    expect(
        "fs88-identities",
        json!({
            "outcome": document["outcome"]["class"],
            "paths": report["summary"]["file_count"],
            "objects": report["summary"]["unique_object_count"],
            "aliases": report["summary"]["hard_link_alias_path_count"],
            "logical_savings": report["summary"]["reclaimable_bytes"],
            "physical": report["duplicate_groups"][0]["physical_reclaimability"]["status"]
        }),
    );
    assert_eq!(report["storage"]["path_logical_bytes"], BYTES.len() * 3);
    assert_eq!(
        report["storage"]["unique_object_logical_bytes"],
        BYTES.len() * 2
    );
    let run_id = report["run"]["run_id"].as_str().unwrap();
    let plan = fixture.command(&[
        "plan".into(),
        "exact-duplicates".into(),
        "--run".into(),
        run_id.into(),
    ]);
    assert_eq!(plan["outcome"]["class"], "success");
    assert_eq!(plan["result"]["safety"]["mutates_files"], false);
    fixture.finish(&before);
}

#[test]
fn native_overlapping_deep_roots_and_links() {
    let fixture = Fixture::new();
    let nested = fixture.input.join("space 🌌 e\u{301}");
    let deep = (0..32).fold(nested.clone(), |path, _| path.join("deep"));
    fs::create_dir_all(&deep).unwrap();
    let unusual = deep.join(OsString::from_vec(b"control\ninvalid-\xff.bin".to_vec()));
    fs::write(&unusual, BYTES).unwrap();
    fs::write(nested.join("other.bin"), b"other").unwrap();
    symlink(&unusual, fixture.input.join("link")).unwrap();
    symlink("absent", fixture.input.join("broken")).unwrap();
    let before = snapshot(&fixture.input);
    let document = fixture.scan(&[fixture.input.clone(), nested, fixture.input.clone(), deep]);
    let report: ScanReport = serde_json::from_value(document["result"].clone()).unwrap();
    assert!(
        report
            .observations
            .iter()
            .any(|o| o.path == NativePath::from_path(&unusual))
    );
    expect(
        "fs88-paths",
        json!({
            "outcome": document["outcome"]["class"], "coverage": document["coverage"]["status"],
            "paths": report.summary.file_count,
            "native_path_roundtrip": report.observations.iter().any(|o| o.path.to_path_buf() == unusual)
        }),
    );
    fixture.finish(&before);
}

#[test]
fn sparse_allocation_and_clone_uncertainty() {
    let fixture = Fixture::new();
    for name in ["one", "two"] {
        fs::File::create(fixture.input.join(name))
            .unwrap()
            .set_len(1024 * 1024)
            .unwrap();
    }
    let before = snapshot(&fixture.input);
    let document = fixture.scan(std::slice::from_ref(&fixture.input));
    let mut report: ScanReport = serde_json::from_value(document["result"].clone()).unwrap();
    for observation in &report.observations {
        let path = observation.path.to_path_buf();
        assert_eq!(
            observation
                .storage_allocation
                .as_ref()
                .unwrap()
                .allocated_size_bytes,
            Some(fs::metadata(path).unwrap().blocks() * 512)
        );
    }
    // A synthetic allocation gap models an unknown/clone-capable filesystem.
    // No portable API currently proves extent independence or actual clone creation.
    for observation in &mut report.observations {
        observation
            .storage_allocation
            .as_mut()
            .unwrap()
            .allocated_size_bytes = None;
    }
    let groups = optiflow::duplicates::exact_groups(&report.observations);
    expect(
        "fs88-allocation",
        json!({
            "logical_savings": groups[0].reclaimable_bytes,
            "physical": groups[0].physical_reclaimability.status,
            "reasons": groups[0].physical_reclaimability.reason_codes
        }),
    );
    fixture.finish(&before);
}

#[test]
fn read_only_volume_and_reconnect() {
    let fixture = Fixture::new();
    let path = fixture.input.join("file.bin");
    fs::write(&path, BYTES).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    fs::set_permissions(&fixture.input, fs::Permissions::from_mode(0o555)).unwrap();
    let before = snapshot(&fixture.input);
    let initial = fixture.scan(std::slice::from_ref(&fixture.input));
    assert_eq!(snapshot(&fixture.input), before);
    let unplugged = fixture.directory.path().join("unplugged");
    fs::rename(&fixture.input, &unplugged).unwrap();
    let absent = fixture.scan(std::slice::from_ref(&fixture.input));
    fs::rename(&unplugged, &fixture.input).unwrap();
    // Harness rename can change directory ctime; compare source file exactly.
    assert_eq!(
        snapshot(&fixture.input)[Path::new("file.bin")],
        before[Path::new("file.bin")]
    );
    let reconnected = snapshot(&fixture.input);
    let returned = fixture.scan(std::slice::from_ref(&fixture.input));
    expect(
        "fs88-volume",
        json!({
            "initial": initial["outcome"]["class"], "disconnected": absent["outcome"]["class"],
            "reconnected": returned["outcome"]["class"], "disconnected_artifacts": absent["artifacts"].as_array().unwrap().len()
        }),
    );
    assert_eq!(snapshot(&fixture.input), reconnected);
    fs::set_permissions(&fixture.input, fs::Permissions::from_mode(0o755)).unwrap();
    let before_cleanup = snapshot(&fixture.input);
    fixture.finish(&before_cleanup);
}

#[test]
fn stale_cache_report_and_plan() {
    let fixture = Fixture::new();
    for name in ["one", "two"] {
        fs::write(fixture.input.join(name), BYTES).unwrap();
    }
    let cold = fixture.scan(std::slice::from_ref(&fixture.input));
    let warm = fixture.scan(std::slice::from_ref(&fixture.input));
    assert_eq!(warm["result"]["summary"]["cache_hits"], 2);
    let displaced = fixture.directory.path().join("displaced");
    fs::rename(fixture.input.join("two"), &displaced).unwrap();
    fs::write(fixture.input.join("two"), vec![b'x'; BYTES.len()]).unwrap();
    let before = snapshot(&fixture.input);
    let fresh = fixture.scan(std::slice::from_ref(&fixture.input));
    let run_id = cold["result"]["run"]["run_id"].as_str().unwrap();
    let run_directory = fixture.state.join("runs").join(run_id);
    let plan_path = fixture.directory.path().join("plan.json");
    let plan = fixture.command(&[
        "plan".into(),
        "exact-duplicates".into(),
        "--run".into(),
        run_id.into(),
        "--output".into(),
        plan_path.clone().into_os_string(),
    ]);
    assert_eq!(
        plan["result"]["safety"]["mutates_files"], false,
        "historical evidence is review-only"
    );
    assert_eq!(
        inspect_plan_set(&plan_path).status,
        ArtifactSetStatus::Committed
    );
    fs::write(&plan_path, b"{}").unwrap();
    let plan_status = inspect_plan_set(&plan_path).status;
    fs::write(run_directory.join("report.json"), b"{}").unwrap();
    let report_status = inspect_scan_set(&run_directory).status;
    let rejected = fixture.command(&[
        "plan".into(),
        "exact-duplicates".into(),
        "--run".into(),
        run_id.into(),
    ]);
    expect(
        "fs88-state",
        json!({
            "cache_hits_after_replace": fresh["result"]["summary"]["cache_hits"],
            "groups_after_replace": fresh["result"]["summary"]["exact_duplicate_groups"],
            "corrupt_report": format!("{report_status:?}").to_lowercase(),
            "corrupt_plan": format!("{plan_status:?}").to_lowercase(),
            "plan_from_corrupt_source": rejected["outcome"]["class"]
        }),
    );
    assert_eq!(fs::read(&displaced).unwrap(), BYTES);
    fixture.finish(&before);
}

#[test]
#[ignore = "explicit scheduled corpus tier"]
fn bounded_many_file_inventory() {
    let fixture = Fixture::new();
    for index in 0..512u32 {
        let directory = fixture.input.join(format!("bucket-{:02}", index % 16));
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join(format!("{index:04}.bin")),
            index.to_le_bytes(),
        )
        .unwrap();
    }
    let before = snapshot(&fixture.input);
    let document = fixture.scan(std::slice::from_ref(&fixture.input));
    expect(
        "fs88-stress",
        json!({"outcome": document["outcome"]["class"], "paths": document["result"]["summary"]["file_count"], "groups": document["result"]["summary"]["exact_duplicate_groups"]}),
    );
    fixture.finish(&before);
}
