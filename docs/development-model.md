# Development model

optiflow combines specification-driven, schema-driven, and test-driven
development into one closed verification loop. These practices are not
independent phases: each one constrains the others, and a change is incomplete
when the four views disagree.

```text
specification
  -> schema and examples
  -> executable tests
  -> implementation
  -> observed evidence
  -> specification, schema, and test refinement
```

## Sources of truth

Each artifact is authoritative for a different concern:

| Concern | Canonical artifact |
| --- | --- |
| Product behavior and non-goals | `docs/mvp-spec.md` |
| Safety authority and invariants | `docs/safety-model.md` |
| State ownership and persistence | `docs/state-model.md` |
| Machine-readable artifact semantics | `docs/json-contract.md` |
| Wire shape and validation rules | `schemas/*.schema.json` |
| Database evolution | `migrations/*.sql` |
| Executable behavioral proof | Rust unit and integration tests |
| End-to-end user proof | `scripts/smoke-test.sh` |

No artifact silently overrides another. A disagreement is a defect that must be
resolved explicitly.

## Change protocol

Every behavior or contract change should move through the following packet:

1. **Specify intent.** Describe the user outcome, constraints, non-goals,
   failure behavior, and safety implications.
2. **Define examples.** Capture at least one accepted example and the important
   rejection or failure examples.
3. **Evolve contracts.** Update JSON Schemas and database migrations when the
   persisted or subprocess-visible shape changes.
4. **Write failing proofs.** Add the smallest unit, integration, contract, or
   end-to-end tests that demonstrate the missing behavior.
5. **Implement narrowly.** Change production code until the proofs pass without
   weakening an existing invariant.
6. **Reconcile evidence.** Feed implementation discoveries back into the spec,
   schema, examples, and tests.
7. **Record compatibility.** State whether the change is additive, breaking,
   migratable, or intentionally unsupported.

## Verification layers

The verification pyramid for optiflow is:

1. Pure domain and algorithm unit tests
2. Filesystem and SQLite component tests
3. CLI integration tests
4. JSON Schema contract tests using real serialized artifacts
5. Synthetic end-to-end safety tests
6. Cross-platform CI on Linux and macOS
7. Release-candidate installation and artifact verification

Higher layers do not replace lower layers. Safety-sensitive behavior should be
proven at the lowest practical layer and repeated at an end-to-end boundary.

## Contract evolution

- Consumers branch on the complete `schema_version` identifier.
- Checked-in schemas must describe the exact version emitted by the binary.
- Additive fields require compatible consumer behavior and updated examples.
- A semantic reinterpretation or incompatible shape requires a new schema
  identifier.
- Historical persisted data is handled through explicit defaults or migrations,
  never by pretending an older artifact was produced by the current contract.
- `flow` consumes released optiflow contracts; it does not couple to an
  unversioned source checkout.

## Safety traceability

Each future mutating capability must be traceable across:

```text
safety invariant
  -> acceptance criterion
  -> schema precondition
  -> failing test
  -> implementation guard
  -> execution evidence
  -> recovery or refusal behavior
```

If any link is absent, the capability is not ready to receive write authority.

## Definition of done

A change is complete only when:

- The relevant specification and non-goals are current.
- Machine contracts and examples match emitted artifacts.
- Tests cover success, refusal, and meaningful failure behavior.
- Local and CI commands exercise the same validation path.
- User-facing documentation describes shipped behavior only.
- Compatibility and migration consequences are recorded.
- No safety invariant was weakened implicitly.
