# Safety Model

## Current authority

optiflow `v0.1.0` has read authority only. It can create state and report
artifacts in its configured state directory, but it cannot change source media.

## Invariants

- Scanning never mutates source files.
- Detection and resolution remain separate phases.
- Equal duration, dimensions, filenames, or partial fingerprints never prove
  exact identity.
- An exact group requires identical byte length and complete BLAKE3-256 hash.
- A plan is immutable evidence, not permission to execute.
- The deterministic `keep_path` is not a quality score.
- Perceptual or containment matches will default to review in future releases.
- RAW files, paired assets, Live Photos, and generated pipeline masters may be
  inventoried but receive no special destructive policy.

## Future apply gate

Before any later release can mutate a duplicate candidate, it must:

1. Require an explicit apply command and approved plan identifier.
2. Verify the plan schema and source-run identity.
3. Re-stat every path and reject changed size or modification time.
4. Recalculate every complete content hash.
5. Compare candidate bytes directly with the selected retained file.
6. Check target filesystem and available space.
7. Use same-filesystem temporary artifacts where atomic rename matters.
8. Validate generated outputs before replacing an original.
9. Record attempts, validation, commits, and failures durably.
10. Default to recoverable backup or quarantine behavior.

Space-reclamation without backup may eventually exist, but it must be a
deliberate policy and never the universal default.

## Threats addressed

- Stale plans after a file changes
- Incorrect assumptions from filename extensions
- Symlink traversal outside the intended collection
- Unexpected mount traversal
- Hash collision risk before destructive resolution
- Interrupted report writes
- Optional adapter absence or malformed output
- External-volume and network-filesystem SQLite behavior

## Remaining risks

- Files can change during a read-only scan; a future apply engine must re-prove
  all preconditions.
- Modification-time precision varies by filesystem.
- Successful decoding by one tool does not guarantee universal compatibility.
- Cryptographic hashes provide extremely strong identity evidence but direct
  byte confirmation remains the final destructive gate.
