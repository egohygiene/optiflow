---
title: Getting started
description: Install or build optiflow, inspect its environment, and run a first read-only scan.
---

# Getting started

The binary release contract supports three Linux and macOS targets. Every
published archive must be checksum-bound, signed, and accompanied by an
artifact SBOM and provenance. Follow the
[independent verification procedure](release-policy.md#independent-verification)
before installing a downloaded binary. Rust is required only when building
from source, and `ffprobe` is optional for stream-level media metadata.

The currently published [`v0.1.0`
bundle](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0) targets
source revision `f04c82a0b0c677a2939ea351c4219602cd7181af`. A build from
current `main` can contain later read-only features; preserve its exact source
revision when comparing behavior with the release.

The `v0.1.1` candidate adds native external-drive pilot qualification; it is
not yet a published release. Use the [pilot operator guide](external-drive-pilot.md)
for separate local state, a first scan without probing, private report handling,
and interruption, reconnect, upgrade, and rollback procedures.

## Install a verified prebuilt binary

First complete the [independent verification
procedure](release-policy.md#independent-verification). It leaves the verified
platform archives under `optiflow-release/`. Select exactly one supported
target:

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`

Then extract that archive, copy the binary into a user-owned executable
directory, and run the read-only environment check:

```bash
release_version="v0.1.0"
release_target="x86_64-unknown-linux-gnu"
unpack_directory="optiflow-${release_version}-${release_target}"

mkdir -p "${unpack_directory}" "${HOME}/.local/bin"
tar --extract --gzip \
  --file "optiflow-release/optiflow-${release_version}-${release_target}.tar.gz" \
  --directory "${unpack_directory}"
cp "${unpack_directory}/optiflow" "${HOME}/.local/bin/optiflow"
chmod 0755 "${HOME}/.local/bin/optiflow"
"${HOME}/.local/bin/optiflow" doctor
```

Choose the target that exactly matches the current host. The release does not
claim support for other architectures or libc variants.

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

When building current `main`, the default probe policy and an available
`ffprobe` allow report v6 to include read-only lossless-PNG profile evidence.
That post-release capability is not present in the `v0.1.0` binary. Pass
`--no-probe` to record that this analysis was not requested. See
[media-profile evidence](media-profiles.md) before interpreting a review
candidate; it is not an output or savings guarantee.

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
- Review the [media-profile evidence boundary](media-profiles.md).
- Review supported platforms, security reporting, and rollback in the
  [release and dependency policy](release-policy.md).
