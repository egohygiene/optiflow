---
title: Architecture
description: How optiflow converts immutable observations into evidence-backed plans.
---

# Architecture

`optiflow` converts immutable filesystem observations into evidence-backed
reports and review-only plans. Detection never implies deletion authority.

## Runtime flow

```text
CLI / environment / TOML
          |
          v
  effective policy
          |
          v
      discovery
          |
          v
 content + media inventory
          |
          v
 persistent observations
          |
          v
 exact relationship evidence
          |
          v
 immutable report + plan
          |
          v
 typed command outcome
```

The `v0.1.x` authority boundary ends at plan generation. Mutation,
transactional replacement, validation, quarantine, and recovery require a
separate specification and execution architecture.

## Ownership boundaries

`optiflow` owns discovery policy, observations, content evidence, exact
relationship derivation, immutable artifacts, local state, and its versioned
CLI contracts.

Specialized tools remain behind typed adapters. The current media probe invokes
`ffprobe` without a shell; future encoders, optimizers, quality metrics, and
fingerprinters must follow the same explicit capability boundary.

## Verification loop

```text
specification
  -> schemas and examples
  -> executable tests
  -> implementation
  -> observed evidence
  -> specification refinement
```

The [development model](development-model.md) defines the required traceability
between behavior, safety invariants, machine contracts, tests, and emitted
evidence.

The repository-level
[architecture reference](https://github.com/egohygiene/optiflow/blob/main/ARCHITECTURE.md)
contains the complete component and data-model inventory.

