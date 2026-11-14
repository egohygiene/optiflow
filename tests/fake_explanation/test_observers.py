"""Acceptance observer regressions; the executable walkthrough is separate."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("proof", Path(__file__).resolve().parents[2] / "scripts/prove-fake-explanation.py")
proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proof)


class ObserverTests(unittest.TestCase):
    def outcome(self, native=2, structured=2, kind="invalid_input"):
        value = {"schema": "optiflow.command-result.v1", "command": "explain invoke",
                 "outcome": {"class": kind, "exit_code": structured},
                 "artifacts": [], "result": None,
                 "diagnostics": [{"code": "execution_plan_invalid", "severity": "error", "classification": "input", "impact": "blocks_command", "message": "expected refusal"}]}
        return proof.envelope(json.dumps(value), native, 2, "explain invoke", "expected refusal")

    def fixture(self):
        packet = {"packet_fingerprint": "pin", "facts": [{"fact_id": "recorded_files", "record_id": "report", "recorded_value": 4}]}
        candidate = {"packet_fingerprint": "pin", "declared_identity": {"model_id": "fake"},
                     "observed_fact_ids": ["recorded_files"], "suggestions": [{"text": "Review", "fact_ids": ["recorded_files"]}], "questions": []}
        sidecar = {"packet_fingerprint": "pin", "declared_identity": {"model_id": "fake"},
                   "observed_record_facts": copy.deepcopy(packet["facts"]), "provider_execution_observed": False,
                   "suggestions": copy.deepcopy(candidate["suggestions"]), "questions": [],
                   "current_source_eligibility_established": False, "physical_space_reclaimed": False, "apply_authority": False,
                   "semantic_grounding": "exact_record_facts_and_citation_membership_only; suggestion_meaning_unverified"}
        return packet, candidate, sidecar

    def test_exact_refusal_passes(self):
        self.outcome()

    def test_panic_exit_cannot_pass_valid_looking_refusal(self):
        with self.assertRaises(RuntimeError):
            self.outcome(native=101)

    def test_wrong_outcome_class_refuses(self):
        with self.assertRaises(RuntimeError):
            self.outcome(kind="internal_failure")

    def test_exact_interruption_diagnostic_required(self):
        value = {"schema": "optiflow.command-result.v1", "command": "explain invoke",
                 "outcome": {"class": "interrupted", "exit_code": 130}, "diagnostics": [], "artifacts": [], "result": None}
        with self.assertRaises(RuntimeError):
            proof.envelope(json.dumps(value), 130, 130, "explain invoke")
        value["diagnostics"] = [{"code": "operation_interrupted", "severity": "error", "classification": "interruption", "impact": "blocks_command", "message": "the operation was interrupted by SIGINT"}]
        proof.envelope(json.dumps(value), 130, 130, "explain invoke")

    def test_each_refusal_requires_exact_closed_envelope(self):
        self.assertEqual(len(proof.REFUSAL_MESSAGES), 12)
        for case in proof.REFUSAL_MESSAGES:
            message = proof.refusal_message(case, Path("/selected/fixture"))
            value = {"schema": "optiflow.command-result.v1", "command": "explain invoke",
                     "outcome": {"class": "invalid_input", "exit_code": 2}, "artifacts": [], "result": None,
                     "diagnostics": [{"code": "execution_plan_invalid", "severity": "error", "classification": "input", "impact": "blocks_command", "message": message}]}
            proof.validate_capture(json.dumps(value).encode(), b"", 2, 2, "explain invoke", message)
            mutations = []
            for field, changed in (("code", "different_error"), ("severity", "warning"), ("classification", "capability"),
                                   ("impact", "none"), ("message", message + "; competing reason"), ("message", "prefix: " + message),
                                   ("message", "another refusal in the same domain"), ("context", {"path": "extra"})):
                altered = copy.deepcopy(value)
                altered["diagnostics"][0][field] = changed
                mutations.append(altered)
            for field, changed in (("result", {}), ("result", {"status": "candidate_recorded"}),
                                   ("artifacts", [{"kind": "unexpected"}]), ("extra", "unrecognized envelope field")):
                altered = copy.deepcopy(value)
                altered[field] = changed
                mutations.append(altered)
            altered = copy.deepcopy(value)
            altered["diagnostics"].append({**altered["diagnostics"][0], "message": "competing failure"})
            mutations.append(altered)
            for altered in mutations:
                with self.subTest(case=case, mutation=altered), self.assertRaises(RuntimeError):
                    proof.validate_capture(json.dumps(altered).encode(), b"", 2, 2, "explain invoke", message)

    def test_panic_or_competing_stderr_refuses(self):
        accepted = self.outcome()
        for stderr in (b"thread main panicked at provider", b"a competing internal failure"):
            with self.subTest(stderr=stderr), self.assertRaises(RuntimeError):
                proof.validate_capture(json.dumps(accepted).encode(), stderr, 2, 2, "explain invoke", "expected refusal")

    def test_duplicate_json_member_refuses(self):
        value = json.dumps(self.outcome())
        duplicated = value[:-1] + ', "result": null}'
        with self.assertRaises(RuntimeError):
            proof.envelope(duplicated, 2, 2, "explain invoke", "expected refusal")

    def test_dynamic_message_uses_selected_fixture_exactly(self):
        for case in ("oversize", "timeout"):
            message = proof.refusal_message(case, Path("/selected/fixture"))
            self.assertIn("/selected/fixture", message)
            self.assertNotEqual(message, proof.refusal_message(case, Path("/substituted/fixture")))

    def test_exact_projection_passes(self):
        proof.verify_projection(*self.fixture())

    def test_fabricated_fact_value_refuses(self):
        packet, candidate, sidecar = self.fixture()
        sidecar["observed_record_facts"][0]["recorded_value"] = 400
        with self.assertRaises(RuntimeError):
            proof.verify_projection(packet, candidate, sidecar)

    def test_unknown_duplicate_or_missing_note_citation_refuses(self):
        for refs in ([], ["invented"], ["recorded_files", "recorded_files"]):
            packet, candidate, sidecar = self.fixture()
            candidate["suggestions"][0]["fact_ids"] = refs
            with self.assertRaises(RuntimeError):
                proof.verify_projection(packet, candidate, sidecar)

    def test_changed_profile_or_authority_refuses(self):
        for field in ("declared_identity", "apply_authority"):
            packet, candidate, sidecar = self.fixture()
            sidecar[field] = {"model_id": "different"} if field == "declared_identity" else True
            with self.assertRaises(RuntimeError):
                proof.verify_projection(packet, candidate, sidecar)

    def test_changed_advisory_text_refuses(self):
        packet, candidate, sidecar = self.fixture()
        sidecar["suggestions"][0]["text"] = "Publish everything"
        with self.assertRaises(RuntimeError):
            proof.verify_projection(packet, candidate, sidecar)


if __name__ == "__main__":
    unittest.main()
