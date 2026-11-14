#!/usr/bin/env python3
"""Owner-side, offline report assembly. No build, execution, signing or publication.

This independent protocol adapter is retained in each owner repository. It never
imports a sibling checkout. Supplied positive records remain claims until the
owner attests the exact output and the consumer independently verifies it.
"""
from __future__ import annotations
import argparse
import base64
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import stat
import tarfile
import time
import tomllib

TOOL = "optiflow"
ROOT = Path(__file__).resolve().parents[1]
MAX_FILE = 536_870_912
MAX_JSON = 1_048_576
MAX_EXPANDED = 1_073_741_824


def require(condition, message):
    if not condition:
        raise ValueError(message)


def closed(value, names):
    require(isinstance(value, dict) and set(value) == set(names), "closed record fields required: " + ",".join(names))
    return value


def sha(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def digest(value):
    return isinstance(value, str) and len(value) == 64 and all(c in "0123456789abcdef" for c in value)


def native_path(value):
    require(isinstance(value, dict), "native path object required")
    if value.get("encoding") == "utf8":
        closed(value, ["encoding", "value"])
        require(isinstance(value["value"], str), "UTF-8 path required")
        text = value["value"]
    else:
        closed(value, ["encoding", "base64"])
        require(value["encoding"] == "unix_bytes" and os.name == "posix", "native path encoding unavailable")
        text = os.fsdecode(base64.b64decode(value["base64"], validate=True))
    path = Path(text)
    require(path.is_absolute() and ".." not in path.parts and "\0" not in text, "absolute normalized path required")
    return path


def encoded_path(path):
    raw = os.fsencode(path)
    try:
        return {"encoding": "utf8", "value": raw.decode("utf-8")}
    except UnicodeDecodeError:
        return {"encoding": "unix_bytes", "base64": base64.b64encode(raw).decode()}


def regular(path, maximum=MAX_FILE):
    path = Path(path)
    require(path.is_absolute() and path.resolve(strict=True) == path, "canonical source path required")
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and 0 < before.st_size <= maximum, "bounded unaliased regular file required")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NONBLOCK", 0)
    with os.fdopen(os.open(path, flags), "rb") as stream:
        opened = os.fstat(stream.fileno())
        data = stream.read(maximum + 1)
        after_fd = os.fstat(stream.fileno())
    after = path.lstat()
    identity = lambda s: (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns, s.st_nlink)
    require(all(identity(s) == identity(before) for s in [opened, after_fd, after]) and len(data) == before.st_size,
            "source changed during observation")
    return data


def pinned(pin, maximum=MAX_FILE):
    closed(pin, ["path", "sha256", "byte_count"])
    require(digest(pin["sha256"]) and isinstance(pin["byte_count"], int) and not isinstance(pin["byte_count"], bool), "exact file pin required")
    data = regular(native_path(pin["path"]), maximum)
    require(len(data) == pin["byte_count"] and sha(data) == pin["sha256"], "selected file pin changed")
    return data


def document(pin):
    def unique(pairs):
        result = {}
        for k, v in pairs:
            require(k not in result, "duplicate JSON member")
            result[k] = v
        return result
    return json.loads(pinned(pin, MAX_JSON), object_pairs_hook=unique, parse_constant=lambda _: require(False, "finite JSON required"))


def file_pin(path, maximum=MAX_FILE):
    data = regular(path, maximum)
    return {"path": encoded_path(path), "sha256": sha(data), "byte_count": len(data)}


class ExpansionBudget:
    def __init__(self, stream):
        self.stream, self.count = stream, 0

    def read(self, size=-1):
        require(size >= 0, "bounded archive read required")
        data = self.stream.read(min(size, MAX_EXPANDED - self.count + 1))
        self.count += len(data)
        require(self.count <= MAX_EXPANDED, "expanded archive budget exceeded")
        return data


def safe_member(name):
    path = PurePosixPath(name)
    require(name and len(name.encode("utf-8")) <= 4096 and not path.is_absolute() and
            all(part not in [".", "..", ""] for part in name.rstrip("/").split("/")) and "\\" not in name,
            "safe relative archive member required")
    return name.rstrip("/")


def native_binary(prefix, target):
    if target == "x86_64-unknown-linux-gnu":
        return len(prefix) >= 64 and prefix[:6] == b"\x7fELF\x02\x01" and int.from_bytes(prefix[18:20], "little") == 62
    if target in ["aarch64-apple-darwin", "x86_64-apple-darwin"]:
        cpu = 0x100000C if target.startswith("aarch64") else 0x1000007
        return len(prefix) >= 32 and prefix[:4] == b"\xcf\xfa\xed\xfe" and int.from_bytes(prefix[4:8], "little") == cpu
    return False


def members(data, executable, target):
    result, names, prefix = [], set(), None
    with gzip.GzipFile(fileobj=io.BytesIO(data)) as expanded:
        budget = ExpansionBudget(expanded)
        with tarfile.open(fileobj=budget, mode="r|") as archive:
            for index, entry in enumerate(archive):
                require(index < 8192, "archive member budget exceeded")
                name = safe_member(entry.name)
                require(name not in names and (entry.isfile() or entry.isdir()), "duplicate, link or special archive member")
                names.add(name)
                if entry.isdir():
                    continue
                require(0 <= entry.size <= MAX_FILE, "archive member byte bound")
                stream, observed, h, first = archive.extractfile(entry), 0, hashlib.sha256(), b""
                require(stream is not None, "regular member stream required")
                for block in iter(lambda: stream.read(65536), b""):
                    observed += len(block)
                    require(observed <= entry.size, "archive member grew")
                    h.update(block)
                    if len(first) < 64:
                        first += block[:64 - len(first)]
                require(observed == entry.size, "truncated archive member")
                result.append({"name": name, "sha256": h.hexdigest(), "byte_count": observed})
                if name == executable:
                    prefix = first
        for tail in iter(lambda: budget.read(65536), b""):
            require(not any(tail), "non-padding bytes after archive end")
    if executable is not None:
        require(prefix is not None and native_binary(prefix, target), "native executable member/CPU identity differs")
    return sorted(result, key=lambda m: m["name"])


def owner_version():
    source = tomllib.loads((ROOT / "Cargo.toml").read_text())
    version = source.get("package", {}).get("version")
    return version if isinstance(version, str) else source["workspace"]["package"]["version"]


def select(request):
    cohort = document(request["cohort"])
    require(cohort.get("schema_version") == "flow.release-cohort/v1" and isinstance(cohort.get("rows"), list), "selected cohort required")
    selected = [r for r in cohort["rows"] if r.get("id") == request["row_id"]]
    require(len(selected) == 1, "unique selected row required")
    row = selected[0]
    artifact = row.get("artifact")
    require(row.get("tool") == TOOL and row.get("implementation") == "implemented" and isinstance(artifact, dict), "implemented owner row with immutable artifact required")
    require(artifact["version"] == owner_version() and len(artifact["source_revision"]) == 40 and
            all(c in "0123456789abcdef" for c in artifact["source_revision"]), "Cargo version/source revision differs")
    return row, artifact, sha(canonical(row))


def publisher(request):
    closed(request, ["schema_version", "cohort", "row_id", "archive", "published_release", "executable_member", "contract_members"] + (["asset_kind"] if "asset_kind" in request else []))
    require(request["schema_version"] == "flow.publisher-report-request/v1", "publisher request schema differs")
    row, artifact, subject = select(request)
    archive = pinned(request["archive"])
    metadata = document(request["published_release"])
    require(metadata.get("draft") is False and metadata.get("tag_name") == "v" + artifact["version"] and
            type(metadata.get("id")) is int and metadata["id"] > 0, "actual retained published release metadata required")
    assets = [a for a in metadata.get("assets", []) if a.get("browser_download_url") == artifact["release_url"] and a.get("state") == "uploaded"]
    require(len(assets) == 1, "exact uploaded asset required")
    asset = assets[0]
    require(type(asset.get("id")) is int and asset["id"] > 0 and asset.get("digest") == "sha256:" + sha(archive) and
            asset.get("size") == len(archive) and artifact["release_url"].startswith("https://github.com/egohygiene/" + TOOL + "/releases/download/"), "archive differs from retained GitHub asset metadata")
    asset_kind = request.get("asset_kind", "tar_gz")
    require(asset_kind in ["tar_gz", "raw_binary"], "explicit supported asset layout required")
    executable = request["executable_member"]
    if row["interface"] == "rust_library":
        require(asset_kind == "tar_gz" and executable is None and artifact["sha256"] == sha(archive) and artifact["byte_count"] == len(archive), "library archive must be the selected immutable artifact")
    else:
        require(isinstance(executable, str) and PurePosixPath(safe_member(executable)).name == TOOL, "exact owner executable member required")
    if asset_kind == "raw_binary":
        require(executable == TOOL and native_binary(archive[:64], row["target"]), "raw native binary identity differs")
        inventory = [{"name": TOOL, "sha256": sha(archive), "byte_count": len(archive)}]
    else:
        inventory = members(archive, executable, row["target"])
    by_name = {m["name"]: m for m in inventory}
    if executable is not None:
        require(by_name[executable]["sha256"] == artifact["sha256"] and by_name[executable]["byte_count"] == artifact["byte_count"], "cohort executable differs from archive")
    require(isinstance(request["contract_members"], list) and len(request["contract_members"]) == len(row["contracts"]) <= 64, "exact contract member inventory required")
    contracts, ids = [], set()
    for selected in request["contract_members"]:
        require(isinstance(selected, dict), "closed contract selection required")
        if set(selected) == {"identifier", "member"}:
            identifier, member = selected["identifier"], safe_member(selected["member"])
            require(member in by_name, "known contract member required")
            item = {"identifier": identifier, "member": member, "sha256": by_name[member]["sha256"], "byte_count": by_name[member]["byte_count"], "publication": None}
        else:
            closed(selected, ["identifier", "file", "published_asset_id"])
            identifier, blob = selected["identifier"], pinned(selected["file"], MAX_JSON)
            sidecars = [a for a in metadata.get("assets", []) if a.get("id") == selected["published_asset_id"] and a.get("state") == "uploaded" and a.get("digest") == "sha256:" + sha(blob) and a.get("size") == len(blob)]
            require(len(sidecars) == 1 and sidecars[0]["browser_download_url"].startswith("https://github.com/egohygiene/" + TOOL + "/releases/download/v" + artifact["version"] + "/"), "exact separately published contract asset required")
            item = {"identifier": identifier, "member": None, "sha256": sha(blob), "byte_count": len(blob), "publication": {"release_url": sidecars[0]["browser_download_url"], "asset_id": sidecars[0]["id"]}}
        require(identifier not in ids and any(c["identifier"] == identifier and c["sha256"] == item["sha256"] for c in row["contracts"]), "unique exact cohort contract required")
        ids.add(identifier)
        contracts.append(item)
    return {"schema_version": "flow.publisher-artifact-report/v1", "tool": TOOL, "cohort_sha256": request["cohort"]["sha256"],
            "row_subject_sha256": subject, "asset_kind": asset_kind, "version": artifact["version"], "source_revision": artifact["source_revision"], "target": row["target"],
            "archive": {"sha256": sha(archive), "byte_count": len(archive), "name": asset["name"]},
            "publication": {"release_url": artifact["release_url"], "release_id": metadata["id"], "asset_id": asset["id"]},
            "executable_member": executable, "contracts": contracts, "members": inventory}


def qualified(request):
    closed(request, ["schema_version", "publisher_request", "clean_install", "qualification", "witness", "requirements", "host_identity"])
    require(request["schema_version"] == "flow.qualified-owner-report-request/v1", "qualified report request schema differs")
    published = publisher(request["publisher_request"])
    row, artifact, subject = select(request["publisher_request"])
    clean, qualification = document(request["clean_install"]), document(request["qualification"])
    witness = closed(request["witness"], ["run_directory", "plan_sha256", "acceptance_sha256", "accepted_receipt"])
    accepted = document(witness["accepted_receipt"])
    pinned(request["host_identity"], 65536)
    require(clean.get("schema_version") == "flow.positive-clean-install/v1" and clean.get("row_subject_sha256") == subject and
            clean.get("installed_executable_sha256") == artifact["sha256"] and clean.get("contracts") == row["contracts"] and
            clean.get("empty_destination_observed") is True and clean.get("positive_native_plan_sha256") == witness["plan_sha256"] and
            clean.get("positive_acceptance_sha256") == witness["acceptance_sha256"], "positive clean-install proof differs; copied-file receipts do not qualify")
    require(qualification.get("schema_version") == "flow.positive-native-qualification/v1" and qualification.get("evidence_kind") == "real_native_positive_acceptance" and
            qualification.get("row_subject_sha256") == subject and qualification.get("target") == row["target"] and
            qualification.get("host_identity_sha256") == request["host_identity"]["sha256"] and qualification.get("native_plan_sha256") == witness["plan_sha256"] and
            qualification.get("acceptance_sha256") == witness["acceptance_sha256"] and qualification.get("missing_checks") == [] and qualification.get("failed_checks") == [], "exact positive native qualification required")
    completed = qualification.get("completed_at_unix")
    require(type(completed) is int and 0 <= time.time() - completed <= 86400, "recent non-future native evidence required")
    if row["interface"] == "flow_route" and TOOL in ["aniflow", "renderflow"]:
        require(witness["run_directory"] is not None and native_path(witness["accepted_receipt"]["path"]) == native_path(witness["run_directory"]) / "accepted.json" and
                accepted.get("schema_version") == "flow.native-receipt/v1" and accepted.get("state") == "complete" and
                accepted.get("row_subject_sha256") == subject and accepted.get("plan_sha256") == witness["plan_sha256"] and
                accepted.get("acceptance_sha256") == witness["acceptance_sha256"], "actual retained Flow native acceptance required")
    else:
        require(witness["run_directory"] is None and accepted.get("schema_version") == "flow.owner-native-release-gate/v1" and
                accepted.get("evidence_kind") == "real_native_release_gate" and accepted.get("row_subject_sha256") == subject and
                accepted.get("artifact_sha256") == artifact["sha256"] and accepted.get("target") == row["target"] and
                accepted.get("host_identity_sha256") == request["host_identity"]["sha256"] and accepted.get("completed_at_unix") == completed and
                accepted.get("plan_sha256") == witness["plan_sha256"] and accepted.get("acceptance_sha256") == witness["acceptance_sha256"] and
                sorted(accepted.get("accepted_checks", [])) == sorted(["native_build", "positive_contract_probe", "clean_install", "positive_smoke", "domain_acceptance"]) and
                accepted.get("failed_checks") == [] and accepted.get("missing_checks") == [], "real producer-owned positive gate required")
    requirements = request["requirements"]
    require(isinstance(requirements, list) and len(requirements) == len(row["requirements"]) <= 256 and
            sorted(r["requirement_id"] for r in requirements) == sorted(r["id"] for r in row["requirements"]), "every exact runtime requirement must be observed")
    for requirement in requirements:
        closed(requirement, ["requirement_id", "file"])
        pinned(requirement["file"])
    return {"schema_version": "flow.authenticated-release-report/v1", "cohort_sha256": published["cohort_sha256"], "row_subject_sha256": subject,
            "publication": {**published["publication"], "published_archive": request["publisher_request"]["archive"],
                            "executable_member_sha256": artifact["sha256"], "executable_member_bytes": artifact["byte_count"]},
            "clean_install": request["clean_install"], "qualification": request["qualification"], "witness": witness, "requirements": requirements}


def assemble(kind, request):
    if kind == "publisher":
        return publisher(request)
    if kind == "qualified":
        return qualified(request)
    require(kind == "mutation-review", "supported owner report kind required")
    import mutation_review
    return mutation_review.build(request)


def emit(kind, request, output):
    initial = canonical(request)
    report = assemble(kind, request)
    output = Path(output).absolute()
    require(output.parent.resolve(strict=True) == output.parent and not output.exists(), "fresh canonical output directory required")
    # Keep output separate from every selected pin, including proof directories.
    def visit(value):
        if isinstance(value, dict):
            if value.get("encoding") in ["utf8", "unix_bytes"]:
                selected = native_path(value)
                require(not selected.is_relative_to(output) and not output.is_relative_to(selected), "report output overlaps selected native path")
            if set(value) == {"path", "sha256", "byte_count"}:
                p = native_path(value["path"])
                require(not p.is_relative_to(output) and not output.is_relative_to(p), "report output overlaps selected source")
            for v in value.values():
                visit(v)
        elif isinstance(value, list):
            for v in value:
                visit(v)
    visit(request)
    output.mkdir(mode=0o700)
    path = output / "report.json"
    data = json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True).encode() + b"\n"
    require(len(data) <= MAX_JSON, "report exceeds 1 MiB")
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0), 0o600), "wb") as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    # A second complete readback prevents a supplied record changing during assembly.
    now = assemble(kind, request)
    require(now == report and canonical(request) == initial, "source evidence changed during report assembly; preserve partial output")
    return {"schema_version": "egohygiene.owner-report-output/v1", "tool": TOOL, "kind": kind, "report": file_pin(path, MAX_JSON),
            "signature_state": "unsigned_candidate", "native_work_performed": False, "publication_performed": False,
            "next_action": ("resolve native/hardware/release review findings; this review cannot activate runtime or publication gates" if kind == "mutation-review" else "review and attest these exact report bytes through the existing owner workflow; consumer verification remains required")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=["publisher", "qualified", "mutation-review"])
    parser.add_argument("--request", required=True, type=Path)
    parser.add_argument("--output-directory", required=True, type=Path)
    args = parser.parse_args()
    request_path = args.request.absolute()
    before = file_pin(request_path, MAX_JSON)
    request = document(before)
    result = emit(args.kind, request, args.output_directory)
    require(file_pin(request_path, MAX_JSON) == before, "selected request changed; preserve output for reconciliation")
    print(json.dumps(result, ensure_ascii=True, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, TypeError, OSError, tarfile.TarError) as problem:
        raise SystemExit("owner report refused: " + str(problem))
