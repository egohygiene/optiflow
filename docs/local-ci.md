# Selected local validation

This repository supplies a reviewed `realm.local-ci/v1` consumer manifest for
[Flow #137](https://github.com/egohygiene/flow/issues/137). The runner remains
[Realm's existing owner](https://github.com/egohygiene/realm/blob/75c1b2e1475b972a0ce1c8594e8daba2e1cf8686/tools/realm_local_ci.py)
at reviewed revision `75c1b2e1475b972a0ce1c8594e8daba2e1cf8686`; it is not copied or reimplemented here.
This is a development-time tool relationship, not a product/sibling dependency.

## Reviewed selection

| Required check | Exact selected scope |
| --- | --- |
| `declared-compiler-library` | `cargo +1.89.0 check --lib --locked --offline --jobs 2`; a locked/offline library compile, bounded to two build jobs and 20 minutes |
| `workflow-budget-regressions` | The existing synthetic Python budget suite used by `task ci:budget:test` and the hosted source gate |
| `workflow-budget-source-plan` | The existing bounded `task ci:budget` plan, retaining deferred/unobserved product states |
| `workflow-budget-act` | Those same budget Python commands through an explicit act fixture, with a ten-minute outer process limit |

The manifest lists all-target/full Rust tests/Clippy, native hosts, real
media/model providers, hosted services and publication/distribution as
**unsupported by this selection**. The product-specific additional omission is
spelled out in the manifest. Optional coverage rows remain visible even when
all four selected required checks pass. A report's `complete: true` means only
this declared selection completed; it is not all CI, a green supported-release
matrix or authority for content, signing, release or publication.

The library target deliberately uses the declared minimum compiler 1.89.0.
Changing a product compiler or selected command requires reviewing this manifest;
installation of a different default `stable` toolchain does not change the check.
The local act fixture is outside `.github/workflows`, so adding it cannot start
GitHub Actions. Existing hosted workflow triggers, budget opt-ins, job conditions
and release-admission guards are unchanged.

## Prerequisites and planning

Use Python 3.10+, Git, the declared Rust toolchain, a reachable Linux Docker engine,
and Realm's checksum-verified **act 0.2.97** client. The runner image is pinned to
`ghcr.io/catthehacker/ubuntu:act-24.04@sha256:19619a010b204f44c5340bff7d39b68551109fe6a56a53f75201ea91e05d4270`. Initial image/Action acquisition can need network and disk space; no
model, application secret or production media is acquired by this selection.
No task installs tools or mutates your shell profile. `task` aliases additionally
need an explicitly installed compatible Go Task v3 client. All commands can be
called through the existing Python runner without Go Task.

The compile command is offline and locked. Provision the selected target's
locked Cargo dependencies explicitly before the first run, using the same
compiler and `cargo fetch --locked --target <selected-host-triple>`; that network
preparation is not compiler or product qualification. Prefer a task-private
`CARGO_TARGET_DIR` when running alongside other work.

With a reviewed Realm checkout and an explicitly selected act executable:

```sh
python3 -B /absolute/path/to/realm/tools/realm_local_ci.py plan --repository .
python3 -B /absolute/path/to/realm/tools/realm_local_ci.py run --repository . \
  --act /absolute/path/to/act --output /absolute/private/local-ci/optiflow-001
```

The output must be a **new** directory. Reports retain revision, dirty state,
selection, bounded logs, process exit/timeout and act selected-job completion.
Check failures, blocked prerequisites and explicit unsupported rows remain
visible. Plans read selected files without executing a check. Keep raw reports
private and inspect their logs before sharing; report presence or process success
alone does not qualify a candidate binary or generated media.

Equivalent Task aliases preserve selected paths with spaces and Unicode:

```sh
task ci:local:plan REALM_DIRECTORY=/absolute/path/to/realm
task ci:local REALM_DIRECTORY=/absolute/path/to/realm \
  ACT=/absolute/path/to/act OUTPUT=/absolute/private/local-ci/optiflow-001
```

Managed cloud workspaces that require their configured Docker proxy/CA can
explicitly append `--cloud-network` to the Python `run` invocation, or use
`task ci:local:cloud` with the same three variables. Ordinary desktop runs should
use `ci:local`. Do not supply production credentials, mount the Docker socket
inside the job, disable TLS or globally trust project act configuration.

## Workflow and native evidence boundary

The fixture's checkout step is act's local-copy emulation, not proof that the
remote checkout Action or GitHub permission context works. The pinned runner
image identifies selected Linux userland, while the Docker host still supplies
the kernel. Linux emulation on macOS and AMD64-on-ARM emulation are not native
macOS or native ARM64 qualification. Source checks do not grant native provider,
removable-volume or private-content mutation authority.

The user authorized the sprint's selected local builds/source checks; earlier
blanket deferrals remain historical for this stated scope. Full native/product
matrices, hosted dispatch/reruns, credentials, publication and Flow #88's final
pass remain separate. Record actual execution against the final integrated
revision rather than declaring a passing result from this authored selection.
