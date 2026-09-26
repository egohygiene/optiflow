#!/usr/bin/env python3
"""Prepare and verify deterministic, Relay-compatible OptiFlow release bundles."""

from __future__ import annotations

import argparse
import datetime as dt
import gzip
import hashlib
import json
import re
import subprocess
import sys
import tarfile
from pathlib import Path, PurePosixPath
from typing import Any

import tomllib
import shutil

import pilot_contract as pilot

PACKAGE_NAME = "optiflow"
REPOSITORY = "https://github.com/egohygiene/optiflow"
WORKFLOW_IDENTITY = (
    "https://github.com/egohygiene/optiflow/.github/workflows/release.yml"
    "@refs/heads/main"
)
OIDC_ISSUER = "https://token.actions.githubusercontent.com"
SUPPORTED_TARGETS = (
    "x86_64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
)
PRIMARY_EVIDENCE = ("provenance.json", "sbom.spdx.json")
PILOT_EVIDENCE = "pilot-qualification.json"
SUBJECTS_MANIFEST = "release-subjects.sha256"
SIGNATURE = "signature.json"
COMPLETE_MANIFEST = "SHA256SUMS"
VERSION_RE = re.compile(r"^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")


class BundleError(RuntimeError):
    """A release bundle violates the repository release contract."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def normalized_timestamp(value: str) -> str:
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise BundleError(f"created-at is not RFC 3339: {value}") from error
    if parsed.tzinfo is None:
        raise BundleError("created-at must include a UTC offset")
    return parsed.astimezone(dt.timezone.utc).isoformat().replace("+00:00", "Z")


def validate_inputs(
    repository_root: Path, release_version: str, source_revision: str
) -> str:
    match = VERSION_RE.fullmatch(release_version)
    if match is None:
        raise BundleError("release-version must be an exact vMAJOR.MINOR.PATCH value")
    if re.fullmatch(r"[0-9a-f]{40}", source_revision) is None:
        raise BundleError("source-revision must be a full lowercase Git commit SHA")

    manifest = tomllib.loads(
        (repository_root / "Cargo.toml").read_text(encoding="utf-8")
    )
    package = manifest.get("package", {})
    manifest_version = package.get("version")
    requested_version = release_version.removeprefix("v")
    if manifest_version != requested_version:
        raise BundleError(
            f"release {release_version} does not match Cargo.toml version {manifest_version}"
        )
    if package.get("name") != PACKAGE_NAME:
        raise BundleError(f"Cargo.toml package must be {PACKAGE_NAME}")
    return requested_version


def deterministic_archive(binary: Path, license_path: Path, output: Path) -> None:
    if not binary.is_file():
        raise BundleError(f"release binary is missing: {binary}")
    if not license_path.is_file():
        raise BundleError(f"release license is missing: {license_path}")
    output.parent.mkdir(parents=True, exist_ok=True)
    with (
        output.open("wb") as raw,
        gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed,
        tarfile.open(
            fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT
        ) as archive,
    ):
        for name, path, mode in (
            ("LICENSE", license_path, 0o644),
            (PACKAGE_NAME, binary, 0o755),
        ):
            info = tarfile.TarInfo(name)
            info.size = path.stat().st_size
            info.mode = mode
            info.uid = 0
            info.gid = 0
            info.uname = ""
            info.gname = ""
            info.mtime = 0
            with path.open("rb") as stream:
                archive.addfile(info, stream)


def cargo_metadata(repository_root: Path) -> dict[str, Any]:
    completed = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        cwd=repository_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise BundleError(f"cargo metadata failed: {completed.stderr.strip()}")
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise BundleError("cargo metadata returned invalid JSON") from error


def spdx_id(prefix: str, value: str) -> str:
    digest = hashlib.sha256(value.encode("utf-8")).hexdigest()[:16]
    return f"SPDXRef-{prefix}-{digest}"


def package_purl(package: dict[str, Any]) -> str:
    name = package["name"]
    version = package["version"]
    return f"pkg:cargo/{name}@{version}"


def spdx_license(package: dict[str, Any]) -> str:
    """Normalize Cargo's legacy slash spelling without inventing a license."""
    license_expression = package.get("license")
    if not isinstance(license_expression, str) or not license_expression.strip():
        return "NOASSERTION"
    return re.sub(r"\s*/\s*", " OR ", license_expression.strip())


def runtime_package_ids(metadata: dict[str, Any]) -> set[str]:
    """Return packages reachable from the root through normal dependency edges."""
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [metadata["resolve"]["root"]]
    reachable: set[str] = set()
    while pending:
        package_id = pending.pop()
        if package_id in reachable:
            continue
        reachable.add(package_id)
        for dependency in nodes[package_id].get("deps", []):
            if any(
                kind.get("kind") is None for kind in dependency.get("dep_kinds", [])
            ):
                pending.append(dependency["pkg"])
    return reachable


def build_sbom(
    metadata: dict[str, Any],
    archives: list[Path],
    release_version: str,
    source_revision: str,
    created_at: str,
) -> dict[str, Any]:
    runtime_ids = runtime_package_ids(metadata)
    packages = sorted(
        (package for package in metadata["packages"] if package["id"] in runtime_ids),
        key=lambda item: item["id"],
    )
    package_ids = {
        package["id"]: spdx_id("Package", package["id"]) for package in packages
    }
    root_id = metadata["resolve"]["root"]
    spdx_packages = []
    for package in packages:
        item = {
            "SPDXID": package_ids[package["id"]],
            "name": package["name"],
            "versionInfo": package["version"],
            "downloadLocation": "NOASSERTION",
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": spdx_license(package),
            "copyrightText": "NOASSERTION",
            "externalRefs": [
                {
                    "referenceCategory": "PACKAGE-MANAGER",
                    "referenceType": "purl",
                    "referenceLocator": package_purl(package),
                }
            ],
        }
        spdx_packages.append(item)

    files = []
    relationships = [
        {
            "spdxElementId": "SPDXRef-DOCUMENT",
            "relationshipType": "DESCRIBES",
            "relatedSpdxElement": package_ids[root_id],
        }
    ]
    for archive in archives:
        file_id = spdx_id("File", archive.name)
        files.append(
            {
                "SPDXID": file_id,
                "fileName": f"./{archive.name}",
                "checksums": [
                    {"algorithm": "SHA256", "checksumValue": sha256(archive)}
                ],
                "licenseConcluded": "NOASSERTION",
                "copyrightText": "NOASSERTION",
            }
        )
        relationships.append(
            {
                "spdxElementId": "SPDXRef-DOCUMENT",
                "relationshipType": "DESCRIBES",
                "relatedSpdxElement": file_id,
            }
        )

    for node in sorted(
        (node for node in metadata["resolve"]["nodes"] if node["id"] in runtime_ids),
        key=lambda item: item["id"],
    ):
        runtime_dependencies = {
            dependency["pkg"]
            for dependency in node.get("deps", [])
            if dependency["pkg"] in runtime_ids
            and any(
                kind.get("kind") is None for kind in dependency.get("dep_kinds", [])
            )
        }
        for dependency in sorted(runtime_dependencies):
            relationships.append(
                {
                    "spdxElementId": package_ids[node["id"]],
                    "relationshipType": "DEPENDS_ON",
                    "relatedSpdxElement": package_ids[dependency],
                }
            )

    return {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"{PACKAGE_NAME}-{release_version}-binary-release",
        "documentNamespace": f"{REPOSITORY}/releases/{release_version}/sbom/{source_revision}",
        "creationInfo": {
            "created": created_at,
            "creators": ["Tool: optiflow-release-bundle/1"],
            "licenseListVersion": "3.27",
        },
        "documentDescribes": [package_ids[root_id]],
        "packages": spdx_packages,
        "files": files,
        "relationships": relationships,
    }


def build_provenance(
    archives: list[Path],
    repository_root: Path,
    release_version: str,
    source_revision: str,
    created_at: str,
) -> dict[str, Any]:
    subjects = [
        {"name": archive.name, "digest": {"sha256": sha256(archive)}}
        for archive in archives
    ]
    return {
        "_type": "https://in-toto.io/Statement/v1",
        "subject": subjects,
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": f"{REPOSITORY}/.github/workflows/release.yml@v1",
                "externalParameters": {
                    "releaseVersion": release_version,
                    "targets": list(SUPPORTED_TARGETS),
                },
                "internalParameters": {},
                "resolvedDependencies": [
                    {
                        "uri": f"git+{REPOSITORY}.git",
                        "digest": {"gitCommit": source_revision},
                    },
                    {
                        "uri": f"{REPOSITORY}/blob/{source_revision}/Cargo.lock",
                        "digest": {"sha256": sha256(repository_root / "Cargo.lock")},
                    },
                ],
            },
            "runDetails": {
                "builder": {"id": WORKFLOW_IDENTITY},
                "metadata": {
                    "invocationId": f"urn:optiflow:release:{source_revision}:{release_version}",
                    "startedOn": created_at,
                    "finishedOn": created_at,
                },
                "byproducts": [],
            },
        },
    }


def write_checksum_manifest(
    directory: Path, names: list[str], output_name: str
) -> None:
    lines = []
    for name in sorted(names):
        path = directory / name
        if not path.is_file() or PurePosixPath(name).name != name:
            raise BundleError(f"checksum subject is missing or unsafe: {name}")
        lines.append(f"{sha256(path)}  {name}\n")
    (directory / output_name).write_text("".join(lines), encoding="utf-8")


def prepare(args: argparse.Namespace) -> None:
    repository_root = args.repository_root.resolve()
    input_directory = args.input_directory.resolve()
    output_directory = args.output_directory.resolve()
    validate_inputs(repository_root, args.release_version, args.source_revision)
    created_at = normalized_timestamp(args.created_at)

    if output_directory.exists() and any(output_directory.iterdir()):
        raise BundleError(f"output directory must be empty: {output_directory}")
    output_directory.mkdir(parents=True, exist_ok=True)

    archives = []
    qualifications = {}
    for target in SUPPORTED_TARGETS:
        binary = input_directory / f"{PACKAGE_NAME}-{target}" / PACKAGE_NAME
        archive = (
            output_directory / f"{PACKAGE_NAME}-{args.release_version}-{target}.tar.gz"
        )
        if args.release_version == "v0.1.0":
            deterministic_archive(binary, repository_root / "LICENSE", archive)
        else:
            qualified_directory = input_directory / f"{PACKAGE_NAME}-{target}"
            qualified_archive = qualified_directory / archive.name
            receipt = pilot.read_receipt(qualified_directory / "qualification.json")
            try:
                pilot.validate_receipt(receipt, target, args.release_version, args.source_revision, qualified_archive)
            except ValueError as error:
                raise BundleError(str(error)) from error
            # Preserve the exact native archive that was clean-installed and tested.
            shutil.copyfile(qualified_archive, archive)
            qualifications[target] = receipt
        archives.append(archive)

    if qualifications:
        write_json(output_directory / PILOT_EVIDENCE, {"schema": "optiflow.pilot-qualification-set.v1", "targets": qualifications})

    metadata = cargo_metadata(repository_root)
    write_json(
        output_directory / "sbom.spdx.json",
        build_sbom(
            metadata, archives, args.release_version, args.source_revision, created_at
        ),
    )
    write_json(
        output_directory / "provenance.json",
        build_provenance(
            archives,
            repository_root,
            args.release_version,
            args.source_revision,
            created_at,
        ),
    )
    write_checksum_manifest(
        output_directory,
        [archive.name for archive in archives] + list(primary_evidence(args.release_version)),
        SUBJECTS_MANIFEST,
    )


def parse_manifest(path: Path) -> dict[str, str]:
    records: dict[str, str] = {}
    for line_number, line in enumerate(
        path.read_text(encoding="utf-8").splitlines(), start=1
    ):
        match = re.fullmatch(r"([0-9a-f]{64})  ([A-Za-z0-9][A-Za-z0-9._-]*)", line)
        if match is None:
            raise BundleError(f"{path.name}:{line_number}: invalid checksum record")
        digest, name = match.groups()
        if name in records:
            raise BundleError(f"{path.name}: duplicate checksum subject: {name}")
        records[name] = digest
    if not records:
        raise BundleError(f"{path.name}: checksum manifest is empty")
    return records


def verify_manifest(
    directory: Path, manifest_name: str, expected_names: set[str]
) -> None:
    manifest = directory / manifest_name
    if not manifest.is_file():
        raise BundleError(f"required checksum manifest is missing: {manifest_name}")
    records = parse_manifest(manifest)
    if set(records) != expected_names:
        missing = sorted(expected_names - set(records))
        unexpected = sorted(set(records) - expected_names)
        raise BundleError(
            f"{manifest_name} inventory mismatch; missing={missing}, unexpected={unexpected}"
        )
    for name, expected in records.items():
        actual = sha256(directory / name)
        if actual != expected:
            raise BundleError(f"{manifest_name}: checksum mismatch for {name}")


def archive_names(release_version: str) -> set[str]:
    return {
        f"{PACKAGE_NAME}-{release_version}-{target}.tar.gz"
        for target in SUPPORTED_TARGETS
    }


def primary_evidence(release_version: str) -> set[str]:
    return set(PRIMARY_EVIDENCE) | (set() if release_version == "v0.1.0" else {PILOT_EVIDENCE})


def verify_evidence(
    directory: Path, release_version: str, source_revision: str, archives: set[str]
) -> None:
    provenance = json.loads((directory / "provenance.json").read_text(encoding="utf-8"))
    if provenance.get("predicateType") != "https://slsa.dev/provenance/v1":
        raise BundleError("provenance predicateType is not SLSA provenance v1")
    subjects = {
        item["name"]: item.get("digest", {}).get("sha256")
        for item in provenance.get("subject", [])
        if isinstance(item, dict) and isinstance(item.get("name"), str)
    }
    if subjects != {name: sha256(directory / name) for name in archives}:
        raise BundleError("provenance subjects do not match release archives")
    predicate = provenance.get("predicate", {})
    builder = predicate.get("runDetails", {}).get("builder", {}).get("id")
    if builder != WORKFLOW_IDENTITY:
        raise BundleError("provenance builder identity is not the release workflow")
    dependencies = predicate.get("buildDefinition", {}).get("resolvedDependencies", [])
    source_digests = [
        item.get("digest", {}).get("gitCommit")
        for item in dependencies
        if item.get("uri") == f"git+{REPOSITORY}.git"
    ]
    if source_digests != [source_revision]:
        raise BundleError(
            "provenance source revision does not match the requested commit"
        )

    sbom = json.loads((directory / "sbom.spdx.json").read_text(encoding="utf-8"))
    if sbom.get("spdxVersion") != "SPDX-2.3":
        raise BundleError("SBOM is not SPDX 2.3")
    described_files = {
        item["fileName"].removeprefix("./"): item.get("checksums", [{}])[0].get(
            "checksumValue"
        )
        for item in sbom.get("files", [])
        if isinstance(item, dict) and isinstance(item.get("fileName"), str)
    }
    if described_files != {name: sha256(directory / name) for name in archives}:
        raise BundleError("SBOM subjects do not match release archives")
    if not sbom.get("packages"):
        raise BundleError("SBOM contains no resolved Cargo components")
    if release_version not in sbom.get("name", ""):
        raise BundleError("SBOM release identity does not match the requested version")
    if release_version != "v0.1.0":
        qualification = pilot.read_receipt(directory / PILOT_EVIDENCE)
        if qualification.get("schema") != "optiflow.pilot-qualification-set.v1" or set(qualification.get("targets", {})) != set(SUPPORTED_TARGETS):
            raise BundleError("pilot qualification target inventory mismatch")
        for target, receipt in qualification["targets"].items():
            try:
                pilot.validate_receipt(receipt, target, release_version, source_revision,
                                       directory / f"{PACKAGE_NAME}-{release_version}-{target}.tar.gz")
            except ValueError as error:
                raise BundleError(str(error)) from error


def finalize(args: argparse.Namespace) -> None:
    directory = args.bundle_directory.resolve()
    if not (directory / SIGNATURE).is_file():
        raise BundleError("signature.json must exist before finalizing the bundle")
    names = sorted(path.name for path in directory.iterdir() if path.is_file())
    if COMPLETE_MANIFEST in names:
        raise BundleError(
            "SHA256SUMS already exists; refuse to overwrite release evidence"
        )
    write_checksum_manifest(directory, names, COMPLETE_MANIFEST)


def verify(args: argparse.Namespace) -> None:
    directory = args.bundle_directory.resolve()
    validate_inputs(
        args.repository_root.resolve(), args.release_version, args.source_revision
    )
    archives = archive_names(args.release_version)
    expected_subjects = archives | primary_evidence(args.release_version)
    expected_complete = expected_subjects | {SUBJECTS_MANIFEST, SIGNATURE}
    actual = {path.name for path in directory.iterdir() if path.is_file()}
    if actual != expected_complete | {COMPLETE_MANIFEST}:
        raise BundleError(
            f"release bundle inventory mismatch; expected={sorted(expected_complete | {COMPLETE_MANIFEST})}, "
            f"actual={sorted(actual)}"
        )
    verify_manifest(directory, SUBJECTS_MANIFEST, expected_subjects)
    verify_manifest(directory, COMPLETE_MANIFEST, expected_complete)
    verify_evidence(directory, args.release_version, args.source_revision, archives)

    if not args.skip_signature_verification:
        completed = subprocess.run(
            [
                args.cosign,
                "verify-blob",
                "--bundle",
                str(directory / SIGNATURE),
                "--certificate-identity",
                WORKFLOW_IDENTITY,
                "--certificate-oidc-issuer",
                OIDC_ISSUER,
                str(directory / SUBJECTS_MANIFEST),
            ],
            check=False,
        )
        if completed.returncode != 0:
            raise BundleError("Cosign rejected the signed release-subjects manifest")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    subparsers = root.add_subparsers(dest="command", required=True)

    prepare_parser = subparsers.add_parser(
        "prepare", help="create unsigned release evidence"
    )
    prepare_parser.add_argument("--repository-root", type=Path, default=Path.cwd())
    prepare_parser.add_argument("--input-directory", type=Path, required=True)
    prepare_parser.add_argument("--output-directory", type=Path, required=True)
    prepare_parser.add_argument("--release-version", required=True)
    prepare_parser.add_argument("--source-revision", required=True)
    prepare_parser.add_argument("--created-at", required=True)
    prepare_parser.set_defaults(function=prepare)

    finalize_parser = subparsers.add_parser(
        "finalize", help="write complete checksums after signature creation"
    )
    finalize_parser.add_argument("--bundle-directory", type=Path, required=True)
    finalize_parser.set_defaults(function=finalize)

    verify_parser = subparsers.add_parser(
        "verify", help="verify release bundle evidence"
    )
    verify_parser.add_argument("--repository-root", type=Path, default=Path.cwd())
    verify_parser.add_argument("--bundle-directory", type=Path, required=True)
    verify_parser.add_argument("--release-version", required=True)
    verify_parser.add_argument("--source-revision", required=True)
    verify_parser.add_argument("--cosign", default="cosign")
    verify_parser.add_argument("--skip-signature-verification", action="store_true")
    verify_parser.set_defaults(function=verify)
    return root


def main() -> int:
    args = parser().parse_args()
    try:
        args.function(args)
    except (BundleError, FileNotFoundError, ValueError, KeyError, tarfile.TarError) as error:
        print(f"release bundle error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
