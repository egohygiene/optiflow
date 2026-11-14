"""Run pinned Actionlint against workflow expressions and a scope regression.

Explicit tool selection only: no download, installation, build or dispatch.
ShellCheck/Pyflakes are separate lanes, outside this expression-only check.
"""
import argparse
from pathlib import Path
import subprocess
import sys

VERSION = "1.7.12"
ROOT = Path(__file__).resolve().parents[1]


def invoke(binary, *arguments):
    return subprocess.run(
        [str(binary), *arguments], cwd=ROOT, check=False,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--actionlint", type=Path, required=True)
    options = parser.parse_args()
    binary = options.actionlint.resolve(strict=True)
    version = invoke(binary, "-version")
    first = (version.stdout + version.stderr).decode("utf8", "replace").splitlines()
    if version.returncode != 0 or not first or first[0].strip().removeprefix("v") != VERSION:
        parser.error("select actionlint " + VERSION + " explicitly")
    flags = ["-oneline", "-shellcheck=", "-pyflakes="]
    invalid = invoke(binary, *flags, "tests/workflows/invalid-job-runner.yml")
    diagnostic = (invalid.stdout + invalid.stderr).decode("utf8", "replace")
    if invalid.returncode == 0 or 'context "runner" is not allowed here' not in diagnostic:
        raise SystemExit("workflow linter did not reject the job-level runner regression")
    workflows = sorted(path for path in (ROOT / ".github/workflows").iterdir()
                       if path.is_file() and path.suffix in {".yml", ".yaml"})
    if not workflows:
        raise SystemExit("no workflows selected")
    valid = invoke(binary, *flags, "tests/workflows/valid-step-runner.yml", *map(str, workflows))
    sys.stdout.buffer.write(valid.stdout)
    sys.stderr.buffer.write(valid.stderr)
    return valid.returncode if valid.returncode >= 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
