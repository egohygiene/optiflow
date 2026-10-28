#!/usr/bin/env python3
"""Propose a lock-only corpus binding update; --write explicitly accepts it.

This is source authoring, not corpus execution or dependency qualification.
The existing generator remains the authority for all expected bindings.
"""

import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
INDEX = ROOT / "tests/fixtures/filesystem/index.json"
LOCK = "cargo_lock_sha256"
INDEX_KEYS = {LOCK, "cases", "catalog_sha256", "generator_sha256", "license", "schema"}


def canonical(value):
    return (json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True) + "\n").encode()


def parse_document(data, label):
    def unique_keys(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key: {key}")
            result[key] = value
        return result

    def invalid_constant(value):
        raise ValueError(f"invalid JSON constant: {value}")

    try:
        return json.loads(data.decode("utf-8"), object_pairs_hook=unique_keys,
                          parse_constant=invalid_constant)
    except (UnicodeError, ValueError) as error:
        raise ValueError(f"{label}: {error}") from error


def validate_index(index, label):
    if not isinstance(index, dict) or set(index) != INDEX_KEYS:
        raise ValueError(f"{label}: unexpected or missing index keys")
    if index["schema"] != "optiflow.filesystem-corpus-index.v1" or index["license"] != "MIT":
        raise ValueError(f"{label}: unsupported schema or license")
    if not isinstance(index[LOCK], str) or re.fullmatch(r"[0-9a-f]{64}", index[LOCK]) is None:
        raise ValueError(f"{label}: expected a lowercase SHA256 lock digest")
    # The existing generator owns the remaining structure and values. Exact
    # non-lock comparison below avoids maintaining a second fixture schema.


def read_index(path):
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode):
        raise ValueError("index must be an existing regular file, not a symlink")
    return path.read_bytes(), metadata.st_mode


def refresh_index(path, expected, write=False):
    """Return (state, old_digest, new_digest), refusing any non-lock change."""
    original, mode = read_index(path)
    current = parse_document(original, "existing index")
    validate_index(current, "existing index")
    validate_index(expected, "generated index")
    if original != canonical(current):
        raise ValueError("existing index is not canonical JSON; review it separately")
    if {key: value for key, value in current.items() if key != LOCK} != {
            key: value for key, value in expected.items() if key != LOCK}:
        raise ValueError("non-lock corpus drift; review generator, catalog, cases and proofs separately")

    old, new = current[LOCK], expected[LOCK]
    if old == new:
        return "unchanged", old, new
    if not write:
        return "proposal", old, new

    # Canonical input and a single field substitution retain every other byte.
    current[LOCK] = new
    replacement = canonical(current)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=f".{path.name}.",
                                         delete=False) as output:
            temporary = Path(output.name)
            os.fchmod(output.fileno(), stat.S_IMODE(mode))
            output.write(replacement)
            output.flush()
            os.fsync(output.fileno())
        if read_index(path)[0] != original:
            raise ValueError("index changed while preparing the update; review and retry")
        os.replace(temporary, path)
        temporary = None
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    return "updated", old, new


def load_generator():
    """Load definitions only; never invoke the runner's main or execute paths."""
    path = ROOT / "scripts/filesystem-corpus.py"
    spec = importlib.util.spec_from_file_location("filesystem_corpus_lock_source", path)
    module = importlib.util.module_from_spec(spec)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true",
                        help="accept only a reviewed Cargo.lock binding change")
    args = parser.parse_args(argv)
    try:
        generator = load_generator()
        catalog = parse_document(generator.CATALOG.read_bytes(), "catalog")
        expected = generator.generated_index(catalog)
        state, old, new = refresh_index(INDEX, expected, write=args.write)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"refused: {error}", file=sys.stderr)
        return 1
    if state == "unchanged":
        print(f"unchanged: Cargo.lock binding is already {new}; no file written")
    else:
        print(f"{state}: cargo_lock_sha256 {old} -> {new}")
        if state == "proposal":
            print("No file written. Review the dependency change, then invoke with --write.")
    print("Source bindings only; no build, tests, or corpus execution performed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
