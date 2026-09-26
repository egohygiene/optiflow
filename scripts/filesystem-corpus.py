#!/usr/bin/env python3
"""Drift-check and run the synthetic filesystem corpus using only Python stdlib.

Compilation has its own timeout and is outside execution resource budgets.
Rust tests own typed assertions; this runner binds their source, refuses empty
test selections, enforces budgets, and retains bounded, reproducible receipts.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "tests/fixtures/filesystem/cases.json"
INDEX = ROOT / "tests/fixtures/filesystem/index.json"


def canonical(value):
    return (json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True) + "\n").encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def generated_index(catalog):
    ids = [case["id"] for case in catalog["cases"]]
    if len(ids) != len(set(ids)):
        raise ValueError("duplicate corpus fixture ID")
    return {
        "schema": "optiflow.filesystem-corpus-index.v1",
        "license": "MIT",
        "generator_sha256": digest(Path(__file__).read_bytes()),
        "catalog_sha256": digest(CATALOG.read_bytes()),
        "cargo_lock_sha256": digest((ROOT / "Cargo.lock").read_bytes()),
        "cases": [{
            "id": case["id"],
            "recipe_sha256": digest(canonical(case["recipe"])),
            "expected_sha256": digest(canonical(case["expected"])),
            "proof_source_sha256": digest((ROOT / case["source"]).read_bytes()),
            "payload_sha256": [digest(text.encode()) for text in case["recipe"]["content_utf8"]],
        } for case in catalog["cases"]],
    }


def tree_bytes(path):
    # No symlink following: fixtures include broken and external-looking links.
    total = 0
    for entry in path.rglob("*"):
        try:
            if not entry.is_symlink() and entry.is_file():
                total += entry.stat().st_size
        except FileNotFoundError:
            pass  # Tests deliberately rename and clean up concurrent with sampling.
    return total


def limits(budget):
    if sys.platform == "linux":
        resource.setrlimit(resource.RLIMIT_AS, (budget["memory_bytes"],) * 2)
    resource.setrlimit(resource.RLIMIT_FSIZE, (budget["file_bytes"],) * 2)
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def resident_bytes(process_group):
    # Linux uses the hard inherited address-space limit; macOS uses group RSS.
    if sys.platform == "linux":
        return None
    listing = subprocess.check_output(["ps", "-axo", "pgid=,rss="], text=True, timeout=2)
    return sum(int(row[1]) * 1024 for line in listing.splitlines()
               if len(row := line.split()) == 2 and int(row[0]) == process_group)


def run_bounded(argv, workspace, log, budget, deadline, evidence_root, env=None):
    """Hard process limits plus wall/aggregate-disk watchdog; kill the process group."""
    environment = os.environ.copy() if env is None else env.copy()
    environment["TMPDIR"] = str(workspace)
    environment["RUST_BACKTRACE"] = "0"
    started = time.monotonic()
    failure = None
    peak_workspace = 0
    peak_memory = None
    with log.open("wb") as output:
        process = subprocess.Popen(
            argv, cwd=ROOT, env=environment, stdout=output, stderr=subprocess.STDOUT,
            start_new_session=True, preexec_fn=lambda: limits(budget),
        )
        try:
            while True:
                status = process.poll()
                used = tree_bytes(workspace)
                peak_workspace = max(used, peak_workspace)
                memory = resident_bytes(process.pid)
                if memory is not None:
                    peak_memory = max(memory, peak_memory or 0)
                if time.monotonic() > min(deadline, started + budget["case_seconds"]):
                    failure = "wall_time_budget"
                elif used > budget["workspace_bytes"]:
                    failure = "workspace_budget"
                elif memory is not None and memory > budget["memory_bytes"]:
                    failure = "memory_budget"
                elif tree_bytes(evidence_root) > budget["evidence_bytes"]:
                    failure = "evidence_budget"
                if failure or status is not None:
                    break
                time.sleep(0.02)
        finally:
            # Also remove descendants left behind by a failed/aborted test.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
    if failure:
        raise RuntimeError(failure)
    if process.returncode:
        raise RuntimeError(f"test_exit_{process.returncode}: {log}")
    return {"elapsed_seconds": round(time.monotonic() - started, 3),
            "peak_workspace_bytes_sampled": peak_workspace,
            "peak_resident_bytes_sampled": peak_memory}


def build_tests():
    build_log = ROOT / "target/filesystem-corpus-build.jsonl"
    build_log.parent.mkdir(parents=True, exist_ok=True)
    with build_log.open("wb") as log:
        subprocess.run([
            "cargo", "test", "--locked", "--no-run", "--lib",
            "--test", "filesystem_corpus", "--test", "cli_outcomes",
            "--jobs", "2", "--message-format", "json",
        ], cwd=ROOT, stdout=log, check=True, timeout=600)
    binaries = {}
    for line in build_log.read_text().splitlines():
        event = json.loads(line)
        if event.get("reason") == "compiler-artifact" and event.get("executable") and event["profile"]["test"]:
            target = "lib" if "lib" in event["target"]["kind"] else event["target"]["name"]
            binaries[target] = event["executable"]
    build_log.unlink()
    return binaries


def execute(catalog, index, tier, output):
    if sys.platform not in catalog["platforms"]:
        raise RuntimeError("filesystem corpus requires Linux or macOS")
    budget = catalog["budgets"][tier]
    output.mkdir(parents=True, exist_ok=True)
    binaries = build_tests()
    # Each invocation gets fresh evidence; never overwrite a previous run.
    evidence = Path(tempfile.mkdtemp(prefix=f"{tier}-", dir=output))
    receipts = []
    started = time.monotonic()
    deadline = started + budget["wall_seconds"]
    indexed = {case["id"]: case for case in index["cases"]}
    selected = [case for case in catalog["cases"] if tier == "scheduled" or case["tier"] == "pr"]
    environment = {key: value for key, value in os.environ.items() if not key.startswith("OPTIFLOW_")}
    status = "interrupted"
    try:
        for case in selected:
            binary = binaries[case["target"]]
            listing = subprocess.check_output([binary, "--list", "--exact", case["test"]], cwd=ROOT, timeout=5, text=True)
            if listing.splitlines() != [f'{case["test"]}: test', "", "1 test, 0 benchmarks"]:
                raise RuntimeError(f"missing, ignored, or ambiguous corpus selector: {case['id']}")
            for repetition in range(budget["repetitions"]):
                log = evidence / f"{case['id']}-{repetition}.log"
                with tempfile.TemporaryDirectory(prefix="work-", dir=output) as temporary:
                    workspace = Path(temporary)
                    command = [binary, "--exact", case["test"], "--test-threads", "1"]
                    if case["tier"] == "scheduled":
                        command.append("--include-ignored")
                    measurement = run_bounded(
                        command,
                        workspace, log, budget, deadline, evidence, environment,
                    )
                    if "test result: ok. 1 passed; 0 failed; 0 ignored;" not in log.read_text():
                        raise RuntimeError(f"fixture did not execute exactly one proof: {case['id']}")
                    if any(workspace.iterdir()):
                        raise RuntimeError(f"fixture leaked its temporary workspace: {case['id']}")
                if workspace.exists():
                    raise RuntimeError("runner cleanup failed")
                receipts.append({"id": case["id"], "repetition": repetition,
                                 "assertions": "passed", "cleanup": "complete",
                                 **indexed[case["id"]], **measurement})
        status = "passed"
    except Exception as error:
        status = str(error)
        raise
    finally:
        report = {
            "schema": "optiflow.filesystem-corpus-evidence.v1", "tier": tier,
            "status": status, "platform": sys.platform, "budgets": budget,
            "index_sha256": digest(canonical(index)), "receipts": receipts,
            "elapsed_seconds": round(time.monotonic() - started, 3),
            "toolchain": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        }
        encoded = canonical(report)
        if tree_bytes(evidence) + len(encoded) > budget["evidence_bytes"]:
            raise RuntimeError("evidence_budget including final receipt")
        (evidence / "summary.json").write_bytes(encoded)
        print(f"{status}: {len(receipts)} corpus executions; evidence: {evidence}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write-index", action="store_true", help="explicitly accept reviewed recipe/proof changes")
    parser.add_argument("--check", action="store_true", help="check generated provenance and expected-evidence digests")
    parser.add_argument("--tier", choices=["pr", "scheduled"])
    parser.add_argument("--output", type=Path, default=ROOT / "target/adversarial-evidence/filesystem")
    args = parser.parse_args()
    catalog = json.loads(CATALOG.read_text())
    index = generated_index(catalog)
    if args.write_index:
        INDEX.write_bytes(canonical(index))
    if not INDEX.exists() or INDEX.read_bytes() != canonical(index):
        parser.error("corpus drift: review recipe/expected/proof changes, then --write-index")
    if args.tier:
        execute(catalog, index, args.tier, args.output.resolve())
    else:
        print(f"corpus index current: {len(index['cases'])} fixtures")


if __name__ == "__main__":
    main()
