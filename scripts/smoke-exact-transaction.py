#!/usr/bin/env python3
"""Exercise the development mutation CLI on newly created synthetic files only.

This is a local smoke test, never a removable-volume release qualification.
The two supplied roots must be on different filesystems. On failure the
synthetic directories are retained for journal inspection.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


SCHEMA = "optiflow.synthetic-transaction-smoke.v1"
PAYLOAD = bytes(range(256)) * 256


def require(condition, message):
    if not condition:
        raise ValueError(message)


def available(path):
    stat = os.statvfs(path)
    return stat.f_bavail * stat.f_frsize


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command(binary, state, *args, success=True):
    completed = subprocess.run(
        [str(binary), "--no-config", "--state-directory", str(state), "--json", *map(str, args)],
        capture_output=True, text=True, timeout=120, check=False,
    )
    require(not completed.stderr, f"unexpected CLI stderr: {completed.stderr[:1000]}")
    document = json.loads(completed.stdout)
    require(document.get("schema") == "optiflow.command-result.v1", "unexpected CLI envelope")
    require(document["outcome"]["exit_code"] == completed.returncode, "CLI exit/envelope mismatch")
    if success:
        require(completed.returncode == 0, f"CLI refused {args}: {document.get('diagnostics')}")
    else:
        require(completed.returncode != 0, f"CLI unexpectedly accepted {args}")
    return document["result"], document


def pair(root):
    root.mkdir()
    keeper, candidate = root / "keeper.bin", root / "candidate.bin"
    keeper.write_bytes(PAYLOAD)
    candidate.write_bytes(PAYLOAD)
    return keeper, candidate


def plan_and_approve(binary, state, root, quarantine, evidence):
    plan_path, approval_path = evidence / "plan.json", evidence / "approval.json"
    plan, _ = command(binary, state, "plan", "execution", "--root", root,
                      "--keep", root / "keeper.bin", "--candidate", root / "candidate.bin",
                      "--quarantine", quarantine, "--max-actions", 1,
                      "--max-in-flight-bytes", len(PAYLOAD), "--reserve-bytes", 1,
                      "--output", plan_path)
    require(plan["body"]["bounds"]["max_in_flight_files"] == 1, "unbounded plan")
    require(len(plan["body"]["actions"]) == 1, "plan action count changed")
    _, _ = command(binary, state, "plan", "approve", "--plan", plan_path,
                   "--fingerprint", plan["fingerprint"], "--approved-by", "synthetic pilot",
                   "--output", approval_path)
    return plan_path, approval_path, plan


def cross_filesystem(binary, workspace, destination):
    source = workspace / "cross-source"
    keeper, candidate = pair(source)
    state, evidence = workspace / "cross-state", workspace / "cross-evidence"
    state.mkdir()
    evidence.mkdir()
    before = available(destination)
    scan, _ = command(binary, state, "scan", "--no-probe", source)
    scan_id = scan["run"]["run_id"]
    review, _ = command(binary, state, "plan", "exact-duplicates", "--run", scan_id,
                        "--output", evidence / "review-plan.json")
    require(review["safety"]["mutates_files"] is False, "review plan gained authority")
    plan_path, approval_path, plan = plan_and_approve(binary, state, source, destination, evidence)
    require(plan["body"]["actions"][0]["topology"] == "cross_filesystem", "wrong topology")
    dry, _ = command(binary, state, "apply", "--plan", plan_path,
                     "--approval", approval_path, "--dry-run")
    require(dry["status"] == "validated" and candidate.exists(), "dry run mutated source")
    _, refused = command(binary, state, "apply", "--plan", plan_path, success=False)
    require(any(d["code"] == "execution_approval_required" for d in refused["diagnostics"]),
            "missing approval was not refused")
    require(candidate.exists() and not any(destination.iterdir()), "refusal mutated source")
    applied, _ = command(binary, state, "apply", "--plan", plan_path, "--approval", approval_path)
    run_id = applied["run_id"]
    status, _ = command(binary, state, "execution", "status", "--run", run_id)
    require(applied["status"] == "completed" and status["actions"][0]["state"] == "quarantined",
            "quarantine did not commit")
    quarantined = destination / plan["fingerprint"] / "action-000001"
    require(not candidate.exists() and digest(quarantined) == digest(keeper), "quarantine bytes differ")
    restored, _ = command(binary, state, "execution", "restore", "--run", run_id,
                          "--action", "action-000001", "--plan", plan_path,
                          "--approval", approval_path)
    require(restored["actions"][0]["state"] == "restored_retained", "restore was not recorded")
    require(digest(candidate) == digest(keeper) == digest(quarantined), "restore bytes differ")
    preview_path, authorization_path = evidence / "preview.json", evidence / "authorization.json"
    preview, _ = command(binary, state, "execution", "finalize", "--plan", plan_path,
                         "--approval", approval_path, "--run", run_id,
                         "--action", "action-000001", "--output", preview_path)
    _, unauthorized = command(binary, state, "execution", "finalize", "--commit",
                              "--plan", plan_path, "--approval", approval_path,
                              "--preview", preview_path, success=False)
    require(unauthorized["diagnostics"] and quarantined.exists(),
            "missing finalization authority was not refused")
    command(binary, state, "execution", "authorize-finalization", "--plan", plan_path,
            "--preview", preview_path, "--fingerprint", preview["fingerprint"],
            "--approved-by", "synthetic pilot", "--output", authorization_path)
    final, _ = command(binary, state, "execution", "finalize", "--commit",
                       "--plan", plan_path, "--approval", approval_path,
                       "--preview", preview_path, "--authorization", authorization_path)
    current, _ = command(binary, state, "execution", "status", "--run", run_id)
    require(final["status"] == current["status"] == "finalized_irreversible", "finalization state differs")
    require([e["phase"] for e in current["events"]] == ["removal_pending", "removed"],
            "finalization journal transition missing")
    require(not quarantined.exists() and digest(candidate) == digest(keeper),
            "finalization changed surviving source")
    require(current["logical_bytes_removed"] == len(PAYLOAD), "logical byte count differs")
    require(current["observed_allocated_bytes_removed"] > 0, "allocation observation missing")
    require(current["shared_extent_bytes"] is None and current["physical_reclaimed_bytes"] is None,
            "physical saving was asserted without proof")
    after = available(destination)
    return {"scan_run": scan_id, "execution_run": run_id, "review_only": True,
            "dry_run": dry["status"], "approval_refused": refused["outcome"]["exit_code"],
            "unauthorized_finalization_refused": unauthorized["outcome"]["exit_code"],
            "journal_phases": [e["phase"] for e in current["events"]],
            "logical_bytes_removed": current["logical_bytes_removed"],
            "observed_allocated_bytes_removed": current["observed_allocated_bytes_removed"],
            "shared_extent_bytes": None, "physical_reclaimed_bytes": None,
            "target_free_bytes_before": before, "target_free_bytes_after": after,
            "target_free_space_change_bytes": after - before,
            "event_free_space_change_bytes": current["events"][-1]["target_free_space_change_bytes"]}


def same_filesystem(binary, workspace):
    source, quarantine = workspace / "same-source", workspace / "same-quarantine"
    keeper, candidate = pair(source)
    quarantine.mkdir()
    state, evidence = workspace / "same-state", workspace / "same-evidence"
    state.mkdir()
    evidence.mkdir()
    plan_path, approval_path, plan = plan_and_approve(binary, state, source, quarantine, evidence)
    require(plan["body"]["actions"][0]["topology"] == "same_filesystem", "wrong topology")
    applied, _ = command(binary, state, "apply", "--plan", plan_path, "--approval", approval_path)
    run_id = applied["run_id"]
    command(binary, state, "execution", "restore", "--run", run_id,
            "--action", "action-000001", "--plan", plan_path, "--approval", approval_path)
    cleaned, _ = command(binary, state, "execution", "cleanup", "--run", run_id,
                         "--plan", plan_path, "--approval", approval_path)
    require(cleaned["status"] == "cleaned" and digest(candidate) == digest(keeper),
            "same-filesystem restore/cleanup failed")
    require(not (quarantine / plan["fingerprint"]).exists(), "owned namespace remains")
    return {"execution_run": run_id, "status": cleaned["status"]}


def disconnected_before_action(binary, workspace):
    source, quarantine = workspace / "disconnect-source", workspace / "disconnect-quarantine"
    keeper, candidate = pair(source)
    quarantine.mkdir()
    state, evidence = workspace / "disconnect-state", workspace / "disconnect-evidence"
    state.mkdir()
    evidence.mkdir()
    plan_path, approval_path, plan = plan_and_approve(binary, state, source, quarantine, evidence)
    detached = workspace / "detached-source"
    source.rename(detached)
    try:
        _, refused = command(binary, state, "apply", "--plan", plan_path,
                             "--approval", approval_path, success=False)
        require(any(d["code"] in ("execution_source_stale", "execution_source_unavailable")
                    for d in refused["diagnostics"]),
                "disconnected root did not yield source-refusal evidence")
        require(not (quarantine / plan["fingerprint"]).exists(),
                "disconnected root created a quarantine namespace")
    finally:
        detached.rename(source)
    require(digest(keeper) == digest(candidate), "reconnected synthetic bytes differ")
    return {"simulated_rename_only": True, "exit_code": refused["outcome"]["exit_code"],
            "source_preserved": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--work-root", type=Path, required=True)
    parser.add_argument("--cross-quarantine-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    work_root = args.work_root.resolve(strict=True)
    cross_root = args.cross_quarantine_root.resolve(strict=True)
    require(binary.is_file() and work_root.is_dir() and cross_root.is_dir(), "invalid paths")
    require(work_root.stat().st_dev != cross_root.stat().st_dev,
            "cross-quarantine root must be a distinct filesystem")
    require(available(cross_root) > 20 * 1024 * 1024, "cross filesystem lacks pilot reserve")
    output = args.output.absolute()
    require(not output.exists() and output.parent.is_dir(), "output must be create-only in an existing directory")
    work = Path(tempfile.mkdtemp(prefix="optiflow-synthetic-", dir=work_root))
    cross = Path(tempfile.mkdtemp(prefix="optiflow-synthetic-quarantine-", dir=cross_root))
    with binary.open("rb") as stream:
        binary_sha256 = hashlib.file_digest(stream, "sha256").hexdigest()
    receipt = {"schema": SCHEMA, "qualified_for_release": False,
               "binary_sha256": binary_sha256,
               "work_filesystem_id": work.stat().st_dev,
               "quarantine_filesystem_id": cross.stat().st_dev,
               "synthetic_work": str(work), "synthetic_quarantine": str(cross),
               "passed": False, "scenarios": {}}
    try:
        receipt["scenarios"]["cross_filesystem"] = cross_filesystem(binary, work, cross)
        receipt["scenarios"]["same_filesystem"] = same_filesystem(binary, work)
        receipt["scenarios"]["disconnected_before_action"] = disconnected_before_action(binary, work)
        shutil.rmtree(work)
        shutil.rmtree(cross)
        receipt["cleaned_synthetic_fixtures"] = True
        receipt["passed"] = True
    except Exception as error:
        receipt["error"] = str(error)
        receipt["cleaned_synthetic_fixtures"] = False
        raise
    finally:
        with output.open("x", encoding="utf-8") as stream:
            json.dump(receipt, stream, indent=2, sort_keys=True)
            stream.write("\n")
    print(f"Synthetic transaction smoke passed: {output}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, subprocess.TimeoutExpired) as error:
        print(f"synthetic transaction smoke failed: {error}", file=sys.stderr)
        raise SystemExit(2)
