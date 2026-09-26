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
sys.path.insert(0, str(REPOSITORY_ROOT / "scripts"))
import pilot_contract as pilot
import release_bundle as release
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
        archive = directory / f"optiflow-v0.1.1-{target}.tar.gz"
        release.deterministic_archive(directory / "optiflow", REPOSITORY_ROOT / "LICENSE", archive)
        scenarios = {name: {"exit_code": code, "outcome": outcome, "elapsed_seconds": 0.1}
                     for name, (code, outcome) in pilot.OUTCOMES.items()}
        scenarios["tree_cold"]["files"] = 4096
        scenarios["tree_warm"]["cache_hits"] = 4096
        scenarios["large_cold"].update(logical_bytes=536870912, groups=1)
        scenarios["large_warm"]["cache_hits"] = 2
        scenarios["plan"]["mutates_files"] = False
        scenarios["partial"]["coverage"] = "partial"
        scenarios["interrupted"]["run_status"] = "interrupted"
        scenarios["restart"]["new_run"] = True
        receipt = {"schema": pilot.SCHEMA, "target": target, "native_target": target,
                   "release_version": "v0.1.1", "source_revision": SOURCE_REVISION,
                   "profile": pilot.PROFILE, "scenarios": scenarios, "passed": True,
                   "source_unchanged": True, "cleanup_complete": True, "json_stdout_clean": True,
                   "previous_signature_verified": True, "previous_version": pilot.PREVIOUS_VERSION,
                   "previous_revision": pilot.PREVIOUS_REVISION, "source_digest": "b" * 64,
                   "peak_child_rss_bytes": 12345, "state_bytes": 6789,
                   "archive_sha256": release.sha256(archive), "binary_sha256": pilot.archive_binary(archive)}
        release.write_json(directory / "qualification.json", receipt)
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
        "v0.1.1",
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
    def test_failed_or_unbound_native_qualification_cannot_be_signed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            inputs = fixture_binaries(Path(temporary))
            target = TARGETS[0]
            directory = inputs / f"optiflow-{target}"
            archive = directory / f"optiflow-v0.1.1-{target}.tar.gz"
            valid = json.loads((directory / "qualification.json").read_text())
            for field, bad in [("passed", False), ("native_target", TARGETS[1]),
                               ("source_revision", "c" * 40), ("archive_sha256", "d" * 64),
                               ("binary_sha256", "e" * 64), ("previous_signature_verified", False),
                               ("cleanup_complete", "true"), ("scenarios", {})]:
                with self.subTest(field=field), self.assertRaises(ValueError):
                    pilot.validate_receipt({**valid, field: bad}, target, "v0.1.1", SOURCE_REVISION, archive)

    def test_missing_qualification_blocks_bundle_preparation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = fixture_binaries(root)
            (inputs / f"optiflow-{TARGETS[1]}" / "qualification.json").unlink()
            result = run_bundle("prepare", "--repository-root", str(REPOSITORY_ROOT),
                                "--input-directory", str(inputs), "--output-directory", str(root / "bundle"),
                                "--release-version", "v0.1.1", "--source-revision", SOURCE_REVISION,
                                "--created-at", CREATED_AT, expect_success=False)
            self.assertEqual(result.returncode, 2)

    def test_native_targets_cannot_skip_installation_trials(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text()
        self.assertIn("os: macos-15-intel", workflow)
        self.assertNotIn("native smoke test not applicable", workflow)
        self.assertIn("scripts/qualify-read-only-pilot.py", workflow)
        self.assertIn("dist/qualified/${{ matrix.target }}", workflow)

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
                workflow, "Require native execution and fetch immutable rollback baseline"
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
                "v0.1.1",
                "--source-revision",
                SOURCE_REVISION,
                "--skip-signature-verification",
            )

    def test_tampered_archive_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            bundle = prepare(Path(temporary), "tampered")
            archive = bundle / "optiflow-v0.1.1-x86_64-unknown-linux-gnu.tar.gz"
            archive.write_bytes(archive.read_bytes() + b"tampered")
            completed = run_bundle(
                "verify",
                "--repository-root",
                str(REPOSITORY_ROOT),
                "--bundle-directory",
                str(bundle),
                "--release-version",
                "v0.1.1",
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
                "v0.1.2",
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
