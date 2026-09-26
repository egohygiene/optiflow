//! Synthetic execution authorization and dry-run safety boundaries.
#![cfg(unix)]
use clap::Parser;
use optiflow::cli::{Cli, ExecutionPlanArgs};
use optiflow::configuration::{self, EffectivePolicyV1};
use optiflow::contracts::{self, Contract};
use optiflow::execution::{self, model::*};
use optiflow::outcome::DiagnosticCode as Code;
use optiflow::signals::SignalState;
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};

struct Fixture {
    _temp: tempfile::TempDir,
    base: PathBuf,
    state: PathBuf,
    args: ExecutionPlanArgs,
    policy: EffectivePolicyV1,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        let root = base.join("source");
        let state = base.join("state");
        let quarantine = base.join("quarantine");
        for dir in [&root, &state, &quarantine] {
            fs::create_dir(dir).unwrap();
        }
        let keep = root.join("keeper.bin");
        let candidate = root.join("candidate.bin");
        for path in [&keep, &candidate] {
            fs::write(path, b"exact duplicate synthetic bytes").unwrap();
        }
        let cli = Cli::parse_from([
            "optiflow",
            "--no-config",
            "--state-directory",
            state.to_str().unwrap(),
            "doctor",
        ]);
        let policy = configuration::resolve(&cli).unwrap().policy;
        let args = ExecutionPlanArgs {
            keep,
            candidate: vec![candidate],
            root: vec![root],
            subtree: Vec::new(),
            quarantine,
            max_actions: 5,
            max_in_flight_bytes: 1024,
            reserve_bytes: 1,
            output: base.join("plan.json"),
        };
        Self {
            _temp: temp,
            base,
            state,
            args,
            policy,
        }
    }
    fn plan(&self) -> ExecutionPlan {
        execution::create_plan(
            &self.args,
            &self.state,
            &self.policy,
            &SignalState::default(),
        )
        .unwrap()
    }
    fn approval(&self, plan: &ExecutionPlan) -> Approval {
        execution::approve(plan, &plan.fingerprint, "fixture operator").unwrap()
    }
    fn run(&self, p: &ExecutionPlan) -> execution::Result<ExecutionRun> {
        execution::dry_run(
            p,
            &self.approval(p),
            &self.state,
            &self.policy,
            &SignalState::default(),
        )
    }
    fn documents(&self) -> ExecutionPlan {
        let p = self.plan();
        execution::write_document(&self.args.output, &p, &p).unwrap();
        execution::write_document(&self.base.join("approval.json"), &self.approval(&p), &p)
            .unwrap();
        p
    }
}
fn seal(p: &mut ExecutionPlan) {
    p.fingerprint =
        blake3::hash(&serde_json::to_vec(&serde_json::to_value(&p.body).unwrap()).unwrap())
            .to_hex()
            .to_string();
}
fn source(path: &Path) -> (Vec<u8>, u64, u64, i64, i64, u32) {
    let m = fs::metadata(path).unwrap();
    (
        fs::read(path).unwrap(),
        m.dev(),
        m.ino(),
        m.mtime_nsec(),
        m.ctime_nsec(),
        m.mode(),
    )
}

#[test]
fn approved_dry_run_is_durable_but_does_not_mutate_sources_or_quarantine() {
    let f = Fixture::new();
    let p = f.plan();
    let before: Vec<_> = [&f.args.keep, &f.args.candidate[0]]
        .iter()
        .map(|p| source(p))
        .collect();
    let run = f.run(&p).unwrap();
    assert_eq!(run.status, Status::Validated);
    assert!(
        run.attempts
            .iter()
            .all(|a| a.hash_confirmed && a.bytes_confirmed && a.identities_confirmed)
    );
    assert!(!run.commit.source_mutated);
    assert_eq!(run.commit.committed_actions, 0);
    assert_eq!(run.savings.immediate_logical_reclaimed_bytes, 0);
    assert_eq!(run.savings.physical_reclaimed_bytes, None);
    assert_eq!(fs::read_dir(&f.args.quarantine).unwrap().count(), 0);
    let after: Vec<_> = [&f.args.keep, &f.args.candidate[0]]
        .iter()
        .map(|p| source(p))
        .collect();
    assert_eq!(before, after);
    let stored = execution::load_execution(&f.state, &run.run_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(stored).unwrap(),
        serde_json::to_value(&run).unwrap()
    );
    contracts::validate(Contract::Execution, &run).unwrap();
    let repeated = f.run(&p).unwrap();
    assert_ne!(run.run_id, repeated.run_id);
    assert_eq!(run.plan_fingerprint, repeated.plan_fingerprint);
}

#[test]
fn stale_same_size_rewrite_and_same_bytes_replacement_are_rejected() {
    for replace in [false, true] {
        let f = Fixture::new();
        let p = f.plan();
        let path = &f.args.candidate[0];
        if replace {
            let other = f.base.join("other");
            fs::write(&other, fs::read(path).unwrap()).unwrap();
            fs::rename(other, path).unwrap();
        } else {
            fs::write(path, vec![b'x'; fs::metadata(path).unwrap().len() as usize]).unwrap();
        }
        let before = source(path);
        let run = f.run(&p).unwrap();
        assert_eq!(run.status, Status::Rejected);
        assert!(
            run.validation
                .diagnostics
                .iter()
                .any(|d| d.code == Code::ExecutionSourceStale)
        );
        assert_eq!(before, source(path));
    }
}

#[test]
fn disappearance_symlink_substitution_and_new_hard_link_fail_closed() {
    for case in 0..3 {
        let f = Fixture::new();
        let p = f.plan();
        let path = &f.args.candidate[0];
        match case {
            0 => fs::rename(path, f.base.join("disconnected")).unwrap(),
            1 => {
                fs::remove_file(path).unwrap();
                symlink(&f.args.keep, path).unwrap();
            }
            _ => fs::hard_link(path, f.base.join("unobserved-link")).unwrap(),
        }
        let run = f.run(&p).unwrap();
        assert_eq!(run.status, Status::Rejected);
        assert_eq!(run.commit.committed_actions, 0);
    }
}

#[test]
fn changed_root_and_quarantine_identities_never_create_state() {
    for root in [true, false] {
        let f = Fixture::new();
        let p = f.plan();
        let path = if root {
            &f.args.root[0]
        } else {
            &f.args.quarantine
        };
        fs::rename(path, f.base.join("disconnected")).unwrap();
        fs::create_dir(path).unwrap();
        assert_eq!(f.run(&p).unwrap_err().code, Code::ExecutionSourceStale);
        assert!(!f.state.join("state.sqlite3").exists());
    }
}

#[test]
fn read_only_sources_and_permission_changes_are_blocked() {
    let f = Fixture::new();
    let p = f.plan();
    fs::set_permissions(&f.args.candidate[0], fs::Permissions::from_mode(0o444)).unwrap();
    assert_eq!(
        execution::create_plan(&f.args, &f.state, &f.policy, &SignalState::default())
            .unwrap_err()
            .code,
        Code::ExecutionReadOnly
    );
    assert_eq!(f.run(&p).unwrap().status, Status::Rejected);
    fs::set_permissions(&f.args.candidate[0], fs::Permissions::from_mode(0o644)).unwrap();
    let p = f.plan();
    fs::set_permissions(&f.args.quarantine, fs::Permissions::from_mode(0o555)).unwrap();
    assert!(f.run(&p).is_err());
    fs::set_permissions(&f.args.quarantine, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn keeper_candidate_aliases_and_duplicate_actions_are_ambiguous() {
    let mut f = Fixture::new();
    fs::remove_file(&f.args.candidate[0]).unwrap();
    fs::hard_link(&f.args.keep, &f.args.candidate[0]).unwrap();
    assert_eq!(
        execution::create_plan(&f.args, &f.state, &f.policy, &SignalState::default())
            .unwrap_err()
            .code,
        Code::ExecutionAmbiguousIdentity
    );
    fs::remove_file(&f.args.candidate[0]).unwrap();
    fs::write(&f.args.candidate[0], fs::read(&f.args.keep).unwrap()).unwrap();
    f.args.candidate.push(f.args.candidate[0].clone());
    assert_eq!(
        execution::create_plan(&f.args, &f.state, &f.policy, &SignalState::default())
            .unwrap_err()
            .code,
        Code::ExecutionAmbiguousIdentity
    );
}

#[test]
fn scope_action_and_byte_limits_are_bound_and_enforced() {
    let f = Fixture::new();
    let original = f.plan();
    for case in 0..6 {
        let mut p = original.clone();
        match case {
            0 => p.body.bounds.max_in_flight_bytes = 1,
            1 => p.body.actions.push(p.body.actions[0].clone()),
            2 => p.body.state = p.body.roots[0].clone(),
            3 => p.body.quarantine = p.body.roots[0].clone(),
            4 => p.body.subtrees = vec![p.body.state.clone()],
            _ => p.body.roots.push(p.body.roots[0].clone()),
        }
        seal(&mut p);
        assert!(execution::approve(&p, &p.fingerprint, "operator").is_err());
    }
    let mut p = original.clone();
    p.body.bounds.max_actions = 1;
    let mut second = p.body.actions[0].clone();
    second.action_id = "action-000002".to_owned();
    p.body.actions.push(second);
    seal(&mut p);
    assert_eq!(
        execution::approve(&p, &p.fingerprint, "operator")
            .unwrap_err()
            .code,
        Code::ExecutionBoundsExceeded
    );
}

#[test]
fn plan_changes_invalidate_existing_approval_and_policy_changes_block() {
    let f = Fixture::new();
    let mut p = f.plan();
    let a = f.approval(&p);
    p.body.bounds.free_space_reserve_bytes += 1;
    assert_eq!(
        execution::dry_run(&p, &a, &f.state, &f.policy, &SignalState::default())
            .unwrap_err()
            .code,
        Code::ExecutionPlanInvalid
    );
    seal(&mut p);
    assert_eq!(
        execution::dry_run(&p, &a, &f.state, &f.policy, &SignalState::default())
            .unwrap_err()
            .code,
        Code::ExecutionApprovalMismatch
    );
    let config = f.base.join("policy.toml");
    // Resolve a real alternate policy; forged fingerprints are not sufficient.
    fs::write(
        &config,
        "schema = \"optiflow.config.v1\"\n[scan]\ninclude_hidden = true\n",
    )
    .unwrap();
    let cli = Cli::parse_from([
        "optiflow",
        "--config",
        config.to_str().unwrap(),
        "--state-directory",
        f.state.to_str().unwrap(),
        "doctor",
    ]);
    let policy = configuration::resolve(&cli).unwrap().policy;
    assert_eq!(
        execution::dry_run(
            &p,
            &f.approval(&p),
            &f.state,
            &policy,
            &SignalState::default()
        )
        .unwrap_err()
        .code,
        Code::ExecutionPolicyMismatch
    );
    assert!(!f.state.join("state.sqlite3").exists());
}

#[test]
fn immutable_outputs_cannot_overwrite_or_write_into_protected_trees() {
    let f = Fixture::new();
    let p = f.documents();
    assert_eq!(
        execution::write_document(&f.args.output, &p, &p)
            .unwrap_err()
            .code,
        Code::ExecutionDestinationOccupied
    );
    for dir in [&f.args.root[0], &f.args.quarantine] {
        assert_eq!(
            execution::write_document(&dir.join("evidence.json"), &p, &p)
                .unwrap_err()
                .code,
            Code::ExecutionScopeInvalid
        );
        assert!(!dir.join("evidence.json").exists());
    }
    symlink(&f.args.root[0], f.base.join("redirect")).unwrap();
    assert_eq!(
        execution::write_document(&f.base.join("redirect/evidence.json"), &p, &p)
            .unwrap_err()
            .code,
        Code::ExecutionScopeInvalid
    );
}

#[test]
fn malformed_legacy_future_and_duplicate_key_documents_never_gain_authority() {
    let f = Fixture::new();
    let p = f.plan();
    let path = f.base.join("bad.json");
    for value in [
        json!({"schema_version":"optiflow.plan.v5","mode":"exact_duplicates_review"}),
        {
            let mut v = serde_json::to_value(&p).unwrap();
            v["schema"] = json!("optiflow.execution-plan.v2");
            v
        },
        {
            let mut v = serde_json::to_value(&p).unwrap();
            v["inferred_approval"] = json!(true);
            v
        },
    ] {
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(execution::load_plan(&path).is_err());
    }
    let text = serde_json::to_string(&p).unwrap().replacen(
        "{",
        "{\"schema\":\"optiflow.execution-plan.v1\",",
        1,
    );
    fs::write(&path, text).unwrap();
    assert!(execution::load_plan(&path).is_err());
    fs::write(&path, vec![b' '; 4 * 1024 * 1024 + 1]).unwrap();
    assert!(execution::load_plan(&path).is_err());
}

#[test]
fn state_database_alias_and_destination_collision_block_without_source_writes() {
    for hard_link in [false, true] {
        let f = Fixture::new();
        let p = f.plan();
        if hard_link {
            fs::hard_link(&f.args.keep, f.state.join("state.sqlite3")).unwrap();
        } else {
            symlink(&f.args.keep, f.state.join("state.sqlite3")).unwrap();
        }
        let before = fs::read(&f.args.keep).unwrap();
        assert_eq!(f.run(&p).unwrap_err().code, Code::ExecutionScopeInvalid);
        assert_eq!(before, fs::read(&f.args.keep).unwrap());
    }
    let f = Fixture::new();
    let p = f.plan();
    fs::create_dir(f.args.quarantine.join(&p.fingerprint)).unwrap();
    assert_eq!(
        f.run(&p).unwrap_err().code,
        Code::ExecutionDestinationOccupied
    );
}

fn cli(f: &Fixture, tail: &[&str]) -> std::process::Output {
    let mut c = assert_cmd::Command::cargo_bin("optiflow").unwrap();
    c.args([
        "--no-config",
        "--state-directory",
        f.state.to_str().unwrap(),
    ])
    .args(tail)
    .output()
    .unwrap()
}
#[test]
fn cli_requires_approval_and_dry_run_and_matches_human_and_json_evidence() {
    let f = Fixture::new();
    let p = f.documents();
    let plan = f.args.output.to_str().unwrap();
    let approval = f.base.join("approval.json");
    let approval = approval.to_str().unwrap();
    for (tail, code) in [
        (
            vec!["--json", "apply", "--plan", plan, "--dry-run"],
            "execution_approval_required",
        ),
        (
            vec!["--json", "apply", "--plan", plan, "--approval", approval],
            "execution_unsupported",
        ),
    ] {
        let o = cli(&f, &tail);
        assert!(!o.status.success());
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(v["diagnostics"][0]["code"], code);
    }
    let o = cli(
        &f,
        &[
            "--json",
            "apply",
            "--plan",
            plan,
            "--approval",
            approval,
            "--dry-run",
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    contracts::validate(Contract::CommandResult, &v).unwrap();
    let o = cli(
        &f,
        &["apply", "--plan", plan, "--approval", approval, "--dry-run"],
    );
    assert!(o.status.success());
    let human = String::from_utf8(o.stdout).unwrap();
    let start = human.find('{').unwrap();
    let h: Value = serde_json::from_str(&human[start..]).unwrap();
    for key in [
        "status",
        "dry_run",
        "plan_fingerprint",
        "authorization_id",
        "savings",
        "commit",
        "recovery",
        "limitations",
        "attempts",
    ] {
        assert_eq!(v["result"][key], h[key], "{key}");
    }
    assert_eq!(h["plan_fingerprint"], p.fingerprint);
}

#[test]
fn cli_creates_and_approves_explicit_paths_without_using_review_defaults() {
    let f = Fixture::new();
    let o = cli(
        &f,
        &[
            "--json",
            "plan",
            "execution",
            "--keep",
            f.args.keep.to_str().unwrap(),
            "--candidate",
            f.args.candidate[0].to_str().unwrap(),
            "--root",
            f.args.root[0].to_str().unwrap(),
            "--quarantine",
            f.args.quarantine.to_str().unwrap(),
            "--reserve-bytes",
            "1",
            "--output",
            f.args.output.to_str().unwrap(),
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let p = execution::load_plan(&f.args.output).unwrap();
    let approval = f.base.join("approval.json");
    let o = cli(
        &f,
        &[
            "--json",
            "plan",
            "approve",
            "--plan",
            f.args.output.to_str().unwrap(),
            "--fingerprint",
            &p.fingerprint,
            "--approved-by",
            "fixture",
            "--output",
            approval.to_str().unwrap(),
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    contracts::validate(Contract::CommandResult, &v).unwrap();
    assert_eq!(
        execution::load_approval(&approval)
            .unwrap()
            .body
            .plan_fingerprint,
        p.fingerprint
    );
}

#[test]
fn native_non_utf8_paths_round_trip_through_approval_and_validation() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let mut f = Fixture::new();
    let path = f.args.root[0].join(OsString::from_vec(b"duplicate-\xff.bin".to_vec()));
    fs::rename(&f.args.candidate[0], &path).unwrap();
    f.args.candidate = vec![path];
    let p = f.documents();
    let loaded = execution::load_plan(&f.args.output).unwrap();
    assert_eq!(p.fingerprint, loaded.fingerprint);
    assert_eq!(f.run(&loaded).unwrap().status, Status::Validated);
}

#[test]
fn frozen_v1_contracts_round_trip_and_cannot_claim_a_source_commit() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/execution-v1");
    for name in [
        "plan",
        "approval",
        "run",
        "attempt",
        "validation",
        "commit",
        "recovery",
        "interrupted",
    ] {
        let value: Value =
            serde_json::from_slice(&fs::read(base.join(format!("{name}.json"))).unwrap()).unwrap();
        contracts::validate(Contract::Execution, &value).unwrap();
        let mut future = value.clone();
        future["schema"] = json!(value["schema"].as_str().unwrap().replace(".v1", ".v2"));
        assert!(contracts::validate(Contract::Execution, &future).is_err());
        let mut unknown = value.clone();
        unknown["implicit_authority"] = json!(true);
        assert!(contracts::validate(Contract::Execution, &unknown).is_err());
    }
    let p = execution::load_plan(&base.join("plan.json")).unwrap();
    let a = execution::load_approval(&base.join("approval.json")).unwrap();
    assert_eq!(p.fingerprint, a.body.plan_fingerprint);
    let mut run: Value = serde_json::from_slice(&fs::read(base.join("run.json")).unwrap()).unwrap();
    run["commit"]["source_mutated"] = json!(true);
    assert!(contracts::validate(Contract::Execution, &run).is_err());
    run["commit"]["source_mutated"] = json!(false);
    run["commit"]["status"] = json!("committed");
    assert!(contracts::validate(Contract::Execution, &run).is_err());
}

#[test]
fn newer_state_schema_is_not_modified_or_opened_for_execution() {
    let f = Fixture::new();
    let p = f.plan();
    let _store = optiflow::state::StateStore::open(&f.state).unwrap();
    let connection = rusqlite::Connection::open(f.state.join("state.sqlite3")).unwrap();
    connection
        .execute("INSERT INTO schema_migrations VALUES (7,'future')", [])
        .unwrap();
    drop(connection);
    let before = fs::read(f.state.join("state.sqlite3")).unwrap();
    assert_eq!(f.run(&p).unwrap_err().code, Code::StoredStateIncompatible);
    assert_eq!(before, fs::read(f.state.join("state.sqlite3")).unwrap());
    assert!(!f.state.join("execution.lock").exists());
}

#[test]
fn journal_column_document_disagreement_is_rejected_without_recovery_rewrite() {
    let f = Fixture::new();
    let p = f.plan();
    let run = f.run(&p).unwrap();
    let c = rusqlite::Connection::open(f.state.join("state.sqlite3")).unwrap();
    c.execute(
        "UPDATE execution_runs SET status = 'validating' WHERE run_id = ?1",
        [&run.run_id],
    )
    .unwrap();
    let before: String = c
        .query_row(
            "SELECT document_json FROM execution_runs WHERE run_id = ?1",
            [&run.run_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        execution::load_execution(&f.state, &run.run_id)
            .unwrap_err()
            .code,
        Code::StoredStateIncompatible
    );
    assert_eq!(f.run(&p).unwrap_err().code, Code::StoredStateIncompatible);
    let after: String = c
        .query_row(
            "SELECT document_json FROM execution_runs WHERE run_id = ?1",
            [&run.run_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn nested_unknown_keys_fail_before_typed_decoding_can_discard_them() {
    let f = Fixture::new();
    let p = f.plan();
    for member in ["identity", "path"] {
        let mut value = serde_json::to_value(&p).unwrap();
        value["body"]["actions"][0]["candidate"][member]["extra_authority"] = json!(true);
        fs::write(&f.args.output, serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(
            execution::load_plan(&f.args.output).unwrap_err().code,
            Code::ExecutionPlanInvalid
        );
    }
}

#[test]
fn empty_duplicates_are_valid_without_inventing_savings() {
    let mut f = Fixture::new();
    fs::write(&f.args.keep, []).unwrap();
    fs::write(&f.args.candidate[0], []).unwrap();
    f.args.max_in_flight_bytes = 0;
    let p = f.plan();
    let run = f.run(&p).unwrap();
    assert_eq!(run.status, Status::Validated);
    assert_eq!(run.savings.selected_logical_bytes, 0);
    assert_eq!(run.savings.physical_reclaimed_bytes, None);
}
