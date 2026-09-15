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
    let envelope: serde_json::Value =
        serde_json::from_slice(&scan_output.stdout).expect("scan report JSON");
    assert_eq!(envelope["schema"], "optiflow.command-result.v1");
    assert_eq!(envelope["outcome"]["class"], "success");
    let report = &envelope["result"];
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
        .stdout(predicate::str::contains("\"mutates_files\": false"))
        .stdout(predicate::str::contains(
            "\"schema\": \"optiflow.command-result.v1\"",
        ));

    assert_eq!(
        fs::read(&first).expect("first remains"),
        b"identical fixture bytes"
    );
    assert_eq!(
        fs::read(&second).expect("second remains"),
        b"identical fixture bytes"
    );
}

#[cfg(unix)]
#[test]
fn scan_classifies_hard_link_aliases_without_counting_duplicates() {
    let workspace = tempdir().expect("temporary workspace");
    let input = workspace.path().join("hard-link fixture");
    let state = workspace.path().join("state");
    fs::create_dir_all(&input).expect("input directory");
    let original = input.join("original.bin");
    let alias = input.join("alias.bin");
    fs::write(&original, b"same object").expect("original fixture");
    fs::hard_link(&original, &alias).expect("hard-link alias");

    let scan_output = Command::cargo_bin("optiflow")
        .expect("binary")
        .args([
            "--state-directory",
            state.to_str().expect("state path"),
            "--output-format",
            "json",
            "scan",
            "--no-probe",
            input.to_str().expect("input path"),
        ])
        .output()
        .expect("scan command");
    assert!(scan_output.status.success());
    let envelope: serde_json::Value =
        serde_json::from_slice(&scan_output.stdout).expect("scan report JSON");
    let report = &envelope["result"];

    assert_eq!(report["summary"]["file_count"], 2);
    assert_eq!(report["summary"]["unique_object_count"], 1);
    assert_eq!(report["summary"]["hard_link_alias_path_count"], 1);
    assert_eq!(report["summary"]["exact_duplicate_groups"], 0);
    assert_eq!(report["hard_link_groups"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        report["hard_link_groups"][0]["observed_paths"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(report["storage"]["path_logical_bytes"], 22);
    assert_eq!(report["storage"]["unique_object_logical_bytes"], 11);
    assert_eq!(report["storage"]["hard_link_alias_logical_bytes"], 11);
    assert_eq!(
        fs::read(&original).expect("original remains"),
        b"same object"
    );
    assert_eq!(fs::read(&alias).expect("alias remains"), b"same object");
}
