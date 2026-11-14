# Flow #137 checkpoint 10 — candidate fixture alias boundary

November 13, 2026. The native macOS failures recorded in the October 21 run
were not freshly reproduced: this execution environment is Linux. The owning
candidate fixture repair is already in the current source and described in
[the earlier Optiflow #137 handoff](HANDOFF.md). Its test-only private root is
created under the canonical system temporary parent, avoiding macOS's common
`/var` ancestor alias. Production `open_directory` still walks every component
with `NOFOLLOW | DIRECTORY`; that defense must not be relaxed to make a fixture
pass.

This checkpoint preserves that correction and adds one targeted regression.
The helper accepts a test-owned parent, canonicalizes it before creating the
private 0700 `TempDir`, then passes the resulting canonical root to production.
The authored Unix test builds an explicit `var-like-alias` parent and checks:

- canonical fixture creation reaches the intended committed-publication path;
- selecting the same directory through its aliased ancestor is refused before
  any staging or committed namespace is created;
- dropping the owned `TempDir` removes its canonical root without deleting the
  separate parent alias or physical parent.

The existing ten candidate commit/recovery/tamper/unknown-stage/symlink cases
remain in place, with this eleventh case added. No production algorithm, native
path admission, dependency, schema, approval or mutation gate changes. No
adoption of #93/#111 qualification or physical-volume support is implied.

## Actual observations

- Rustfmt 1.85.0 applied to and checked `src/candidate_artifact.rs`: passed.
- `python -B scripts/filesystem-corpus.py --check`: passed, 17 bindings; this
  test-only candidate module is not a catalog proof source and introduces no
  further index change.
- Rust compilation, all eleven candidate tests (including the new regression),
  native macOS/APFS, real removable volumes and hosted CI: **not run**.

Formatting success is syntax/layout evidence only. This source regression does
not certify a passing Linux test or native macOS fix. The historical native
failure, already-authored canonical-parent correction and still-missing native
execution evidence remain distinct.

## Qualification handoff

At the later authorized supported-host pass, use the exact selected source
revision and recorded toolchain. On native macOS and Linux, run the complete
`candidate_artifact::tests::` unit module through the existing locked Cargo
contract; preserve the actual OS/architecture, toolchain, canonical temporary
parent and test result in a bounded receipt. Confirm committed publication
reaches its intended assertions, alias/member refusal remains enforced and
unknown-stage preservation still holds. Do not replace Mac evidence with Linux
results or infer physical disconnect/power-loss, APFS properties, source-media
mutation authority or a release from fixture success. #88 stays unchanged and
last; #93/#111 retain their actual native/volume prerequisites.
