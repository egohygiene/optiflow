"""The smoke runner must refuse unsafe topology before it creates fixtures."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "smoke-exact-transaction.py"


class SyntheticTransactionSmokeTests(unittest.TestCase):
    def test_same_filesystem_refuses_before_creating_anything(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "synthetic-binary"
            binary.write_bytes(b"not executed")
            work, quarantine = root / "work", root / "quarantine"
            work.mkdir()
            quarantine.mkdir()
            result = subprocess.run(
                [sys.executable, str(SCRIPT), "--binary", str(binary),
                 "--work-root", str(work), "--cross-quarantine-root", str(quarantine),
                 "--output", str(root / "receipt.json")],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 2)
            self.assertIn("distinct filesystem", result.stderr)
            self.assertEqual(list(work.iterdir()), [])
            self.assertEqual(list(quarantine.iterdir()), [])
            self.assertFalse((root / "receipt.json").exists())


if __name__ == "__main__":
    unittest.main()
