"""Authored/unrun synthetic refusals; never invokes hdiutil or mounts a volume."""
import os
from pathlib import Path
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import apfs_fault_trials as trial


class ImageFaultBoundaryTests(unittest.TestCase):
    root = Path("/synthetic/owned-trial")

    def info(self, image=None, mount=None):
        return {"images": [{"image-path": image or str(self.root / "trial.dmg"),
                            "system-entities": [{"mount-point": mount or str(self.root / "mount"), "dev-entry": "/dev/disk99s1"}]}]}

    def test_only_the_exact_owned_image_can_select_a_detach_mount(self):
        self.assertEqual(trial.matching_image(self.info(), self.root)["mount"], str(self.root / "mount"))
        self.assertIsNone(trial.matching_image(self.info(image="/Volumes/private-original.dmg"), self.root))
        self.assertIsNone(trial.matching_image({"images": []}, self.root))

    def test_foreign_and_ambiguous_mount_associations_refuse(self):
        with self.assertRaises(trial.Refusal):
            trial.native(self.root, "detach_force", ["detach", "-force", "/dev/disk0"])
        with self.assertRaises(trial.Refusal):
            trial.matching_image(self.info(mount="/Volumes/personal-drive"), self.root)
        value = self.info(); value["images"][0]["system-entities"] *= 2
        with self.assertRaises(trial.Refusal):
            trial.matching_image(value, self.root)
        value = self.info(); value["images"] *= 2
        with self.assertRaises(trial.Refusal):
            trial.matching_image(value, self.root)

    @unittest.skipUnless(hasattr(os, "O_NOFOLLOW"), "Unix file observation required")
    def test_existing_intent_is_never_replayed_or_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            trial.reserve(root, "disconnect", "a" * 64)
            original = (root / "disconnect-intent.json").read_bytes()
            with self.assertRaises(FileExistsError):
                trial.reserve(root, "disconnect", "a" * 64)
            self.assertEqual((root / "disconnect-intent.json").read_bytes(), original)

    @unittest.skipUnless(hasattr(os, "O_NOFOLLOW"), "Unix file observation required")
    def test_aliased_metadata_cannot_be_used_as_an_owned_trial_member(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); source = root / "source.json"; source.write_bytes(b"generated")
            alias = root / "alias.json"; alias.symlink_to(source)
            with self.assertRaises(trial.Refusal):
                trial.regular(alias, 4096)
            alias.unlink(); os.link(source, alias)
            with self.assertRaises(trial.Refusal):
                trial.regular(source, 4096)
            self.assertEqual(source.read_bytes(), b"generated")
