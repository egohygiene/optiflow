#!/usr/bin/env python3
"""Qualify an installed native archive using disposable, bounded synthetic sources."""
import argparse
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import signal
import sqlite3
import subprocess
import sys
import tarfile
import tempfile
import time

import pilot_contract as pilot
import release_bundle as release

ROOT = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def source_snapshot(root):
    rows = []
    for path in sorted(root.rglob("*")):
        metadata = path.lstat()
        rows.append([str(path.relative_to(root)), metadata.st_mode, metadata.st_size,
                     metadata.st_dev, metadata.st_ino, metadata.st_nlink,
                     metadata.st_mtime_ns, metadata.st_ctime_ns,
                     pilot.sha256(path) if path.is_file() else None])
    # Root ctime is omitted: the test operator renames the synthetic volume.
    return rows


def content_digest(rows):
    return hashlib.sha256(json.dumps([[r[0], r[-1]] for r in rows], separators=(",", ":")).encode()).hexdigest()


def tree_bytes(path):
    return sum(p.stat().st_size for p in path.rglob("*") if p.is_file())


def command(binary, state, arguments):
    return [str(binary), "--no-config", "--state-directory", str(state), "--json", *map(str, arguments)]


def checked_output(completed, elapsed, expected):
    require(completed.returncode == expected, f"unexpected exit {completed.returncode}: {completed.stderr[:1000]}")
    require(not completed.stderr, "machine command unexpectedly emitted stderr")
    document = json.loads(completed.stdout)  # Reject human progress or trailing JSON on stdout.
    require(document.get("schema") == "optiflow.command-result.v1", "unexpected command schema")
    require(document["outcome"]["exit_code"] == expected, "exit/envelope mismatch")
    row = {"exit_code": expected, "outcome": document["outcome"]["class"],
           "elapsed_seconds": round(elapsed, 6), "coverage": (document.get("coverage") or {}).get("status")}
    report = document.get("result") or {}
    summary = report.get("summary", {})
    row.update({"files": summary.get("file_count"), "cache_hits": summary.get("cache_hits"),
                "logical_bytes": summary.get("total_bytes"), "groups": summary.get("exact_duplicate_groups")})
    return document, row


def run(binary, state, arguments, expected=0):
    started = time.monotonic()
    completed = subprocess.run(command(binary, state, arguments), capture_output=True,
                               text=True, timeout=180, check=False, cwd=state.parent)
    return checked_output(completed, time.monotonic() - started, expected)


def interrupt(binary, state, source):
    """Synchronize on the durable running row, never on an arbitrary sleep."""
    started = time.monotonic()
    with subprocess.Popen(command(binary, state, ["scan", "--no-probe", source]),
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) as process:
        try:
            run_id = None
            while time.monotonic() - started < 10:
                require(process.poll() is None, "scan finished before the interruption barrier")
                try:
                    with closing(sqlite3.connect(f"file:{state / 'state.sqlite3'}?mode=ro", uri=True, timeout=0.01)) as connection:
                        row = connection.execute("SELECT run_id FROM scan_runs WHERE status = 'running'").fetchone()
                    if row:
                        run_id = row[0]
                        break
                except sqlite3.Error:
                    pass
                time.sleep(0.001)
            require(run_id is not None, "running-row interruption barrier timed out")
            process.send_signal(signal.SIGINT)
            stdout, stderr = process.communicate(timeout=30)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
    completed = subprocess.CompletedProcess([], process.returncode, stdout, stderr)
    document, measurement = checked_output(completed, time.monotonic() - started, 130)
    require(not document["artifacts"], "interrupted run exposed completed artifacts")
    with closing(sqlite3.connect(state / "state.sqlite3")) as connection:
        status = connection.execute("SELECT status FROM scan_runs WHERE run_id = ?", (run_id,)).fetchone()[0]
    require(status == "interrupted", "interrupted run was promoted")
    measurement["run_status"] = status
    return run_id, measurement


def previous_install(outer, destination, target, cosign):
    """The signed inner manifest is the trust boundary, not download transport."""
    directory = destination / "previous-bundle"
    directory.mkdir()
    expected = release.archive_names(pilot.PREVIOUS_VERSION) | set(release.PRIMARY_EVIDENCE) | {
        release.SUBJECTS_MANIFEST, release.COMPLETE_MANIFEST, release.SIGNATURE}
    with tarfile.open(outer, "r:gz") as bundle:
        seen = set()
        for item in bundle.getmembers():
            if item.name in (".", "./") and item.isdir():
                continue
            name = item.name.removeprefix("./")
            require(name in expected and name not in seen and item.isfile() and 0 < item.size <= 128 * 1024 * 1024,
                    "unexpected previous release bundle member")
            seen.add(name)
            with bundle.extractfile(item) as stream, (directory / name).open("xb") as output:
                shutil.copyfileobj(stream, output)
        require(seen == expected, "incomplete previous release bundle")
    subjects = release.archive_names(pilot.PREVIOUS_VERSION) | set(release.PRIMARY_EVIDENCE)
    release.verify_manifest(directory, release.SUBJECTS_MANIFEST, subjects)
    release.verify_manifest(directory, release.COMPLETE_MANIFEST, subjects | {release.SUBJECTS_MANIFEST, release.SIGNATURE})
    release.verify_evidence(directory, pilot.PREVIOUS_VERSION, pilot.PREVIOUS_REVISION, release.archive_names(pilot.PREVIOUS_VERSION))
    subprocess.run([cosign, "verify-blob", "--bundle", str(directory / release.SIGNATURE),
                    "--certificate-identity", release.WORKFLOW_IDENTITY,
                    "--certificate-oidc-issuer", release.OIDC_ISSUER,
                    str(directory / release.SUBJECTS_MANIFEST)], check=True, timeout=90,
                   stdout=sys.stderr)
    install = destination / "previous-install"
    pilot.archive_binary(directory / f"optiflow-{pilot.PREVIOUS_VERSION}-{target}.tar.gz", install)
    return install / "optiflow"


def qualify(args):
    require(args.target == pilot.native_target(), "cross-compiled binaries cannot qualify a native target")
    release.validate_inputs(ROOT, args.release_version, args.source_revision)
    actual_revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    require(actual_revision == args.source_revision, "qualification revision differs from checkout")
    subprocess.run(["git", "diff", "--quiet", "HEAD", "--"], cwd=ROOT, check=True)
    output = args.output_directory.resolve()
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"optiflow-{args.release_version}-{args.target}.tar.gz"
    receipt_path = output / "qualification.json"
    require(not archive.exists() and not receipt_path.exists(), "refusing to overwrite qualification evidence")
    release.deterministic_archive(args.binary.resolve(strict=True), ROOT / "LICENSE", archive)
    receipt = {"schema": pilot.SCHEMA, "release_version": args.release_version,
               "source_revision": args.source_revision, "target": args.target,
               "native_target": pilot.native_target(), "profile": pilot.PROFILE,
               "archive_sha256": pilot.sha256(archive), "binary_sha256": pilot.archive_binary(archive),
               "previous_version": pilot.PREVIOUS_VERSION, "previous_revision": pilot.PREVIOUS_REVISION,
               "previous_signature_verified": False, "passed": False, "scenarios": {}}
    rows = receipt["scenarios"]
    environment = {key: value for key, value in os.environ.items() if not key.startswith("OPTIFLOW_")}
    # subprocesses inherit this isolated configuration policy, without replacing HOME.
    original_environment = os.environ.copy()
    os.environ.clear()
    os.environ.update(environment)
    try:
        with tempfile.TemporaryDirectory(prefix="optiflow-pilot-") as temporary:
            work = Path(temporary)
            install = work / "clean-install"
            pilot.archive_binary(archive, install)
            binary = install / "optiflow"
            version = subprocess.check_output([str(binary), "--version"], text=True, timeout=10).strip()
            require(version == f"optiflow {args.release_version.removeprefix('v')}", "installed binary version mismatch")
            previous = previous_install(args.previous_bundle.resolve(strict=True), work, args.target, args.cosign)
            previous_version = subprocess.check_output([str(previous), "--version"], text=True, timeout=10).strip()
            require(previous_version == "optiflow 0.1.0", "previous installed version mismatch")
            receipt["previous_signature_verified"] = True
            source = work / "synthetic-volume"
            tree, large = source / "tree", source / "large"
            tree.mkdir(parents=True)
            large.mkdir()
            for index in range(pilot.PROFILE["tree_files"]):
                bucket = tree / f"bucket-{index % 32:02}"
                bucket.mkdir(exist_ok=True)
                (bucket / f"file-{index:05}.bin").write_bytes(index.to_bytes(8, "little") + b"pilot-89\n")
            chunk = bytes(range(256)) * 4096
            for member in range(2):
                with (large / f"large-{member}.bin").open("wb") as stream:
                    for _ in range(pilot.PROFILE["large_file_bytes"] // len(chunk)):
                        stream.write(chunk)
            for path in source.rglob("*"):
                if path.is_file():
                    path.chmod(0o444)
            before = source_snapshot(source)
            receipt["source_digest"] = content_digest(before)
            state = work / "local-state"
            state.mkdir()
            documents = {}
            for name, root, storage in [("tree_cold", tree, state / "tree"), ("tree_warm", tree, state / "tree"),
                                        ("large_cold", large, state / "large"), ("large_warm", large, state / "large")]:
                documents[name], rows[name] = run(binary, storage, ["scan", "--no-probe", root])
                rows[name]["state_bytes"] = tree_bytes(storage)
            run_id = documents["large_cold"]["result"]["run"]["run_id"]
            _, rows["report"] = run(binary, state / "large", ["report", run_id])
            plan, rows["plan"] = run(binary, state / "large", ["plan", "exact-duplicates", "--run", run_id,
                                                                  "--output", work / "review-plan.json"])
            rows["plan"]["mutates_files"] = plan["result"]["safety"]["mutates_files"]
            _, rows["partial"] = run(binary, state / "partial", ["scan", "--no-probe", tree, work / "absent"], 3)
            require(source_snapshot(source) == before, "source changed during ordinary commands")
            detached = work / "detached-volume"
            source.rename(detached)
            try:
                document, rows["disconnected"] = run(binary, state / "large", ["scan", "--no-probe", large], 2)
                require(not document["artifacts"], "disconnected source exposed artifacts")
            finally:
                detached.rename(source)
            _, rows["reconnected"] = run(binary, state / "large", ["scan", "--no-probe", large])
            interrupted_id, rows["interrupted"] = interrupt(binary, state / "cancel", large)
            restarted, rows["restart"] = run(binary, state / "cancel", ["scan", "--no-probe", large])
            rows["restart"]["new_run"] = restarted["result"]["run"]["run_id"] != interrupted_id
            # Upgrade the original state location, then restore its complete offline
            # backup at that same path: artifact references may contain absolute paths.
            compatible_state = state / "upgrade"
            old, _ = run(previous, compatible_state, ["scan", "--no-probe", tree])
            old_id = old["result"]["run"]["run_id"]
            backup = work / "pre-upgrade-backup"
            shutil.copytree(compatible_state, backup)
            _, rows["upgrade"] = run(binary, compatible_state, ["report", old_id])
            run(binary, compatible_state, ["scan", "--no-probe", tree])
            compatible_state.rename(work / "retained-upgraded-state")
            shutil.copytree(backup, compatible_state)
            _, rows["rollback"] = run(previous, compatible_state, ["report", old_id])
            require(source_snapshot(source) == before, "source content or metadata changed")
            receipt["source_unchanged"] = True
            receipt["json_stdout_clean"] = True
            receipt["state_bytes"] = tree_bytes(state)
            maximum_rss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
            receipt["peak_child_rss_bytes"] = int(maximum_rss * (1 if sys.platform == "darwin" else 1024))
        require(not work.exists(), "pilot workspace cleanup failed")
        receipt["cleanup_complete"] = True
        receipt["passed"] = True
        pilot.validate_receipt(receipt, args.target, args.release_version, args.source_revision, archive)
    except Exception as error:
        receipt["passed"] = False
        receipt["error"] = str(error)
        raise
    finally:
        os.environ.clear()
        os.environ.update(original_environment)
        release.write_json(receipt_path, receipt)
    print(f"Native read-only pilot passed: {args.target}; {receipt_path}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", choices=release.SUPPORTED_TARGETS, required=True)
    parser.add_argument("--release-version", required=True)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--previous-bundle", type=Path, required=True)
    parser.add_argument("--cosign", default="cosign")
    parser.add_argument("--output-directory", type=Path, required=True)
    qualify(parser.parse_args())


if __name__ == "__main__":
    main()
