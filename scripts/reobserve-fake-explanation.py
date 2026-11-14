#!/usr/bin/env python3
"""Revalidate retained refusal captures; never execute the CLI or fixture."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import time

spec = importlib.util.spec_from_file_location("fake_proof", Path(__file__).resolve().with_name("prove-fake-explanation.py"))
proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proof)


def reobserve(captures, fixture, receipt_pin):
    original_path = captures / "receipt.json"
    original_bytes = original_path.read_bytes()
    proof.require(hashlib.sha256(original_bytes).hexdigest() == receipt_pin, "original receipt pin differs")
    original = proof.strict_json(original_bytes)
    proof.require(original["schema"] == "optiflow.fake-explanation-walkthrough.v1" and original["status"] == "passed", "passing original receipt required")
    proof.require(proof.file_record(fixture) == original["fixture"], "selected fixture differs from original receipt")
    nonzero = [item for item in original["commands"] if item["native_exit"] != 0]
    proof.require(len(nonzero) == 13 and len({item["case"] for item in nonzero}) == 13, "exact thirteen nonzero cases required")
    proof.require({item["case"] for item in nonzero} == set(proof.REFUSAL_MESSAGES) | {"cancel"}, "refusal case membership differs")
    validated = []
    for item in nonzero:
        case = item["case"]
        expected = 130 if case == "cancel" else 2
        command = "explain inspect" if case in ("stale-report", "changed-sidecar") else "explain invoke"
        proof.require(item["native_exit"] == expected and item["command"] == command, "original command/native correlation differs")
        stdout = (captures / f"{case}.stdout.json").read_bytes()
        stderr = (captures / f"{case}.stderr.txt").read_bytes()
        proof.require(len(stdout) == item["stdout_bytes"] and len(stderr) == item["stderr_bytes"], "retained capture byte count differs")
        message = proof.refusal_message(case, fixture) if expected == 2 else None
        value = proof.validate_capture(stdout, stderr, item["native_exit"], expected, command, message)
        proof.require(value["outcome"]["class"] == item["outcome_class"], "original structured outcome differs")
        validated.append({"case": case, "native_exit_from_original_receipt": expected,
                          "structured_outcome": value["outcome"], "exact_closed_envelope_accepted": True,
                          "stdout_sha256_observed_now": hashlib.sha256(stdout).hexdigest(),
                          "stderr_sha256_observed_now": hashlib.sha256(stderr).hexdigest()})
    return {
        "schema": "optiflow.fake-explanation-capture-reobservation.v1", "status": "passed",
        "observed_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "original_whole_run_reexecuted": False, "cli_or_fixture_executed": False,
        "original_receipt_sha256": receipt_pin, "original_observer_source_sha256": original["observer_source_sha256"],
        "strict_observer_source_sha256": proof.sha(Path(proof.__file__).resolve()),
        "reobserver_source_sha256": proof.sha(Path(__file__).resolve()),
        "selected_fixture": original["fixture"], "validated_refusal_count": 12, "validated_interruption_count": 1,
        "captures": validated,
        "limits": ["native exits come from the pinned original receipt, not a new process observation",
                   "capture hashes are bound at reobservation; original receipt recorded capture byte counts",
                   "eight successful CLI captures and whole-run filesystem checks were not reexecuted",
                   "original receipt/build source hashes remain historical bindings to checkpoint 795c96f"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--captures", required=True, type=Path)
    parser.add_argument("--fixture", required=True, type=Path)
    parser.add_argument("--receipt-sha256", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    proof.require(args.captures.is_absolute() and args.captures.is_dir() and args.fixture.is_absolute() and args.fixture.is_file(), "absolute existing captures and fixture required")
    proof.require(args.output.is_absolute() and not args.output.exists(), "absolute new receipt file required")
    value = reobserve(args.captures, args.fixture, args.receipt_sha256)
    descriptor = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w") as handle:
        handle.write(json.dumps(value, indent=2) + "\n")
    print(json.dumps({"status": value["status"], "refusals_reobserved": 12, "interruptions_reobserved": 1, "cli_executed": False}))


if __name__ == "__main__":
    main()
