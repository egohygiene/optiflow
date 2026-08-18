use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::prelude::*;
use rusqlite::OptionalExtension;
use tempfile::tempdir;

fn command(state: &std::path::Path) -> Command {
    let mut command = Command::cargo_bin("optiflow").expect("compiled optiflow binary");
    command.args([
        "--state-directory",
        state.to_str().expect("UTF-8 state path"),
        "--output-format",
        "json",
    ]);
    command
}

fn json_output(mut command: Command) -> (std::process::ExitStatus, serde_json::Value, Vec<u8>) {
    let output = command.output().expect("command output");
    let document = serde_json::from_slice(&output.stdout).expect("one valid JSON document");
    (output.status, document, output.stderr)
}

#[test]
fn complete_empty_scan_is_success() {
    let workspace = tempdir().expect("workspace");
    let input = workspace.path().join("empty");
    let state = workspace.path().join("state");
    fs::create_dir_all(&input).expect("empty input");

    let mut process = command(&state);
    process.args(["scan", "--no-probe", input.to_str().expect("input path")]);
    let (status, document, stderr) = json_output(process);

    assert_eq!(status.code(), Some(0));
    assert_eq!(document["outcome"]["class"], "success");
    assert_eq!(document["outcome"]["exit_code"], 0);
    assert_eq!(document["coverage"]["status"], "complete");
    assert_eq!(document["result"]["summary"]["file_count"], 0);
    assert!(stderr.is_empty());
}

#[test]
fn one_valid_and_one_missing_input_is_partial_success() {
    let workspace = tempdir().expect("workspace");
    let input = workspace.path().join("media");
    let missing = workspace.path().join("missing");
    let state = workspace.path().join("state");
    fs::create_dir_all(&input).expect("input");
    fs::write(input.join("unique.bin"), b"unique").expect("fixture");

    let mut process = command(&state);
    process.args([
        "scan",
        "--no-probe",
        input.to_str().expect("input path"),
        missing.to_str().expect("missing path"),
    ]);
    let (status, document, stderr) = json_output(process);

    assert_eq!(status.code(), Some(3));
    assert_eq!(document["outcome"]["class"], "partial_success");
    assert_eq!(document["coverage"]["status"], "partial");
    assert_eq!(document["diagnostics"][0]["code"], "partial_inventory");
    assert_eq!(document["artifacts"].as_array().map(Vec::len), Some(3));
    assert!(stderr.is_empty());
}

#[test]
fn every_invalid_scan_input_is_invalid_input_without_artifacts() {
    let workspace = tempdir().expect("workspace");
    let state = workspace.path().join("state");
    let missing = workspace.path().join("missing");

    let mut process = command(&state);
    process.args([
        "scan",
        "--no-probe",
        missing.to_str().expect("missing path"),
    ]);
    let (status, document, stderr) = json_output(process);

    assert_eq!(status.code(), Some(2));
    assert_eq!(document["outcome"]["class"], "invalid_input");
    assert_eq!(document["diagnostics"][0]["code"], "invalid_command_input");
    assert_eq!(document["artifacts"].as_array().map(Vec::len), Some(0));
    assert!(stderr.is_empty());
    assert!(!state.exists());
}

#[test]
fn parse_failure_remains_structured_in_json_mode() {
    let output = Command::cargo_bin("optiflow")
        .expect("binary")
        .args(["--json", "scan"])
        .output()
        .expect("parse failure");
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("structured parse failure");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(document["schema"], "optiflow.command-result.v1");
    assert_eq!(document["outcome"]["class"], "invalid_input");
    assert_eq!(document["diagnostics"][0]["code"], "invalid_invocation");
    assert!(output.stderr.is_empty());
}

#[test]
fn top_level_help_publishes_the_stable_exit_matrix() {
    Command::cargo_bin("optiflow")
        .expect("binary")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("0 success"))
        .stdout(predicates::str::contains("3 partial success"))
        .stdout(predicates::str::contains("130 SIGINT"))
        .stdout(predicates::str::contains("143 SIGTERM"));
}

#[test]
fn malformed_and_missing_run_references_have_distinct_outcomes() {
    let workspace = tempdir().expect("workspace");
    let state = workspace.path().join("state");

    let mut malformed = command(&state);
    malformed.args(["report", "not-a-run-id"]);
    let (malformed_status, malformed_document, _) = json_output(malformed);
    assert_eq!(malformed_status.code(), Some(2));
    assert_eq!(
        malformed_document["diagnostics"][0]["code"],
        "invalid_command_input"
    );

    let mut missing = command(&state);
    missing.args(["report", "018f47a2-4f17-7b00-8000-000000000000"]);
    let (missing_status, missing_document, _) = json_output(missing);
    assert_eq!(missing_status.code(), Some(5));
    assert_eq!(missing_document["outcome"]["class"], "stale_state");
    assert_eq!(
        missing_document["diagnostics"][0]["code"],
        "stored_run_not_found"
    );
}

#[test]
fn repeated_input_is_coalesced_without_degrading_coverage() {
    let workspace = tempdir().expect("workspace");
    let input = workspace.path().join("media");
    let state = workspace.path().join("state");
    fs::create_dir_all(&input).expect("input");
    fs::write(input.join("unique.bin"), b"unique").expect("fixture");

    let mut process = command(&state);
    process.args([
        "scan",
        "--no-probe",
        input.to_str().expect("input path"),
        input.to_str().expect("input path"),
    ]);
    let (status, document, _) = json_output(process);

    assert_eq!(status.code(), Some(0));
    assert_eq!(document["coverage"]["status"], "complete");
    assert_eq!(document["diagnostics"].as_array().map(Vec::len), Some(0));
}

#[test]
fn report_and_plan_preserve_partial_source_coverage() {
    let workspace = tempdir().expect("workspace");
    let input = workspace.path().join("media");
    let missing = workspace.path().join("missing");
    let state = workspace.path().join("state");
    fs::create_dir_all(&input).expect("input");
    fs::write(input.join("unique.bin"), b"unique").expect("fixture");

    let mut scan = command(&state);
    scan.args([
        "scan",
        "--no-probe",
        input.to_str().expect("input path"),
        missing.to_str().expect("missing path"),
    ]);
    let (_, scan_document, _) = json_output(scan);
    let run_id = scan_document["result"]["run"]["run_id"]
        .as_str()
        .expect("run identifier");

    let mut report = command(&state);
    report.args(["report", run_id]);
    let (report_status, report_document, _) = json_output(report);
    assert_eq!(report_status.code(), Some(3));
    assert_eq!(report_document["coverage"]["status"], "partial");
    assert!(
        report_document["diagnostics"]
            .as_array()
            .expect("report diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["code"] == "source_run_partial")
    );

    let mut plan = command(&state);
    plan.args(["plan", "exact-duplicates", "--run", run_id]);
    let (plan_status, plan_document, _) = json_output(plan);
    assert_eq!(plan_status.code(), Some(3));
    assert_eq!(plan_document["coverage"]["status"], "partial");
    assert_eq!(plan_document["result"]["safety"]["mutates_files"], false);
}

#[test]
fn malformed_and_unsupported_report_files_are_distinct() {
    let workspace = tempdir().expect("workspace");
    let state = workspace.path().join("state");
    let malformed_path = workspace.path().join("malformed.json");
    fs::write(&malformed_path, b"{not json").expect("malformed fixture");

    let mut malformed = command(&state);
    malformed.args(["report", malformed_path.to_str().expect("report path")]);
    let (malformed_status, malformed_document, _) = json_output(malformed);
    assert_eq!(malformed_status.code(), Some(2));
    assert_eq!(malformed_document["outcome"]["class"], "invalid_input");

    let unsupported_path = workspace.path().join("unsupported.json");
    fs::write(
        &unsupported_path,
        br#"{"schema_version":"optiflow.report.v99"}"#,
    )
    .expect("unsupported fixture");
    let mut unsupported = command(&state);
    unsupported.args(["report", unsupported_path.to_str().expect("report path")]);
    let (unsupported_status, unsupported_document, _) = json_output(unsupported);
    assert_eq!(unsupported_status.code(), Some(5));
    assert_eq!(unsupported_document["outcome"]["class"], "stale_state");
}

#[test]
fn historical_v1_and_v2_reports_remain_reviewable() {
    let workspace = tempdir().expect("workspace");
    let input = workspace.path().join("media");
    let state = workspace.path().join("state");
    fs::create_dir_all(&input).expect("input");
    fs::write(input.join("unique.bin"), b"unique").expect("fixture");

    let mut scan = command(&state);
    scan.args(["scan", "--no-probe", input.to_str().expect("input path")]);
    let (scan_status, current, _) = json_output(scan);
    assert_eq!(scan_status.code(), Some(0));
    assert_eq!(current["result"]["summary"]["exact_duplicate_groups"], 0);

    for version in [1, 2] {
        let mut historical = current["result"].clone();
        historical["schema_version"] = serde_json::json!(format!("optiflow.report.v{version}"));
        historical["run"]["schema_version"] = serde_json::json!(format!("optiflow.run.v{version}"));
        historical["summary"]
            .as_object_mut()
            .expect("summary")
            .remove("unstable_observation_count");
        for observation in historical["observations"]
            .as_array_mut()
            .expect("observations")
        {
            let observation = observation.as_object_mut().expect("observation");
            observation.remove("observation_stability");
            observation.remove("evidence_validity");
            observation.remove("attempt_count");
        }
        if version == 1 {
            let report = historical.as_object_mut().expect("report");
            report.remove("hard_link_groups");
            report.remove("storage");
            let summary = report["summary"].as_object_mut().expect("summary");
            summary.remove("unique_object_count");
            summary.remove("hard_link_alias_path_count");
            for observation in report["observations"].as_array_mut().expect("observations") {
                let observation = observation.as_object_mut().expect("observation");
                observation.remove("filesystem_identity");
                observation.remove("storage_allocation");
            }
        }

        let path = workspace.path().join(format!("report-v{version}.json"));
        fs::write(
            &path,
            serde_json::to_vec_pretty(&historical).expect("historical JSON"),
        )
        .expect("historical fixture");
        let mut report = command(&state);
        report.args(["report", path.to_str().expect("historical path")]);
        let (status, document, _) = json_output(report);
        assert_eq!(status.code(), Some(0));
        assert_eq!(document["outcome"]["class"], "success");
        assert_eq!(
            document["result"]["schema_version"],
            format!("optiflow.report.v{version}")
        );
    }
}

#[test]
fn human_diagnostics_use_stderr_and_escape_hostile_filenames() {
    let workspace = tempdir().expect("workspace");
    let state = workspace.path().join("state");
    let hostile = workspace.path().join("missing\n\u{1b}[31m.bin");
    let output = Command::cargo_bin("optiflow")
        .expect("binary")
        .args([
            "--state-directory",
            state.to_str().expect("state path"),
            "scan",
            hostile.to_str().expect("hostile UTF-8 path"),
        ])
        .output()
        .expect("human command");

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");
    assert!(stdout.contains("Outcome: invalid input"));
    assert!(!stdout.contains("could not inspect"));
    assert!(stderr.contains("\\n"));
    assert!(stderr.contains("\\u{1b}"));
    assert_eq!(stderr.lines().count(), 1);
    assert!(!stderr.as_bytes().contains(&0x1b));
}

#[cfg(unix)]
#[test]
fn sigint_returns_130_and_never_completes_the_run() {
    assert_signal_outcome("INT", 130, "interrupted");
}

#[cfg(unix)]
#[test]
fn sigterm_returns_143_and_never_completes_the_run() {
    assert_signal_outcome("TERM", 143, "terminated");
}

#[cfg(unix)]
fn assert_signal_outcome(signal: &str, expected_code: i32, expected_class: &str) {
    let workspace = tempdir().expect("workspace");
    let input = workspace.path().join("media");
    let state = workspace.path().join("state");
    fs::create_dir_all(&input).expect("input");
    let first = input.join("first.bin");
    let second = input.join("second.bin");
    fs::File::create(&first)
        .and_then(|file| file.set_len(512 * 1024 * 1024))
        .expect("first sparse fixture");
    fs::File::create(&second)
        .and_then(|file| file.set_len(512 * 1024 * 1024))
        .expect("second sparse fixture");
    let first_before = fs::metadata(&first).expect("first metadata");
    let second_before = fs::metadata(&second).expect("second metadata");

    let mut process = command(&state);
    process
        .args(["scan", "--no-probe", input.to_str().expect("input path")])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = process.spawn().expect("scan process");
    let run_id = wait_for_running_run(&state, &mut child);

    let signal_status = Command::new("kill")
        .args(["-s", signal, &child.id().to_string()])
        .status()
        .expect("signal command");
    assert!(signal_status.success());
    let output = child.wait_with_output().expect("interrupted output");
    assert_eq!(output.status.code(), Some(expected_code));
    assert!(output.stderr.is_empty());
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one complete JSON result");
    assert_eq!(document["outcome"]["class"], expected_class);
    assert_eq!(document["outcome"]["exit_code"], expected_code);
    assert_eq!(document["artifacts"].as_array().map(Vec::len), Some(0));

    let connection = rusqlite::Connection::open(state.join("state.sqlite3")).expect("state");
    let status: String = connection
        .query_row(
            "SELECT status FROM scan_runs WHERE run_id = ?1",
            [&run_id],
            |row| row.get(0),
        )
        .expect("run status");
    assert_eq!(status, "interrupted");
    let completed: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM scan_runs WHERE status = 'completed'",
            [],
            |row| row.get(0),
        )
        .expect("completed count");
    assert_eq!(completed, 0);

    let mut report = command(&state);
    report.args(["report", &run_id]);
    let (report_status, report_document, _) = json_output(report);
    assert_eq!(report_status.code(), Some(5));
    assert_eq!(
        report_document["diagnostics"][0]["code"],
        "source_run_interrupted"
    );

    assert_eq!(
        fs::metadata(&first).expect("first after").len(),
        first_before.len()
    );
    assert_eq!(
        fs::metadata(&second).expect("second after").len(),
        second_before.len()
    );
}

#[cfg(unix)]
fn wait_for_running_run(state: &Path, child: &mut std::process::Child) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            child.try_wait().expect("process status").is_none(),
            "scan exited before the signal synchronization barrier"
        );
        let database = state.join("state.sqlite3");
        if database.exists() {
            let running_id = rusqlite::Connection::open(&database).and_then(|connection| {
                connection
                    .query_row(
                        "SELECT run_id FROM scan_runs WHERE status = 'running' LIMIT 1",
                        [],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
            });
            if let Ok(Some(run_id)) = running_id {
                return run_id;
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for active run"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
