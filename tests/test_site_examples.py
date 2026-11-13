"""Synthetic public-example mount regressions; no product, network or deploy."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts/site"))
from stage_examples import MAX_EXAMPLE_BYTES, PUBLIC_EXAMPLES, stage
from verify import verify_examples


class PublicExampleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.source = self.root / "examples"
        self.source.mkdir()
        self.stage = self.root / ".site/staging/examples"
        self.stage.parent.mkdir(parents=True)
        for name in PUBLIC_EXAMPLES:
            (self.source / name).write_bytes(("synthetic public " + name).encode())

    def test_only_allowlisted_regular_bytes_reach_public_mount(self):
        (self.source / "PRIVATE-unreviewed.json").write_bytes(b"PRIVATE_CANARY")
        stage(self.root, self.stage)
        self.assertEqual({path.name for path in self.stage.iterdir()}, set(PUBLIC_EXAMPLES))
        for name in PUBLIC_EXAMPLES:
            self.assertEqual((self.stage / name).read_bytes(), (self.source / name).read_bytes())
        self.assertEqual(verify_examples(self.stage.parent), [])
        self.assertFalse(any(b"PRIVATE_CANARY" in path.read_bytes() for path in self.stage.iterdir()))
        with self.assertRaises(FileExistsError):
            stage(self.root, self.stage)

    def test_aliases_and_oversize_are_refused_before_output_creation(self):
        selected = self.source / PUBLIC_EXAMPLES[0]
        private = self.root / "private.json"
        private.write_bytes(b"PRIVATE_CANARY")
        selected.unlink()
        selected.symlink_to(private)
        with self.assertRaises(ValueError):
            stage(self.root, self.stage)
        self.assertFalse(self.stage.exists())
        selected.unlink()
        os.link(private, selected)
        with self.assertRaises(ValueError):
            stage(self.root, self.stage)
        self.assertFalse(self.stage.exists())
        selected.unlink()
        selected.write_bytes(b"x" * (MAX_EXAMPLE_BYTES + 1))
        with self.assertRaises(ValueError):
            stage(self.root, self.stage)
        self.assertFalse(self.stage.exists())

    def test_verifier_refuses_missing_extra_and_aliased_public_assets(self):
        stage(self.root, self.stage)
        selected = self.stage / PUBLIC_EXAMPLES[0]
        selected.unlink()
        self.assertIn("allowlist", " ".join(verify_examples(self.stage.parent)))
        selected.write_bytes(b"synthetic")
        extra = self.stage / "private.json"
        extra.write_bytes(b"PRIVATE_CANARY")
        self.assertIn("allowlist", " ".join(verify_examples(self.stage.parent)))
        extra.unlink()
        selected.unlink()
        selected.symlink_to(self.source / PUBLIC_EXAMPLES[0])
        self.assertIn("unsafe public example", " ".join(verify_examples(self.stage.parent)))

    def test_collection_budget_refuses_individually_small_files(self):
        for name in PUBLIC_EXAMPLES:
            (self.source / name).write_bytes(b"x" * 150_000)
        with self.assertRaises(ValueError):
            stage(self.root, self.stage)
        self.assertFalse(self.stage.exists())
        self.stage.mkdir()
        for name in PUBLIC_EXAMPLES:
            (self.stage / name).write_bytes(b"x" * 150_000)
        self.assertIn("byte limit", " ".join(verify_examples(self.stage.parent)))

    def test_landing_mount_collision_preserves_the_existing_dist(self):
        scripts = self.root / "scripts/site"
        scripts.mkdir(parents=True)
        for name in ["build.sh", "stage_examples.py"]:
            shutil.copyfile(ROOT / "scripts/site" / name, scripts / name)
        landing = self.root / "web/landing"
        landing.mkdir(parents=True)
        (landing / "index.html").write_text("synthetic")
        (landing / "examples").mkdir()
        architecture = self.root / "web/architecture"
        (architecture / "assets").mkdir(parents=True)
        (architecture / "architecture.config.json").write_text("{}")
        (self.root / "schemas").mkdir()
        dist = self.root / "dist"
        dist.mkdir()
        (dist / "index.html").write_text("retained artifact")
        result = subprocess.run(["bash", str(scripts / "build.sh")], cwd=self.root,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"reserved /examples/ mount", result.stderr)
        self.assertEqual((dist / "index.html").read_text(), "retained artifact")


if __name__ == "__main__":
    unittest.main()
