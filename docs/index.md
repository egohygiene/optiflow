---
title: optiflow documentation
description: Evidence-first media inventory, exact relationship proof, and review-only optimization planning.
---

# Know what is actually on disk

`optiflow` is a local-first Rust CLI that inventories media collections, proves
byte-identical relationships, calculates conservative storage outcomes, and
produces immutable plans for human review.

**Observe first. Prove relationships. Plan safely.**

!!! note "Read-only by design"

    The `v0.1.x` product has no apply, delete, replace, move, quarantine, or
    optimization command. A finding never becomes mutation authority.

## Choose a path

- **[Install and run a first scan](getting-started.md)** to exercise the CLI
  against a directory you control.
- **[Understand the safety boundary](safety-model.md)** before integrating
  `optiflow` with automation.
- **[Inspect the handle-bound observation protocol](observation-protocol.md)**
  to see how file races, retries, cache reuse, and refusal are handled.
- **[See the current v0.1.x milestone](current-release-milestone.md)** for the
  dependency-ready hardening queue and release exit criteria.
- **[Consume the CLI contract](cli-contract.md)** from `flow`, a shell script,
  or another subprocess client.
- **[Configure an effective policy](configuration.md)** with deterministic
  precedence, provenance, and fingerprints.
- **[Explore the architecture](/architecture/)** for the generated system and
  document graph, then **[inspect the runtime architecture](architecture.md)**
  to see how observations become evidence-backed reports and plans.
- **[Place OptiFlow in the platform](cloud-native-placement.md)** to understand
  how CNCF capabilities fit around the portable product boundary.

## What the current release proves

An exact duplicate group requires equal logical size and equal complete
BLAKE3-256 content hashes. The report preserves the evidence used to make that
claim. A generated plan is a separate, immutable, review-only artifact with
preconditions for a future execution boundary.

```text
inputs
  -> effective policy
  -> conservative discovery
  -> content inventory
  -> exact evidence
  -> immutable report
  -> review-only plan
  -> typed command result
```

## Machine-readable contracts

Every JSON command returns one `optiflow.command-result.v1` envelope. Run,
report, plan, configuration, and effective-policy documents have independent
schema identifiers so consumers can evolve safely without coupling to the Rust
implementation.

The checked-in contracts live in the
[`schemas/`](https://github.com/egohygiene/optiflow/tree/main/schemas)
directory.
