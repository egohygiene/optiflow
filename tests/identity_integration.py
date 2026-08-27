# Copyright 2026 Ego Hygiene
# SPDX-License-Identifier: MIT

"""Evidence for OptiFlow's immutable, product-distinct Identity v1 consumer."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import unittest

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
IDENTITY_ROOT = REPOSITORY_ROOT / "identity"
LOCK_PATH = REPOSITORY_ROOT / ".config/identity/consumer-lock.json"
PROJECT_PATH = REPOSITORY_ROOT / ".identity/identity.json"
ORGANIZATION_DEFAULT_SHA256 = "6098e60eaab67887c597327e55da646685443eeddc1e27188beab4a1311e36aa"


class IdentityIntegrationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.lock = json.loads(LOCK_PATH.read_text(encoding="utf-8"))
        self.project = json.loads(PROJECT_PATH.read_text(encoding="utf-8"))

    def test_immutable_identity_contract_is_explicit(self) -> None:
        self.assertEqual(self.lock["schema"], "identity.consumer-lock/v1")
        self.assertEqual(self.lock["consumer"], "egohygiene/optiflow")
        self.assertEqual(self.lock["repository"], "egohygiene/identity")
        self.assertEqual(self.lock["revision_kind"], "git-commit")
        self.assertRegex(self.lock["revision"], r"^[0-9a-f]{40}$")
        self.assertIn('path = identity', (REPOSITORY_ROOT / ".gitmodules").read_text())

    def test_only_reviewed_product_differences_override_shared_defaults(self) -> None:
        self.assertEqual(self.project["schema"], "identity.project/v1")
        self.assertEqual(self.project["project"]["id"], "optiflow")
        layers = self.project["layers"]
        self.assertEqual([layer["kind"] for layer in layers], ["organization-defaults", "product-override"])
        self.assertEqual(layers[0]["sha256"], ORGANIZATION_DEFAULT_SHA256)
        self.assertEqual(
            hashlib.sha256(
                (REPOSITORY_ROOT / layers[0]["tokens"]).read_bytes()
            ).hexdigest(),
            ORGANIZATION_DEFAULT_SHA256,
        )
        overrides = json.loads(
            (REPOSITORY_ROOT / layers[1]["tokens"]).read_text(encoding="utf-8")
        )
        primary = overrides["color"]["brand"]["primary"]
        self.assertEqual(primary["$value"]["components"], [0, 0.51, 0.62])
        self.assertEqual(
            primary["$extensions"]["org.egohygiene.identity"]["override"]["approval"],
            "approve-optiflow-primary",
        )
        self.assertEqual(overrides["color"]["action"]["primary"]["$value"], "{color.brand.primary}")

    def test_selected_profiles_match_the_current_cli_product_surface(self) -> None:
        profiles = json.loads(
            (REPOSITORY_ROOT / ".identity/targets/profiles.json").read_text(encoding="utf-8")
        )
        self.assertEqual(profiles["schema"], "identity.targets/v1")
        self.assertEqual(
            [profile["id"] for profile in profiles["enabled"]],
            ["core", "github", "docs", "tokens", "metadata", "archive"],
        )
        self.assertEqual(profiles["inapplicable"], ["web", "pwa", "social"])
        self.assertTrue(all(profile["version"] == "1.0.0" for profile in profiles["enabled"]))

    @unittest.skipUnless((IDENTITY_ROOT / "Cargo.toml").is_file(), "Identity submodule is not initialized")
    def test_pinned_v1_validator_and_compiler_detect_no_generated_state_drift(self) -> None:
        validator = IDENTITY_ROOT / "scripts/validate_identity.py"
        subprocess.run(
            ["python3", str(validator), "--repository-root", str(REPOSITORY_ROOT)],
            check=True,
        )
        subprocess.run(
            [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                str(IDENTITY_ROOT / "Cargo.toml"),
                "--",
                "v1-verify",
                "--repository-root",
                str(REPOSITORY_ROOT),
            ],
            check=True,
        )

    def test_no_consumer_owned_identity_implementation_exists(self) -> None:
        self.assertFalse((REPOSITORY_ROOT / "src/identity").exists())
        self.assertEqual(self.lock["path"], "identity")


if __name__ == "__main__":
    unittest.main()
