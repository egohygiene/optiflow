"""Exercise PNG discovery/authority/refusal on generated disposable files only.

Supply an explicitly built/pinned development executable. This uses the public
CLI, never selects a provider or enables replacement, and prints a small receipt.
"""
from __future__ import annotations

import argparse
import binascii
from collections import Counter
from datetime import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import stat
import struct
import subprocess
import tempfile
import zlib

ACTOR = "synthetic-proof-only"
NATIVE_REFUSAL = "Replacement native qualification is pending (#93/#95/#146); no source mutation or writable journal access"
RECURSIVE_NAMES = {".hidden.png", "a.png", "hard-a.png", "hard-b.png", "invalid.png",
                   "nested/雪 song.png", "notes.txt", "outside-link.png"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def file_hash(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def unique_json(raw):
    def object_members(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "duplicate JSON member")
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=object_members)


def observe_envelope(value, command, expected_code, reason=None):
    require(type(value) is dict and set(value) == {"schema", "command", "outcome", "artifacts", "diagnostics", "result"},
            "closed command envelope required")
    require(value["schema"] == "optiflow.command-result.v1" and value["command"] == command,
            "command schema/name differs")
    classes = {0: "success", 2: "invalid_input", 4: "capability_unavailable"}
    require(expected_code in classes and type(value["outcome"]) is dict
            and type(value["outcome"].get("exit_code")) is int
            and value["outcome"] == {"class": classes[expected_code], "exit_code": expected_code},
            "declared command outcome differs")
    require(value["artifacts"] == [], "unexpected artifact authority")
    observation = {"command": command, "outcome": classes[expected_code]}
    if expected_code == 0:
        require(reason is None and value["diagnostics"] == [] and type(value["result"]) is dict,
                "success contains diagnostics or lacks its result")
    else:
        require(type(reason) is str and bool(reason), "an exact refusal reason is required")
        require(expected_code != 4 or reason == NATIVE_REFUSAL, "unknown native qualification reason")
        code, classification = (("execution_unsupported", "capability") if expected_code == 4
                                else ("execution_plan_invalid", "input"))
        expected = {"code": code, "severity": "error", "classification": classification,
                    "impact": "blocks_command", "message": reason}
        require(value["result"] is None and value["diagnostics"] == [expected],
                "refusal diagnostic/reason differs")
        observation.update(diagnostic_code=code, diagnostic_classification=classification, exact_reason_verified=True)
    return observation


def indexed_entries(inventory, source, expected_names):
    require(inventory.get("schema") == "optiflow.png-folder-inventory.v1"
            and inventory.get("one_file_in_flight") is True
            and inventory.get("physical_space_reclaimed") is False, "inventory schema/authority differs")
    entries = inventory.get("entries")
    require(type(entries) is list, "inventory entry list required")
    names = []
    for entry in entries:
        path = entry.get("path")
        require(type(path) is dict and set(path) == {"encoding", "value"}
                and path["encoding"] == "utf8" and type(path["value"]) is str, "native path differs")
        names.append(str(Path(path["value"]).relative_to(source)))
    require(len(names) == len(set(names)) and set(names) == expected_names and names == sorted(names),
            "inventory has duplicate, extra, missing or unsorted paths")
    return dict(zip(names, entries))


def require_eligible(entries, expected_names):
    require(sorted(name for name, entry in entries.items() if entry["disposition"] == "eligible") == expected_names,
            "eligible path membership differs")


def observe_approval(approval, saved, request, inventory):
    require(type(approval) is dict
            and set(approval) == {"schema", "authorization_id", "request_fingerprint", "inventory_fingerprint",
                                  "disposition", "approved_by", "approved_at", "maximum_actions"}
            and saved == approval, "saved approval differs from closed command result")
    require(approval.get("schema") == "optiflow.png-folder-approval.v1" and approval.get("approved_by") == ACTOR,
            "approval schema/actor differs")
    for key in ("request_fingerprint", "inventory_fingerprint", "authorization_id"):
        require(type(approval.get(key)) is str and re.fullmatch(r"[0-9a-f]{64}", approval[key]) is not None,
                "approval digest identity malformed")
    require(type(approval["approved_at"]) is str
            and datetime.fromisoformat(approval["approved_at"].replace("Z", "+00:00")).tzinfo is not None,
            "approval timestamp is not an explicit zoned time")
    require(approval["request_fingerprint"] == inventory["request_fingerprint"]
            and approval["inventory_fingerprint"] == inventory["fingerprint"]
            and approval.get("disposition") == request["original_disposition"]
            and type(approval.get("maximum_actions")) is int
            and approval["maximum_actions"] == request["batch"]["maximum_actions"],
            "approval request/inventory/disposition/action identity differs")


def file_observation(path):
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode), "inventory record is not regular")
    return [info.st_dev, info.st_ino, info.st_mode, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns, file_hash(path)]


def require_same_record(path, observed):
    require(file_observation(path) == observed, "existing inventory record identity/mode/time/bytes changed")


def png():
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", binascii.crc32(kind + data))
    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(b"\x00\x20\x80\x40"))
            + chunk(b"IEND", b""))


def snapshot(root):
    observations = {}
    for path in [root, *sorted(root.rglob("*"))]:
        info = path.lstat()
        entry = [info.st_dev, info.st_ino, info.st_mode, info.st_nlink,
                 info.st_size, info.st_mtime_ns, info.st_ctime_ns]
        if stat.S_ISREG(info.st_mode):
            entry.append(file_hash(path))
        elif stat.S_ISLNK(info.st_mode):
            entry.append(os.readlink(path))
        observations[str(path.relative_to(root))] = entry
    return observations


def prove(executable, expected):
    require(platform.system() == "Linux", "this selected proof requires the Linux host")
    executable = executable.resolve(strict=True)
    require(executable.is_file() and os.access(executable, os.X_OK), "select an executable regular file")
    require(file_hash(executable) == expected, "executable differs from independent expected SHA-256")
    cases = []
    with tempfile.TemporaryDirectory(prefix="optiflow-png-folder-proof-") as temporary:
        root = Path(temporary).resolve(strict=True)
        for name in ("source", "state", "retained", "candidates", "run", "records", "outside", "config", "data"):
            (root / name).mkdir(mode=0o700)
        source = root / "source"
        (source / "nested").mkdir(mode=0o700)
        for relative in ("a.png", "nested/雪 song.png", ".hidden.png", "hard-a.png"):
            (source / relative).write_bytes(png())
        os.link(source / "hard-a.png", source / "hard-b.png")
        (source / "notes.txt").write_bytes(b"generated unsupported source")
        (source / "invalid.png").write_bytes(b"not PNG")
        (root / "outside/untouched.png").write_bytes(png())
        (source / "outside-link.png").symlink_to(root / "outside/untouched.png")
        before_source, before_outside = snapshot(source), snapshot(root / "outside")
        environment = {"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8",
                       "XDG_CONFIG_HOME": str(root / "config"),
                       "XDG_DATA_HOME": str(root / "data"),
                       "XDG_STATE_HOME": str(root / "state")}
        prefix = [str(executable), "--no-config", "--state-directory", str(root / "state"), "--json"]

        def invoke(identity, arguments, expected_code=0, reason=None):
            completed = subprocess.run(prefix + arguments, cwd=root, env=environment,
                                       capture_output=True, timeout=20, check=False)
            require(len(completed.stdout) + len(completed.stderr) <= 262144, identity + ": output exceeds proof bound")
            require(completed.returncode == expected_code, identity + ": unexpected exit status")
            value = unique_json(completed.stdout)
            observation = observe_envelope(value, " ".join(arguments[:2]), expected_code, reason)
            cases.append({"id": identity, "exit_code": completed.returncode, **observation})
            return value

        def record(name, value):
            path = root / "records" / name
            with path.open("x", encoding="utf8") as stream:
                json.dump(value, stream, ensure_ascii=False)
            path.chmod(0o600)
            return str(path)

        config = invoke("effective-policy", ["config", "show"])
        policy = config["result"]["policy"]["fingerprints"]["evidence_policy"]["value"]
        request = {"schema": "optiflow.png-folder-request.v1", "source_root": str(source),
                   "oxipng": str(root / "never-installed-or-opened-provider"),
                   "state_directory": str(root / "state"), "retention_directory": str(root / "retained"),
                   "artifact_root": str(root / "candidates"), "run_directory": str(root / "run"),
                   "policy_fingerprint": policy, "original_disposition": "retain_for_restore",
                   "recursive": True, "include_hidden": False,
                   "per_file": {"maximum_file_bytes": 1048576, "free_space_reserve_bytes": 1024,
                                "journal_budget_bytes": 1048576},
                   "batch": {"maximum_entries": 32, "maximum_files": 16, "maximum_actions": 4,
                             "maximum_depth": 4, "maximum_total_input_bytes": 1048576, "elapsed_ms": 10000}}
        request_path = record("request.json", request)
        inventory_path = root / "records/inventory.json"
        inventory = invoke("recursive-discovery", ["optimize", "discover", "--request", request_path,
                                                    "--output", str(inventory_path)])["result"]
        require(inventory == unique_json(inventory_path.read_text()), "saved inventory differs from command result")
        entries = indexed_entries(inventory, source, RECURSIVE_NAMES)
        eligible = sorted(name for name, entry in entries.items() if entry["disposition"] == "eligible")
        require_eligible(entries, ["a.png", "nested/雪 song.png"])
        for name, reason in [(".hidden.png", "hidden_entry_excluded"), ("outside-link.png", "symlink_not_followed"),
                             ("hard-a.png", "link_empty_or_file_bound"), ("hard-b.png", "link_empty_or_file_bound")]:
            require(entries[name]["reason"] == reason and entries[name]["disposition"] == "skipped", "link/hidden refusal differs")
        require(all(entries[name]["disposition"] == "unsupported" for name in ("notes.txt", "invalid.png")),
                "unsupported files became eligible")
        approval_path = root / "records/approval.json"
        approval = invoke("explicit-retain-authority", ["optimize", "approve", "--request", request_path,
                          "--inventory", str(inventory_path), "--approved-by", ACTOR,
                          "--output", str(approval_path)])["result"]
        observe_approval(approval, unique_json(approval_path.read_text()), request, inventory)
        retained_inventory = file_observation(inventory_path)
        invoke("no-inventory-overwrite", ["optimize", "discover", "--request", request_path,
                                         "--output", str(inventory_path)], 2,
               "record output already exists or cannot be inspected")
        require_same_record(inventory_path, retained_inventory)
        flat = dict(request, recursive=False)
        flat_path = record("flat-request.json", flat)
        flat_inventory = invoke("explicit-nonrecursive-selection", ["optimize", "discover", "--request", flat_path,
                                "--output", str(root / "records/flat-inventory.json")])["result"]
        flat_entries = indexed_entries(flat_inventory, source, RECURSIVE_NAMES - {"nested/雪 song.png"} | {"nested"})
        require_eligible(flat_entries, ["a.png"])
        require(not any(name.startswith("nested/") for name in flat_entries), "nonrecursive selection traversed nested paths")
        bounded = dict(request, batch=dict(request["batch"], maximum_entries=1, maximum_files=1, maximum_actions=1))
        bound_path = record("bounded-request.json", bounded)
        bound_output = root / "records/bounded-inventory.json"
        invoke("entry-bound-refusal", ["optimize", "discover", "--request", bound_path, "--output", str(bound_output)], 2,
               "entry bound exceeded; no provider started")
        require(not bound_output.exists(), "failed discovery published inventory")
        discard = dict(request, original_disposition="discard_after_durable_replacement")
        discard_path = record("discard-request.json", discard)
        crossed = root / "records/crossed-approval.json"
        invoke("retention-cannot-become-discard", ["optimize", "approve", "--request", discard_path,
               "--inventory", str(inventory_path), "--approved-by", ACTOR, "--output", str(crossed)], 2,
               "PNG inventory identity or bounds differ")
        require(not crossed.exists(), "changed disposition published crossed authority")
        discard_inventory_path = root / "records/discard-inventory.json"
        discard_inventory = invoke("explicit-discard-discovery", ["optimize", "discover", "--request", discard_path,
               "--output", str(discard_inventory_path)])["result"]
        require(discard_inventory == unique_json(discard_inventory_path.read_text()), "saved discard inventory differs")
        require_eligible(indexed_entries(discard_inventory, source, RECURSIVE_NAMES), eligible)
        discard_approval_path = root / "records/discard-approval.json"
        discard_approval = invoke("explicit-discard-authority", ["optimize", "approve", "--request", discard_path,
               "--inventory", str(discard_inventory_path), "--approved-by", ACTOR,
               "--output", str(discard_approval_path)])["result"]
        observe_approval(discard_approval, unique_json(discard_approval_path.read_text()), discard, discard_inventory)
        invoke("crossed-approval-status-refusal", ["optimize", "status", "--request", request_path,
               "--inventory", str(inventory_path), "--approval", str(discard_approval_path)], 2,
               "explicit batch authority differs")
        unknown_value = dict(request, bypass_qualification=True)
        unknown = record("unknown-field-request.json", unknown_value)
        unknown_output = root / "records/unknown-inventory.json"
        unknown_reason = ("contract validation failed at : "
                          + json.dumps(unknown_value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
                          + " is not valid under any of the schemas listed in the 'oneOf' keyword")
        invoke("closed-request-refusal", ["optimize", "discover", "--request", unknown,
                                         "--output", str(unknown_output)], 2, unknown_reason)
        require(not unknown_output.exists(), "unknown request published inventory")
        for command in ("run", "resume"):
            invoke(command + "-qualification-before-inputs", ["optimize", command,
                      "--request", str(root / "missing-request"), "--inventory", str(root / "missing-inventory"),
                      "--approval", str(root / "missing-approval")], 4, NATIVE_REFUSAL)
        require(snapshot(source) == before_source and snapshot(root / "outside") == before_outside,
                "source or outside canary bytes/membership/identity changed")
        require(all(not any((root / name).iterdir()) for name in ("state", "retained", "candidates", "run")),
                "discovery/authority/refusal created mutation or provider state")
        require(file_hash(executable) == expected, "executable changed during proof")
        return {"schema": "optiflow.png-folder-cli-proof.v1", "executable_sha256": expected,
                "host": "Linux", "synthetic_cases": cases, "eligible_files": eligible,
                "observed_dispositions": dict(Counter(item["disposition"] for item in entries.values())),
                "originals_and_outside_canary_unchanged": True, "mutation_directories_untouched": True,
                "provider_invocation_selected": False, "synthetic_authority_only": True,
                "physical_space_reclaimed": False, "native_qualified": False, "publication_performed": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--expected-executable-sha256", required=True)
    args = parser.parse_args()
    try:
        report = json.dumps(prove(args.executable, args.expected_executable_sha256), ensure_ascii=False, sort_keys=True)
        require(len(report.encode()) <= 32768, "proof receipt exceeds bound")
        print(report)
    except ValueError as error:
        raise SystemExit("PNG folder proof failed: " + str(error)[:500])
    except (OSError, KeyError, subprocess.SubprocessError):
        raise SystemExit("PNG folder proof failed; inspect the selected executable and local checkpoint")


if __name__ == "__main__":
    main()
