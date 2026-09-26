"""Repository-owned contract for the v0.1.1 native read-only release trial."""
import hashlib
import json
import platform
import re
import shutil
import tarfile
from pathlib import Path

SCHEMA = "optiflow.read-only-pilot.v1"
PREVIOUS_VERSION = "v0.1.0"
PREVIOUS_REVISION = "f04c82a0b0c677a2939ea351c4219602cd7181af"
PROFILE = {"seed": 89, "tree_files": 4096, "large_file_bytes": 268435456, "large_members": 2}
OUTCOMES = {
    "tree_cold": (0, "success"), "tree_warm": (0, "success"),
    "large_cold": (0, "success"), "large_warm": (0, "success"),
    "report": (0, "success"), "plan": (0, "success"),
    "partial": (3, "partial_success"), "disconnected": (2, "invalid_input"),
    "reconnected": (0, "success"), "interrupted": (130, "interrupted"),
    "restart": (0, "success"), "upgrade": (0, "success"), "rollback": (0, "success"),
}


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def native_target():
    architecture = platform.machine().lower()
    if platform.system() == "Linux" and architecture == "x86_64":
        return "x86_64-unknown-linux-gnu"
    if platform.system() == "Darwin" and architecture in ("x86_64", "arm64", "aarch64"):
        return ("x86_64" if architecture == "x86_64" else "aarch64") + "-apple-darwin"
    raise ValueError("pilot requires one of the three supported native hosts")


def archive_binary(archive, destination=None):
    """Validate the exact two-file install surface; never use extractall."""
    with tarfile.open(archive, "r:gz") as bundle:
        members = bundle.getmembers()
        if len(members) != 2 or {item.name for item in members} != {"LICENSE", "optiflow"}:
            raise ValueError("platform archive must contain exactly LICENSE and optiflow")
        for item in members:
            if not item.isfile() or not 0 < item.size <= 128 * 1024 * 1024:
                raise ValueError("unsafe or oversized platform archive member")
            if item.mode != (0o755 if item.name == "optiflow" else 0o644):
                raise ValueError("platform archive mode mismatch")
        with bundle.extractfile("optiflow") as stream:
            binary_digest = hashlib.file_digest(stream, "sha256").hexdigest()
        if destination is not None:
            destination.mkdir(parents=True, exist_ok=False)
            for item in members:
                with bundle.extractfile(item) as stream, (destination / item.name).open("xb") as output:
                    shutil.copyfileobj(stream, output)
                (destination / item.name).chmod(item.mode)
        return binary_digest


def validate_receipt(receipt, target, version, revision, archive):
    """A signed release must bind complete native trials to its actual archive."""
    if not isinstance(receipt, dict):
        raise ValueError("pilot receipt must be an object")
    expected = {"schema": SCHEMA, "target": target, "native_target": target,
                "release_version": version, "source_revision": revision, "profile": PROFILE,
                "passed": True, "source_unchanged": True, "cleanup_complete": True,
                "json_stdout_clean": True, "previous_signature_verified": True,
                "previous_version": PREVIOUS_VERSION, "previous_revision": PREVIOUS_REVISION}
    for key, value in expected.items():
        if receipt.get(key) != value or (isinstance(value, bool) and receipt.get(key) is not value):
            raise ValueError(f"pilot receipt has invalid {key}")
    if receipt.get("archive_sha256") != sha256(archive):
        raise ValueError("pilot receipt archive digest mismatch")
    if receipt.get("binary_sha256") != archive_binary(archive):
        raise ValueError("pilot receipt binary digest mismatch")
    scenarios = receipt.get("scenarios", {})
    if not isinstance(scenarios, dict) or set(scenarios) != set(OUTCOMES):
        raise ValueError("pilot receipt scenario inventory mismatch")
    for name, (code, outcome) in OUTCOMES.items():
        row = scenarios[name]
        if not isinstance(row, dict) or type(row.get("exit_code")) is not int or row.get("exit_code") != code or row.get("outcome") != outcome:
            raise ValueError(f"pilot receipt outcome mismatch: {name}")
        elapsed = row.get("elapsed_seconds")
        if type(elapsed) not in (float, int) or not 0 <= elapsed <= 180:
            raise ValueError(f"pilot receipt elapsed time invalid: {name}")
    if scenarios["tree_cold"].get("files") != PROFILE["tree_files"]:
        raise ValueError("pilot tree trial is not representative")
    if scenarios["tree_warm"].get("cache_hits") != PROFILE["tree_files"]:
        raise ValueError("pilot warm tree cache was not proven")
    if scenarios["large_cold"].get("logical_bytes") != PROFILE["large_file_bytes"] * 2:
        raise ValueError("pilot large-file trial is not representative")
    if scenarios["large_cold"].get("groups") != 1 or scenarios["large_warm"].get("cache_hits") != 2:
        raise ValueError("pilot large-file duplicate/cache evidence missing")
    if scenarios["plan"].get("mutates_files") is not False:
        raise ValueError("pilot plan is not read-only")
    if scenarios["partial"].get("coverage") != "partial":
        raise ValueError("pilot partial coverage missing")
    if scenarios["interrupted"].get("run_status") != "interrupted" or scenarios["restart"].get("new_run") is not True:
        raise ValueError("pilot interruption/restart evidence missing")
    for key in ("peak_child_rss_bytes", "state_bytes"):
        if type(receipt.get(key)) is not int or receipt[key] <= 0:
            raise ValueError(f"pilot measurement missing: {key}")
    if re.fullmatch(r"[0-9a-f]{64}", receipt.get("source_digest", "")) is None:
        raise ValueError("pilot source digest missing")


def read_receipt(path):
    if path.stat().st_size > 1024 * 1024:
        raise ValueError("pilot receipt exceeds 1 MiB")
    return json.loads(path.read_text(encoding="utf-8"))
