"""Authored #93 negative evidence cases; unexecuted source, not qualification."""
import sys
import unittest
import hashlib
import json
import tempfile
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import mutation_review as review


class NativeMutationReviewTests(unittest.TestCase):
    def test_apfs_image_observations_leave_physical_disconnect_coverage_missing(self):
        # Synthetic packet for the parser only; no native evidence is asserted.
        target, revision, archive, binary = review.TARGETS[1], "a" * 40, "b" * 64, "c" * 64
        facts = {"schema": "optiflow.native-volume-observation.v1", "target": target, "source_revision": revision,
                 "archive_sha256": archive, "filesystem": "apfs", "kind": "apfs_disk_image", "disposable": True,
                 "disconnect_reconnect_observed": False}
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary).resolve() / "generated-volume.json"; data = json.dumps(facts).encode(); path.write_bytes(data)
            evidence = {"path": {"encoding": "utf8", "value": str(path)}, "sha256": hashlib.sha256(data).hexdigest(), "byte_count": len(data)}
            receipt = {"schema": "optiflow.native-mutation-review-evidence.v1", "release_version": "v0.2.0", "source_revision": revision,
                       "target": target, "native_target": target, "profile": "exact-dedup-transaction-v0.2",
                       "archive_sha256": archive, "binary_sha256": binary, "evidence_kind": "native_disposable_volume",
                       "synthetic": False, "fixture_policy": "generated_disposable_only",
                       "volumes": [{"filesystem": "apfs", "kind": "apfs_disk_image", "disposable": True,
                                    "disconnect_reconnect_observed": False, "evidence": evidence}],
                       "cases": {name: {"status": "unavailable", "reason": "synthetic parser fixture only"} for name in review.CASES},
                       "claims": {name: None for name in review.CLAIMS},
                       "space": {"logical_bytes_finalized": 0, "allocated_bytes_observed": None, "free_bytes_before": 100, "free_bytes_after": 100}}
            result = review.validate_receipt(receipt, target, "v0.2.0", revision, archive, binary, review.Budget(4096))
            self.assertIn("real removable-volume disconnect/reconnect evidence missing", result["findings"])
            self.assertFalse(result["native_qualification_established"])
    def test_read_only_or_synthetic_receipt_cannot_stand_for_mutation(self):
        with self.assertRaises(ValueError):
            review.validate_receipt({"schema": "optiflow.read-only-pilot.v1", "passed": True},
                                    review.TARGETS[0], "v0.2.0", "a" * 40, "b" * 64, "c" * 64, review.Budget(4096))

    def test_success_capture_cannot_pass_a_refusal_scenario(self):
        value = {"schema": "optiflow.command-result.v1", "command": "apply", "outcome": {"class": "success", "exit_code": 0}}
        with self.assertRaises(ValueError):
            review.command_result(value, review.CASES["source_change_refusal"])
        value["outcome"] = {"class": "stale_state", "exit_code": 5}
        review.command_result(value, review.CASES["source_change_refusal"])

    def test_read_only_scan_cannot_pass_a_quarantine_case(self):
        value = {"schema": "optiflow.command-result.v1", "command": "scan", "outcome": {"class": "success", "exit_code": 0}, "result": {}}
        with self.assertRaises(ValueError):
            review.command_result(value, review.CASES["quarantine"], "quarantine")

    def test_current_dry_run_and_committed_mutation_use_distinct_contracts(self):
        value = {"schema": "optiflow.command-result.v1", "command": "apply", "outcome": {"class": "success", "exit_code": 0},
                 "result": {"schema": "optiflow.execution-run.v1", "dry_run": True, "status": "validated",
                            "commit": {"schema": "optiflow.execution-commit.v1", "source_mutated": False, "committed_actions": 0}}}
        review.command_result(value, review.CASES["dry_run"], "dry_run")
        with self.assertRaises(ValueError):
            review.command_result(value, review.CASES["quarantine"], "quarantine")
        value["result"] = {"schema": "optiflow.execution-mutation.v2", "dry_run": False, "status": "completed", "committed_actions": 1}
        review.command_result(value, review.CASES["quarantine"], "quarantine")
        with self.assertRaises(ValueError):
            review.command_result(value, review.CASES["dry_run"], "dry_run")
