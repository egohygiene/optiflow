"""Negative controls for the CLI proof observer; no product/provider is started."""
import copy
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("png_folder_proof", ROOT / "scripts/prove-png-folder-cli.py")
proof = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(proof)


def envelope(command="optimize discover", code=2, message="entry bound exceeded; no provider started"):
    return {"schema": "optiflow.command-result.v1", "command": command,
            "outcome": {"class": {0: "success", 2: "invalid_input", 4: "capability_unavailable"}[code], "exit_code": code},
            "artifacts": [], "diagnostics": [] if code == 0 else [{
                "code": "execution_unsupported" if code == 4 else "execution_plan_invalid",
                "classification": "capability" if code == 4 else "input",
                "severity": "error", "impact": "blocks_command", "message": message}],
            "result": {"observed": True} if code == 0 else None}


def approval_fixture():
    request = {"original_disposition": "retain_for_restore", "batch": {"maximum_actions": 4}}
    inventory = {"request_fingerprint": "a" * 64, "fingerprint": "b" * 64}
    approval = {"schema": "optiflow.png-folder-approval.v1", "approved_by": proof.ACTOR,
                "approved_at": "2026-11-14T00:00:00+00:00", "authorization_id": "c" * 64,
                "request_fingerprint": inventory["request_fingerprint"],
                "inventory_fingerprint": inventory["fingerprint"],
                "disposition": "retain_for_restore", "maximum_actions": 4}
    return approval, request, inventory


def inventory_fixture(names, source=Path("/synthetic/source")):
    return {"schema": "optiflow.png-folder-inventory.v1", "one_file_in_flight": True,
            "physical_space_reclaimed": False,
            "entries": [{"path": {"encoding": "utf8", "value": str(source / name)},
                         "disposition": "eligible" if name in {"a.png", "nested/雪 song.png"} else "skipped"}
                        for name in sorted(names)]}


class PngFolderObserverTests(unittest.TestCase):
    def test_same_exit_failure_needs_exact_command_outcome_and_diagnostic(self):
        reason = "entry bound exceeded; no provider started"
        valid = envelope(message=reason)
        proof.observe_envelope(valid, "optimize discover", 2, reason)
        variants = []
        for field, value in [("message", "unrelated missing configuration"), ("code", "execution_bounds_exceeded"),
                             ("classification", "state"), ("impact", "none"), ("severity", "warning")]:
            item = copy.deepcopy(valid); item["diagnostics"][0][field] = value; variants.append(item)
        item = copy.deepcopy(valid); item["command"] = "optimize approve"; variants.append(item)
        item = copy.deepcopy(valid); item["outcome"]["class"] = "internal_failure"; variants.append(item)
        item = copy.deepcopy(valid); item["diagnostics"].append(copy.deepcopy(item["diagnostics"][0])); variants.append(item)
        item = copy.deepcopy(valid); item["result"] = {"accepted": True}; variants.append(item)
        for value in variants:
            with self.subTest(value=value), self.assertRaises(ValueError):
                proof.observe_envelope(value, "optimize discover", 2, reason)

    def test_qualification_keyword_cannot_substitute_for_exact_native_refusal(self):
        valid = envelope("optimize run", 4, proof.NATIVE_REFUSAL)
        proof.observe_envelope(valid, "optimize run", 4, proof.NATIVE_REFUSAL)
        for message in ("qualification failed for an unrelated provider", "missing request file"):
            value = copy.deepcopy(valid); value["diagnostics"][0]["message"] = message
            with self.subTest(message=message), self.assertRaises(ValueError):
                proof.observe_envelope(value, "optimize run", 4, proof.NATIVE_REFUSAL)
        with self.assertRaises(ValueError):
            proof.observe_envelope(valid, "optimize resume", 4, proof.NATIVE_REFUSAL)

    def test_success_cannot_hide_other_outcomes_or_authority(self):
        valid = envelope("config show", 0)
        proof.observe_envelope(valid, "config show", 0)
        for mutation in (lambda v: v["outcome"].update(exit_code=False),
                         lambda v: v["outcome"].update({"class": "partial_success"}),
                         lambda v: v.update(artifacts=[{"authority": True}]),
                         lambda v: v.update(diagnostics=envelope()["diagnostics"]),
                         lambda v: v.update(result=None)):
            value = copy.deepcopy(valid); mutation(value)
            with self.subTest(value=value), self.assertRaises(ValueError):
                proof.observe_envelope(value, "config show", 0)

    def test_approval_result_saved_file_and_context_must_agree(self):
        valid, request, inventory = approval_fixture()
        proof.observe_approval(valid, copy.deepcopy(valid), request, inventory)
        for field, value in [("schema", "other.schema.v1"), ("approved_by", "another actor"),
                             ("request_fingerprint", "d" * 64), ("inventory_fingerprint", "e" * 64),
                             ("authorization_id", "malformed"), ("disposition", "discard_after_durable_replacement"),
                             ("maximum_actions", 3), ("approved_at", "2026-11-14T00:00:00"),
                             ("apply_authority", True)]:
            changed = dict(valid, **{field: value})
            with self.subTest(field=field), self.assertRaises(ValueError):
                proof.observe_approval(changed, copy.deepcopy(changed), request, inventory)
        changed_file = dict(valid, approved_by="substituted saved actor")
        with self.assertRaises(ValueError):
            proof.observe_approval(valid, changed_file, request, inventory)

    def test_inventory_requires_unique_exact_paths_and_flat_membership(self):
        source = Path("/synthetic/source")
        valid = inventory_fixture(proof.RECURSIVE_NAMES)
        indexed = proof.indexed_entries(valid, source, proof.RECURSIVE_NAMES)
        proof.require_eligible(indexed, ["a.png", "nested/雪 song.png"])
        duplicate = copy.deepcopy(valid); duplicate["entries"].append(copy.deepcopy(duplicate["entries"][0]))
        extra = copy.deepcopy(valid); extra["entries"][0]["path"]["value"] = str(source / "unexpected.png")
        for value in (duplicate, extra):
            with self.subTest(value=value), self.assertRaises(ValueError):
                proof.indexed_entries(value, source, proof.RECURSIVE_NAMES)
        flat_names = proof.RECURSIVE_NAMES - {"nested/雪 song.png"} | {"nested"}
        flat = inventory_fixture(flat_names)
        flat_index = proof.indexed_entries(flat, source, flat_names)
        proof.require_eligible(flat_index, ["a.png"])
        wrong_single = copy.deepcopy(indexed)
        wrong_single["a.png"]["disposition"] = "unsupported"
        with self.assertRaises(ValueError):
            proof.require_eligible(wrong_single, ["a.png"])
        with self.assertRaises(ValueError):
            proof.indexed_entries(valid, source, flat_names)

    def test_no_clobber_proof_rejects_byte_identical_record_replacement(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "inventory.json"; path.write_bytes(b"same bytes")
            before = proof.file_observation(path)
            proof.require_same_record(path, before)
            replacement = path.with_name("replacement.json"); replacement.write_bytes(path.read_bytes())
            os.replace(replacement, path)
            self.assertEqual(path.read_bytes(), b"same bytes")
            with self.assertRaises(ValueError): proof.require_same_record(path, before)
            before = proof.file_observation(path)
            path.chmod(0o640 if path.stat().st_mode & 0o777 != 0o640 else 0o600)
            with self.assertRaises(ValueError): proof.require_same_record(path, before)

    def test_duplicate_json_members_never_collapse_to_last_value(self):
        self.assertEqual(proof.unique_json('{"field":1}'), {"field": 1})
        with self.assertRaises(ValueError): proof.unique_json('{"field":0,"field":1}')


if __name__ == "__main__":
    unittest.main()
