# optiflow architecture

## System overview

optiflow converts immutable filesystem observations into evidence-backed plans.
It does not let detection imply deletion.

```text
inputs
  -> conservative discovery
  -> content and media inventory
  -> persistent observations
  -> exact relationship evidence
  -> immutable report
  -> immutable review plan
```

The `v0.1.0` execution boundary ends at plan generation. Mutation, transactional
optimization, validation, quarantine, and recovery belong to later milestones
and must cross a separate explicit apply boundary.

## Ownership boundaries

optiflow owns:

- Input selection and filesystem traversal policy
- Path observations and content-derived evidence
- Cache invalidation from size and modification-time changes
- Media inventory normalization
- Duplicate relationship derivation
- Explainable reports and plans
- Schema-versioned subprocess contracts

optiflow delegates:

- Container and stream inspection to `ffprobe`
- Future media encoding and decoding to specialized adapters
- Future image optimization to tools such as OxiPNG or libvips
- Future audio fingerprinting to Chromaprint
- Future reference-based video quality measurement to VMAF

optiflow is not a media library, video editor, codec implementation, or general
system cleaner.

## Components

| Component | Responsibility |
| --- | --- |
| `cli` | Parse explicit commands, global output mode, and traversal policies |
| `app` | Coordinate command-level use cases and artifact boundaries |
| `discovery` | Traverse inputs without following links, hidden trees, or mount boundaries by default |
| `inventory` | Inspect actual content and normalize high-level media categories |
| `adapters::ffprobe` | Discover and invoke `ffprobe` directly without a shell |
| `hashing` | Stream complete BLAKE3-256 content hashes |
| `duplicates` | Derive exact groups from identical size and complete hash evidence |
| `state` | Persist runs, observations, groups, and reusable file analysis in SQLite |
| `planning` | Convert exact groups into non-mutating actions and explicit preconditions |
| `reports` | Commit JSON artifacts atomically and render CLI results |

## Data model

A path is an observation location, not the identity of content.

| Entity | Meaning |
| --- | --- |
| `ScanRun` | Immutable record of one scan invocation and policy set |
| `FileObservation` | Scan-time path, size, time, filesystem hints, classification, and evidence |
| `MediaDescriptor` | Normalized format and stream metadata returned by an adapter |
| `DuplicateGroup` | Derived exact relationship backed by complete-hash evidence |
| `Plan` | Immutable set of review actions with source-run identity |
| `FilePrecondition` | Facts that a future apply engine must re-prove |
| `CachedAnalysis` | Reusable analysis for an unchanged path, size, and modification time |

Future execution adds `ExecutionRun`, `Attempt`, `Artifact`,
`ValidationResult`, and `CommitRecord`; none are simulated in the current
schema.

## Exact-duplicate pipeline

1. Discover unique file paths.
2. Record filesystem metadata once.
3. Reuse analysis only when path, byte length, and modification nanoseconds
   match the cache key.
4. Classify actual content and optionally inspect media streams.
5. Group files by byte length.
6. Calculate complete BLAKE3 hashes only for groups with at least two members.
7. Derive groups from equal byte length and complete content hash.
8. Persist the report before presenting it to another pipeline.
9. Generate a separate review plan with stale-plan and byte-confirmation
   requirements.

## Failure model

- Traversal failures become run warnings and do not erase successful evidence.
- Content or hash read failures become unreadable observations.
- `ffprobe` failures become per-file warnings; hashing remains independent.
- Artifacts are written to a sibling temporary file, flushed, synchronized, and
  renamed into place.
- A scan row remains `running` after an unexpected interruption, making
  incomplete state distinguishable from a completed report.
- Source paths are opened read-only; no source-write API is present.

## Integration boundary

`optiflow` remains independently installable. In the Ego Hygiene suite, `flow`
invokes the CLI as a subprocess and consumes its versioned JSON artifacts.
Sibling tools compose through `flow` rather than embedding optiflow source or
taking unversioned dependencies on its default branch. This preserves
independent release cycles and keeps contract ownership with optiflow.

Schema identifiers are explicit. Consumers must branch on the complete schema
identifier and ignore additive fields they do not understand within a supported
contract version.

## Deferred architecture

The following are intentionally deferred until their transactional contracts
can be implemented and tested together:

- Destructive exact-duplicate apply
- Backup and quarantine strategies
- Temporary media outputs and atomic replacement
- Image, audio, and video optimization profiles
- Output validation and complete decodes
- Perceptual similarity and containment detection
- Content-addressed cache recognition after arbitrary moves
- Portable state export for removable volumes
