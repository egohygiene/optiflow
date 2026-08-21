# Configuration and Effective Policy

OptiFlow resolves configuration once, before any domain command runs. The
resolver captures the initial working directory and recognized environment,
selects files, reads each selected file into one stable in-memory snapshot,
merges typed leaves, validates the result, materializes every default, and
calculates two semantic identities.

Configuration is always read-only. Invalid configuration prevents scanning,
probing, reporting, planning, and state creation.

## Configuration document

The first public TOML contract is `optiflow.config.v1`. Every file must declare
the schema:

```toml
schema = "optiflow.config.v1"

[output]
format = "json"

[state]
directory = "./.optiflow-state"

[scan]
follow_symlinks = false
include_hidden = false
cross_filesystems = false
probe_media = true
```

The checked-in structural contract is
[`schemas/config-v1.schema.json`](../schemas/config-v1.schema.json). Missing or
unknown schemas, unknown sections or keys, duplicate keys, invalid scalar
types, and empty state paths are errors. OptiFlow does not preserve unknown
keys in an extension map.

## Source selection and precedence

Automatic resolution uses this complete order:

```text
compiled defaults
  < user configuration
  < nearest project configuration
  < recognized environment variables
  < explicit CLI arguments
```

The optional user file is:

- Linux: `$XDG_CONFIG_HOME/optiflow/optiflow.toml`, or
  `~/.config/optiflow/optiflow.toml` when `XDG_CONFIG_HOME` is unset.
- macOS: `~/Library/Application Support/optiflow/optiflow.toml`.

Project discovery starts at the initial working directory, walks its ancestors,
and selects only the nearest `optiflow.toml`. Scan input roots are never used as
independent discovery anchors.

`--config <FILE>` selects exactly one file and suppresses automatic user and
project discovery. `OPTIFLOW_CONFIG` does the same when `--config` is absent;
the CLI selector wins when both exist. `--no-config` suppresses all file
loading, including `OPTIFLOW_CONFIG`, but recognized setting environment
variables and CLI overrides still apply. `--config` and `--no-config` together
return `invalid_input`.

Missing automatic files are normal. A discovered file that exists but is
invalid, unreadable, not regular, or a symbolic link blocks execution. A
missing explicitly selected file is also invalid input.

## Supported settings

| Setting | Type and values | Default | File | Environment | CLI | Category | Evidence fingerprint |
| --- | --- | --- | :---: | :---: | :---: | --- | :---: |
| `output.format` | `human`, `json` | `human` | Yes | `OPTIFLOW_OUTPUT_FORMAT` | `--output-format`, `--json` | Presentation | No |
| `state.directory` | non-empty native path | platform state directory | Yes | `OPTIFLOW_STATE_DIRECTORY` | `--state-directory` | Operational | No |
| `scan.follow_symlinks` | boolean | `false` | Yes | `OPTIFLOW_FOLLOW_SYMLINKS` | `--follow-symlinks`, `--no-follow-symlinks` | Evidence | Yes |
| `scan.include_hidden` | boolean | `false` | Yes | `OPTIFLOW_INCLUDE_HIDDEN` | `--include-hidden`, `--exclude-hidden` | Evidence | Yes |
| `scan.cross_filesystems` | boolean | `false` | Yes | `OPTIFLOW_CROSS_FILESYSTEMS` | `--cross-filesystems`, `--stay-on-filesystem` | Evidence | Yes |
| `scan.probe_media` | boolean | `true` | Yes | `OPTIFLOW_PROBE_MEDIA` | `--probe`, `--no-probe` | Evidence | Yes |

Environment booleans accept exactly `true` or `false`, lower case. Output format
accepts exactly `human` or `json`. An empty recognized environment value is not
treated as absent. Unknown `OPTIFLOW_` variables do not automatically become
configuration keys.

Positional scan roots remain CLI-only. Worker counts, color preferences,
progress toggles, arbitrary executable paths or argument arrays, retry bounds,
exit codes, diagnostic wording, and mutation authorization are not public
configuration keys in v1.

## Path rules

- File-relative paths resolve from the declaring configuration file's
  directory.
- CLI-relative paths resolve from the captured initial working directory.
- Environment-relative paths resolve from the captured initial working
  directory.
- Compiled defaults use the platform-directory abstraction.

Paths are not canonicalized as proof of identity. OptiFlow performs no `~`
expansion, variable interpolation, command substitution, wildcard expansion,
or shell execution. Native paths remain lossless in structured source records;
terminal display escapes control characters.

## Inspection commands

All inspection commands use the same resolver as scan, report, and plan:

```bash
optiflow config validate
optiflow config show
optiflow config explain output.format
optiflow config explain scan.include_hidden
optiflow --config "./optiflow.toml" config validate
optiflow --no-config config show
```

`validate` checks selected sources and the generated effective policy without
creating state. `show` emits the complete policy, source records, provenance,
shadowed values, and fingerprints. `explain` accepts a documented file-style
path or canonical policy path and reports the winning value, lower-precedence
values, ownership category, evidence impact, locked status, constraints,
environment name, and CLI arguments. Unknown paths return `invalid_input`.

Human results use stdout and escaped diagnostics use stderr. JSON results use
one `optiflow.command-result.v1` document and preserve the actual exit status.

## Effective policy and fingerprints

Configuration documents contain partial declarations. Runtime code receives a
fully populated `optiflow.effective-policy.v1` object validated by
[`schemas/effective-policy-v1.schema.json`](../schemas/effective-policy-v1.schema.json).
It separates:

- evidence policy: eligibility, traversal, probing, exact grouping, stability,
  exclusions, and the bounded observation-attempt contract;
- operational policy: the concrete state directory;
- presentation policy: the concrete output format;
- locked safety invariants;
- per-leaf provenance;
- semantic fingerprints.

The `blake3-256` effective-configuration digest covers normalized evidence,
operational, presentation, and safety values. The separate evidence-policy
fingerprint covers evidence values and evidence-relevant locked invariants.
Neither includes provenance, shadow traces, diagnostics, timestamps, process
identifiers, unrelated environment state, or secrets. Presentation changes can
change the full digest but cannot change the evidence fingerprint. Equal policy
values reached through different sources have equal fingerprints.

Canonicalization uses the deterministic serialized field order of the typed
identity structures and the lossless tagged native-path representation. A
generated or loaded policy whose declared digests do not match its normalized
contents fails integrity validation. Fingerprints are provenance identifiers,
never mutation authorization.

## Locked safety invariants

The effective policy explicitly records that:

- source mutation is disabled;
- apply behavior is disabled;
- shell execution is disabled;
- generated artifacts must be schema-validated;
- artifact commits must remain atomic;
- exact grouping requires current stable evidence.

These values cannot be set in TOML, environment variables, or CLI arguments.
`[safety]` and `[safety_invariants]` attempts fail with
`configuration_locked_invariant`. Existing explicit traversal flags remain
evidence policy; their final values are recorded and fingerprinted.

## Historical policy

Every new scan publishes `effective-policy.json`, `run.json`, and `report.json`
as one marker-sealed set in the run artifact directory. The existing v3 run
field `artifact_directory` remains the immutable location reference. Historical
v1-v4 contracts and database migrations remain unchanged; current v5 documents
add artifact-set identity bindings.

Report and plan commands validate and expose the source policy artifact. A
different current evidence policy produces the informational
`source_policy_differs` diagnostic without overwriting historical evidence.
Older runs without the sidecar remain reviewable and report
`historical_policy_unknown`; OptiFlow never reconstructs missing historical
policy from current defaults. Future current-evidence or apply operations must
revalidate independently.

## Compatibility and security

- Unknown configuration schema identifiers and unknown keys fail closed.
- Existing keys and enum values do not silently change meaning.
- User files are never rewritten or automatically migrated.
- Artifact, effective-policy, configuration, and binary versions are distinct.
- A future configuration schema transition or `config migrate` command is
  explicit work, not an implicit rewrite.
- Files are captured into memory with before/after identity, length, and
  modification checks and at most two bounded read attempts.
- Configuration never executes a shell, expands commands or globs, stores raw
  environment snapshots, persists secrets, enables mutation, or bypasses
  validation and atomic commit.
