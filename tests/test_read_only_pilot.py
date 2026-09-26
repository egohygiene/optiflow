"""Safety regressions for archive installation and pilot receipt admission."""
import copy
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import pilot_contract as pilot
from tests.test_release_bundle import fixture_binaries, SOURCE_REVISION, TARGETS


class PilotAdmission(unittest.TestCase):
    def test_incomplete_or_mutating_trials_cannot_qualify(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = fixture_binaries(Path(temporary)) / f"optiflow-{TARGETS[0]}"
            archive = directory / f"optiflow-v0.1.1-{TARGETS[0]}.tar.gz"
            valid = json.loads((directory / "qualification.json").read_text())
            changes = [
                ("plan", "mutates_files", True), ("partial", "coverage", "complete"),
                ("interrupted", "run_status", "completed"), ("restart", "new_run", False),
                ("tree_cold", "files", 4), ("tree_warm", "cache_hits", 0),
                ("large_cold", "logical_bytes", 10), ("large_warm", "cache_hits", 0),
                ("rollback", "exit_code", 5), ("upgrade", "outcome", "stale_state"),
                ("report", "elapsed_seconds", float("nan")),
            ]
            for name, key, value in changes:
                with self.subTest(name=name, key=key), self.assertRaises(ValueError):
                    receipt = copy.deepcopy(valid)
                    receipt["scenarios"][name][key] = value
                    pilot.validate_receipt(receipt, TARGETS[0], "v0.1.1", SOURCE_REVISION, archive)

    def test_archive_path_traversal_and_links_are_refused_before_extraction(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for bad_name, kind in [("../optiflow", tarfile.REGTYPE), ("optiflow", tarfile.SYMTYPE)]:
                archive = root / "bad.tar.gz"
                with tarfile.open(archive, "w:gz") as output:
                    for name in ["LICENSE", bad_name]:
                        item = tarfile.TarInfo(name)
                        item.mode = 0o644 if name == "LICENSE" else 0o755
                        item.size = 4
                        item.type = tarfile.REGTYPE if name == "LICENSE" else kind
                        item.linkname = "../outside"
                        output.addfile(item, io.BytesIO(b"test"))
                destination = root / "installed"
                with self.assertRaises(ValueError):
                    pilot.archive_binary(archive, destination)
                self.assertFalse(destination.exists())

    def test_installed_bytes_match_the_qualified_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = fixture_binaries(root) / f"optiflow-{TARGETS[0]}"
            archive = directory / f"optiflow-v0.1.1-{TARGETS[0]}.tar.gz"
            installed = root / "installed"
            expected = pilot.archive_binary(archive, installed)
            self.assertEqual(expected, pilot.sha256(installed / "optiflow"))
            self.assertEqual((installed / "optiflow").stat().st_mode & 0o777, 0o755)


if __name__ == "__main__":
    unittest.main()
