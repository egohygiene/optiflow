#!/usr/bin/env python3
"""Add an unsigned cohort index over existing qualified archives and sidecars.

Retains original archives and the existing release bundle verification owner.
No build, binary/installer invocation, signing, publishing or new native proof.
"""
import argparse
import json
from pathlib import Path
import owner_reports as reports
import release_bundle as bundle


def prepare(args):
    request_pin = reports.file_pin(args.request.absolute(), reports.MAX_JSON)
    request = reports.document(request_pin)
    reports.closed(request, ["schema_version", "version", "source_revision", "bundle_directory", "targets"])
    reports.require(request["schema_version"] == "optiflow.release-cohort-request/v1" and request["version"] == reports.owner_version(), "cohort Cargo version differs")
    root = reports.native_path(request["bundle_directory"])
    reports.require(root.resolve(strict=True) == root and root.is_dir(), "canonical existing release bundle required")
    tag, source = "v" + request["version"], request["source_revision"]
    archives = bundle.archive_names(tag)
    # Preserve the existing positive native receipt and exact qualified archive
    # owner. This is local supplied-evidence verification, not a new native run.
    bundle.verify_evidence(root, tag, source, archives)
    reports.require(0 < len(request["targets"]) <= 3, "bounded native target selection required")
    rows, targets = [], set()
    for selected in request["targets"]:
        reports.closed(selected, ["target", "archive", "descriptor", "executable_member"])
        target = selected["target"]
        reports.require(target in bundle.SUPPORTED_TARGETS and target not in targets, "unique supported native target required")
        targets.add(target)
        expected = root / ("optiflow-" + tag + "-" + target + ".tar.gz")
        reports.require(reports.native_path(selected["archive"]["path"]) == expected, "selected archive is not the retained qualified release asset")
        data = reports.pinned(selected["archive"])
        members = reports.members(data, selected["executable_member"], target)
        executable = next(m for m in members if m["name"] == selected["executable_member"])
        descriptor = reports.document(selected["descriptor"])
        reports.require(descriptor.get("schema_version") == "optiflow.release-contract/v1" and descriptor.get("tool") == "optiflow" and
                        descriptor.get("version") == request["version"] and descriptor.get("qualification") == "not_established_by_contract_description", "compiled descriptor identity differs")
        for schema in descriptor.get("schemas", []):
            reports.closed(schema, ["name", "document"])
            reports.require(schema["name"] in ["command-result.schema.json", "extension-lock-v1.schema.json"] and
                            schema["document"] == (reports.ROOT / "schemas" / schema["name"]).read_text(), "descriptor schemas differ from represented source")
        reports.require(len(descriptor.get("schemas", [])) == 2 and len({s["name"] for s in descriptor["schemas"]}) == 2, "complete compiled schema inventory required")
        rows.append({"target": target, "archive": selected["archive"], "descriptor": selected["descriptor"],
                     "executable_member": executable, "profiles": descriptor["profiles"]})
    output = args.output_directory.absolute()
    reports.require(output.parent.resolve(strict=True) == output.parent and not output.exists() and
                    not output.is_relative_to(root) and not root.is_relative_to(output), "fresh output separate from immutable bundle required")
    for selected in request["targets"]:
        for name in ["archive", "descriptor"]:
            path = reports.native_path(selected[name]["path"])
            reports.require(not path.is_relative_to(output) and not output.is_relative_to(path), "cohort output overlaps selected original")
    output.mkdir(mode=0o700)
    manifest = {"schema_version": "optiflow.release-cohort/v1", "version": request["version"], "source_revision": source,
                "targets": rows, "retained_target_objectives": ["x86_64-pc-windows-msvc"],
                "state": "unsigned_cohort_index_over_supplied_owner_evidence", "native_work_performed": False,
                "publisher_authenticated": False, "mutation_qualification_granted": False}
    path = output / "optiflow-release-cohort-v1.json"
    with path.open("x", encoding="utf-8") as stream:
        import os
        os.chmod(path, 0o600)
        json.dump(manifest, stream, ensure_ascii=True, sort_keys=True, indent=2)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    bundle.verify_evidence(root, tag, source, archives)
    reports.require(reports.file_pin(args.request.absolute(), reports.MAX_JSON) == request_pin, "request changed during assembly")
    for selected in request["targets"]:
        reports.pinned(selected["archive"])
        reports.pinned(selected["descriptor"], reports.MAX_JSON)
    print(json.dumps({"schema_version": "optiflow.release-cohort-output/v1", "manifest": reports.file_pin(path, reports.MAX_JSON),
                      "publication_performed": False, "qualification_granted": False}, sort_keys=True))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request", type=Path, required=True)
    parser.add_argument("--output-directory", type=Path, required=True)
    try:
        prepare(parser.parse_args())
    except (ValueError, KeyError, TypeError, OSError, bundle.BundleError) as error:
        raise SystemExit("cohort preparation refused: " + str(error))
