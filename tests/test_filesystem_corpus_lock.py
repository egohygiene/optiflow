"""Pure authoring checks for the lock-only companion; never load the runner."""

import copy
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest import mock


SPEC = importlib.util.spec_from_file_location(
    "filesystem_corpus_lock",
    Path(__file__).resolve().parents[1] / "scripts/update-filesystem-corpus-lock.py",
)
HELPER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HELPER)


def fixture():
    return {
        "schema": "optiflow.filesystem-corpus-index.v1",
        "license": "MIT",
        "cargo_lock_sha256": "a" * 64,
        "catalog_sha256": "b" * 64,
        "generator_sha256": "c" * 64,
        "cases": [{
            "id": "synthetic",
            "recipe_sha256": "d" * 64,
            "expected_sha256": "e" * 64,
            "proof_source_sha256": "f" * 64,
            "payload_sha256": ["0" * 64],
        }],
    }


class LockBindingAuthoring(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.path = Path(self.temporary.name) / "index.json"
        self.current = fixture()
        self.original = HELPER.canonical(self.current)
        self.path.write_bytes(self.original)
        self.expected = copy.deepcopy(self.current)
        self.expected[HELPER.LOCK] = "1" * 64

    def test_default_proposes_without_writing(self):
        state, old, new = HELPER.refresh_index(self.path, self.expected)
        self.assertEqual((state, old, new), ("proposal", "a" * 64, "1" * 64))
        self.assertEqual(self.path.read_bytes(), self.original)

    def test_explicit_write_changes_only_lock_bytes(self):
        state, _, _ = HELPER.refresh_index(self.path, self.expected, write=True)
        self.assertEqual(state, "updated")
        self.assertEqual(self.path.read_bytes(),
                         self.original.replace(b"a" * 64, b"1" * 64, 1))
        self.assertEqual(list(self.path.parent.iterdir()), [self.path])

    def test_current_binding_is_noop_even_with_write(self):
        with mock.patch.object(HELPER.os, "replace") as replace:
            state, _, _ = HELPER.refresh_index(self.path, self.current, write=True)
        self.assertEqual(state, "unchanged")
        replace.assert_not_called()
        self.assertEqual(self.path.read_bytes(), self.original)

    def assert_refused_without_writes(self, expected):
        before = self.path.read_bytes()
        with self.assertRaises(ValueError):
            HELPER.refresh_index(self.path, expected, write=True)
        self.assertEqual(self.path.read_bytes(), before)
        self.assertEqual(list(self.path.parent.iterdir()), [self.path])

    def test_every_nonlock_field_is_refused(self):
        variants = []
        for key in ("catalog_sha256", "generator_sha256", "schema", "license"):
            changed = copy.deepcopy(self.expected)
            changed[key] = "2" * 64
            variants.append(changed)
        for key in ("expected_sha256", "proof_source_sha256", "recipe_sha256", "id"):
            changed = copy.deepcopy(self.expected)
            changed["cases"][0][key] = "2" * 64
            variants.append(changed)
        changed = copy.deepcopy(self.expected)
        changed["cases"][0]["payload_sha256"] = ["2" * 64]
        variants.extend([changed, dict(self.expected, cases=[]),
                         dict(self.expected, unexpected=True)])
        for variant in variants:
            with self.subTest(variant=variant):
                self.assert_refused_without_writes(variant)

    def test_missing_keys_and_invalid_old_or_new_digest_are_refused(self):
        for key in HELPER.INDEX_KEYS:
            for document in ("existing", "generated"):
                with self.subTest(key=key, document=document):
                    current, expected = fixture(), copy.deepcopy(self.expected)
                    del (current if document == "existing" else expected)[key]
                    self.path.write_bytes(HELPER.canonical(current))
                    self.assert_refused_without_writes(expected)
        for value in (None, 7, "short", "A" * 64, "g" * 64):
            for document in ("existing", "generated"):
                with self.subTest(value=value, document=document):
                    current, expected = fixture(), copy.deepcopy(self.expected)
                    (current if document == "existing" else expected)[HELPER.LOCK] = value
                    self.path.write_bytes(HELPER.canonical(current))
                    self.assert_refused_without_writes(expected)

    def test_malformed_duplicate_and_noncanonical_json_are_refused(self):
        documents = [b"{", b"\xff", b"null", b'{"cases": [], "cases": []}',
                     b'{"cases": [{"id": "a", "id": "b"}]}',
                     b'{"cases": NaN}', self.original.rstrip(b"\n")]
        for document in documents:
            with self.subTest(document=document):
                self.path.write_bytes(document)
                self.assert_refused_without_writes(self.expected)

    def test_missing_index_is_not_created(self):
        self.path.unlink()
        with self.assertRaises(FileNotFoundError):
            HELPER.refresh_index(self.path, self.expected, write=True)
        self.assertFalse(self.path.exists())

    def test_symlink_index_is_refused_without_changing_target(self):
        target = self.path.with_name("target.json")
        self.path.rename(target)
        self.path.symlink_to(target)
        with self.assertRaisesRegex(ValueError, "regular file"):
            HELPER.refresh_index(self.path, self.expected, write=True)
        self.assertEqual(target.read_bytes(), self.original)

    def test_concurrent_index_edit_is_retained_and_temporary_is_removed(self):
        read_index = HELPER.read_index
        changed = self.original + b"\n"
        calls = 0

        def concurrent_read(path):
            nonlocal calls
            calls += 1
            if calls == 2:
                path.write_bytes(changed)
            return read_index(path)

        with mock.patch.object(HELPER, "read_index", side_effect=concurrent_read):
            with self.assertRaisesRegex(ValueError, "index changed"):
                HELPER.refresh_index(self.path, self.expected, write=True)
        self.assertEqual(self.path.read_bytes(), changed)
        self.assertEqual(list(self.path.parent.iterdir()), [self.path])

    def test_cli_uses_only_source_generation_and_does_not_write_by_default(self):
        catalog = self.path.with_name("cases.json")
        catalog.write_bytes(b'{"cases": []}')
        runner = mock.Mock(CATALOG=catalog)
        runner.generated_index.return_value = self.expected
        with mock.patch.object(HELPER, "load_generator", return_value=runner), \
                mock.patch.object(HELPER, "INDEX", self.path), \
                mock.patch("sys.stdout", new_callable=io.StringIO) as output:
            self.assertEqual(HELPER.main([]), 0)
        self.assertIn("proposal:", output.getvalue())
        self.assertEqual(runner.method_calls, [mock.call.generated_index({"cases": []})])
        self.assertEqual(self.path.read_bytes(), self.original)

    def test_cli_refuses_duplicate_catalog_before_generation(self):
        catalog = self.path.with_name("cases.json")
        catalog.write_bytes(b'{"cases": [], "cases": []}')
        runner = mock.Mock(CATALOG=catalog)
        with mock.patch.object(HELPER, "load_generator", return_value=runner), \
                mock.patch.object(HELPER, "INDEX", self.path), \
                mock.patch("sys.stderr", new_callable=io.StringIO) as output:
            self.assertEqual(HELPER.main(["--write"]), 1)
        self.assertIn("refused:", output.getvalue())
        self.assertEqual(runner.method_calls, [])
        self.assertEqual(self.path.read_bytes(), self.original)


if __name__ == "__main__":
    unittest.main()
