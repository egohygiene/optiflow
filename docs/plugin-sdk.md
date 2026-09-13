# Safe extension SDK

OptiFlow's extension boundary is additive and read-only. It lets explicitly
selected providers contribute evidence, policy or normalization facts, plans,
validation facts, reports, and lifecycle observations without acquiring a path
to source-media mutation or artifact publication.

The host contract identifier is `optiflow.extension-sdk.v1`. The four wire
documents have independent identifiers:

| Document | Identifier | Checked-in schema |
| --- | --- | --- |
| Provider declaration | `optiflow.extension-manifest.v1` | `schemas/extension-manifest-v1.schema.json` |
| Operator decision | `optiflow.extension-lock.v1` | `schemas/extension-lock-v1.schema.json` |
| Bounded request | `optiflow.extension-invocation.v1` | `schemas/extension-invocation-v1.schema.json` |
| Proposed result | `optiflow.extension-result.v1` | `schemas/extension-result-v1.schema.json` |

These documents do not replace run, report, plan, artifact-set, effective-policy,
or command-result contracts. A provider result is a proposed contribution. Core
OptiFlow must validate and incorporate it through the existing typed pipeline.

## Supported roles

| Role | Rust trait | Permitted contribution |
| --- | --- | --- |
| Inspector | `Inspector` | Fingerprinted observations about immutable artifact references |
| Analyzer | `Analyzer` | Derived, fingerprinted evidence |
| Normalization or policy contributor | `PolicyContributor` and `NormalizationPolicyContributor` | Typed evidence for later core policy resolution |
| Planner | `Planner` | Review-only operation descriptions that retain every core precondition |
| Validator | `Validator` | Versioned validation evidence |
| Report or export provider | `ReportProvider` and `ExportProvider` | Fingerprinted report projections |
| Lifecycle observer | `LifecycleObserver` | Read-only notification; its method returns no contribution |

Registration is typed. A handler cannot be registered for a manifest capability
with a different role, and duplicate registration fails. Dynamic-library loading,
source-tree scanning, executable search, and environment-based discovery are not
part of this contract.

## Declaration is not authorization

A provider manifest declares identity, version, publisher, configuration schemas,
capabilities, accepted and produced schemas, preconditions and postconditions,
determinism, cacheability, intended coverage, resource needs, locality, effects,
execution modes, replacements, fallbacks, and observer hooks.

The manifest is inert. An operator-owned lock must separately:

1. Pin the exact manifest bytes with BLAKE3-256.
2. Select exactly one declared version and execution mode.
3. Enable or disable the provider and choose `trusted_embedded`,
   `trusted_process`, or `disabled`.
4. Grant only a subset of declared effects.
5. Pin validated configuration values and their fingerprints.
6. For a process provider, name an absolute executable path, pin its bytes, and
   select a non-symlink working directory.
7. Set precedence and explicitly allow any declared replacement.

The lock's grant fingerprint binds enabled state, trust mode, effects,
precedence, replacement and fallback decisions, and execution mode. Editing
one of those values without updating the fingerprint makes the provider
unavailable. Fallback additionally requires both the provider declaration and
the operator lock's `allowed_fallbacks`; a declaration alone never authorizes
failover.

SDK v1 rejects requests for `source-mutation`, `destructive`, `sign`, and
`publish`. Other effects remain declarative and require an exact operator grant.
A process mode additionally requires `subprocess` in both manifest and lock.

## Deterministic resolution

Applications pass exact manifest and lock files to `ExtensionCatalog::load`.
The loader performs bounded stable reads, rejects symlinks and non-regular
documents, validates closed JSON Schemas, verifies internal fingerprints, and
requires one lock for each manifest. Only one version of a qualified extension
ID may be selected in a catalog.

For each capability ID, unavailable providers are retained with reasons.
Providers are sorted by operator precedence and stable identity. A unique
eligible provider at the highest precedence wins. Equal highest precedence is a
conflict and fails closed; lexical order is never used as hidden authorization.
If a unique preferred provider is unavailable, a lower provider is selected
only when its manifest declares the fallback and its operator lock allows that
exact relationship. Otherwise resolution stays unavailable. Replacement intent
is advisory: it must be declared and allowed for auditable evidence, but it
never overrides the operator-owned precedence decision.

Nothing searches `$PATH`, plugin directories, package registries, sibling
repositories, or the network.

## Embedded providers

An embedded provider is compiled into a trusted host binary and registered with
the role-specific method on `ExtensionRegistry`. The registry is created only
from an available `trusted_embedded` selection. It validates the invocation
before calling the handler and validates the result afterward.

`examples/embedded_roles.rs` contains minimal inspector, analyzer, planner, and
validator implementations. Its test loads the exact manifest and operator lock,
registers every typed role, and sends each result through the host acceptance
boundary:

```console
cargo test --locked --example embedded_roles
```

```rust
use std::sync::Arc;
use optiflow::extensions::{ExtensionRegistry, Inspector};

fn register(
    registry: &mut ExtensionRegistry,
    capability_id: String,
    inspector: Arc<dyn Inspector>,
) -> Result<(), optiflow::extensions::RegistryError> {
    registry.register_inspector(capability_id, inspector)
}
```

`ExtensionContext` exposes cooperative cancellation and a host-owned progress
callback. Cancellation is checked before and after handler execution. A
successful return is still only a candidate: schema, identity, authorization,
coverage, provenance, effects, fingerprints, role restrictions, and planning
preconditions must all pass.

## Process providers

`ProcessExtensionClient` accepts only an available `trusted_process` selection.
Immediately before execution it revalidates the non-symlink working directory
and stable BLAKE3 digest of the absolute executable. It invokes that exact path
directly with the manifest's argument array, never through a shell. The inherited
environment is cleared and the pinned working directory is used.

One bounded `optiflow.extension-invocation.v1` JSON document is written to
stdin. One bounded `optiflow.extension-result.v1` JSON document is expected on
stdout. Stderr is diagnostic-only and bounded. Timeout, output truncation,
non-zero exit, cancellation, stdin failure, and malformed JSON are distinct
typed failures. The executable digest and working directory are checked again
before any result is accepted.

The process contract does not pass source paths, writable state handles, secrets,
or publication credentials. Inputs are immutable artifact IDs, schema IDs,
content digests, and optional bounded normalized JSON evidence whose fingerprint
must equal its matching artifact reference. Checkpoint references are immutable
evidence references, so a fresh invocation can recover after a provider crash
without trusting process memory.

SDK v1 applies host ceilings even when a manifest asks for more: 10 minutes of
runtime, 4 MiB of stdin, 16 MiB of stdout, and 1 MiB of stderr. Cancellation is
polled by the host and terminates the child immediately. Manifests outside the
ceilings are invalid, not merely unavailable.

`trusted_process` is a trust decision, not an operating-system sandbox. A binary
running as the current user may exercise ambient OS access on its own. Network
and resource declarations are therefore auditable admission facts, not claims
of kernel enforcement. Run untrusted providers inside a separately configured
OS or container sandbox; OptiFlow does not silently treat an unknown executable
as safe.

The runnable reference provider is `examples/process_extension.rs`. It can emit
its matching manifest and uses only immutable references:

```console
cargo run --locked --example process_extension -- --print-manifest
```

`examples/create_reference_lock.rs` demonstrates creation of an explicit local
operator lock after the provider has been built:

```console
cargo build --locked --examples
cargo run --locked --example create_reference_lock -- \
  examples/extensions/reference-inspector.manifest.json \
  "$(pwd)/target/debug/examples/process_extension" \
  "$(pwd)"
```

Review the emitted lock before storing or using it. Changing either the manifest
or executable requires a new reviewed lock; no fallback to current bytes occurs.

## Result acceptance

The host rejects a result unless all of the following remain true:

- invocation, extension, capability, manifest digest, lock ID, exact lock digest, grant digest,
  configuration fingerprint, input fingerprints, and execution mode agree;
- every observed effect was declared by the capability and granted by the lock;
- evidence, report, and operation fingerprints match their values;
- consumed artifact IDs came from the invocation;
- unsuccessful outcomes publish no contributions;
- complete coverage has no limitations and consumes every invoked artifact,
  while partial coverage names at least one limitation;
- a partial-by-design provider never claims complete coverage;
- only planners contribute operations, only validators contribute validations,
  and only report providers contribute reports; and
- progress sequence numbers strictly increase, completed counts never decrease,
  and completed counts never exceed a declared total.

Planner contributions cannot execute. Each contribution must say
`mutates_source: false`, say `requires_core_execution: true`, and retain all six
preconditions: observation revalidation, full hashing, byte confirmation, plan
identity, explicit authorization, and provenance. Core planning, authorization,
execution, validation, evidence commit, and recovery remain outside the plugin.

Lifecycle observers are narrower still. Their Rust callback returns `()` and is
called only for declared read-only phases. Process observer invocations bind the
immutable lifecycle event inside the invocation and may return only diagnostics
and progress, never contributions, effects, or checkpoints. An observer cannot
replace, suppress, or alter the lifecycle event or its outcome.

## Operator inspection

All commands require repeated exact file selectors; omitting them is an error.
There is no default discovery location.

```console
optiflow extensions list \
  --manifest provider.manifest.json \
  --lock operator.lock.json

optiflow extensions inspect example.provider \
  --manifest provider.manifest.json \
  --lock operator.lock.json

optiflow extensions doctor \
  --manifest provider.manifest.json \
  --lock operator.lock.json
```

Use `--json` for the standard `optiflow.command-result.v1` envelope. Human and
JSON modes expose pinned identities, selection state, availability reasons, and
resolution conclusions. Unavailability or conflicts produce honest partial
coverage and exit code `3`; malformed declarations or selector failures produce
invalid input and exit code `2`.

## Compatibility and Flow integration

Contract identifiers are complete versions. Additive binary releases may add
new SDK implementations without changing `optiflow.extension-sdk.v1`; a semantic
wire break requires a new manifest, lock, invocation, or result identifier and
an explicit migration. Unknown SDK versions remain inspectable but unavailable.

The boundary follows the suite contract established by Flow issue #7: provider
manifests declare needs, while operator locks grant trust, effects, precedence,
and fallback. OptiFlow owns these native contracts and releases them with the
OptiFlow repository. A Flow adapter may map a released OptiFlow contract into
Flow's released extension envelopes, but neither repository imports the other's
source or follows an unpinned default branch.

The completed adversarial and signed-release prerequisites tracked by OptiFlow
#26 and #27 supplied the baseline for this SDK. OptiFlow #29 can extend CLI and
performance behavior later without bypassing this boundary or blocking its
independent release.

## Validation

Run the full repository gate plus the focused contract checks:

```console
task validate
task extensions:examples
./scripts/run-adversarial-tests.sh properties faults
```

`task contracts` validates every checked-in schema, regenerates the capability
reference in check mode, and compares the reference manifest byte-for-byte with
the Rust example's output.
