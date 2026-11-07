# Dependency merge and lock binding receipt — November 6, 2026

The maintainer authorized merging all open Flow-suite PRs. Only Optiflow
Dependabot patch updates were open; their exact file patches were reviewed:

| PR | Change | Merge commit |
| --- | --- | --- |
| #152 | signal-hook 0.4.5 → 0.4.6 | 1f9d93a36c3f881bc77e52bc6fa2ea522ece87f9 |
| #153 | crc32fast 1.5.2 → 1.5.3 | 04a1dcb6d6acb7d4bff7e940f97900c2c0f8f37c |
| #154 | jsonschema 0.60.1 → 0.60.2 | 8ccecad36dcea0df76230f34ced65df626c09612 |
| #155 | rustix 1.1.5 → 1.1.6 | 16b14116aaefbee0f74cdfe17d44071b5f1eba6e |
| #156 | setup-uv 10.3.2 → 10.3.3 | 384924c670e9009528b612be8832e41e333c8109 |

The final combined Cargo.lock SHA-256 is
`fc6655ba90b172dcb4ae925497978b6cbcd53b987a93b2f7753b20231207c901`.
Only cargo_lock_sha256 was replaced directly in the filesystem index; every
other index byte is preserved. No corpus helper, generator, validator or proof
runner was invoked. Source/Git inspection shows no changed catalog, generator
or referenced proof-source path in the sweep/preceding video checkpoint.
This is bookkeeping, not passing dependency/corpus/native compatibility proof.

The setup-uv SHA in pages.yml was checked against upstream v10.3.3
(`68559e99cc807c1eb7e9238fd43a15bba460eec2`); its stale version comment is corrected.
No workflow was dispatched. Source/merge commit messages and PR metadata include
[skip ci]. Tests, compiler/build, dependency resolution/install, lint/format,
schema/corpus/drift/docs validators, native/provider/media execution and CI remain
deferred. #136/#65 retain qualification; prior evidence/history is unchanged.
No release, tag, media, source replacement/disposal or new issue was created.
