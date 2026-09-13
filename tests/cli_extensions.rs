use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn extension_commands_are_visible_in_help() {
    Command::cargo_bin("optiflow")
        .unwrap()
        .args(["extensions", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("inspect"))
        .stdout(predicate::str::contains("doctor"));
}

#[test]
fn extension_selection_never_falls_back_to_discovery() {
    Command::cargo_bin("optiflow")
        .unwrap()
        .args(["--json", "extensions", "list"])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("optiflow.command-result.v1"))
        .stdout(predicate::str::contains("--manifest"))
        .stdout(predicate::str::contains("--lock"));
}
