---
title: Release and dependency policy
description: Dependency admission, security gates, signed binary publication, support, and rollback for OptiFlow.
---

# Release and dependency policy

OptiFlow is an organization trust-policy class `R2` command-line application.
Its current adoption state is **piloting**: repository checks and the immutable
release path are enforced by their workflows. The public `v0.1.0` release is
complete; promotion to `conformant` still waits for organization ruleset
evidence.

| Adoption field | Value |
| --- | --- |
| Repository | `egohygiene/optiflow` |
| Class | `R2` versioned distributable software |
| Accountable owner | `szmyty` |
| Organization policy | `egohygiene/.github` trust policy at `b415c8029bf2fb5d474f367e7129791588ba3860` |
| Relay profile | `binary` from Relay `v1.5.0`, pinned to `1eada5142f7fc7da7862f335589e3b8f5884ffaf` |
| Effective state | `piloting` from 2026-09-12 |
| Latest public release | [`v0.1.0`](https://github.com/egohygiene/optiflow/releases/tag/v0.1.0) from `f04c82a0b0c677a2939ea351c4219602cd7181af` |
| Bot exemptions | Dependabot-authored lockfile and immutable action-pin updates; review and checks remain required |
| Active exceptions | None |

The organization policy defines the trust outcome. OptiFlow owns its binaries,
SBOM, provenance, signing identity, support boundary, and release decision.
Relay validates and publishes the reviewed bundle without inheriting that
authority.

## Dependency admission

Every new direct dependency or feature expansion must be justified in its pull
request. Reviewers evaluate:

- whether the capability belongs in OptiFlow and can be implemented safely
  without another runtime dependency;
- maintenance activity, security history, publisher identity, and the size of
  the resolved transitive graph;
- license compatibility with MIT distribution;
- default features, native build code, unsafe code, network behavior, and
  platform impact; and
- reproducibility under the minimum supported Rust version and `--locked`.

Released builds use crates.io dependencies resolved by committed lockfiles.
Mutable Git branches and undeclared registries are forbidden. Path dependencies
are permitted only for components released from this repository and must not
cross into a sibling checkout. Default features should be disabled when they
introduce unused capability or hidden network behavior.

`deny.toml` is the machine-enforced admission policy. It denies yanked crates,
unknown registries, unknown Git sources, wildcard requirements, and licenses
outside the reviewed allowlist. A policy exception must be narrow, identify the
crate and requirement, link evidence, name an owner, and include an expiry date.
There are currently no exceptions.

## Updates and vulnerability response

Dependabot proposes Cargo and GitHub Actions updates weekly. Patch and minor
updates may be grouped but still require the full pull-request checks. Major
updates receive an explicit compatibility review. Lockfile-only changes are
reviewed as executable supply-chain changes, not treated as generated noise.

An advisory affecting a reachable runtime path blocks a release. The preferred
response is an updated or removed dependency. If no fixed version exists, the
release remains blocked unless a time-limited documented exception demonstrates
that the vulnerable path is unreachable. Suppression without evidence is not
an accepted response.

## Automated security gates

The `Security and dependency policy` workflow runs on pull requests, `main`, a
weekly schedule, and manual dispatch.

| Gate | Enforcement |
| --- | --- |
| Licenses | `cargo-deny` validates the explicit SPDX allowlist |
| Vulnerabilities | RustSec advisories and yanked versions fail `cargo-deny` |
| Secrets | TruffleHog scans changed Git history and fails for verified or unknown findings |
| Supply chain | GitHub dependency review blocks high-severity additions; Cargo sources and release evidence are checked separately |
| Release evidence | Deterministic archive, SPDX SBOM, SLSA provenance, checksum, and tamper tests run at Rust 1.85 |

New release workflow actions are pinned to full commit SHAs. Updates must retain
the adjacent human-readable version comment and review the upstream change.
The release path uses only the permissions required by each job; OIDC authority
exists only while signing the release-subject manifest, and release write
authority exists only in Relay's publication job.

## Supported artifacts

The `v0.1.x` binary release supports these exact targets:

| Target | Support boundary |
| --- | --- |
| `x86_64-unknown-linux-gnu` | 64-bit Linux with glibc 2.35 or newer |
| `x86_64-apple-darwin` | Intel macOS supported by the current stable Rust toolchain |
| `aarch64-apple-darwin` | Apple silicon macOS supported by the current stable Rust toolchain |

`ffprobe` remains optional and is not bundled. Other operating systems,
architectures, libc implementations, package managers, installers, and code
signing/notarization channels are unsupported until they appear in this table
and in the release workflow.

When present, `ffprobe` media evidence records the exact discovered executable
path, version, and binary digest. The release does not claim compatibility with
every `ffprobe` build merely because the executable starts; each bounded result
must also satisfy the current semantic evidence contract.

Each platform archive contains one executable named `optiflow` plus `LICENSE`. The signed
bundle also contains:

- `release-subjects.sha256`, covering every platform archive plus the SBOM and
  provenance;
- `signature.json`, a keyless Sigstore/Cosign bundle for that subject manifest;
- `sbom.spdx.json`, an SPDX 2.3 inventory of the resolved Cargo graph bound to
  every archive digest;
- `provenance.json`, SLSA v1 provenance binding every archive to the repository,
  commit, lockfile, targets, and release workflow; and
- `SHA256SUMS`, covering every other file in the Relay input bundle.

The archive writer fixes member order, ownership, permissions, and timestamps.
The provenance timestamp derives from the source commit, so rerunning the same
release request produces identical unsigned bytes. The keyless signature is
the only intentionally fresh cryptographic evidence.

## Release procedure

1. Update `Cargo.toml` to the exact intended version, refresh `Cargo.lock`, and
   complete `CHANGELOG.md` through a reviewed pull request.
2. Require CI, adversarial, documentation, and security workflows to pass on
   the final default-branch commit. Resolve every vulnerability, secret, and
   release-evidence failure before continuing.
3. From the Actions page, dispatch **Publish signed binary release** on `main`
   with the matching unused `vMAJOR.MINOR.PATCH` value.
4. The workflow builds the three supported targets from the locked graph,
   smoke-tests native binaries, prepares deterministic archives and evidence,
   signs and verifies `release-subjects.sha256`, and uploads one complete
   bundle.
5. Relay's full-SHA-pinned `binary` profile revalidates the bundle and current
   default-branch identity before creating an annotated immutable tag and
   GitHub Release. A moved branch, reused contradictory tag, bad digest,
   unexpected file, invalid signature, or incomplete evidence fails closed.
6. Download the published assets and perform the independent verification below
   before announcing or distributing the release.

The release workflow does not publish crates.io packages, Homebrew formulas,
installers, or mutable “latest” aliases.

## Independent verification

Install GitHub CLI and Cosign, then verify the outer Relay artifact and inner
OptiFlow bundle. Replace the version as needed.

```bash
release_version="v0.1.0"
gh release download "${release_version}" \
  --repo "egohygiene/optiflow" \
  --pattern "binary-${release_version}.tar.gz" \
  --pattern "release-asset.sha256" \
  --pattern "release-evidence.json"
sha256sum --check "release-asset.sha256"
mkdir "optiflow-release"
tar --extract --gzip \
  --file "binary-${release_version}.tar.gz" \
  --directory "optiflow-release"
cd "optiflow-release"
sha256sum --check "SHA256SUMS"
cosign verify-blob \
  --bundle "signature.json" \
  --certificate-identity "https://github.com/egohygiene/optiflow/.github/workflows/release.yml@refs/heads/main" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  "release-subjects.sha256"
```

Also inspect `release-evidence.json`, `provenance.json`, and `sbom.spdx.json`.
Their repository, source revision, release version, workflow identity, and
archive digests must agree with the release you intended to install.

## Rollback and correction

Published tags, assets, signatures, SBOMs, and provenance are immutable
historical evidence. Do not delete, overwrite, or retarget them.

For a bad release:

1. stop any mutable distribution or announcement channel and publish a security
   advisory when confidentiality or user safety requires it;
2. identify the affected immutable version, artifact digests, impact, and safe
   workaround without exposing secrets;
3. revert or fix the source through normal review, advance the patch version,
   and publish a new signed release; and
4. link the corrective release from the affected release notes while retaining
   all original evidence for audit and incident review.

Because OptiFlow has no package-manager channel yet, rollback means withdrawing
recommendation of the affected version and publishing a corrective successor.
Local source media and OptiFlow state are never modified as part of a release
rollback.
