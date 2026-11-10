# ADR-0004: Bind stronger properties to held descriptors

Status: **Proposed**, 2026-11-10. This record grants no native qualification or release authority.

The existing macOS cross-copy property declaration does not prove which file
was observed. Add a separate versioned observation record and a safe borrowed
`File` backend port. Host code binds mode, ownership, timestamps, identity and
bounded complete content around two property reads. Darwin also compares
birthtime and BSD flags to descriptor metadata. ACL order and absence remain
owned by the existing property contract. A copy installer runs once, then the
host independently reads back both subjects. Failure is not retried or treated
as successful preservation. These functions publish and remove no paths.

Live observation tokens are private and cannot be deserialized. Persisted
records prove only self-consistency; their flags explicitly deny qualification
and source-removal authority. Their version and domain do not change historical
execution v1–v4 hashes or recovery eligibility.

The default backend refuses. The upstream descriptor ACL proposal in
`docs/work/111/acl-adapter/` is neither adopted nor qualified by this checkpoint.
No local unsafe code, subprocess ACL probe or path reopening is introduced.
Backend adoption, APFS durability, cross-volume restore, rename inheritance and
native qualification remain #111/#93 gates. Synthetic Linux cases are authored
and unrun. Tests, compiler, schema validation and CI remain deferred by the user.
