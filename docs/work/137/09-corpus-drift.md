# Flow #137 checkpoint 9 — reviewed filesystem corpus bindings

November 13, 2026. The owning baseline is the checkpoint 4 source commit
`c6acb6b17d3b` (Optiflow PR #209), with the reviewed root lock from checkpoint 3.
This checkpoint refreshes source bookkeeping, not expected results or execution
proofs.

The generated index differs in exactly two places: the root Cargo.lock digest
and the `fs88-cache-signature` proof-source digest for `src/state.rs`. The old
source digest matches local ancestor `6d9f41703de1c2b920de91eba96326175079b3ab`.
The reviewed source diff adds migration 0010 dispatch/implementation plus the
canonical formatting pass; the selected cache-signature test assertions are
unchanged. All 17 fixture IDs/order, generator/catalog digests, recipe digests,
expected-outcome digests, literal payload digests and other proof-source digests
are preserved.

After that review, the existing generator's explicit `--write-index` authoring
path regenerated the canonical index. A restricted before/after comparison
confirmed only those two permitted differences. CI remains check-only and never
rewrites its own fixture expectations. The narrow lock-only update helper still
refuses any non-lock drift; its guard was not weakened to accept this source
change. The adjoining JSON records the actual digests and limited observations.

## Validation and remaining gates

- `python -B scripts/filesystem-corpus.py --check`: passed, 17 fixture bindings.
- `python -B -m unittest tests/test_filesystem_corpus_bindings.py tests/test_filesystem_corpus_lock.py`:
  passed, 14 synthetic Python checks. New cases distinguish source, lock and
  expectation identities and prohibit calls into the Rust build/execution paths.

No Rust compiler/tests, Linux property/fault/corpus tier, native macOS/APFS,
provider/media or hosted Actions execution ran. A current index does not mean
its bound proof passed. The bounded suites still need execution on both declared
hosts before checkpoint 9 is accepted. #88 remains the unchanged final pass;
physical/removable-volume qualification retains its original owners and limits.
