"""Contract tests for deterministic signed-release bundle preparation."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPOSITORY_ROOT / "scripts" / "release_bundle.py"
RELEASE_WORKFLOW = REPOSITORY_ROOT / ".github" / "workflows" / "release.yml"
SOURCE_REVISION = "a" * 40
CREATED_AT = "2026-09-12T00:00:00Z"
MINIMUM_SUPPORTED_RUST = "1.85.0"
RELAY_RELEASE_REVISION = "1eada5142f7fc7da7862f335589e3b8f5884ffaf"
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


def workflow_step_script(workflow: str, step_name: str) -> str:
    _, separator, remainder = workflow.partition(f"      - name: {step_name}\n")
    if not separator:
        raise AssertionError(f"release workflow has no {step_name} step")
    step, _, _ = remainder.partition("\n      - name: ")
    _, separator, script = step.partition("        run: |\n")
    if not separator:
        raise AssertionError(f"release workflow step {step_name} has no run block")
    return textwrap.dedent(script)


class ReleaseBundleTests(unittest.TestCase):
    def test_publish_job_pins_the_released_relay_contract(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        policy = (REPOSITORY_ROOT / "docs/release-policy.md").read_text(
            encoding="utf-8"
        )

        reference = (
            "egohygiene/relay/.github/workflows/release-artifact.yml@"
            f"{RELAY_RELEASE_REVISION}  # v1.5.0"
        )
        self.assertIn(reference, workflow)
        self.assertIn(
            f"Relay `v1.5.0`, pinned to `{RELAY_RELEASE_REVISION}`",
            policy,
        )

    def test_release_jobs_pin_the_installed_rust_toolchain(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        for job_name in ("build", "bundle"):
            with self.subTest(job=job_name):
                match = re.search(
                    rf"(?ms)^  {re.escape(job_name)}:\n"
                    rf"(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
                    workflow,
                )
                self.assertIsNotNone(
                    match, f"release workflow has no {job_name} job"
                )
                assert match is not None
                self.assertIn(
                    f'      RUSTUP_TOOLCHAIN: "{MINIMUM_SUPPORTED_RUST}"\n',
                    match.group("body"),
                )

    def test_release_smoke_test_is_valid_shell(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        completed = subprocess.run(
            ["bash", "--noprofile", "--norc", "-n"],
            input=workflow_step_script(
                workflow, "Smoke-test native release binary"
            ),
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)

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
