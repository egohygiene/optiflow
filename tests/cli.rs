use std::fs;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn scan_and_plan_exact_duplicates_without_mutating_inputs() {
    let workspace = tempdir().expect("temporary workspace");
    let input = workspace.path().join("media with spaces");
    let state = workspace.path().join("state");
    fs::create_dir_all(&input).expect("input directory");
    let first = input.join("first-🌌.bin");
    let second = input.join("second.bin");
    fs::write(&first, b"identical fixture bytes").expect("first fixture");
    fs::copy(&first, &second).expect("second fixture");

    let scan_output = Command::cargo_bin("optiflow")
        .expect("binary")
        .args([
            "--state-directory",
            state.to_str().expect("state path"),
            "--json",
            "scan",
            "--no-probe",
            input.to_str().expect("input path"),
        ])
        .output()
        .expect("scan command");
    assert!(scan_output.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&scan_output.stdout).expect("scan report JSON");
    assert_eq!(report["summary"]["exact_duplicate_groups"], 1);
    assert_eq!(report["summary"]["reclaimable_bytes"], 23);

    let run_id = report["run"]["run_id"].as_str().expect("run identifier");
    Command::cargo_bin("optiflow")
        .expect("binary")
        .args([
            "--state-directory",
            state.to_str().expect("state path"),
            "--json",
            "plan",
            "exact-duplicates",
            "--run",
            run_id,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"mutates_files\": false"));

    assert_eq!(
        fs::read(&first).expect("first remains"),
        b"identical fixture bytes"
    );
    assert_eq!(
        fs::read(&second).expect("second remains"),
        b"identical fixture bytes"
    );
}
