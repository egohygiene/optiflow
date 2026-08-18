---
title: Getting started
description: Build optiflow, inspect its environment, and run a first read-only scan.
---

# Getting started

The current release supports macOS and Linux. Rust is required when building
from source, and `ffprobe` is optional for stream-level media metadata.

## Build from source

```bash
git clone "https://github.com/egohygiene/optiflow.git"
cd "optiflow"
cargo build --locked --release
```

The release binary is written to `target/release/optiflow`.

## Inspect the environment

```bash
./target/release/optiflow doctor
```

`doctor` reports the local state location and optional capability availability.
It does not modify source media.

## Run a first scan

Start with a directory you control:

```bash
./target/release/optiflow scan "/path/to/Media"
```

The command prints a run identifier and commits immutable evidence beneath the
local state directory:

```text
runs/<run-id>/
├── effective-policy.json
├── run.json
└── report.json
```

## Generate a review-only plan

```bash
./target/release/optiflow plan exact-duplicates \
  --run "<run-id>"
```

The generated plan declares `"mutates_files": false`. Its proposed keep path
is a deterministic review default, not a claim that one copy is objectively
better.

## Use machine output

```bash
./target/release/optiflow \
  --output-format "json" \
  scan "/path/to/Media"
```

JSON owns standard output in machine mode. Diagnostics use the typed command
result instead of contaminating the JSON stream. See the
[CLI outcome contract](cli-contract.md) before automating on exit codes or
result fields.

## Next steps

- Review the [configuration precedence](configuration.md).
- Learn which claims are guaranteed by the [safety model](safety-model.md).
- Read the [state model](state-model.md) before moving or sharing state.

