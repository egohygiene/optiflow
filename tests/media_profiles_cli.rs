#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use tempfile::tempdir;

const PNG_FIXTURE: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x10, 0x32, 0x09, 0xfb,
    0x0f, 0x00, 0x02, 0x94, 0x01, 0x9c, 0x1d, 0x5b, 0x46, 0x5f, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn install_provider(directory: &Path, inspection_body: &str) -> PathBuf {
    let executable = directory.join("ffprobe");
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then\n  printf '%s\\n' \"ffprobe version hermetic-1.0\"\n  exit 0\nfi\n{inspection_body}\n"
    );
    fs::write(&executable, script).expect("provider fixture");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))
        .expect("provider permissions");
    executable
}

fn scan(state: &Path, provider_directory: &Path, input: &Path, probe: bool) -> serde_json::Value {
    let output = Command::cargo_bin("optiflow")
        .expect("compiled optiflow binary")
        .env("PATH", provider_directory)
        .args([
            "--state-directory",
            state.to_str().expect("state path"),
            "--output-format",
            "json",
            "scan",
            if probe { "--probe" } else { "--no-probe" },
            input.to_str().expect("input path"),
        ])
        .output()
        .expect("scan output");
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON document");
    assert_eq!(
        output.status.code(),
        document["outcome"]["exit_code"]
            .as_i64()
            .map(|code| code as i32)
    );
    assert!(output.stderr.is_empty());
    document
}

#[test]
fn valid_png_provider_evidence_is_committed_without_changing_the_source() {
    let workspace = tempdir().expect("workspace");
    let providers = workspace.path().join("providers");
    let input = workspace.path().join("input");
    let state = workspace.path().join("state");
    fs::create_dir_all(&providers).expect("provider directory");
    fs::create_dir_all(&input).expect("input directory");
    let provider = install_provider(
        &providers,
        r#"printf '%s\n' '{"streams":[{"index":0,"codec_type":"video","codec_name":"png","width":1,"height":1}],"format":{"format_name":"png_pipe","duration":"N/A","bit_rate":"N/A"}}'"#,
    );
    let source = input.join("one pixel with spaces.png");
    fs::write(&source, PNG_FIXTURE).expect("PNG fixture");
    let before = fs::read(&source).expect("source before scan");

    let first = scan(&state, &providers, &input, true);
    assert_eq!(first["outcome"]["exit_code"], 0);
    assert_eq!(first["coverage"]["status"], "complete");
    assert_eq!(first["result"]["schema_version"], "optiflow.report.v6");
    let evidence = &first["result"]["media_profile_evidence"][0];
    assert_eq!(evidence["schema"], "optiflow.media-profile-evidence.v1");
    assert_eq!(
        evidence["profile"]["id"],
        "optiflow.builtin.lossless-png-review"
    );
    assert_eq!(evidence["profile"]["version"], "1.0.0");
    assert_eq!(evidence["coverage"]["status"], "complete");
    assert_eq!(evidence["coverage"]["candidate_media_count"], 1);
    assert_eq!(evidence["coverage"]["opportunity_count"], 1);
    assert_eq!(evidence["entries"][0]["status"], "opportunity");
    assert_eq!(
        evidence["entries"][0]["opportunity"]["savings_claim"],
        "not_estimated"
    );
    assert!(evidence["entries"][0]["opportunity"]["estimated_output_bytes"].is_null());
    assert!(evidence["entries"][0]["opportunity"]["estimated_logical_savings_bytes"].is_null());
    assert_eq!(
        evidence["entries"][0]["provider"]["version"],
        "ffprobe version hermetic-1.0"
    );
    assert_eq!(
        evidence["entries"][0]["provider"]["executable"]["value"],
        provider
            .canonicalize()
            .expect("provider path")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(fs::read(&source).expect("source after scan"), before);

    let run_id = first["result"]["run"]["run_id"]
        .as_str()
        .expect("run identifier");
    let committed: serde_json::Value = serde_json::from_slice(
        &fs::read(state.join("runs").join(run_id).join("report.json")).expect("committed report"),
    )
    .expect("committed report JSON");
    assert_eq!(committed, first["result"]);

    let report_output = Command::cargo_bin("optiflow")
        .expect("compiled optiflow binary")
        .args([
            "--state-directory",
            state.to_str().expect("state path"),
            "--output-format",
            "json",
            "report",
            run_id,
        ])
        .output()
        .expect("report output");
    assert_eq!(report_output.status.code(), Some(0));

    let second = scan(&state, &providers, &input, true);
    let first_evidence = &first["result"]["media_profile_evidence"][0];
    let second_evidence = &second["result"]["media_profile_evidence"][0];
    assert_eq!(
        first_evidence["analysis_id"],
        second_evidence["analysis_id"]
    );
    assert_eq!(
        first_evidence["entries"][0]["entry_id"],
        second_evidence["entries"][0]["entry_id"]
    );
    assert_eq!(
        first_evidence["entries"][0]["opportunity"]["opportunity_id"],
        second_evidence["entries"][0]["opportunity"]["opportunity_id"]
    );

    let no_probe = scan(&state, &providers, &input, false);
    let no_probe_entry = &no_probe["result"]["media_profile_evidence"][0]["entries"][0];
    assert_eq!(
        no_probe["result"]["media_profile_evidence"][0]["coverage"]["status"],
        "not_requested"
    );
    assert!(no_probe_entry["provider"].is_null());
    assert!(no_probe_entry["observations"]["format_name"].is_null());
    assert!(no_probe_entry["observations"]["codec_name"].is_null());
    assert_eq!(no_probe_entry["observations"]["stream_count"], 0);
    assert_eq!(fs::read(&source).expect("source after rerun"), before);
}

#[test]
fn disabled_probe_reports_absent_evidence_without_claiming_an_opportunity() {
    let workspace = tempdir().expect("workspace");
    let providers = workspace.path().join("providers");
    let input = workspace.path().join("input");
    let state = workspace.path().join("state");
    fs::create_dir_all(&providers).expect("provider directory");
    fs::create_dir_all(&input).expect("input directory");
    let source = input.join("source.png");
    fs::write(&source, PNG_FIXTURE).expect("PNG fixture");

    let result = scan(&state, &providers, &input, false);

    assert_eq!(result["outcome"]["exit_code"], 0);
    let evidence = &result["result"]["media_profile_evidence"][0];
    assert_eq!(evidence["coverage"]["status"], "not_requested");
    assert_eq!(evidence["coverage"]["opportunity_count"], 0);
    assert_eq!(evidence["entries"][0]["status"], "insufficient_evidence");
    assert_eq!(
        evidence["entries"][0]["limitations"][0],
        "media_probe_disabled"
    );
    assert!(evidence["entries"][0]["opportunity"].is_null());
    assert_eq!(fs::read(&source).expect("source after scan"), PNG_FIXTURE);
}

#[test]
fn successful_process_with_incomplete_evidence_degrades_coverage() {
    assert_rejected_provider(
        r#"printf '%s\n' '{"streams":[],"format":{"format_name":"png_pipe"}}'"#,
        "provider_result_unavailable",
    );
}

#[test]
fn successful_process_with_malformed_json_degrades_coverage() {
    assert_rejected_provider(
        r#"printf '%s\n' 'not provider JSON'"#,
        "provider_result_unavailable",
    );
}

#[test]
fn contradictory_media_evidence_is_retained_as_invalid_without_an_opportunity() {
    assert_rejected_provider(
        r#"printf '%s\n' '{"streams":[{"index":0,"codec_type":"video","codec_name":"mjpeg","width":1,"height":1}],"format":{"format_name":"jpeg_pipe"}}'"#,
        "provider_evidence_invalid",
    );
}

#[test]
fn failed_provider_degrades_coverage_and_preserves_the_source() {
    assert_rejected_provider(
        "printf '%s\\n' \"fixture probe failure\" >&2\nexit 9",
        "provider_result_unavailable",
    );
}

#[test]
fn transient_provider_failure_is_retried_instead_of_becoming_cached_evidence() {
    let workspace = tempdir().expect("workspace");
    let providers = workspace.path().join("providers");
    let input = workspace.path().join("input");
    let state = workspace.path().join("state");
    fs::create_dir_all(&providers).expect("provider directory");
    fs::create_dir_all(&input).expect("input directory");
    install_provider(
        &providers,
        r#"marker="$0.once"
if [ ! -e "$marker" ]; then
  : > "$marker"
  printf '%s\n' "transient failure" >&2
  exit 9
fi
printf '%s\n' '{"streams":[{"index":0,"codec_type":"video","codec_name":"png","width":1,"height":1}],"format":{"format_name":"png_pipe"}}'"#,
    );
    let source = input.join("source.png");
    fs::write(&source, PNG_FIXTURE).expect("PNG fixture");

    let first = scan(&state, &providers, &input, true);
    assert_eq!(first["outcome"]["exit_code"], 3);
    assert_eq!(
        first["result"]["media_profile_evidence"][0]["coverage"]["status"],
        "unavailable"
    );

    let second = scan(&state, &providers, &input, true);
    assert_eq!(second["outcome"]["exit_code"], 0);
    assert_eq!(
        second["result"]["media_profile_evidence"][0]["coverage"]["status"],
        "complete"
    );
    assert_eq!(
        second["result"]["media_profile_evidence"][0]["coverage"]["opportunity_count"],
        1
    );
    assert_eq!(fs::read(&source).expect("source after scans"), PNG_FIXTURE);
}

#[test]
fn mixed_provider_results_publish_partial_profile_coverage() {
    let workspace = tempdir().expect("workspace");
    let providers = workspace.path().join("providers");
    let input = workspace.path().join("input");
    let state = workspace.path().join("state");
    fs::create_dir_all(&providers).expect("provider directory");
    fs::create_dir_all(&input).expect("input directory");
    install_provider(
        &providers,
        r#"marker="$0.seen"
if [ ! -e "$marker" ]; then
  : > "$marker"
  printf '%s\n' '{"streams":[{"index":0,"codec_type":"video","codec_name":"png","width":1,"height":1}],"format":{"format_name":"png_pipe"}}'
  exit 0
fi
printf '%s\n' "second input rejected" >&2
exit 9"#,
    );
    let first_source = input.join("a.png");
    let second_source = input.join("b.png");
    fs::write(&first_source, PNG_FIXTURE).expect("first PNG fixture");
    fs::write(&second_source, PNG_FIXTURE).expect("second PNG fixture");

    let result = scan(&state, &providers, &input, true);

    assert_eq!(result["outcome"]["exit_code"], 3);
    let evidence = &result["result"]["media_profile_evidence"][0];
    assert_eq!(evidence["coverage"]["status"], "partial");
    assert_eq!(evidence["coverage"]["candidate_media_count"], 2);
    assert_eq!(evidence["coverage"]["complete_evidence_count"], 1);
    assert_eq!(evidence["coverage"]["limited_evidence_count"], 1);
    assert_eq!(evidence["coverage"]["opportunity_count"], 1);
    assert_eq!(fs::read(&first_source).expect("first source"), PNG_FIXTURE);
    assert_eq!(
        fs::read(&second_source).expect("second source"),
        PNG_FIXTURE
    );
}

fn assert_rejected_provider(inspection_body: &str, expected_limitation: &str) {
    let workspace = tempdir().expect("workspace");
    let providers = workspace.path().join("providers");
    let input = workspace.path().join("input");
    let state = workspace.path().join("state");
    fs::create_dir_all(&providers).expect("provider directory");
    fs::create_dir_all(&input).expect("input directory");
    install_provider(&providers, inspection_body);
    let source = input.join("source.png");
    fs::write(&source, PNG_FIXTURE).expect("PNG fixture");
    let before = fs::read(&source).expect("source before scan");

    let result = scan(&state, &providers, &input, true);

    assert_eq!(result["outcome"]["exit_code"], 3);
    assert_eq!(result["coverage"]["status"], "partial");
    assert!(result["diagnostics"].as_array().is_some_and(|diagnostics| {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "result_produced_with_exclusions")
    }));
    let evidence = &result["result"]["media_profile_evidence"][0];
    assert_eq!(evidence["coverage"]["status"], "unavailable");
    assert_eq!(evidence["coverage"]["opportunity_count"], 0);
    assert_eq!(
        evidence["entries"][0]["limitations"][0],
        expected_limitation
    );
    assert!(evidence["entries"][0]["opportunity"].is_null());
    assert_eq!(fs::read(&source).expect("source after scan"), before);
}
