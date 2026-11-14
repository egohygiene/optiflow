#!/usr/bin/env python3
"""Stage only reviewed, repository-owned public examples for the composed site."""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import stat

PUBLIC_EXAMPLES = (
    "execution-properties-macos-cross-copy-v1.json",
    "video-candidate-request-v1.json",
    "video-production-request-v1.json",
    "video-production-request-v2.json",
    "video-folder-request-v1.json",
    "video_candidate.rs",
    "video_folder.rs",
)
MAX_EXAMPLE_BYTES = 262_144
MAX_TOTAL_BYTES = 1_048_576


def regular_bytes(path: Path) -> bytes:
    """Refuse aliases and observe the same small regular file around the read."""
    if path.is_symlink():
        raise ValueError("public example must not be a symlink")
    flags = (os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
             | getattr(os, "O_NONBLOCK", 0) | getattr(os, "O_CLOEXEC", 0))
    descriptor = os.open(path, flags)
    with os.fdopen(descriptor, "rb") as source:
        before = os.fstat(source.fileno())
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            raise ValueError("public example must be an unaliased regular file")
        if before.st_size > MAX_EXAMPLE_BYTES:
            raise ValueError("public example exceeds its byte limit")
        data = source.read(MAX_EXAMPLE_BYTES + 1)
        after = os.fstat(source.fileno())
    current = path.lstat()
    def observed(value):
        return (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns,
                value.st_ctime_ns, value.st_mode, value.st_nlink)
    if (len(data) != before.st_size or observed(before) != observed(after)
            or observed(before) != observed(current)):
        raise ValueError("public example changed during staging")
    return data


def stage(repository_root: Path, output: Path) -> None:
    root = repository_root.resolve(strict=True)
    source_root = root / "examples"
    if (source_root.is_symlink() or not source_root.is_dir()
            or source_root.resolve(strict=True) != source_root):
        raise ValueError("public example source must be the owning examples directory")
    expected = root / ".site/staging/examples"
    if output.absolute() != expected or output.parent.resolve(strict=True) != output.parent:
        raise ValueError("public example stage must use the managed canonical output")
    selected = [(name, regular_bytes(source_root / name)) for name in PUBLIC_EXAMPLES]
    if sum(len(data) for _, data in selected) > MAX_TOTAL_BYTES:
        raise ValueError("public example set exceeds its byte limit")
    output.mkdir()  # Fresh stage only; never adopt an existing directory or link.
    for name, data in selected:
        with (output / name).open("xb") as destination:
            destination.write(data)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    stage(arguments.repository_root, arguments.output)
    print(f"staged {len(PUBLIC_EXAMPLES)} explicitly reviewed public examples")


if __name__ == "__main__":
    main()
