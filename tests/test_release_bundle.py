"""Contract tests for deterministic signed-release bundle preparation."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPOSITORY_ROOT / "scripts" / "release_bundle.py"
SOURCE_REVISION = "a" * 40
CREATED_AT = "2026-09-12T00:00:00Z"
TARGETS = (
    "x86_64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
)


def run_bundle(
    *arguments: str, expect_success: bool = True
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        [sys.executable, str(SCRIPT), *arguments],
        cwd=REPOSITORY_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if expect_success and completed.returncode != 0:
        raise AssertionError(completed.stderr)
    return completed


def fixture_binaries(root: Path) -> Path:
    inputs = root / "input"
    for target in TARGETS:
        directory = inputs / f"optiflow-{target}"
        directory.mkdir(parents=True)
        (directory / "optiflow").write_bytes(
            f"synthetic optiflow for {target}\n".encode()
        )
    return inputs


def prepare(root: Path, name: str) -> Path:
    inputs = fixture_binaries(root / name)
    output = root / name / "bundle"
    run_bundle(
        "prepare",
        "--repository-root",
        str(REPOSITORY_ROOT),
        "--input-directory",
        str(inputs),
        "--output-directory",
        str(output),
        "--release-version",
        "v0.1.0",
        "--source-revision",
        SOURCE_REVISION,
        "--created-at",
        CREATED_AT,
    )
    (output / "signature.json").write_text(
        json.dumps({"synthetic": True}, sort_keys=True) + "\n", encoding="utf-8"
    )
    run_bundle("finalize", "--bundle-directory", str(output))
    return output


def tree_digests(directory: Path) -> dict[str, str]:
    return {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(directory.iterdir())
    }


class ReleaseBundleTests(unittest.TestCase):
    def test_preparation_is_byte_reproducible(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first = prepare(root, "first")
            second = prepare(root, "second")
            self.assertEqual(tree_digests(first), tree_digests(second))
            sbom = json.loads((first / "sbom.spdx.json").read_text(encoding="utf-8"))
            self.assertTrue(sbom["packages"])
            package_names = {package["name"] for package in sbom["packages"]}
            self.assertIn("optiflow", package_names)
            self.assertNotIn("proptest", package_names)
            self.assertTrue(
                all(
                    "/" not in package["licenseDeclared"]
                    for package in sbom["packages"]
                )
            )

    def test_complete_bundle_verifies_without_external_signature_check(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            bundle = prepare(Path(temporary), "verified")
            run_bundle(
                "verify",
                "--repository-root",
                str(REPOSITORY_ROOT),
                "--bundle-directory",
                str(bundle),
                "--release-version",
                "v0.1.0",
                "--source-revision",
                SOURCE_REVISION,
                "--skip-signature-verification",
            )

    def test_tampered_archive_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            bundle = prepare(Path(temporary), "tampered")
            archive = bundle / "optiflow-v0.1.0-x86_64-unknown-linux-gnu.tar.gz"
            archive.write_bytes(archive.read_bytes() + b"tampered")
            completed = run_bundle(
                "verify",
                "--repository-root",
                str(REPOSITORY_ROOT),
                "--bundle-directory",
                str(bundle),
                "--release-version",
                "v0.1.0",
                "--source-revision",
                SOURCE_REVISION,
                "--skip-signature-verification",
                expect_success=False,
            )
            self.assertEqual(completed.returncode, 2)
            self.assertIn("checksum mismatch", completed.stderr)

    def test_manifest_version_is_the_release_authority(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            completed = run_bundle(
                "prepare",
                "--repository-root",
                str(REPOSITORY_ROOT),
                "--input-directory",
                str(fixture_binaries(root)),
                "--output-directory",
                str(root / "bundle"),
                "--release-version",
                "v0.1.1",
                "--source-revision",
                SOURCE_REVISION,
                "--created-at",
                CREATED_AT,
                expect_success=False,
            )
            self.assertEqual(completed.returncode, 2)
            self.assertIn("does not match Cargo.toml version", completed.stderr)


if __name__ == "__main__":
    unittest.main()
