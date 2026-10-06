//! Synthetic execution authorization and dry-run safety boundaries.
#![cfg(unix)]
use clap::Parser;
use optiflow::cli::{Cli, ExecutionPlanArgs};
use optiflow::configuration::{self, EffectivePolicyV1};
use optiflow::contracts::{self, Contract};
use optiflow::domain::NativePath;
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn assert_rename_properties(before: &fs::Metadata, path: &Path) {
    let after = fs::metadata(path).unwrap();
    assert_eq!(before.dev(), after.dev());
    assert_eq!(before.ino(), after.ino());
    assert_eq!(before.nlink(), after.nlink());
    assert_eq!(before.uid(), after.uid());
    assert_eq!(before.gid(), after.gid());
    assert_eq!(before.mode(), after.mode());
    assert_eq!(before.len(), after.len());
    assert_eq!(before.mtime(), after.mtime());
    assert_eq!(before.mtime_nsec(), after.mtime_nsec());
    #[cfg(target_os = "macos")]
    assert_eq!(before.created().unwrap(), after.created().unwrap());
    // Reading content may update atime; a rename may update ctime. Neither
    // belongs in the identity-preserving rename property assertion.
    let mut attribute = [0u8; 64];
    let length = rustix::fs::getxattr(path, "user.optiflow_recovery", &mut attribute).unwrap();
    assert_eq!(&attribute[..length], b"preserved synthetic metadata");
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
fn cli_requires_approval_and_matches_human_and_json_dry_run_evidence() {
    let f = Fixture::new();
    let p = f.documents();
    let plan = f.args.output.to_str().unwrap();
    let approval = f.base.join("approval.json");
    let approval = approval.to_str().unwrap();
    let o = cli(&f, &["--json", "apply", "--plan", plan, "--dry-run"]);
    assert!(!o.status.success());
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["diagnostics"][0]["code"], "execution_approval_required");
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
fn native_paths_round_trip_through_approval_and_validation() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let mut f = Fixture::new();
    let raw_path = f.args.root[0].join(OsString::from_vec(b"duplicate-\xff.bin".to_vec()));
    let raw = NativePath::from_path(&raw_path);
    assert!(matches!(raw, NativePath::UnixBytes { .. }));
    let encoded = serde_json::to_vec(&raw).unwrap();
    let decoded: NativePath = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.to_path_buf(), raw_path);
    // APFS refuses invalid UTF-8 filenames; keep the byte encoding proof
    // above and use an actual Unicode path for the macOS filesystem proof.
    #[cfg(target_os = "macos")]
    let path = f.args.root[0].join("duplicate-🌌\n.bin");
    #[cfg(not(target_os = "macos"))]
    let path = raw_path;
    fs::rename(&f.args.candidate[0], &path).unwrap();
    f.args.candidate = vec![path.clone()];
    let p = f.documents();
    let loaded = execution::load_plan(&f.args.output).unwrap();
    assert_eq!(p.fingerprint, loaded.fingerprint);
    assert_eq!(loaded.body.actions[0].candidate.path.to_path_buf(), path);
    let approval = execution::load_approval(&f.base.join("approval.json")).unwrap();
    assert_eq!(approval.body.plan_fingerprint, p.fingerprint);
    assert_eq!(
        execution::dry_run(
            &loaded,
            &approval,
            &f.state,
            &f.policy,
            &SignalState::default()
        )
        .unwrap()
        .status,
        Status::Validated
    );
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
        .execute("INSERT INTO schema_migrations VALUES (10,'future')", [])
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn approved_quarantine_moves_synthetic_duplicates_one_at_a_time_with_durable_v2_evidence() {
    let mut f = Fixture::new();
    let second = f.args.root[0].join("second.bin");
    fs::write(&second, fs::read(&f.args.keep).unwrap()).unwrap();
    f.args.candidate.push(second);
    let p = f.plan();
    let source_states: Vec<_> = p
        .body
        .actions
        .iter()
        .map(|a| {
            let path = a.candidate.path.to_path_buf();
            (path.clone(), source(&path))
        })
        .collect();
    let run = execution::apply_quarantine(
        &p,
        &f.approval(&p),
        &f.state,
        &f.policy,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(run.status, execution::MutationStatus::Completed);
    assert_eq!(run.committed_actions, 2);
    assert!(run.attempts.iter().all(|a| a.committed));
    for ((original, before), attempt) in source_states.iter().zip(&run.attempts) {
        assert!(!original.exists());
        let moved = attempt.destination.to_path_buf();
        let after = source(&moved);
        assert_eq!(before.0, after.0);
        assert_eq!((before.1, before.2, before.5), (after.1, after.2, after.5));
    }
    assert!(f.args.keep.exists());
    assert_eq!(run.savings.immediate_logical_reclaimed_bytes, 0);
    assert_eq!(run.savings.physical_reclaimed_bytes, None);
    contracts::validate(Contract::ExecutionMutation, &run).unwrap();
    let mut future = serde_json::to_value(&run).unwrap();
    future["schema"] = json!("optiflow.execution-mutation.v3");
    assert!(contracts::validate(Contract::ExecutionMutation, &future).is_err());
    let mut invented = serde_json::to_value(&run).unwrap();
    invented["attempts"][0]["implicit_cleanup"] = json!(true);
    assert!(contracts::validate(Contract::ExecutionMutation, &invented).is_err());
    let mut false_commit = serde_json::to_value(&run).unwrap();
    false_commit["attempts"][0]["phase"] = json!("rename_pending");
    assert!(contracts::validate(Contract::ExecutionMutation, &false_commit).is_err());
    assert_eq!(
        execution::load_mutation(&f.state, &run.run_id)
            .unwrap()
            .unwrap()
            .committed_actions,
        2
    );
    // The same approval cannot silently reuse an occupied quarantine namespace.
    assert_eq!(
        execution::apply_quarantine(
            &p,
            &f.approval(&p),
            &f.state,
            &f.policy,
            &SignalState::default()
        )
        .unwrap_err()
        .code,
        Code::ExecutionDestinationOccupied
    );
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn live_apply_refuses_stale_missing_links_bounds_and_wrong_authority() {
    for case in 0..5 {
        let f = Fixture::new();
        let mut p = f.plan();
        let mut approval = f.approval(&p);
        let candidate = &f.args.candidate[0];
        match case {
            0 => fs::write(candidate, b"same-length-but-different-content").unwrap(),
            1 => {
                fs::remove_file(candidate).unwrap();
                symlink(&f.args.keep, candidate).unwrap();
            }
            2 => {
                fs::hard_link(candidate, f.base.join("alias")).unwrap();
            }
            3 => {
                p.body.bounds.max_actions = 0;
                seal(&mut p);
            }
            _ => {
                approval.body.plan_fingerprint = "0".repeat(64);
            }
        }
        let before = fs::symlink_metadata(candidate).unwrap();
        let result = execution::apply_quarantine(
            &p,
            &approval,
            &f.state,
            &f.policy,
            &SignalState::default(),
        );
        if let Ok(run) = result {
            assert_ne!(run.status, execution::MutationStatus::Completed);
        }
        assert_eq!(fs::symlink_metadata(candidate).unwrap().ino(), before.ino());
        assert!(
            !f.args
                .quarantine
                .join(&p.fingerprint)
                .join("action-000001")
                .exists()
        );
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn live_cli_requires_approval_and_reports_same_versioned_result() {
    let f = Fixture::new();
    let p = f.documents();
    let no_approval = cli(
        &f,
        &["--json", "apply", "--plan", f.args.output.to_str().unwrap()],
    );
    assert!(!no_approval.status.success());
    let v: Value = serde_json::from_slice(&no_approval.stdout).unwrap();
    assert_eq!(v["diagnostics"][0]["code"], "execution_approval_required");
    let output = cli(
        &f,
        &[
            "--json",
            "apply",
            "--plan",
            f.args.output.to_str().unwrap(),
            "--approval",
            f.base.join("approval.json").to_str().unwrap(),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["result"]["plan_fingerprint"], p.fingerprint);
    assert_eq!(v["result"]["status"], "completed");
    contracts::validate(Contract::ExecutionMutation, &v["result"]).unwrap();
}

#[cfg(target_os = "macos")]
fn assert_macos_cross_filesystem_mutation_refused(
    f: &Fixture,
    p: &ExecutionPlan,
    run_id: &str,
) {
    let approval = f.approval(p);
    assert_eq!(
        execution::apply_quarantine(
            p,
            &approval,
            &f.state,
            &f.policy,
            &SignalState::default()
        )
        .unwrap_err()
        .code,
        Code::ExecutionUnsupported
    );
    for result in [
        execution::resume(
            p,
            &approval,
            &f.state,
            &f.policy,
            run_id,
            &SignalState::default(),
        ),
        execution::restore(
            p,
            &approval,
            &f.state,
            &f.policy,
            run_id,
            &p.body.actions[0].action_id,
            &SignalState::default(),
        ),
        execution::cleanup(
            p,
            &approval,
            &f.state,
            &f.policy,
            run_id,
            &SignalState::default(),
        ),
    ] {
        assert_eq!(result.unwrap_err().code, Code::ExecutionUnsupported);
    }
}

#[cfg(target_os = "macos")]
#[test]
fn macos_cross_filesystem_intent_is_refused_before_journal_or_source_mutation() {
    let f = Fixture::new();
    let mut p = f.plan();
    // Model an explicitly cross-volume plan without mounting a volume or
    // depending on a second disk. This is a declaration-boundary proof, not
    // evidence of physical external-volume behavior or native qualification.
    let source_device = p.body.actions[0]
        .candidate
        .identity
        .filesystem_id
        .parse::<u64>()
        .unwrap();
    p.body.quarantine.identity.filesystem_id = source_device.wrapping_add(1).to_string();
    p.body.actions[0].topology = Topology::CrossFilesystem;
    seal(&mut p);
    // The complete versioned plan is valid; unsupported topology must win
    // before fresh directory comparison or opening the writable journal.
    execution::write_document(&f.args.output, &p, &p).unwrap();
    assert_eq!(
        execution::load_plan(&f.args.output).unwrap().fingerprint,
        p.fingerprint
    );
    let keeper = source(&f.args.keep);
    let candidate = source(&f.args.candidate[0]);
    assert_macos_cross_filesystem_mutation_refused(&f, &p, &uuid::Uuid::now_v7().to_string());
    assert_eq!(source(&f.args.keep), keeper);
    assert_eq!(source(&f.args.candidate[0]), candidate);
    assert_eq!(fs::read_dir(&f.args.quarantine).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&f.state).unwrap().count(), 0);
}

#[cfg(target_os = "macos")]
#[test]
fn macos_mixed_topology_refusal_preserves_existing_journal_and_retained_bytes() {
    let mut f = Fixture::new();
    let second_root = f.base.join("z-second-source");
    fs::create_dir(&second_root).unwrap();
    let second_candidate = second_root.join("candidate.bin");
    fs::write(&second_candidate, fs::read(&f.args.keep).unwrap()).unwrap();
    f.args.root.push(second_root.clone());
    f.args.candidate.push(second_candidate.clone());
    let mut p = f.plan();

    // Keep valid preexisting evidence that a writable journal opener would
    // recover. Refusal must neither rewrite it nor recreate its released lock.
    let mut unfinished = f.run(&p).unwrap();
    assert_eq!(unfinished.status, Status::Validated);
    unfinished.status = Status::Validating;
    unfinished.completed_at = None;
    unfinished.attempts.clear();
    unfinished.validation.status = Status::Validating;
    unfinished.commit.status = "not_started".to_owned();
    contracts::validate(Contract::Execution, &unfinished).unwrap();
    let connection = rusqlite::Connection::open(f.state.join("state.sqlite3")).unwrap();
    connection
        .execute(
            "UPDATE execution_runs SET status = 'validating', document_json = ?2 WHERE run_id = ?1",
            rusqlite::params![unfinished.run_id, serde_json::to_string(&unfinished).unwrap()],
        )
        .unwrap();
    drop(connection);
    fs::remove_file(f.state.join("execution.lock")).unwrap();

    // Keep the first action executable by declaration. Only the later action
    // crosses the declared volume boundary, so a per-action gate is too late.
    let other_device = p
        .body
        .quarantine
        .identity
        .filesystem_id
        .parse::<u64>()
        .unwrap()
        .wrapping_add(1)
        .to_string();
    for directory in p.body.roots.iter_mut().chain(&mut p.body.subtrees) {
        if directory.path.to_path_buf() == second_root {
            directory.identity.filesystem_id = other_device.clone();
        }
    }
    assert_eq!(p.body.actions[0].topology, Topology::SameFilesystem);
    assert_eq!(
        p.body.actions[1].candidate.path.to_path_buf(),
        second_candidate
    );
    p.body.actions[1].candidate.identity.filesystem_id = other_device.clone();
    p.body.actions[1].candidate.directory.identity.filesystem_id = other_device;
    p.body.actions[1].topology = Topology::CrossFilesystem;
    seal(&mut p);
    execution::write_document(&f.args.output, &p, &p).unwrap();
    assert_eq!(
        execution::load_plan(&f.args.output).unwrap().fingerprint,
        p.fingerprint
    );

    let retained_namespace = f.args.quarantine.join("retained-evidence");
    fs::create_dir(&retained_namespace).unwrap();
    let retained = retained_namespace.join("retained.bin");
    fs::write(&retained, b"previously retained synthetic bytes").unwrap();
    let retained_before = source(&retained);
    let keeper_before = source(&f.args.keep);
    let candidates_before: Vec<_> = f.args.candidate.iter().map(|p| source(p)).collect();
    let database_before = source(&f.state.join("state.sqlite3"));
    let entries = |directory: &Path| {
        let mut names: Vec<_> = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        names
    };
    let state_entries = entries(&f.state);
    let quarantine_entries = entries(&f.args.quarantine);

    assert_macos_cross_filesystem_mutation_refused(&f, &p, &unfinished.run_id);

    assert_eq!(source(&f.args.keep), keeper_before);
    for (path, before) in f.args.candidate.iter().zip(candidates_before) {
        assert_eq!(source(path), before);
    }
    assert_eq!(source(&retained), retained_before);
    assert_eq!(source(&f.state.join("state.sqlite3")), database_before);
    assert_eq!(entries(&f.state), state_entries);
    assert_eq!(entries(&f.args.quarantine), quarantine_entries);
    assert_eq!(
        entries(&retained_namespace),
        vec![std::ffi::OsString::from("retained.bin")]
    );
    assert!(!f.state.join("execution.lock").exists());
    assert!(!f.args.quarantine.join(&p.fingerprint).exists());
    assert_eq!(
        serde_json::to_value(
            execution::load_execution(&f.state, &unfinished.run_id)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(&unfinished).unwrap()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn cross_filesystem_copy_commits_destination_before_source_removal_when_available() {
    let mut f = Fixture::new();
    let alternate = Path::new("/dev/shm");
    if !alternate.is_dir()
        || fs::metadata(alternate).unwrap().dev() == fs::metadata(&f.base).unwrap().dev()
    {
        return;
    }
    let quarantine = tempfile::tempdir_in(alternate).unwrap();
    f.args.quarantine = fs::canonicalize(quarantine.path()).unwrap();
    fs::set_permissions(&f.args.candidate[0], fs::Permissions::from_mode(0o640)).unwrap();
    rustix::fs::setxattr(
        &f.args.candidate[0],
        "user.optiflow_test",
        b"preserved",
        rustix::fs::XattrFlags::CREATE,
    )
    .unwrap();
    let p = f.plan();
    assert_eq!(p.body.actions[0].topology, Topology::CrossFilesystem);
    let original = source(&f.args.candidate[0]);
    let run = execution::apply_quarantine(
        &p,
        &f.approval(&p),
        &f.state,
        &f.policy,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(
        run.status,
        execution::MutationStatus::Completed,
        "{:?}",
        run.diagnostics
    );
    assert!(!f.args.candidate[0].exists());
    let copied = source(&run.attempts[0].destination.to_path_buf());
    assert_eq!(copied.0, original.0);
    assert_eq!(copied.5, original.5);
    assert_eq!(
        fs::metadata(run.attempts[0].destination.to_path_buf())
            .unwrap()
            .mtime(),
        fs::metadata(&f.args.keep).unwrap().mtime()
    );
    let mut attribute = vec![0u8; 64];
    let length = rustix::fs::getxattr(
        run.attempts[0].destination.to_path_buf(),
        "user.optiflow_test",
        &mut attribute,
    )
    .unwrap();
    assert_eq!(&attribute[..length], b"preserved");
    assert_eq!(run.savings.physical_reclaimed_bytes, None);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn recovery_status_restore_and_empty_cleanup_are_idempotent() {
    let f = Fixture::new();
    fs::set_permissions(&f.args.candidate[0], fs::Permissions::from_mode(0o640)).unwrap();
    rustix::fs::setxattr(
        &f.args.candidate[0],
        "user.optiflow_recovery",
        b"preserved synthetic metadata",
        rustix::fs::XattrFlags::CREATE,
    )
    .unwrap();
    let p = f.documents();
    let a = f.approval(&p);
    let original = source(&f.args.candidate[0]);
    let properties = fs::metadata(&f.args.candidate[0]).unwrap();
    let run =
        execution::apply_quarantine(&p, &a, &f.state, &f.policy, &SignalState::default()).unwrap();
    assert_eq!(run.status, execution::MutationStatus::Completed);
    let destination = run.attempts[0].destination.to_path_buf();
    assert_rename_properties(&properties, &destination);
    let status = execution::execution_status(&f.state, &run.run_id).unwrap();
    assert_eq!(status.status, "quarantined");
    assert_eq!(status.recovery_authority, "bound_v3_context");
    contracts::validate(Contract::ExecutionRecoveryReport, &status).unwrap();
    for event in &status.events {
        contracts::validate(Contract::ExecutionRecoveryEvent, event).unwrap();
    }
    // Resuming a fully committed run must revalidate it without another move
    // or a second commit event. Interrupted remaining-action coverage lives
    // with the private recovery fixture, without timing-dependent signals.
    let resumed = execution::resume(
        &p,
        &a,
        &f.state,
        &f.policy,
        &run.run_id,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(resumed.status, "quarantined");
    assert_eq!(
        serde_json::to_value(&resumed.events).unwrap(),
        serde_json::to_value(&status.events).unwrap()
    );
    assert_rename_properties(&properties, &destination);
    assert!(!f.args.candidate[0].exists());
    let status_cli = cli(&f, &["--json", "execution", "status", "--run", &run.run_id]);
    assert!(status_cli.status.success());
    let value: Value = serde_json::from_slice(&status_cli.stdout).unwrap();
    assert_eq!(value["result"]["status"], "quarantined");
    contracts::validate(Contract::ExecutionRecoveryReport, &value["result"]).unwrap();
    let db = rusqlite::Connection::open(f.state.join("state.sqlite3")).unwrap();
    assert!(
        db.execute(
            "UPDATE execution_recovery_events SET phase = 'cleaned' WHERE run_id = ?1",
            [&run.run_id]
        )
        .is_err()
    );
    assert!(
        db.execute(
            "DELETE FROM execution_recovery_events WHERE run_id = ?1",
            [&run.run_id]
        )
        .is_err()
    );
    let restored = execution::restore(
        &p,
        &a,
        &f.state,
        &f.policy,
        &run.run_id,
        &p.body.actions[0].action_id,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(restored.status, "restored");
    let returned = source(&f.args.candidate[0]);
    assert_eq!(returned.0, original.0);
    assert_eq!((returned.1, returned.2), (original.1, original.2));
    assert_rename_properties(&properties, &f.args.candidate[0]);
    assert!(!destination.exists());
    let again = execution::restore(
        &p,
        &a,
        &f.state,
        &f.policy,
        &run.run_id,
        &p.body.actions[0].action_id,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(restored.events.len(), again.events.len());
    let unowned = run.namespace.to_path_buf().join("unowned-synthetic-file");
    fs::write(&unowned, b"not an execution artifact").unwrap();
    assert!(
        execution::cleanup(
            &p,
            &a,
            &f.state,
            &f.policy,
            &run.run_id,
            &SignalState::default()
        )
        .is_err()
    );
    assert_eq!(fs::read(&unowned).unwrap(), b"not an execution artifact");
    assert_rename_properties(&properties, &f.args.candidate[0]);
    fs::remove_file(unowned).unwrap();
    let cleaned = execution::cleanup(
        &p,
        &a,
        &f.state,
        &f.policy,
        &run.run_id,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(cleaned.status, "cleaned");
    assert!(!run.namespace.to_path_buf().exists());
    let repeated = execution::cleanup(
        &p,
        &a,
        &f.state,
        &f.policy,
        &run.run_id,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(cleaned.events.len(), repeated.events.len());
    assert_eq!(
        execution::load_mutation(&f.state, &run.run_id)
            .unwrap()
            .unwrap()
            .status,
        execution::MutationStatus::Completed
    );
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn recovery_refuses_collision_and_changed_policy_without_moving_quarantine() {
    let f = Fixture::new();
    let p = f.plan();
    let a = f.approval(&p);
    let run =
        execution::apply_quarantine(&p, &a, &f.state, &f.policy, &SignalState::default()).unwrap();
    let destination = run.attempts[0].destination.to_path_buf();
    let saved = fs::read(&destination).unwrap();
    fs::write(&f.args.candidate[0], b"collision with independent data").unwrap();
    let collision = source(&f.args.candidate[0]);
    let quarantined = source(&destination);
    assert_eq!(
        execution::restore(
            &p,
            &a,
            &f.state,
            &f.policy,
            &run.run_id,
            &p.body.actions[0].action_id,
            &SignalState::default()
        )
        .unwrap_err()
        .code,
        Code::ExecutionDestinationOccupied
    );
    assert_eq!(fs::read(&destination).unwrap(), saved);
    assert_eq!(source(&f.args.candidate[0]), collision);
    assert_eq!(source(&destination), quarantined);
    fs::remove_file(&f.args.candidate[0]).unwrap();
    let config = f.base.join("alternate-policy.toml");
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
    let changed = configuration::resolve(&cli).unwrap().policy;
    assert!(
        execution::restore(
            &p,
            &a,
            &f.state,
            &changed,
            &run.run_id,
            &p.body.actions[0].action_id,
            &SignalState::default()
        )
        .is_err()
    );
    assert_eq!(fs::read(&destination).unwrap(), saved);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn recovery_refuses_changed_extended_attributes_after_commit() {
    let f = Fixture::new();
    rustix::fs::setxattr(
        &f.args.candidate[0],
        "user.optiflow_recovery",
        b"original",
        rustix::fs::XattrFlags::CREATE,
    )
    .unwrap();
    // Plan after metadata changes so its bound identity remains current.
    let p = f.plan();
    let a = f.approval(&p);
    let run =
        execution::apply_quarantine(&p, &a, &f.state, &f.policy, &SignalState::default()).unwrap();
    let destination = run.attempts[0].destination.to_path_buf();
    rustix::fs::setxattr(
        &destination,
        "user.optiflow_recovery",
        b"changed",
        rustix::fs::XattrFlags::empty(),
    )
    .unwrap();
    assert_eq!(
        execution::restore(
            &p,
            &a,
            &f.state,
            &f.policy,
            &run.run_id,
            &p.body.actions[0].action_id,
            &SignalState::default()
        )
        .unwrap_err()
        .code,
        Code::ExecutionSourceStale
    );
    assert!(!f.args.candidate[0].exists());
    assert!(destination.exists());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn recovery_refuses_contention_permission_change_and_disconnected_quarantine() {
    use rustix::fs::{FlockOperation, flock};
    let f = Fixture::new();
    let p = f.plan();
    let a = f.approval(&p);
    let run =
        execution::apply_quarantine(&p, &a, &f.state, &f.policy, &SignalState::default()).unwrap();
    let destination = run.attempts[0].destination.to_path_buf();
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(f.state.join("execution.lock"))
        .unwrap();
    flock(&lock, FlockOperation::NonBlockingLockExclusive).unwrap();
    assert!(
        execution::restore(
            &p,
            &a,
            &f.state,
            &f.policy,
            &run.run_id,
            &p.body.actions[0].action_id,
            &SignalState::default()
        )
        .is_err()
    );
    drop(lock);
    assert!(destination.exists());
    fs::set_permissions(
        f.args.candidate[0].parent().unwrap(),
        fs::Permissions::from_mode(0o555),
    )
    .unwrap();
    assert!(
        execution::restore(
            &p,
            &a,
            &f.state,
            &f.policy,
            &run.run_id,
            &p.body.actions[0].action_id,
            &SignalState::default()
        )
        .is_err()
    );
    fs::set_permissions(
        f.args.candidate[0].parent().unwrap(),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let disconnected = f.base.join("disconnected");
    fs::rename(&f.args.quarantine, &disconnected).unwrap();
    assert!(
        execution::restore(
            &p,
            &a,
            &f.state,
            &f.policy,
            &run.run_id,
            &p.body.actions[0].action_id,
            &SignalState::default()
        )
        .is_err()
    );
    fs::rename(&disconnected, &f.args.quarantine).unwrap();
    assert!(destination.exists());
    assert!(!f.args.candidate[0].exists());
    let restored = execution::restore(
        &p,
        &a,
        &f.state,
        &f.policy,
        &run.run_id,
        &p.body.actions[0].action_id,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(restored.status, "restored");
}

#[cfg(target_os = "linux")]
#[test]
fn pending_reverse_copy_and_unowned_temporary_remain_for_inspection() {
    let mut f = Fixture::new();
    let alternate = Path::new("/dev/shm");
    if !alternate.is_dir()
        || fs::metadata(alternate).unwrap().dev() == fs::metadata(&f.base).unwrap().dev()
    {
        return;
    }
    let quarantine = tempfile::tempdir_in(alternate).unwrap();
    f.args.quarantine = fs::canonicalize(quarantine.path()).unwrap();
    let p = f.plan();
    let a = f.approval(&p);
    let run =
        execution::apply_quarantine(&p, &a, &f.state, &f.policy, &SignalState::default()).unwrap();
    let mut pending = serde_json::to_value(
        &execution::execution_status(&f.state, &run.run_id)
            .unwrap()
            .events[0],
    )
    .unwrap();
    let temporary = f.args.candidate[0].parent().unwrap().join(format!(
        ".optiflow-{}-{}.restore.part",
        run.run_id, p.body.actions[0].action_id
    ));
    fs::write(&temporary, b"partial reverse copy").unwrap();
    pending["event_id"] = json!(uuid::Uuid::now_v7().to_string());
    pending["operation_id"] = json!(uuid::Uuid::now_v7().to_string());
    pending["operation"] = json!("restore");
    pending["phase"] = json!("restore_pending");
    pending["action_id"] = json!(p.body.actions[0].action_id);
    pending["source"] = serde_json::to_value(&p.body.actions[0].candidate.path).unwrap();
    pending["destination"] = serde_json::to_value(&run.attempts[0].destination).unwrap();
    pending["temporary"] =
        serde_json::to_value(optiflow::domain::NativePath::from_path(&temporary)).unwrap();
    let db = rusqlite::Connection::open(f.state.join("state.sqlite3")).unwrap();
    db.execute("INSERT INTO execution_recovery_events (event_id,run_id,operation_id,action_id,operation,phase,document_json) VALUES (?1,?2,?3,?4,?5,?6,?7)", rusqlite::params![
        pending["event_id"].as_str().unwrap(), run.run_id, pending["operation_id"].as_str().unwrap(),
        p.body.actions[0].action_id, "restore", "restore_pending", pending.to_string()]).unwrap();
    let status = execution::execution_status(&f.state, &run.run_id).unwrap();
    assert_eq!(status.status, "attention_required");
    assert_eq!(status.actions[0].state, "ambiguous");
    assert!(
        execution::restore(
            &p,
            &a,
            &f.state,
            &f.policy,
            &run.run_id,
            &p.body.actions[0].action_id,
            &SignalState::default()
        )
        .is_err()
    );
    assert!(
        execution::cleanup(
            &p,
            &a,
            &f.state,
            &f.policy,
            &run.run_id,
            &SignalState::default()
        )
        .is_err()
    );
    assert_eq!(fs::read(&temporary).unwrap(), b"partial reverse copy");
    assert!(run.attempts[0].destination.to_path_buf().exists());
    assert!(!f.args.candidate[0].exists());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn read_only_status_keeps_a_pending_transition_ambiguous() {
    let f = Fixture::new();
    let p = f.plan();
    let a = f.approval(&p);
    let run =
        execution::apply_quarantine(&p, &a, &f.state, &f.policy, &SignalState::default()).unwrap();
    let database = rusqlite::Connection::open(f.state.join("state.sqlite3")).unwrap();
    let mut interrupted = serde_json::to_value(&run).unwrap();
    interrupted["status"] = json!("interrupted");
    interrupted["attempts"][0]["committed"] = json!(false);
    interrupted["attempts"][0]["phase"] = json!("rename_pending");
    interrupted["committed_actions"] = json!(0);
    database.execute("UPDATE execution_mutation_runs SET status = 'interrupted', document_json = ?2 WHERE run_id = ?1",
        rusqlite::params![run.run_id, serde_json::to_string(&interrupted).unwrap()]).unwrap();
    let before = fs::read(run.attempts[0].destination.to_path_buf()).unwrap();
    let status = execution::execution_status(&f.state, &run.run_id).unwrap();
    assert_eq!(status.status, "attention_required");
    assert_eq!(status.actions[0].state, "ambiguous");
    assert!(
        execution::resume(
            &p,
            &a,
            &f.state,
            &f.policy,
            &run.run_id,
            &SignalState::default()
        )
        .is_err()
    );
    assert_eq!(
        fs::read(run.attempts[0].destination.to_path_buf()).unwrap(),
        before
    );
}

#[cfg(target_os = "linux")]
#[test]
fn cross_filesystem_restore_recreates_source_and_retains_verified_quarantine_copy() {
    let mut f = Fixture::new();
    let alternate = Path::new("/dev/shm");
    if !alternate.is_dir()
        || fs::metadata(alternate).unwrap().dev() == fs::metadata(&f.base).unwrap().dev()
    {
        return;
    }
    let quarantine = tempfile::tempdir_in(alternate).unwrap();
    f.args.quarantine = fs::canonicalize(quarantine.path()).unwrap();
    fs::set_permissions(&f.args.candidate[0], fs::Permissions::from_mode(0o640)).unwrap();
    rustix::fs::setxattr(
        &f.args.candidate[0],
        "user.optiflow_restore_test",
        b"retained",
        rustix::fs::XattrFlags::CREATE,
    )
    .unwrap();
    let p = f.plan();
    let a = f.approval(&p);
    let original = source(&f.args.candidate[0]);
    let run =
        execution::apply_quarantine(&p, &a, &f.state, &f.policy, &SignalState::default()).unwrap();
    let destination = run.attempts[0].destination.to_path_buf();
    let restored = execution::restore(
        &p,
        &a,
        &f.state,
        &f.policy,
        &run.run_id,
        &p.body.actions[0].action_id,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(restored.actions[0].state, "restored_retained");
    assert_eq!(source(&f.args.candidate[0]).0, original.0);
    assert_eq!(fs::read(&destination).unwrap(), original.0);
    assert_eq!(
        fs::metadata(&f.args.candidate[0]).unwrap().mode(),
        original.5
    );
    let mut xattr = [0u8; 64];
    let count = rustix::fs::getxattr(
        &f.args.candidate[0],
        "user.optiflow_restore_test",
        &mut xattr,
    )
    .unwrap();
    assert_eq!(&xattr[..count], b"retained");
    assert!(
        execution::cleanup(
            &p,
            &a,
            &f.state,
            &f.policy,
            &run.run_id,
            &SignalState::default()
        )
        .is_err()
    );
    let repeated = execution::restore(
        &p,
        &a,
        &f.state,
        &f.policy,
        &run.run_id,
        &p.body.actions[0].action_id,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(restored.events.len(), repeated.events.len());
}

#[cfg(target_os = "linux")]
fn retained_cross_fixture() -> Option<(
    Fixture,
    tempfile::TempDir,
    ExecutionPlan,
    Approval,
    String,
    PathBuf,
)> {
    retained_cross_fixture_with_json(false)
}

#[cfg(target_os = "linux")]
fn retained_cross_fixture_with_json(
    json: bool,
) -> Option<(
    Fixture,
    tempfile::TempDir,
    ExecutionPlan,
    Approval,
    String,
    PathBuf,
)> {
    let mut f = Fixture::new();
    if json {
        let cli = Cli::parse_from([
            "optiflow",
            "--no-config",
            "--state-directory",
            f.state.to_str()?,
            "--json",
            "doctor",
        ]);
        f.policy = configuration::resolve(&cli).ok()?.policy;
    }
    let alternate = Path::new("/dev/shm");
    if !alternate.is_dir()
        || fs::metadata(alternate).ok()?.dev() == fs::metadata(&f.base).ok()?.dev()
    {
        return None;
    }
    let quarantine = tempfile::tempdir_in(alternate).ok()?;
    f.args.quarantine = fs::canonicalize(quarantine.path()).ok()?;
    let plan = f.plan();
    let approval = f.approval(&plan);
    let run = execution::apply_quarantine(
        &plan,
        &approval,
        &f.state,
        &f.policy,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(run.status, execution::MutationStatus::Completed);
    let destination = run.attempts[0].destination.to_path_buf();
    let restored = execution::restore(
        &plan,
        &approval,
        &f.state,
        &f.policy,
        &run.run_id,
        &plan.body.actions[0].action_id,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(restored.actions[0].state, "restored_retained");
    Some((f, quarantine, plan, approval, run.run_id, destination))
}

#[cfg(target_os = "linux")]
#[test]
fn finalization_preview_is_read_only_and_commit_is_durable_and_idempotent() {
    use optiflow::execution::finalization;
    let Some((f, _quarantine, plan, approval, run, destination)) = retained_cross_fixture() else {
        return;
    };
    let selected = vec![plan.body.actions[0].action_id.clone()];
    let before_source = source(&f.args.candidate[0]);
    let before_destination = source(&destination);
    let state_meta = fs::metadata(f.state.join("state.sqlite3"))
        .unwrap()
        .modified()
        .unwrap();
    let preview = finalization::preview(
        &plan,
        &approval,
        &f.state,
        &f.policy,
        &run,
        &selected,
        &SignalState::default(),
    )
    .unwrap();
    contracts::validate(Contract::ExecutionFinalizationPreview, &preview).unwrap();
    assert_eq!(source(&f.args.candidate[0]), before_source);
    assert_eq!(source(&destination), before_destination);
    assert_eq!(
        fs::metadata(f.state.join("state.sqlite3"))
            .unwrap()
            .modified()
            .unwrap(),
        state_meta
    );
    assert!(finalization::authorize(&preview, "incorrect", "operator").is_err());
    let auth = finalization::authorize(&preview, &preview.fingerprint, "fixture operator").unwrap();
    contracts::validate(Contract::ExecutionFinalizationAuthorization, &auth).unwrap();
    let committed = finalization::commit(
        &plan,
        &approval,
        &f.state,
        &f.policy,
        &preview,
        &auth,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(committed.status, "finalized_irreversible");
    assert_eq!(committed.actions[0].state, "finalized_irreversible");
    assert_eq!(committed.events.len(), 2);
    assert_eq!(committed.events[0].phase, "removal_pending");
    assert_eq!(committed.events[1].phase, "removed");
    assert_eq!(
        committed.logical_bytes_removed,
        before_destination.0.len() as u64
    );
    assert_eq!(committed.physical_reclaimed_bytes, None);
    assert_eq!(committed.shared_extent_bytes, None);
    assert_eq!(
        committed.events[1].target_free_space_change_bytes,
        i64::try_from(
            i128::from(committed.events[1].target_available_after.unwrap())
                - i128::from(committed.events[1].target_available_before)
        )
        .ok()
    );
    assert!(!destination.exists());
    assert_eq!(source(&f.args.candidate[0]), before_source);
    assert_eq!(fs::read(&f.args.keep).unwrap(), before_source.0);
    let stored = finalization::status(&f.state, &run).unwrap();
    assert_eq!(stored.status, committed.status);
    assert_eq!(stored.events.len(), 2);
    let repeated = finalization::commit(
        &plan,
        &approval,
        &f.state,
        &f.policy,
        &preview,
        &auth,
        &SignalState::default(),
    )
    .unwrap();
    assert_eq!(repeated.events.len(), 2);
    assert!(
        execution::restore(
            &plan,
            &approval,
            &f.state,
            &f.policy,
            &run,
            &selected[0],
            &SignalState::default()
        )
        .is_err()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn finalization_refuses_missing_or_changed_survivor_and_tampered_manifest() {
    use optiflow::execution::finalization;
    for case in 0..3 {
        let Some((f, _quarantine, plan, approval, run, destination)) = retained_cross_fixture()
        else {
            return;
        };
        let selected = vec![plan.body.actions[0].action_id.clone()];
        let preview = finalization::preview(
            &plan,
            &approval,
            &f.state,
            &f.policy,
            &run,
            &selected,
            &SignalState::default(),
        )
        .unwrap();
        let auth =
            finalization::authorize(&preview, &preview.fingerprint, "fixture operator").unwrap();
        match case {
            0 => fs::remove_file(&f.args.candidate[0]).unwrap(),
            1 => fs::write(&f.args.candidate[0], b"different synthetic bytes size").unwrap(),
            _ => {
                let mut altered = preview.clone();
                altered.body.entries[0].logical_bytes += 1;
                assert!(
                    finalization::commit(
                        &plan,
                        &approval,
                        &f.state,
                        &f.policy,
                        &altered,
                        &auth,
                        &SignalState::default()
                    )
                    .is_err()
                );
            }
        }
        if case < 2 {
            assert!(
                finalization::commit(
                    &plan,
                    &approval,
                    &f.state,
                    &f.policy,
                    &preview,
                    &auth,
                    &SignalState::default()
                )
                .is_err()
            );
        }
        assert!(destination.exists());
        assert!(
            finalization::status(&f.state, &run)
                .unwrap()
                .events
                .is_empty()
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn pending_finalization_never_retries_or_claims_removal() {
    use optiflow::execution::finalization::{self, FinalizationEvent};
    let Some((f, _quarantine, plan, approval, run, destination)) = retained_cross_fixture() else {
        return;
    };
    let selected = vec![plan.body.actions[0].action_id.clone()];
    let preview = finalization::preview(
        &plan,
        &approval,
        &f.state,
        &f.policy,
        &run,
        &selected,
        &SignalState::default(),
    )
    .unwrap();
    let auth = finalization::authorize(&preview, &preview.fingerprint, "fixture operator").unwrap();
    let item = &preview.body.entries[0];
    let event = FinalizationEvent {
        schema: finalization::EVENT_SCHEMA.to_owned(),
        event_id: uuid::Uuid::now_v7().to_string(),
        run_id: run.clone(),
        action_id: selected[0].clone(),
        phase: "removal_pending".to_owned(),
        preview_fingerprint: preview.fingerprint.clone(),
        authorization_id: auth.authorization_id.clone(),
        manifest_digest: preview.body.quarantine_manifest_digest.clone(),
        recorded_at: chrono::Utc::now().to_rfc3339(),
        binary_version: env!("CARGO_PKG_VERSION").to_owned(),
        configuration_fingerprint: f.policy.fingerprints.effective_configuration.value.clone(),
        policy_fingerprint: f.policy.fingerprints.evidence_policy.value.clone(),
        quarantine_identity: item.quarantine_identity.clone(),
        survivor_identity: item.survivor_identity.clone(),
        logical_bytes: item.logical_bytes,
        observed_allocated_bytes_before: item.observed_allocated_bytes,
        target_available_before: 1,
        target_available_after: None,
        target_free_space_change_bytes: None,
        shared_extent_bytes: None,
        physical_reclaimed_bytes: None,
        recoverability: "irreversible_pending_inspection".to_owned(),
        reason: "synthetic interruption".to_owned(),
    };
    let connection = rusqlite::Connection::open(f.state.join("state.sqlite3")).unwrap();
    connection.execute("INSERT INTO execution_finalization_events (event_id, run_id, action_id, phase, preview_fingerprint, authorization_id, document_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![event.event_id, event.run_id, event.action_id, event.phase,
            event.preview_fingerprint, event.authorization_id, serde_json::to_string(&event).unwrap()]).unwrap();
    drop(connection);
    let status = finalization::status(&f.state, &run).unwrap();
    assert_eq!(status.status, "attention_required");
    assert_eq!(status.actions[0].state, "pending_irreversible_inspection");
    assert_eq!(status.logical_bytes_removed, 0);
    assert!(
        finalization::commit(
            &plan,
            &approval,
            &f.state,
            &f.policy,
            &preview,
            &auth,
            &SignalState::default()
        )
        .is_err()
    );
    assert!(destination.exists());
    assert!(
        execution::restore(
            &plan,
            &approval,
            &f.state,
            &f.policy,
            &run,
            &selected[0],
            &SignalState::default()
        )
        .is_err()
    );
    // Model a process exit after unlink but before the durable completion row.
    fs::remove_file(&destination).unwrap();
    let ambiguous = finalization::status(&f.state, &run).unwrap();
    assert_eq!(ambiguous.status, "attention_required");
    assert_eq!(ambiguous.logical_bytes_removed, 0);
    assert!(
        finalization::commit(
            &plan,
            &approval,
            &f.state,
            &f.policy,
            &preview,
            &auth,
            &SignalState::default()
        )
        .is_err()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn finalization_rejects_other_authority_aliases_and_stale_quarantine() {
    use optiflow::execution::finalization;
    let Some((f, _quarantine, plan, approval, run, destination)) = retained_cross_fixture() else {
        return;
    };
    let selected = vec![plan.body.actions[0].action_id.clone()];
    assert!(
        finalization::preview(
            &plan,
            &approval,
            &f.state,
            &f.policy,
            &run,
            &[selected[0].clone(), selected[0].clone()],
            &SignalState::default()
        )
        .is_err()
    );
    let preview = finalization::preview(
        &plan,
        &approval,
        &f.state,
        &f.policy,
        &run,
        &selected,
        &SignalState::default(),
    )
    .unwrap();
    let auth = finalization::authorize(&preview, &preview.fingerprint, "fixture operator").unwrap();
    let mut other = auth.clone();
    other.body.selected_actions.clear();
    assert!(
        finalization::commit(
            &plan,
            &approval,
            &f.state,
            &f.policy,
            &preview,
            &other,
            &SignalState::default()
        )
        .is_err()
    );
    fs::hard_link(
        &destination,
        destination.with_file_name("unexpected-hardlink"),
    )
    .unwrap();
    assert!(
        finalization::commit(
            &plan,
            &approval,
            &f.state,
            &f.policy,
            &preview,
            &auth,
            &SignalState::default()
        )
        .is_err()
    );
    assert!(destination.exists());
    assert!(
        finalization::status(&f.state, &run)
            .unwrap()
            .events
            .is_empty()
    );
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn finalization_refuses_same_filesystem_restore_without_retained_copy() {
    use optiflow::execution::finalization;
    let f = Fixture::new();
    let plan = f.plan();
    let approval = f.approval(&plan);
    let run = execution::apply_quarantine(
        &plan,
        &approval,
        &f.state,
        &f.policy,
        &SignalState::default(),
    )
    .unwrap();
    execution::restore(
        &plan,
        &approval,
        &f.state,
        &f.policy,
        &run.run_id,
        &plan.body.actions[0].action_id,
        &SignalState::default(),
    )
    .unwrap();
    let selected = vec![plan.body.actions[0].action_id.clone()];
    let before = source(&f.args.candidate[0]);
    let error = finalization::preview(
        &plan,
        &approval,
        &f.state,
        &f.policy,
        &run.run_id,
        &selected,
        &SignalState::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.code,
        if cfg!(target_os = "macos") {
            Code::ExecutionUnsupported
        } else {
            Code::ExecutionSourceStale
        }
    );
    assert_eq!(source(&f.args.candidate[0]), before);
}

#[cfg(target_os = "linux")]
#[test]
fn finalization_cli_requires_explicit_preview_and_separate_authorization() {
    use std::process::Command;
    let Some((f, _quarantine, plan, approval, run, destination)) =
        retained_cross_fixture_with_json(true)
    else {
        return;
    };
    let plan_path = f.base.join("plan.json");
    let approval_path = f.base.join("approval.json");
    let preview_path = f.base.join("finalization-preview.json");
    let authorization_path = f.base.join("finalization-authorization.json");
    execution::write_document(&plan_path, &plan, &plan).unwrap();
    execution::write_document(&approval_path, &approval, &plan).unwrap();
    let command = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_optiflow"));
        cmd.arg("--no-config")
            .arg("--state-directory")
            .arg(&f.state)
            .arg("--json");
        cmd
    };
    let preview = command()
        .arg("execution")
        .arg("finalize")
        .arg("--plan")
        .arg(&plan_path)
        .arg("--approval")
        .arg(&approval_path)
        .arg("--run")
        .arg(&run)
        .arg("--action")
        .arg("action-000001")
        .arg("--output")
        .arg(&preview_path)
        .output()
        .unwrap();
    assert!(
        preview.status.success(),
        "{} {}",
        String::from_utf8_lossy(&preview.stderr),
        String::from_utf8_lossy(&preview.stdout)
    );
    assert!(destination.exists());
    let report: Value = serde_json::from_slice(&preview.stdout).unwrap();
    let fingerprint = report["result"]["fingerprint"].as_str().unwrap();
    let unauthorized = command()
        .arg("execution")
        .arg("finalize")
        .arg("--commit")
        .arg("--plan")
        .arg(&plan_path)
        .arg("--approval")
        .arg(&approval_path)
        .arg("--preview")
        .arg(&preview_path)
        .output()
        .unwrap();
    assert!(!unauthorized.status.success());
    assert!(destination.exists());
    let recorded = command()
        .arg("execution")
        .arg("authorize-finalization")
        .arg("--plan")
        .arg(&plan_path)
        .arg("--preview")
        .arg(&preview_path)
        .arg("--fingerprint")
        .arg(fingerprint)
        .arg("--approved-by")
        .arg("fixture operator")
        .arg("--output")
        .arg(&authorization_path)
        .output()
        .unwrap();
    assert!(
        recorded.status.success(),
        "{}",
        String::from_utf8_lossy(&recorded.stderr)
    );
    let committed = command()
        .arg("execution")
        .arg("finalize")
        .arg("--commit")
        .arg("--plan")
        .arg(&plan_path)
        .arg("--approval")
        .arg(&approval_path)
        .arg("--preview")
        .arg(&preview_path)
        .arg("--authorization")
        .arg(&authorization_path)
        .output()
        .unwrap();
    assert!(
        committed.status.success(),
        "{}",
        String::from_utf8_lossy(&committed.stderr)
    );
    let result: Value = serde_json::from_slice(&committed.stdout).unwrap();
    assert_eq!(result["result"]["status"], "finalized_irreversible");
    assert!(!destination.exists());
}
