# Flow #137 checkpoint 3 — optiflow, November 13, 2026

The root lock remains byte-identical. The isolated fuzz lock still recorded jsonschema 0.56 even though the owning optiflow manifest requires 0.60. Cargo 1.85.0 refreshes jsonschema/jsonschema-regex/jsonschema-value/referencing to 0.60.2, matching the existing root lock. SQLite versions are unchanged; checkpoint 6 remains unresolved.

The maintainer selected the lock/format checkpoints, authorizing generated-source
maintenance. `cargo update --workspace` refreshed the owning metadata; registry
index metadata was read, with no package build scripts, product compiler or tests.
The subsequent explicit-toolchain `update --workspace --locked --offline` exited
successfully with the exact lock SHA-256 unchanged. The adjoining JSON records
that narrow observation. It is not all-target compilation, native qualification,
a security-policy result or proof that every locked dependency supports the MSRV.

`task ci:lock` and the existing CI lanes now enforce `--locked` metadata resolution;
Optiflow also checks its separate fuzz workspace. CI does not remove `--locked`
to pass or regenerate its expectations. A cold local index needs a separately
allowed metadata preparation step before adding `--offline`; the CI alias can
read registry metadata to avoid treating a missing cold cache as a source defect.

No Cargo manifest requirements, feature or MSRV policy changes here. All five
owning lockfiles were inspected/resolved across the suite; clean files were not
rewritten gratuitously. The separate Identity workspace is outside this product
checkpoint. Tooling was installed into a task-private directory solely for
Cargo metadata generation and the following formatting source edits.

Tests/builds, Clippy and native/hosted execution remain deferred. Before closing
checkpoint 3, run the selected locked compiler/build/conformance lanes when
authorized; this metadata result alone cannot establish a green Actions suite.
