"""Source-binding checks only; never compile or execute the Rust corpus."""
import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock


SPEC = importlib.util.spec_from_file_location(
    "filesystem_corpus_bindings",
    Path(__file__).resolve().parents[1] / "scripts/filesystem-corpus.py",
)
CORPUS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CORPUS)


class SourceBindingSeparation(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.proof = self.root / "proof.rs"
        self.proof.write_bytes(b"synthetic proof source\n")
        (self.root / "Cargo.lock").write_bytes(b"synthetic lock metadata\n")
        self.catalog_path = self.root / "cases.json"
        self.catalog_path.write_bytes(b"synthetic reviewed catalog bytes\n")
        self.catalog = {"cases": [{
            "id": "binding-only",
            "source": "proof.rs",
            "recipe": {"content_utf8": ["synthetic payload"]},
            "expected": {"state": "declared_expectation"},
        }]}

    def index(self, catalog=None):
        with mock.patch.object(CORPUS, "ROOT", self.root), \
                mock.patch.object(CORPUS, "CATALOG", self.catalog_path), \
                mock.patch.object(CORPUS, "build_tests", side_effect=AssertionError("build forbidden")), \
                mock.patch.object(CORPUS, "execute", side_effect=AssertionError("execution forbidden")):
            return CORPUS.generated_index(catalog or self.catalog)

    def test_proof_source_refresh_changes_only_its_binding(self):
        before = self.index()
        self.proof.write_bytes(b"reviewed changed proof source\n")
        after = self.index()
        self.assertNotEqual(before["cases"][0]["proof_source_sha256"],
                            after["cases"][0]["proof_source_sha256"])
        before["cases"][0]["proof_source_sha256"] = after["cases"][0]["proof_source_sha256"]
        self.assertEqual(before, after)

    def test_lock_refresh_does_not_rewrite_recipe_or_expectations(self):
        before = self.index()
        (self.root / "Cargo.lock").write_bytes(b"reviewed changed lock metadata\n")
        after = self.index()
        self.assertNotEqual(before["cargo_lock_sha256"], after["cargo_lock_sha256"])
        before["cargo_lock_sha256"] = after["cargo_lock_sha256"]
        self.assertEqual(before, after)

    def test_expected_outcome_is_separate_from_proof_and_recipe(self):
        before = self.index()
        changed = copy.deepcopy(self.catalog)
        changed["cases"][0]["expected"]["state"] = "different_declared_expectation"
        after = self.index(changed)
        self.assertNotEqual(before["cases"][0]["expected_sha256"],
                            after["cases"][0]["expected_sha256"])
        before["cases"][0]["expected_sha256"] = after["cases"][0]["expected_sha256"]
        self.assertEqual(before, after)
        self.assertEqual(self.proof.read_bytes(), b"synthetic proof source\n")


if __name__ == "__main__":
    unittest.main()
