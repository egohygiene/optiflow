#!/usr/bin/env python3
"""Generate and drift-check the small, original PNG candidate corpus.

The encoder is deliberately implemented here rather than delegated to a PNG
library or an installed optimizer. Its stored and fixed-Huffman zlib streams
have byte-for-byte identical output on every supported Python installation.
This is fixture construction, not the OxiPNG provider under test in #94.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import tempfile
import zlib


ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "tests/fixtures/media/cases.json"
INDEX = ROOT / "tests/fixtures/media/index.json"
SIGNATURE = b"\x89PNG\r\n\x1a\n"
MAX_FILE_BYTES = 16 * 1024
MAX_TOTAL_BYTES = 128 * 1024
PROOF_SOURCES = (
    "examples/png-candidate-contract-v1.json",
    "tests/fixtures/media/provider_cases.json",
    "tests/png_corpus.rs",
    "tests/png_provider_corpus.rs",
)


def canonical(value):
    return (json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True) + "\n").encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def adler32(data):
    first = 1
    second = 0
    for byte in data:
        first = (first + byte) % 65521
        second = (second + first) % 65521
    return (second << 16) | first


class BitWriter:
    def __init__(self):
        self.data = bytearray()
        self.bits = 0
        self.count = 0

    def bits_lsb_first(self, value, count):
        for index in range(count):
            self.bits |= ((value >> index) & 1) << self.count
            self.count += 1
            if self.count == 8:
                self.data.append(self.bits)
                self.bits = 0
                self.count = 0

    def huffman(self, code, count):
        # RFC 1951 Huffman codes are sent high bit first, unlike other fields.
        for index in range(count - 1, -1, -1):
            self.bits_lsb_first((code >> index) & 1, 1)

    def finish(self):
        if self.count:
            self.data.append(self.bits)
        return bytes(self.data)


def zlib_stream(data, encoding):
    if len(data) > 65535:
        raise ValueError("one stored DEFLATE block limit exceeded")
    if encoding == "stored":
        # 78 01 is a valid zlib header; final uncompressed DEFLATE block.
        block = b"\x01" + struct.pack("<HH", len(data), len(data) ^ 0xffff) + data
    elif encoding == "fixed":
        writer = BitWriter()
        writer.bits_lsb_first(0b011, 3)  # BFINAL=1, BTYPE=01 (fixed Huffman).
        for symbol in data:
            if symbol < 144:
                writer.huffman(0x30 + symbol, 8)
            else:
                writer.huffman(0x190 + symbol - 144, 9)
        writer.huffman(0, 7)  # End-of-block symbol 256.
        block = writer.finish()
    else:
        raise ValueError(f"unknown DEFLATE encoding: {encoding}")
    return b"\x78\x01" + block + struct.pack(">I", adler32(data))


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def image(recipe):
    color = recipe["color"]
    encoding = recipe["encoding"]
    transform = recipe.get("transform", "none")
    width, height = 16, 8
    if color not in ("rgb", "rgba", "gray"):
        raise ValueError("unsupported corpus color recipe")
    pixel = bytearray({"rgb": (11, 22, 33), "rgba": (11, 22, 33, 0), "gray": (11,)}[color])
    if transform == "hidden_rgb_change":
        if color != "rgba":
            raise ValueError("hidden RGB change requires RGBA")
        pixel[0] = 12  # Alpha remains zero: rendering can hide this difference.
    raw = (b"\x00" + bytes(pixel) * width) * height  # Filter 0 on every row.
    header = struct.pack(">IIBBBBB", width, height, 8, {"rgb": 2, "rgba": 6, "gray": 0}[color], 0, 0, 0)
    if transform == "indexed":
        header = header[:9] + b"\x03" + header[10:]
    elif transform == "interlaced":
        header = header[:12] + b"\x01"
    compressed = zlib_stream(raw, encoding)
    if transform == "extra_zlib_byte":
        compressed += b"\x00"  # PNG chunk CRC remains correct; zlib stream is not complete.
    parts = [chunk(b"IHDR", header)]
    if color == "rgba":
        before = [chunk(b"tEXt", b"Author\x00Optiflow original fixture"), chunk(b"raNd", b"safe")]
        after = [chunk(b"tEXt", b"After\x00IDAT")]
        if transform == "metadata_moved":
            before, after = [before[1]], [before[0], *after]
        parts.extend(before)
    else:
        after = []
    if transform == "apng":
        parts.append(chunk(b"acTL", struct.pack(">II", 1, 0)))
    if transform == "iccp":
        parts.append(chunk(b"iCCP", b""))
    idat = chunk(b"IDAT", compressed)
    if transform == "bad_crc":
        idat = idat[:-1] + bytes((idat[-1] ^ 1,))
    parts.append(idat)
    parts.extend(after)
    parts.append(chunk(b"IEND", b""))
    encoded = SIGNATURE + b"".join(parts)
    if transform == "trailing_byte":
        encoded += b"\x00"
    elif transform == "truncated":
        encoded = encoded[:-1]  # Incomplete IEND CRC, with no oversized payload.
    if transform not in (
        "none", "hidden_rgb_change", "metadata_moved", "indexed", "interlaced",
        "extra_zlib_byte", "apng", "iccp", "bad_crc", "trailing_byte", "truncated",
    ):
        raise ValueError(f"unknown corpus transform: {transform}")
    return encoded


def generated(catalog):
    if (catalog.get("schema") != "optiflow.png-corpus.v1"
            or catalog.get("generator") != "scripts/png-corpus.py"
            or catalog.get("generator_version") != "1"):
        raise ValueError("unknown PNG corpus schema or generator")
    ids = [case["id"] for case in catalog["cases"]]
    if len(ids) != len(set(ids)) or not ids or len(ids) > 32:
        raise ValueError("duplicate, missing, or excessive PNG fixture IDs")
    output = {}
    for case in catalog["cases"]:
        if case["media_type"] != "image/png" or case["license"] != "MIT" or case["recipe"]["seed"] != 0:
            raise ValueError(f"invalid fixture provenance: {case['id']}")
        for role in ("source", "candidate"):
            path = case[role]
            prefix = "tests/fixtures/media/generated/"
            name = path.removeprefix(prefix)
            if (not path.startswith(prefix) or not name.endswith(".png")
                    or name in (".", "..") or "/" in name or "\\" in name):
                raise ValueError(f"unsafe generated path: {path}")
            data = image(case["recipe"][role])
            if len(data) > MAX_FILE_BYTES:
                raise ValueError(f"fixture exceeds file budget: {path}")
            if path in output and output[path] != data:
                raise ValueError(f"conflicting recipes for {path}")
            output[path] = data
        expected = case["expected"]
        if "limits" in case:
            if (set(case["limits"]) != {"decoded_bytes_per_image"}
                    or not 0 < case["limits"]["decoded_bytes_per_image"] < 1024 * 1024):
                raise ValueError(f"invalid bounded limit override: {case['id']}")
        if expected["outcome"] == "valid":
            reduction = len(output[case["source"]]) - len(output[case["candidate"]])
            if reduction <= 0 or expected["logical_reduction_bytes"] != reduction:
                raise ValueError(f"incorrect logical reduction: {case['id']}")
        elif expected["outcome"] != "refused" or not expected.get("reason"):
            raise ValueError(f"invalid expected outcome: {case['id']}")
    if sum(map(len, output.values())) > MAX_TOTAL_BYTES:
        raise ValueError("PNG corpus exceeds total file budget")
    return output


def index_for(catalog, output):
    return {
        "schema": "optiflow.png-corpus-index.v1",
        "generator": catalog["generator"],
        "generator_version": catalog["generator_version"],
        "generator_sha256": digest(Path(__file__).read_bytes()),
        "catalog_sha256": digest(CATALOG.read_bytes()),
        "license": "MIT",
        "proof_sources": [
            {"path": path, "sha256": digest((ROOT / path).read_bytes())}
            for path in PROOF_SOURCES
        ],
        "files": [{"path": path, "bytes": len(data), "sha256": digest(data)}
                  for path, data in sorted(output.items())],
        "cases": [{"id": case["id"], "recipe_sha256": digest(canonical(case["recipe"])),
                   "expected_sha256": digest(canonical(case["expected"])),
                   "limits_sha256": digest(canonical(case.get("limits", {}))),
                   "source_sha256": digest(output[case["source"]]),
                   "candidate_sha256": digest(output[case["candidate"]])}
                  for case in catalog["cases"]],
    }


def reject_symlink_components(path):
    # A reviewed regeneration must not follow a checkout symlink.
    current = ROOT
    for component in path.relative_to(ROOT).parts:
        current /= component
        if current.is_symlink():
            raise ValueError(f"symlinked corpus output path: {current}")


def replace_fixture(path, data):
    # Replace the directory entry so a hard-linked file outside this fixture
    # directory cannot have its contents rewritten.
    reject_symlink_components(path)
    if path.exists() and not path.is_file():
        raise ValueError(f"non-file corpus output path: {path}")
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as handle:
            temporary = Path(handle.name)
            handle.write(data)
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    choice = parser.add_mutually_exclusive_group(required=True)
    choice.add_argument("--check", action="store_true", help="refuse corpus/index drift without writing")
    choice.add_argument("--write-index", action="store_true", help="explicitly accept reviewed recipe changes")
    args = parser.parse_args()
    catalog = json.loads(CATALOG.read_bytes())
    output = generated(catalog)
    index = canonical(index_for(catalog, output))
    generated_dir = ROOT / "tests/fixtures/media/generated"
    if args.write_index:
        reject_symlink_components(generated_dir)
        generated_dir.mkdir(parents=True, exist_ok=True)
        for path, data in output.items():
            replace_fixture(ROOT / path, data)
        replace_fixture(INDEX, index)
        print(f"wrote {len(output)} generated PNGs and {len(catalog['cases'])} indexed cases")
    else:
        found = {str(path.relative_to(ROOT)) for path in generated_dir.glob("*.png")}
        if found != set(output) or not INDEX.exists() or INDEX.read_bytes() != index:
            parser.error("PNG corpus index or generated-file set drift")
        for path, data in output.items():
            if (ROOT / path).read_bytes() != data:
                parser.error(f"PNG corpus byte drift: {path}")
        print(f"PNG corpus current: {len(catalog['cases'])} cases, {len(output)} files")


if __name__ == "__main__":
    main()
