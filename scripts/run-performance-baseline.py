#!/usr/bin/env python3
"""Measure and enforce optiflow's synthetic read-only performance budgets."""

from __future__ import annotations

import argparse
import json
import math
import platform
import resource
import statistics
import subprocess
import sys
import tempfile
import time
from datetime import UTC, datetime
from pathlib import Path
from typing import Any


BUDGET_SCHEMA = "optiflow.performance-budgets.v1"
RESULT_SCHEMA = "optiflow.performance-baseline.v2"
MEASUREMENT_TRIAL_COUNT = 3
EXPECTED_FIXTURE_KEYS = {
    "discovery_file_count",
    "hash_group_count",
    "hash_members_per_group",
    "hash_file_bytes",
}
EXPECTED_BUDGET_KEYS = {
    "discovery_cold_max_seconds",
    "hash_cold_max_seconds",
    "hash_warm_max_seconds",
    "maximum_peak_rss_bytes",
    "maximum_artifact_bytes_per_observation",
}
EXPECTED_SCENARIO_KEYS = {
    "discovery_cold",
    "hash_cold",
    "hash_warm",
}


def parse_arguments() -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description="Run the deterministic optiflow performance fixture."
    )
    parser.add_argument(
        "--binary",
        type=Path,
        required=True,
        help="Path to the release-mode optiflow binary.",
    )
    parser.add_argument(
        "--budgets",
        type=Path,
        required=True,
        help="Path to the versioned performance-budget document.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Optional destination for the JSON measurement report.",
    )
    parser.add_argument(
        "--enforce",
        action="store_true",
        help="Return a failure status when any published budget is exceeded.",
    )
    return parser.parse_args()


def require_exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    """Reject missing or unknown keys in a small repository-owned contract."""
    actual = set(value)
    if actual != expected:
        missing = sorted(expected - actual)
        unknown = sorted(actual - expected)
        raise ValueError(f"{label} keys differ; missing={missing}, unknown={unknown}")


def load_budgets(path: Path) -> dict[str, Any]:
    """Load and validate the checked-in performance-budget document."""
    document = json.loads(path.read_text(encoding="utf-8"))
    require_exact_keys(document, {"schema", "fixture", "budgets"}, "document")
    if document["schema"] != BUDGET_SCHEMA:
        raise ValueError(f"unsupported budget schema: {document['schema']}")
    require_exact_keys(document["fixture"], EXPECTED_FIXTURE_KEYS, "fixture")
    require_exact_keys(document["budgets"], EXPECTED_BUDGET_KEYS, "budgets")

    for name, value in document["fixture"].items():
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            raise ValueError(f"fixture value {name} must be a positive integer")
    for name, value in document["budgets"].items():
        if not isinstance(value, (int, float)) or isinstance(value, bool) or value <= 0:
            raise ValueError(f"budget value {name} must be a positive number")
    return document


def create_discovery_fixture(root: Path, file_count: int) -> None:
    """Create files with unique sizes so discovery does not trigger hashing."""
    root.mkdir(parents=True)
    for index in range(file_count):
        size = index + 1
        content = bytes([(index % 251) + 1]) * size
        (root / f"unique-{index:05}.bin").write_bytes(content)


def create_hash_fixture(
    root: Path,
    group_count: int,
    members_per_group: int,
    file_bytes: int,
) -> None:
    """Create equal-sized duplicate groups that require complete hashing."""
    root.mkdir(parents=True)
    for group in range(group_count):
        content = bytes([(group % 251) + 1]) * file_bytes
        for member in range(members_per_group):
            (root / f"group-{group:03}-member-{member:03}.bin").write_bytes(content)


def run_scan(binary: Path, state: Path, collection: Path, timeout: float) -> dict[str, Any]:
    """Run one scan and return validated timing and artifact measurements."""
    command = [
        str(binary),
        "--state-directory",
        str(state),
        "--output-format",
        "json",
        "scan",
        "--no-probe",
        str(collection),
    ]
    started = time.perf_counter()
    completed = subprocess.run(
        command,
        check=False,
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    elapsed_seconds = time.perf_counter() - started
    if completed.returncode != 0:
        raise RuntimeError(
            "performance scan failed with "
            f"exit code {completed.returncode}: {completed.stderr.strip()}"
        )
    try:
        result = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError("performance scan did not emit valid JSON") from error
    if result.get("schema") != "optiflow.command-result.v1":
        raise RuntimeError("performance scan emitted an unexpected command-result schema")
    if result.get("outcome", {}).get("exit_code") != 0:
        raise RuntimeError("performance scan JSON did not report complete success")

    report = result.get("result")
    if not isinstance(report, dict):
        raise RuntimeError("performance scan omitted its report result")
    run = report.get("run")
    summary = report.get("summary")
    if not isinstance(run, dict) or not isinstance(summary, dict):
        raise RuntimeError("performance scan omitted run or summary evidence")
    run_id = run.get("run_id")
    analyzed_files = summary.get("file_count")
    if not isinstance(run_id, str) or not isinstance(analyzed_files, int):
        raise RuntimeError("performance scan emitted invalid run identity or file count")

    artifact_directory = state / "runs" / run_id
    artifact_files = [path for path in artifact_directory.rglob("*") if path.is_file()]
    artifact_bytes = sum(path.stat().st_size for path in artifact_files)
    bytes_per_observation = math.ceil(artifact_bytes / max(analyzed_files, 1))

    return {
        "elapsed_seconds": round(elapsed_seconds, 6),
        "analyzed_files": analyzed_files,
        "cache_hits": summary.get("cache_hits"),
        "exact_duplicate_groups": summary.get("exact_duplicate_groups"),
        "artifact_file_count": len(artifact_files),
        "artifact_bytes": artifact_bytes,
        "artifact_bytes_per_observation": bytes_per_observation,
    }


def peak_rss_bytes() -> int:
    """Normalize child-process peak resident memory for Linux and macOS."""
    maximum_rss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    if sys.platform == "darwin":
        return int(maximum_rss)
    return int(maximum_rss * 1024)


def binary_version(binary: Path) -> str:
    """Return the tested binary's public version string."""
    completed = subprocess.run(
        [str(binary), "--version"],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    return completed.stdout.strip()


def validate_scenarios(
    scenarios: dict[str, dict[str, Any]],
    fixture: dict[str, Any],
) -> None:
    """Validate the correctness evidence emitted by one independent trial."""
    require_exact_keys(scenarios, EXPECTED_SCENARIO_KEYS, "trial scenarios")
    expected_hash_files = fixture["hash_group_count"] * fixture["hash_members_per_group"]
    if scenarios["discovery_cold"]["analyzed_files"] != fixture["discovery_file_count"]:
        raise RuntimeError("discovery fixture file count did not round-trip")
    if scenarios["hash_cold"]["analyzed_files"] != expected_hash_files:
        raise RuntimeError("hash fixture file count did not round-trip")
    if scenarios["hash_cold"]["exact_duplicate_groups"] != fixture["hash_group_count"]:
        raise RuntimeError("hash fixture duplicate groups did not round-trip")
    if scenarios["hash_warm"]["cache_hits"] != expected_hash_files:
        raise RuntimeError("warm-cache fixture did not reuse every observation")


def aggregate_scenarios(
    trials: list[dict[str, dict[str, Any]]],
) -> dict[str, dict[str, Any]]:
    """Aggregate independent trials without hiding raw measurements."""
    if not trials or len(trials) % 2 == 0:
        raise ValueError("performance measurement requires a positive odd trial count")

    for trial in trials:
        require_exact_keys(trial, EXPECTED_SCENARIO_KEYS, "trial scenarios")

    scenarios = {}
    invariant_fields = (
        "analyzed_files",
        "cache_hits",
        "exact_duplicate_groups",
    )
    maximum_fields = (
        "artifact_file_count",
        "artifact_bytes",
        "artifact_bytes_per_observation",
    )
    for scenario_name in sorted(EXPECTED_SCENARIO_KEYS):
        samples = [trial[scenario_name] for trial in trials]
        aggregate = {}
        for field in invariant_fields:
            values = [sample[field] for sample in samples]
            if any(value != values[0] for value in values[1:]):
                raise RuntimeError(
                    f"{scenario_name}.{field} differed across trials: {values}"
                )
            aggregate[field] = values[0]

        elapsed_samples = [sample["elapsed_seconds"] for sample in samples]
        aggregate["elapsed_seconds"] = round(
            statistics.median(elapsed_samples),
            6,
        )
        aggregate["elapsed_seconds_samples"] = elapsed_samples
        for field in maximum_fields:
            values = [sample[field] for sample in samples]
            aggregate[field] = max(values)
            aggregate[f"{field}_samples"] = values
        scenarios[scenario_name] = aggregate
    return scenarios


def evaluate(
    scenarios: dict[str, dict[str, Any]],
    measured_peak_rss_bytes: int,
    budgets: dict[str, Any],
) -> list[str]:
    """Return every exceeded performance budget."""
    violations = []
    comparisons = [
        (
            "discovery_cold.elapsed_seconds",
            scenarios["discovery_cold"]["elapsed_seconds"],
            budgets["discovery_cold_max_seconds"],
        ),
        (
            "hash_cold.elapsed_seconds",
            scenarios["hash_cold"]["elapsed_seconds"],
            budgets["hash_cold_max_seconds"],
        ),
        (
            "hash_warm.elapsed_seconds",
            scenarios["hash_warm"]["elapsed_seconds"],
            budgets["hash_warm_max_seconds"],
        ),
        (
            "peak_rss_bytes",
            measured_peak_rss_bytes,
            budgets["maximum_peak_rss_bytes"],
        ),
    ]
    for scenario_name, scenario in scenarios.items():
        comparisons.append(
            (
                f"{scenario_name}.artifact_bytes_per_observation",
                scenario["artifact_bytes_per_observation"],
                budgets["maximum_artifact_bytes_per_observation"],
            )
        )
    for name, measured, maximum in comparisons:
        if measured > maximum:
            violations.append(f"{name} measured {measured}, maximum {maximum}")
    return violations


def main() -> int:
    """Generate fixtures, measure the release binary, and enforce budgets."""
    arguments = parse_arguments()
    document = load_budgets(arguments.budgets)
    binary = arguments.binary.resolve(strict=True)
    fixture = document["fixture"]
    budgets = document["budgets"]
    timeout = max(
        float(budgets["discovery_cold_max_seconds"]),
        float(budgets["hash_cold_max_seconds"]),
        float(budgets["hash_warm_max_seconds"]),
        30.0,
    ) * 3

    with tempfile.TemporaryDirectory(prefix="optiflow-performance-") as directory:
        workspace = Path(directory)
        discovery_collection = workspace / "discovery-collection"
        hash_collection = workspace / "hash-collection"
        create_discovery_fixture(
            discovery_collection,
            fixture["discovery_file_count"],
        )
        create_hash_fixture(
            hash_collection,
            fixture["hash_group_count"],
            fixture["hash_members_per_group"],
            fixture["hash_file_bytes"],
        )

        trials = []
        for trial_index in range(MEASUREMENT_TRIAL_COUNT):
            trial_workspace = workspace / f"trial-{trial_index + 1}"
            scenarios = {
                "discovery_cold": run_scan(
                    binary,
                    trial_workspace / "discovery-state",
                    discovery_collection,
                    timeout,
                ),
                "hash_cold": run_scan(
                    binary,
                    trial_workspace / "hash-state",
                    hash_collection,
                    timeout,
                ),
            }
            scenarios["hash_warm"] = run_scan(
                binary,
                trial_workspace / "hash-state",
                hash_collection,
                timeout,
            )
            validate_scenarios(scenarios, fixture)
            trials.append(scenarios)

    scenarios = aggregate_scenarios(trials)

    measured_peak_rss_bytes = peak_rss_bytes()
    violations = evaluate(scenarios, measured_peak_rss_bytes, budgets)
    report = {
        "schema": RESULT_SCHEMA,
        "generated_at": datetime.now(UTC).isoformat(),
        "binary_version": binary_version(binary),
        "host": {
            "operating_system": platform.system().lower(),
            "architecture": platform.machine().lower(),
        },
        "fixture": fixture,
        "budgets": budgets,
        "sampling": {
            "trial_count": MEASUREMENT_TRIAL_COUNT,
            "elapsed_seconds_statistic": "median",
            "artifact_size_statistic": "maximum",
            "peak_rss_statistic": "maximum",
        },
        "measurements": {
            "scenarios": scenarios,
            "peak_rss_bytes": measured_peak_rss_bytes,
            "trials": [
                {
                    "trial": index + 1,
                    "scenarios": trial,
                }
                for index, trial in enumerate(trials)
            ],
        },
        "violations": violations,
        "passed": not violations,
    }
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if arguments.output is not None:
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return 1 if arguments.enforce and violations else 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"performance baseline failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
