"""Offline #93 review of pinned native evidence; never a release/native gate.

Uses the existing owner-report I/O/archive boundary. Supplied hardware claims
remain claims until independent review/attestation; missing coverage stays visible.
"""
from __future__ import annotations
import json
import re
import owner_reports as owner

TARGETS = ("x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "aarch64-apple-darwin")
SUCCESS = {(0, "success")}
CASES = {
    "clean_install": SUCCESS, "upgrade_from_v0_1_1": SUCCESS, "rollback": SUCCESS,
    "scan": SUCCESS, "plan": SUCCESS, "dry_run": SUCCESS,
    "quarantine": SUCCESS, "status": SUCCESS, "resume": SUCCESS, "restore": SUCCESS,
    "finalize_preview": SUCCESS, "finalize_authorize": SUCCESS, "finalize_commit": SUCCESS,
    "finalize_status": SUCCESS, "cleanup": SUCCESS,
    "process_interruption": {(130, "interrupted"), (143, "terminated")},
    "physical_disconnect_reconnect": SUCCESS,
    "collision_refusal": {(2, "invalid_input"), (5, "stale_state")},
    "capacity_refusal": {(2, "invalid_input"), (4, "capability_unavailable"), (5, "stale_state")},
    "source_change_refusal": {(5, "stale_state")},
}
CLAIMS = ("protected_keeper_unchanged", "restored_bytes_properties_equal",
          "separate_finalization_authority", "finalization_quarantine_only",
          "uncertain_states_never_committed", "idempotent_resume", "stdout_machine_clean",
          "previous_release_signature_independently_verified")
COMMANDS = {
    "clean_install": {"release-contract"}, "upgrade_from_v0_1_1": {"release-contract"},
    "rollback": {"scan", "doctor"}, "scan": {"scan"}, "plan": {"plan"},
    "dry_run": {"apply"}, "quarantine": {"apply"}, "status": {"execution status"},
    "resume": {"execution resume"}, "restore": {"execution restore"},
    "finalize_preview": {"execution finalize"}, "finalize_authorize": {"execution authorize-finalization"},
    "finalize_commit": {"execution finalize"}, "finalize_status": {"execution status"},
    "cleanup": {"execution cleanup"}, "process_interruption": {"apply", "execution resume"},
    "physical_disconnect_reconnect": {"execution status", "execution resume", "execution restore"},
    "collision_refusal": {"apply", "execution restore"}, "capacity_refusal": {"apply"},
    "source_change_refusal": {"apply", "execution resume", "execution restore"},
}


def unique(pairs):
    value = {}
    for key, item in pairs:
        owner.require(key not in value, "duplicate native review JSON member")
        value[key] = item
    return value


class Budget:
    def __init__(self, maximum):
        owner.require(type(maximum) is int and 1 <= maximum <= 1_073_741_824,
                      "explicit bounded review input bytes required")
        self.maximum, self.used, self.files = maximum, 0, 0

    def read(self, pin, maximum):
        owner.closed(pin, ["path", "sha256", "byte_count"])
        size = pin["byte_count"]
        owner.require(type(size) is int and 0 < size <= min(maximum, self.maximum - self.used)
                      and self.files < 256, "native review read budget exhausted")
        data = owner.pinned(pin, maximum)
        self.used += len(data)
        self.files += 1
        return data

    def document(self, pin):
        return json.loads(self.read(pin, owner.MAX_JSON), object_pairs_hook=unique,
                          parse_constant=lambda _: owner.require(False, "finite JSON required"))


def command_result(value, allowed, scenario=None):
    owner.require(isinstance(value, dict) and value.get("schema") == "optiflow.command-result.v1",
                  "native case must retain actual versioned command-result JSON")
    outcome = value.get("outcome", {})
    owner.require(isinstance(outcome, dict) and type(outcome.get("exit_code")) is int
                  and (outcome["exit_code"], outcome.get("class")) in allowed,
                  "native case command outcome does not match its scenario")
    owner.require(isinstance(value.get("command"), str) and value["command"], "command identity missing")
    if scenario is not None:
        owner.require(value["command"] in COMMANDS[scenario], "read-only/foreign command cannot stand for mutation scenario")
        result = value.get("result")
        if scenario in ["clean_install", "upgrade_from_v0_1_1"]:
            owner.require(isinstance(result, dict) and result.get("schema_version") == "optiflow.release-contract/v1"
                          and result.get("tool") == "optiflow" and result.get("version") == "0.2.0",
                          "installed candidate version/contract identity differs")
        if scenario in ["dry_run", "quarantine"]:
            owner.require(isinstance(result, dict) and result.get("schema") == "optiflow.execution-mutation.v2"
                          and result.get("dry_run") is (scenario == "dry_run"),
                          "dry-run and mutation captures cannot substitute for one another")
            if scenario == "quarantine":
                owner.require(type(result.get("committed_actions")) is int and result["committed_actions"] > 0
                              and result.get("status") == "completed", "actual committed quarantine result required")


def validate_receipt(receipt, target, version, revision, archive_sha256, binary_sha256, budget):
    owner.closed(receipt, ["schema", "release_version", "source_revision", "target", "native_target",
                          "profile", "archive_sha256", "binary_sha256", "evidence_kind", "synthetic",
                          "fixture_policy", "volumes", "cases", "claims", "space"])
    owner.require(receipt["schema"] == "optiflow.native-mutation-review-evidence.v1"
                  and receipt["release_version"] == version and receipt["source_revision"] == revision
                  and receipt["target"] == target and receipt["native_target"] == target
                  and receipt["profile"] == "exact-dedup-transaction-v0.2"
                  and receipt["archive_sha256"] == archive_sha256 and receipt["binary_sha256"] == binary_sha256
                  and receipt["evidence_kind"] == "native_disposable_volume"
                  and receipt["synthetic"] is False and receipt["fixture_policy"] == "generated_disposable_only",
                  "read-only/synthetic/foreign-archive evidence cannot stand for native mutation")
    findings, captures, volumes = [], [], []
    owner.require(isinstance(receipt["volumes"], list) and 1 <= len(receipt["volumes"]) <= 8,
                  "bounded explicit native volume inventory required")
    for row in receipt["volumes"]:
        owner.closed(row, ["filesystem", "kind", "disposable", "disconnect_reconnect_observed", "evidence"])
        owner.require(isinstance(row["filesystem"], str) and re.fullmatch(r"[a-z0-9_-]{1,32}", row["filesystem"])
                      and row["kind"] in ["apfs_disk_image", "physical_removable"]
                      and row["disposable"] is True and type(row["disconnect_reconnect_observed"]) is bool,
                      "disposable native volume scope required")
        facts = budget.document(row["evidence"])
        owner.closed(facts, ["schema", "target", "source_revision", "archive_sha256", "filesystem",
                             "kind", "disposable", "disconnect_reconnect_observed"])
        owner.require(facts["schema"] == "optiflow.native-volume-observation.v1"
                      and facts["target"] == target and facts["source_revision"] == revision
                      and facts["archive_sha256"] == archive_sha256 and facts["disposable"] is True
                      and type(facts["disconnect_reconnect_observed"]) is bool
                      and all(facts[key] == row[key] for key in ["filesystem", "kind", "disposable", "disconnect_reconnect_observed"]),
                      "volume capture differs from supplied native claim")
        if row["kind"] == "apfs_disk_image":
            owner.require(row["filesystem"] == "apfs" and row["disconnect_reconnect_observed"] is False,
                          "disk-image detach is not physical disconnect evidence")
        volumes.append({key: row[key] for key in row if key != "evidence"})
        captures.append({"role": "volume", "sha256": row["evidence"]["sha256"], "byte_count": row["evidence"]["byte_count"]})
    if target.endswith("apple-darwin") and not any(v["filesystem"] == "apfs" for v in volumes):
        findings.append("native APFS coverage missing")
    physical = any(v["kind"] == "physical_removable" and v["disconnect_reconnect_observed"] for v in volumes)
    if not physical:
        findings.append("real removable-volume disconnect/reconnect evidence missing")
    if not any(v["kind"] == "physical_removable" and v["filesystem"] not in ["apfs"] for v in volumes):
        findings.append("portability-relevant external filesystem coverage requires explicit disposition")
    owner.closed(receipt["cases"], CASES)
    for name, allowed in CASES.items():
        row = receipt["cases"][name]
        owner.require(isinstance(row, dict), "native case record required")
        if row.get("status") == "unavailable":
            owner.closed(row, ["status", "reason"])
            owner.require(isinstance(row["reason"], str) and 0 < len(row["reason"]) <= 512, "unavailable reason required")
            findings.append(name + ": " + row["reason"])
            continue
        owner.closed(row, ["status", "command_results"])
        owner.require(row["status"] == "passed" and isinstance(row["command_results"], list)
                      and 1 <= len(row["command_results"]) <= 8, "bounded actual command captures required")
        owner.require(name != "physical_disconnect_reconnect" or physical,
                      "physical disconnect case cannot pass from a disk image")
        for pin in row["command_results"]:
            command_result(budget.document(pin), allowed, name)
            captures.append({"role": name, "sha256": pin["sha256"], "byte_count": pin["byte_count"]})
    owner.closed(receipt["claims"], CLAIMS)
    for name, value in receipt["claims"].items():
        owner.require(value is None or type(value) is bool, "native claim must be observed true/false or unknown")
        if value is not True:
            findings.append(name + " is not established")
    space = owner.closed(receipt["space"], ["logical_bytes_finalized", "allocated_bytes_observed", "free_bytes_before", "free_bytes_after"])
    for name, value in space.items():
        owner.require((name == "allocated_bytes_observed" and value is None) or
                      (type(value) is int and 0 <= value <= 2**64 - 1), "bounded explicit space observations required")
    delta = space["free_bytes_after"] - space["free_bytes_before"]
    if space["logical_bytes_finalized"] == 0 or delta <= 0:
        findings.append("positive finalized-byte/free-space observation missing; physical reclamation is not proved")
    return {"target": target, "archive_sha256": archive_sha256, "binary_sha256": binary_sha256,
            "volumes": volumes, "captures": captures, "claims": receipt["claims"], "space": space,
            "observed_free_space_delta_bytes": delta, "findings": findings,
            "physical_reclamation_verified": False, "native_qualification_established": False}


def build(request):
    owner.closed(request, ["schema", "release_version", "source_revision", "maximum_input_bytes", "targets"])
    version, revision = request["release_version"], request["source_revision"]
    owner.require(request["schema"] == "optiflow.mutation-release-review-request.v1"
                  and version == "v0.2.0" and owner.owner_version() == "0.2.0"
                  and isinstance(revision, str) and re.fullmatch(r"[0-9a-f]{40}", revision),
                  "v0.2 review requires exact candidate source and Cargo-authoritative version")
    owner.require(isinstance(request["targets"], list) and 1 <= len(request["targets"]) <= 3,
                  "one to three explicit target rows required; missing targets remain blocked")
    budget, rows, seen = Budget(request["maximum_input_bytes"]), [], set()
    for row in request["targets"]:
        owner.closed(row, ["target", "archive", "qualification"])
        target = row["target"]
        owner.require(target in TARGETS and target not in seen, "supported unique native target required")
        seen.add(target)
        members = owner.members(budget.read(row["archive"], 134_217_728), "optiflow", target)
        owner.require({v["name"] for v in members} == {"LICENSE", "optiflow"}, "exact qualified two-file archive surface required")
        binary = next(v for v in members if v["name"] == "optiflow")
        receipt = budget.document(row["qualification"])
        rows.append(validate_receipt(receipt, target, version, revision, row["archive"]["sha256"], binary["sha256"], budget))
    missing = [target for target in TARGETS if target not in seen]
    return {"schema": "optiflow.mutation-release-review.v1", "release_version": version,
            "source_revision": revision, "targets": rows, "missing_targets": missing,
            "review_state": "blocked" if missing or any(row["findings"] for row in rows) else "independent_review_required",
            "signature_state": "unsigned_candidate", "native_work_performed": False,
            "runtime_gate_activation": False, "publication_authority": False,
            "scope": "pinned bytes, CPU identities and supplied native claims only; independent physical witness, signing and release approval remain separate"}
