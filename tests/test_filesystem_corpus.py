"""Prove resource gates fail closed without allocating large test data."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import time
import unittest

SPEC = importlib.util.spec_from_file_location("filesystem_corpus", Path(__file__).resolve().parents[1] / "scripts/filesystem-corpus.py")
CORPUS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CORPUS)


class CorpusBudgets(unittest.TestCase):
    def invoke(self, code, **overrides):
        budget = dict(memory_bytes=256 * 1024 * 1024, file_bytes=65536,
                      workspace_bytes=65536, evidence_bytes=65536, case_seconds=5)
        budget.update(overrides)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            work = root / "work"
            evidence = root / "evidence"
            work.mkdir()
            evidence.mkdir()
            return CORPUS.run_bounded([sys.executable, "-c", code], work,
                                      evidence / "test.log", budget,
                                      time.monotonic() + 5, evidence)

    def test_normal_child_finishes(self):
        self.assertIn("elapsed_seconds", self.invoke("print('bounded')"))

    def test_deadline_kills_child(self):
        with self.assertRaisesRegex(RuntimeError, "wall_time_budget"):
            self.invoke("import time; time.sleep(5)", case_seconds=0.05)

    def test_memory_limit_is_inherited(self):
        with self.assertRaisesRegex(RuntimeError, "test_exit_|memory_budget"):
            self.invoke("import time; data = bytearray(512 * 1024 * 1024); time.sleep(1)")

    def test_per_file_limit_is_enforced(self):
        with self.assertRaisesRegex(RuntimeError, "test_exit_"):
            self.invoke("import os; open(os.environ['TMPDIR'] + '/large', 'wb').write(b'x' * 8192)", file_bytes=1024)

    def test_aggregate_workspace_limit_is_enforced(self):
        with self.assertRaisesRegex(RuntimeError, "workspace_budget"):
            self.invoke("import os; open(os.environ['TMPDIR'] + '/data', 'wb').write(b'x' * 256)", workspace_bytes=128)

    def test_evidence_limit_is_enforced(self):
        with self.assertRaisesRegex(RuntimeError, "evidence_budget"):
            self.invoke("print('x' * 256)", evidence_bytes=128)

    def test_duplicate_fixture_ids_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "duplicate corpus fixture ID"):
            CORPUS.generated_index({"cases": [{"id": "duplicate"}] * 2})


if __name__ == "__main__":
    unittest.main()
