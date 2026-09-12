# Security policy

## Supported versions

OptiFlow is pre-1.0. Security fixes are provided for the latest published
`v0.1.x` release only. Older pre-1.0 releases may be superseded without a
backport when a corrective release is available.

The supported release targets and verification contract are documented in
[Release and dependency policy](docs/release-policy.md). Building from source
requires the minimum Rust version declared in `Cargo.toml`.

## Report a vulnerability

Use GitHub's private
[security-advisory form](https://github.com/egohygiene/optiflow/security/advisories/new).
Do not disclose a suspected vulnerability, credential, exploit, or private
filesystem evidence in a public issue.

Include the affected version or commit, platform, impact, reproduction steps,
and any safe diagnostic evidence you can share. Remove personal media, paths,
tokens, and other secrets. Maintainers will acknowledge the report, establish a
private remediation plan, and publish a security advisory when disclosure is
appropriate. Response times are best effort while the project is pre-1.0.

For a leaked credential, revoke or rotate the credential before investigating
repository history. A scanner result is not evidence that a credential remains
usable.

## Scope

Security reports include unsafe source-media mutation, path or artifact
identity confusion, unbounded external processes, release-integrity failures,
and dependency or credential exposure. General bugs and feature requests belong
in the public issue tracker when they do not contain sensitive information.
