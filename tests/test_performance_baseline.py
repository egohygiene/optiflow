"""Regression tests for the shared-runner performance measurement policy."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path
from types import ModuleType


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPOSITORY_ROOT / "scripts" / "run-performance-baseline.py"
BUDGETS = REPOSITORY_ROOT / "performance" / "budgets-v1.json"


def load_baseline_module() -> ModuleType:
    """Load the hyphenated performance script as a testable module."""
    specification = importlib.util.spec_from_file_location(
        "optiflow_performance_baseline",
        SCRIPT,
    )
    if specification is None or specification.loader is None:
        raise RuntimeError("could not load the performance baseline module")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


BASELINE = load_baseline_module()


def measurement(
    elapsed_seconds: float,
    *,
    analyzed_files: int,
    cache_hits: int,
    exact_duplicate_groups: int,
    artifact_bytes: int,
) -> dict[str, int | float]:
    """Build one synthetic scenario measurement."""
    return {
        "elapsed_seconds": elapsed_seconds,
        "analyzed_files": analyzed_files,
        "cache_hits": cache_hits,
        "exact_duplicate_groups": exact_duplicate_groups,
        "artifact_file_count": 4,
        "artifact_bytes": artifact_bytes,
        "artifact_bytes_per_observation": 1_000,
    }


def trial(discovery_seconds: float, artifact_bytes: int) -> dict[str, dict]:
    """Build a complete synthetic trial with valid correctness evidence."""
    return {
        "discovery_cold": measurement(
            discovery_seconds,
            analyzed_files=1_000,
            cache_hits=0,
            exact_duplicate_groups=0,
            artifact_bytes=artifact_bytes,
        ),
        "hash_cold": measurement(
            1.0,
            analyzed_files=32,
            cache_hits=0,
            exact_duplicate_groups=8,
            artifact_bytes=artifact_bytes,
        ),
        "hash_warm": measurement(
            0.5,
            analyzed_files=32,
            cache_hits=32,
            exact_duplicate_groups=8,
            artifact_bytes=artifact_bytes,
        ),
    }


class PerformanceBaselineTests(unittest.TestCase):
    def test_isolated_wall_time_outlier_is_visible_but_does_not_fail(self) -> None:
        trials = [trial(1.0, 100), trial(40.0, 120), trial(2.0, 110)]

        scenarios = BASELINE.aggregate_scenarios(trials)
        discovery = scenarios["discovery_cold"]

        self.assertEqual(discovery["elapsed_seconds"], 2.0)
        self.assertEqual(discovery["elapsed_seconds_samples"], [1.0, 40.0, 2.0])
        self.assertEqual(discovery["artifact_bytes"], 120)
        self.assertEqual(discovery["artifact_bytes_samples"], [100, 120, 110])
        self.assertEqual(
            BASELINE.evaluate(
                scenarios,
                measured_peak_rss_bytes=1,
                budgets=BASELINE.load_budgets(BUDGETS)["budgets"],
            ),
            [],
        )

    def test_sustained_wall_time_regression_still_fails(self) -> None:
        scenarios = BASELINE.aggregate_scenarios(
            [trial(16.0, 100), trial(18.0, 100), trial(20.0, 100)]
        )

        violations = BASELINE.evaluate(
            scenarios,
            measured_peak_rss_bytes=1,
            budgets=BASELINE.load_budgets(BUDGETS)["budgets"],
        )

        self.assertEqual(
            violations,
            ["discovery_cold.elapsed_seconds measured 18.0, maximum 15.0"],
        )

    def test_even_trial_count_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "positive odd trial count"):
            BASELINE.aggregate_scenarios([trial(1.0, 100), trial(2.0, 100)])

    def test_cross_trial_correctness_drift_is_rejected(self) -> None:
        trials = [trial(1.0, 100), trial(2.0, 100), trial(3.0, 100)]
        trials[1]["hash_warm"]["cache_hits"] = 31

        with self.assertRaisesRegex(RuntimeError, "cache_hits differed"):
            BASELINE.aggregate_scenarios(trials)

    def test_report_retains_every_independent_trial(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "fake-optiflow"
            output = root / "performance.json"
            binary.write_text(
                textwrap.dedent(
                    """\
                    #!/usr/bin/env python3
                    import json
                    import sys
                    from pathlib import Path

                    if "--version" in sys.argv:
                        print("optiflow 0.1.0")
                        raise SystemExit(0)

                    state = Path(sys.argv[sys.argv.index("--state-directory") + 1])
                    collection = Path(sys.argv[-1])
                    files = list(collection.iterdir())
                    seen = state / "seen"
                    warm = seen.exists()
                    seen.parent.mkdir(parents=True, exist_ok=True)
                    seen.write_text("seen", encoding="utf-8")
                    artifact = state / "runs" / "synthetic" / "report.json"
                    artifact.parent.mkdir(parents=True, exist_ok=True)
                    artifact.write_text("{}", encoding="utf-8")
                    hashing = collection.name == "hash-collection"
                    print(json.dumps({
                        "schema": "optiflow.command-result.v1",
                        "outcome": {"exit_code": 0},
                        "result": {
                            "run": {"run_id": "synthetic"},
                            "summary": {
                                "file_count": len(files),
                                "cache_hits": len(files) if warm else 0,
                                "exact_duplicate_groups": 8 if hashing else 0,
                            },
                        },
                    }))
                    """
                ),
                encoding="utf-8",
            )
            binary.chmod(0o755)

            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--binary",
                    str(binary),
                    "--budgets",
                    str(BUDGETS),
                    "--output",
                    str(output),
                    "--enforce",
                ],
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(completed.returncode, 0, completed.stderr)
            report = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(report["schema"], "optiflow.performance-baseline.v2")
            self.assertEqual(report["sampling"]["trial_count"], 3)
            self.assertEqual(
                report["sampling"]["elapsed_seconds_statistic"],
                "median",
            )
            self.assertEqual(len(report["measurements"]["trials"]), 3)
            samples = report["measurements"]["scenarios"]["discovery_cold"][
                "elapsed_seconds_samples"
            ]
            self.assertEqual(len(samples), 3)


if __name__ == "__main__":
    unittest.main()
