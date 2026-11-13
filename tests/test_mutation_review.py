"""Authored #93 negative evidence cases; unexecuted source, not qualification."""
import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import mutation_review as review


class NativeMutationReviewTests(unittest.TestCase):
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
