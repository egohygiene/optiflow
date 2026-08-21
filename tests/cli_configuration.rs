#[cfg(target_os = "linux")]
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use tempfile::tempdir;

const CONFIGURATION_ENVIRONMENT: &[&str] = &[
    "OPTIFLOW_CONFIG",
    "OPTIFLOW_STATE_DIRECTORY",
    "OPTIFLOW_OUTPUT_FORMAT",
    "OPTIFLOW_FOLLOW_SYMLINKS",
    "OPTIFLOW_INCLUDE_HIDDEN",
    "OPTIFLOW_CROSS_FILESYSTEMS",
    "OPTIFLOW_PROBE_MEDIA",
    "XDG_CONFIG_HOME",
    "XDG_STATE_HOME",
];

fn command(working_directory: &Path) -> Command {
    let mut command = Command::cargo_bin("optiflow").expect("compiled optiflow binary");
    command.current_dir(working_directory);
    for name in CONFIGURATION_ENVIRONMENT {
        command.env_remove(name);
    }
    command.env("HOME", working_directory.join(".home"));
    command
}

fn json_output(mut command: Command) -> (std::process::ExitStatus, serde_json::Value, Vec<u8>) {
    let output = command.output().expect("command output");
    let document = serde_json::from_slice(&output.stdout).expect("one valid JSON document");
    (output.status, document, output.stderr)
}

fn write_config(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("configuration parent");
    }
    fs::write(path, body).expect("configuration fixture");
}

fn captured_working_directory(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("captured working directory")
}

fn user_configuration_path(_workspace: &Path, _xdg: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        _workspace.join(".home/Library/Application Support/optiflow/optiflow.toml")
    }
    #[cfg(not(target_os = "macos"))]
    {
        _xdg.join("optiflow/optiflow.toml")
    }
}

#[test]
fn defaults_only_are_fully_materialized_without_creating_state() {
    let workspace = tempdir().expect("workspace");
    let mut process = command(workspace.path());
    process.args(["--json", "--no-config", "config", "show"]);
    let (status, document, stderr) = json_output(process);

    assert_eq!(status.code(), Some(0));
    assert!(stderr.is_empty());
    assert_eq!(
        document["result"]["policy"]["schema"],
        "optiflow.effective-policy.v1"
    );
    assert_eq!(
        document["result"]["policy"]["evidence_policy"]["follow_symlinks"],
        false
    );
    assert_eq!(
        document["result"]["policy"]["safety_invariants"]["source_mutation"],
        false
    );
    assert!(!workspace.path().join(".optiflow").exists());
}

#[test]
fn precedence_is_defaults_user_project_environment_then_cli() {
    let workspace = tempdir().expect("workspace");
    let xdg = workspace.path().join("xdg");
    let user = user_configuration_path(workspace.path(), &xdg);
    write_config(
        &user,
        "schema = \"optiflow.config.v1\"\n[state]\ndirectory = \"user-state\"\n",
    );
    let project = workspace.path().join("optiflow.toml");
    write_config(
        &project,
        "schema = \"optiflow.config.v1\"\n[state]\ndirectory = \"project-state\"\n",
    );

    let mut process = command(workspace.path());
    process
        .env("XDG_CONFIG_HOME", &xdg)
        .env("OPTIFLOW_STATE_DIRECTORY", "environment-state")
        .args(["--json", "--state-directory", "cli-state", "config", "show"]);
    let (status, document, _) = json_output(process);

    assert_eq!(status.code(), Some(0));
    assert_eq!(
        document["result"]["policy"]["operational_policy"]["state_directory"]["value"],
        captured_working_directory(workspace.path())
            .join("cli-state")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(
        document["result"]["policy"]["provenance"]["operational_policy.state_directory"]["source"],
        "command_line"
    );
    let shadowed = document["result"]["shadowed_values"]
        .as_array()
        .expect("shadowed values")
        .iter()
        .filter(|value| value["setting"] == "operational_policy.state_directory")
        .count();
    assert_eq!(shadowed, 4);
}

#[test]
fn explicit_config_suppresses_automatic_files_and_cli_overrides_environment_selector() {
    let workspace = tempdir().expect("workspace");
    let project = workspace.path().join("optiflow.toml");
    let environment = workspace.path().join("environment.toml");
    let explicit = workspace.path().join("explicit.toml");
    write_config(
        &project,
        "schema = \"optiflow.config.v1\"\n[scan]\ninclude_hidden = true\n",
    );
    write_config(
        &environment,
        "schema = \"optiflow.config.v1\"\n[scan]\nprobe_media = false\n",
    );
    write_config(
        &explicit,
        "schema = \"optiflow.config.v1\"\n[scan]\nfollow_symlinks = true\n",
    );

    let mut process = command(workspace.path());
    process
        .env("OPTIFLOW_CONFIG", &environment)
        .arg("--json")
        .arg("--config")
        .arg(&explicit)
        .args(["config", "show"]);
    let (status, document, _) = json_output(process);

    assert_eq!(status.code(), Some(0));
    let policy = &document["result"]["policy"]["evidence_policy"];
    assert_eq!(policy["follow_symlinks"], true);
    assert_eq!(policy["include_hidden"], false);
    assert_eq!(policy["probe_media"], true);
    assert_eq!(
        document["result"]["sources"][0]["source"],
        "explicit_configuration"
    );
}

#[test]
fn no_config_suppresses_file_selection_but_keeps_environment_overrides() {
    let workspace = tempdir().expect("workspace");
    let selected = workspace.path().join("selected.toml");
    write_config(
        &selected,
        "schema = \"optiflow.config.v1\"\n[scan]\nfollow_symlinks = true\n",
    );

    let mut process = command(workspace.path());
    process
        .env("OPTIFLOW_CONFIG", &selected)
        .env("OPTIFLOW_INCLUDE_HIDDEN", "true")
        .args(["--json", "--no-config", "config", "show"]);
    let (status, document, _) = json_output(process);

    assert_eq!(status.code(), Some(0));
    let policy = &document["result"]["policy"]["evidence_policy"];
    assert_eq!(policy["follow_symlinks"], false);
    assert_eq!(policy["include_hidden"], true);
}

#[test]
fn file_and_environment_can_select_machine_output_without_cli_aliases() {
    let workspace = tempdir().expect("workspace");
    let config = workspace.path().join("output.toml");
    write_config(
        &config,
        "schema = \"optiflow.config.v1\"\n[output]\nformat = \"json\"\n",
    );

    let mut from_file = command(workspace.path());
    from_file
        .arg("--config")
        .arg(&config)
        .args(["config", "validate"]);
    let (status, document, stderr) = json_output(from_file);
    assert_eq!(status.code(), Some(0));
    assert!(stderr.is_empty());
    assert_eq!(document["schema"], "optiflow.command-result.v1");

    let mut from_environment = command(workspace.path());
    from_environment
        .env("OPTIFLOW_OUTPUT_FORMAT", "json")
        .args(["--no-config", "config", "validate"]);
    let (status, document, _) = json_output(from_environment);
    assert_eq!(status.code(), Some(0));
    assert_eq!(document["schema"], "optiflow.command-result.v1");
}

#[test]
fn nearest_project_configuration_wins() {
    let workspace = tempdir().expect("workspace");
    let child = workspace.path().join("child");
    let nested = child.join("nested");
    fs::create_dir_all(&nested).expect("nested directory");
    write_config(
        &workspace.path().join("optiflow.toml"),
        "schema = \"optiflow.config.v1\"\n[scan]\ninclude_hidden = false\n",
    );
    write_config(
        &child.join("optiflow.toml"),
        "schema = \"optiflow.config.v1\"\n[scan]\ninclude_hidden = true\n",
    );

    let mut process = command(&nested);
    process.args(["--json", "config", "show"]);
    let (status, document, _) = json_output(process);

    assert_eq!(status.code(), Some(0));
    assert_eq!(
        document["result"]["policy"]["evidence_policy"]["include_hidden"],
        true
    );
    assert_eq!(
        document["result"]["sources"][1]["path"]["value"],
        captured_working_directory(&child)
            .join("optiflow.toml")
            .to_string_lossy()
            .as_ref()
    );
}

#[test]
fn scan_input_roots_are_not_configuration_discovery_anchors() {
    let workspace = tempdir().expect("workspace");
    let command_directory = workspace.path().join("command-directory");
    let input = workspace.path().join("media");
    let state = workspace.path().join("state");
    fs::create_dir_all(&command_directory).expect("command directory");
    fs::create_dir_all(&input).expect("input");
    fs::write(input.join(".hidden.bin"), b"hidden").expect("hidden fixture");
    write_config(
        &input.join("optiflow.toml"),
        "schema = \"optiflow.config.v1\"\n[scan]\ninclude_hidden = true\n",
    );

    let mut process = command(&command_directory);
    process
        .arg("--json")
        .arg("--state-directory")
        .arg(&state)
        .args(["scan", "--no-probe"])
        .arg(&input);
    let (status, document, _) = json_output(process);

    assert_eq!(status.code(), Some(0));
    assert_eq!(document["result"]["summary"]["file_count"], 1);
    assert_eq!(document["result"]["summary"]["unsupported_files"], 1);
    assert!(
        document["result"]["observations"]
            .as_array()
            .expect("observations")
            .iter()
            .all(|observation| observation["path"]
                != input.join(".hidden.bin").to_string_lossy().as_ref())
    );
}

#[test]
fn explicit_relative_config_and_file_relative_state_use_documented_bases() {
    let workspace = tempdir().expect("workspace");
    let config_directory = workspace.path().join("settings");
    let config = config_directory.join("policy.toml");
    write_config(
        &config,
        "schema = \"optiflow.config.v1\"\n[state]\ndirectory = \"../state with spaces\"\n",
    );

    let mut process = command(workspace.path());
    process.args([
        "--json",
        "--config",
        "settings/policy.toml",
        "config",
        "show",
    ]);
    let (status, document, _) = json_output(process);

    assert_eq!(status.code(), Some(0));
    assert_eq!(
        document["result"]["sources"][0]["path"]["value"],
        captured_working_directory(workspace.path())
            .join("settings/policy.toml")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(
        document["result"]["policy"]["operational_policy"]["state_directory"]["value"],
        captured_working_directory(workspace.path())
            .join("settings")
            .join("../state with spaces")
            .to_string_lossy()
            .as_ref()
    );
}

#[test]
fn invalid_configuration_scenarios_return_typed_input_failures() {
    let workspace = tempdir().expect("workspace");
    let cases = [
        ("malformed.toml", "schema = [", "configuration_parse_failed"),
        (
            "missing-schema.toml",
            "[output]\nformat = \"json\"\n",
            "configuration_schema_missing",
        ),
        (
            "unsupported.toml",
            "schema = \"optiflow.config.v2\"\n",
            "configuration_schema_unsupported",
        ),
        (
            "unknown.toml",
            "schema = \"optiflow.config.v1\"\n[output]\ncolour = \"auto\"\n",
            "configuration_unknown_key",
        ),
        (
            "duplicate.toml",
            "schema = \"optiflow.config.v1\"\nschema = \"optiflow.config.v1\"\n",
            "configuration_parse_failed",
        ),
        (
            "empty-path.toml",
            "schema = \"optiflow.config.v1\"\n[state]\ndirectory = \"\"\n",
            "configuration_value_invalid",
        ),
        (
            "locked.toml",
            "schema = \"optiflow.config.v1\"\n[safety]\nsource_mutation = true\n",
            "configuration_locked_invariant",
        ),
    ];

    for (name, body, expected_code) in cases {
        let path = workspace.path().join(name);
        write_config(&path, body);
        let mut process = command(workspace.path());
        process
            .arg("--json")
            .arg("--config")
            .arg(&path)
            .args(["config", "validate"]);
        let (status, document, stderr) = json_output(process);
        assert_eq!(status.code(), Some(2), "{name}");
        assert!(stderr.is_empty(), "{name}");
        assert_eq!(document["diagnostics"][0]["code"], expected_code, "{name}");
    }
}

#[test]
fn selector_and_environment_failures_are_typed() {
    let workspace = tempdir().expect("workspace");
    let missing = workspace.path().join("missing.toml");

    let mut conflict = command(workspace.path());
    conflict.arg("--json").arg("--config").arg(&missing).args([
        "--no-config",
        "config",
        "validate",
    ]);
    let (status, document, _) = json_output(conflict);
    assert_eq!(status.code(), Some(2));
    assert_eq!(
        document["diagnostics"][0]["code"],
        "configuration_selector_conflict"
    );

    let mut missing_file = command(workspace.path());
    missing_file
        .arg("--json")
        .arg("--config")
        .arg(&missing)
        .args(["config", "validate"]);
    let (status, document, _) = json_output(missing_file);
    assert_eq!(status.code(), Some(2));
    assert_eq!(
        document["diagnostics"][0]["code"],
        "configuration_not_found"
    );

    let mut invalid_environment = command(workspace.path());
    invalid_environment
        .env("OPTIFLOW_INCLUDE_HIDDEN", "yes")
        .args(["--json", "--no-config", "config", "validate"]);
    let (status, document, _) = json_output(invalid_environment);
    assert_eq!(status.code(), Some(2));
    assert_eq!(
        document["diagnostics"][0]["code"],
        "configuration_environment_invalid"
    );
    assert_eq!(
        document["diagnostics"][0]["context"]["environment_variable"],
        "OPTIFLOW_INCLUDE_HIDDEN"
    );

    let directory = workspace.path().join("directory.toml");
    fs::create_dir(&directory).expect("configuration directory");
    let mut not_regular = command(workspace.path());
    not_regular
        .arg("--json")
        .arg("--config")
        .arg(&directory)
        .args(["config", "validate"]);
    let (status, document, _) = json_output(not_regular);
    assert_eq!(status.code(), Some(2));
    assert_eq!(
        document["diagnostics"][0]["code"],
        "configuration_not_regular_file"
    );
}

#[test]
fn explain_reports_provenance_shadowing_and_unknown_settings() {
    let workspace = tempdir().expect("workspace");
    let config = workspace.path().join("policy.toml");
    write_config(
        &config,
        "schema = \"optiflow.config.v1\"\n[scan]\nprobe_media = false\n",
    );
    let mut explain = command(workspace.path());
    explain.arg("--json").arg("--config").arg(&config).args([
        "config",
        "explain",
        "scan.probe_media",
    ]);
    let (status, document, _) = json_output(explain);
    assert_eq!(status.code(), Some(0));
    assert_eq!(document["result"]["value"], false);
    assert_eq!(
        document["result"]["provenance"]["source"],
        "explicit_configuration"
    );
    assert_eq!(document["result"]["affects_evidence"], true);

    let mut unknown = command(workspace.path());
    unknown.args([
        "--json",
        "--no-config",
        "config",
        "explain",
        "unknown.setting",
    ]);
    let (status, document, _) = json_output(unknown);
    assert_eq!(status.code(), Some(2));
    assert_eq!(
        document["diagnostics"][0]["code"],
        "configuration_setting_unknown"
    );
}

#[test]
fn semantic_fingerprints_ignore_provenance_and_presentation() {
    let workspace = tempdir().expect("workspace");
    let config = workspace.path().join("policy.toml");
    write_config(
        &config,
        "schema = \"optiflow.config.v1\"\n[scan]\ninclude_hidden = true\n",
    );

    let mut from_file = command(workspace.path());
    from_file
        .arg("--json")
        .arg("--config")
        .arg(&config)
        .args(["config", "show"]);
    let (_, file_document, _) = json_output(from_file);

    let mut from_environment = command(workspace.path());
    from_environment
        .env("OPTIFLOW_INCLUDE_HIDDEN", "true")
        .args(["--json", "--no-config", "config", "show"]);
    let (_, environment_document, _) = json_output(from_environment);
    assert_eq!(
        file_document["result"]["policy"]["fingerprints"]["evidence_policy"],
        environment_document["result"]["policy"]["fingerprints"]["evidence_policy"]
    );

    let mut human = command(workspace.path());
    human
        .env("OPTIFLOW_INCLUDE_HIDDEN", "true")
        .args(["--no-config", "config", "show"]);
    let output = human.output().expect("human output");
    let human_text = String::from_utf8(output.stdout).expect("human UTF-8");
    assert!(human_text.contains("\"output_format\": \"human\""));

    assert_ne!(
        file_document["result"]["policy"]["fingerprints"]["effective_configuration"],
        serde_json::Value::Null
    );
}

#[test]
fn scan_consumes_policy_and_persists_historical_evidence() {
    let workspace = tempdir().expect("workspace");
    let input = workspace.path().join("media");
    fs::create_dir_all(&input).expect("input");
    fs::write(input.join(".hidden.bin"), b"hidden").expect("hidden fixture");
    let config = workspace.path().join("policy.toml");
    write_config(
        &config,
        "schema = \"optiflow.config.v1\"\n[state]\ndirectory = \"configured-state\"\n[scan]\ninclude_hidden = true\n",
    );

    let mut scan = command(workspace.path());
    scan.arg("--json").arg("--config").arg(&config).args([
        "scan",
        "--no-probe",
        input.to_str().expect("input path"),
    ]);
    let (status, document, _) = json_output(scan);
    assert_eq!(status.code(), Some(0));
    assert_eq!(document["result"]["summary"]["file_count"], 1);
    assert!(
        document["artifacts"]
            .as_array()
            .expect("artifacts")
            .iter()
            .any(|artifact| artifact["kind"] == "effective_policy")
    );
    let run_id = document["result"]["run"]["run_id"]
        .as_str()
        .expect("run id");
    let policy_path = workspace
        .path()
        .join("configured-state/runs")
        .join(run_id)
        .join("effective-policy.json");
    assert!(policy_path.is_file());

    let mut report = command(workspace.path());
    report
        .arg("--json")
        .arg("--config")
        .arg(&config)
        .args(["report", run_id]);
    let (report_status, report_document, _) = json_output(report);
    assert_eq!(report_status.code(), Some(0));
    assert!(
        report_document["artifacts"]
            .as_array()
            .expect("report artifacts")
            .iter()
            .any(|artifact| artifact["kind"] == "effective_policy")
    );
    assert!(
        report_document["artifacts"]
            .as_array()
            .expect("report artifacts")
            .iter()
            .any(|artifact| artifact["kind"] == "artifact_set")
    );

    let plan_path = workspace.path().join("review-plan.json");
    let mut plan = command(workspace.path());
    plan.arg("--json")
        .arg("--config")
        .arg(&config)
        .args(["plan", "exact-duplicates", "--run", run_id, "--output"])
        .arg(&plan_path);
    let (plan_status, plan_document, _) = json_output(plan);
    assert_eq!(plan_status.code(), Some(0));
    assert!(plan_path.is_file());
    assert!(
        plan_document["artifacts"]
            .as_array()
            .expect("plan artifacts")
            .iter()
            .any(|artifact| artifact["kind"] == "effective_policy")
    );
    assert!(
        plan_document["artifacts"]
            .as_array()
            .expect("plan artifacts")
            .iter()
            .any(|artifact| artifact["kind"] == "artifact_set")
    );

    fs::remove_file(&policy_path).expect("remove policy fixture");
    let mut historical = command(workspace.path());
    historical
        .arg("--json")
        .arg("--config")
        .arg(&config)
        .args(["report", run_id]);
    let (historical_status, historical_document, _) = json_output(historical);
    assert_eq!(historical_status.code(), Some(5));
    assert_eq!(
        historical_document["diagnostics"][0]["code"],
        "artifact_set_incomplete"
    );
}

#[test]
fn explicit_negative_scan_flag_overrides_file_boolean() {
    let workspace = tempdir().expect("workspace");
    let input = workspace.path().join("media");
    fs::create_dir_all(&input).expect("input");
    fs::write(input.join(".hidden.bin"), b"hidden").expect("hidden fixture");
    let config = workspace.path().join("policy.toml");
    write_config(
        &config,
        "schema = \"optiflow.config.v1\"\n[scan]\ninclude_hidden = true\n",
    );

    let mut scan = command(workspace.path());
    scan.arg("--json").arg("--config").arg(&config).args([
        "scan",
        "--exclude-hidden",
        "--no-probe",
        input.to_str().expect("input path"),
    ]);
    let (status, document, _) = json_output(scan);
    assert_eq!(status.code(), Some(0));
    assert_eq!(document["result"]["summary"]["file_count"], 0);
}

#[test]
fn invalid_config_prevents_scan_state_and_artifacts() {
    let workspace = tempdir().expect("workspace");
    let input = workspace.path().join("media");
    let state = workspace.path().join("state");
    fs::create_dir_all(&input).expect("input");
    fs::write(input.join("file.bin"), b"content").expect("fixture");
    let invalid = workspace.path().join("invalid.toml");
    write_config(&invalid, "schema = [");

    let mut process = command(workspace.path());
    process
        .arg("--json")
        .arg("--state-directory")
        .arg(&state)
        .arg("--config")
        .arg(&invalid)
        .args(["scan", input.to_str().expect("input path")]);
    let (status, document, _) = json_output(process);
    assert_eq!(status.code(), Some(2));
    assert!(!state.exists());
    assert_eq!(document["artifacts"].as_array().map(Vec::len), Some(0));
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_explicit_configuration_path_remains_lossless() {
    use std::os::unix::ffi::OsStringExt;

    let workspace = tempdir().expect("workspace");
    let mut bytes = workspace.path().as_os_str().as_encoded_bytes().to_vec();
    bytes.extend_from_slice(b"/policy-");
    bytes.push(0x80);
    bytes.extend_from_slice(b".toml");
    let path = PathBuf::from(OsString::from_vec(bytes));
    write_config(&path, "schema = \"optiflow.config.v1\"\n");

    let mut process = command(workspace.path());
    process
        .arg("--json")
        .arg("--config")
        .arg(&path)
        .args(["config", "show"]);
    let (status, document, _) = json_output(process);
    assert_eq!(status.code(), Some(0));
    assert_eq!(
        document["result"]["sources"][0]["path"]["encoding"],
        "unix_bytes"
    );
}

#[cfg(unix)]
#[test]
fn configuration_symlinks_are_rejected() {
    use std::os::unix::fs::symlink;

    let workspace = tempdir().expect("workspace");
    let target = workspace.path().join("target.toml");
    let link = workspace.path().join("link.toml");
    write_config(&target, "schema = \"optiflow.config.v1\"\n");
    symlink(&target, &link).expect("configuration symlink");

    let mut process = command(workspace.path());
    process
        .arg("--json")
        .arg("--config")
        .arg(&link)
        .args(["config", "validate"]);
    let (status, document, _) = json_output(process);
    assert_eq!(status.code(), Some(2));
    assert_eq!(
        document["diagnostics"][0]["code"],
        "configuration_not_regular_file"
    );
}
