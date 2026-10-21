//! Pure synthetic property evidence: no filesystem observation, ACL calls or copy operations.

use optiflow::contracts::{self, Contract};
use optiflow::execution::property_evidence::{
    Ace, AceKind, AclSnapshot, ExtendedAttribute, MacApfsCrossCopyPropertiesV1, PROFILE,
    PropertyError, ReviewedProperties, SCHEMA, Timestamp, parse, review,
};
use serde_json::{Value, json};

fn ace(kind: AceKind, principal: &str) -> Ace {
    Ace {
        principal_hex: principal.to_owned(),
        kind,
        flags: 0,
        permissions: 2,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn attribute(index: usize, value_bytes: usize) -> ExtendedAttribute {
    ExtendedAttribute {
        name_hex: hex(format!("user.fixture.{index:03}").as_bytes()),
        value_hex: "a5".repeat(value_bytes),
    }
}

fn snapshot() -> MacApfsCrossCopyPropertiesV1 {
    MacApfsCrossCopyPropertiesV1 {
        schema: SCHEMA.to_owned(),
        profile: PROFILE.to_owned(),
        uid: 501,
        gid: 20,
        mode: 0o100640,
        mtime: Timestamp {
            seconds: -1,
            nanoseconds: 999_999_999,
        },
        birthtime: Timestamp {
            seconds: -2,
            nanoseconds: 123,
        },
        bsd_flags: 0,
        xattrs: vec![attribute(0, 2)],
        acl: AclSnapshot::Present {
            flags: 0,
            entries: vec![ace(AceKind::Allow, "0123456789abcdef0123456789abcdef")],
        },
    }
}

fn value() -> Value {
    serde_json::to_value(snapshot()).unwrap()
}

fn parse_value(value: &Value) -> Result<ReviewedProperties, PropertyError> {
    parse(&serde_json::to_vec(value).unwrap())
}

fn fingerprint(snapshot: MacApfsCrossCopyPropertiesV1) -> String {
    review(snapshot).unwrap().fingerprint().to_owned()
}

#[test]
fn published_example_agrees_with_the_public_schema_and_reviewed_snapshot() {
    let bytes = include_str!("../examples/execution-properties-macos-cross-copy-v1.json").as_bytes();
    let document: Value = serde_json::from_slice(bytes).unwrap();
    contracts::validate(Contract::ExecutionMacosCopyProperties, &document).unwrap();
    let reviewed = parse(bytes).unwrap();
    assert_eq!(serde_json::to_value(reviewed.snapshot()).unwrap(), document);
    assert_eq!(reviewed.snapshot().schema, SCHEMA);
    assert_eq!(reviewed.snapshot().profile, PROFILE);
    assert_eq!(reviewed.fingerprint().len(), 64);
    assert!(
        reviewed
            .fingerprint()
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert_eq!(
        parse(&serde_json::to_vec(reviewed.snapshot()).unwrap())
            .unwrap()
            .fingerprint(),
        reviewed.fingerprint()
    );
}

#[test]
fn legacy_or_future_profiles_and_missing_acl_cannot_be_promoted() {
    for schema in [
        "optiflow.execution-properties.v1",
        "optiflow.execution-properties-macos-cross-copy.v2",
    ] {
        let mut document = value();
        document["schema"] = json!(schema);
        assert!(matches!(parse_value(&document), Err(PropertyError::Schema(_))));
    }
    for profile in ["linux/v3", "macos-apfs/v1", "macos-apfs-cross-copy/v2"] {
        let mut document = value();
        document["profile"] = json!(profile);
        assert!(matches!(parse_value(&document), Err(PropertyError::Unsupported(_))));
    }
    let mut missing = value();
    missing.as_object_mut().unwrap().remove("acl");
    assert!(matches!(parse_value(&missing), Err(PropertyError::Malformed(_))));
    let mut null = value();
    null["acl"] = Value::Null;
    assert!(matches!(parse_value(&null), Err(PropertyError::Malformed(_))));
}

#[test]
fn unknown_fields_are_refused_at_every_object_boundary_including_absent_acl() {
    for pointer in [
        "",
        "/mtime",
        "/birthtime",
        "/xattrs/0",
        "/acl",
        "/acl/entries/0",
    ] {
        let mut document = value();
        document
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unreviewed".to_owned(), json!(true));
        assert!(matches!(parse_value(&document), Err(PropertyError::Malformed(_))), "{pointer}");
        assert!(contracts::validate(Contract::ExecutionMacosCopyProperties, &document).is_err(), "{pointer}");
    }
    for acl in [
        json!({"state": "absent", "flags": 0}),
        json!({"state": "absent", "entries": []}),
        json!({"state": "absent", "unreviewed": null}),
        json!({"state": "present", "flags": 0, "entries": null}),
    ] {
        let mut document = value();
        document["acl"] = acl;
        assert!(matches!(parse_value(&document), Err(PropertyError::Malformed(_))));
        assert!(contracts::validate(Contract::ExecutionMacosCopyProperties, &document).is_err());
    }
}

#[test]
fn positional_sequences_cannot_bypass_object_only_schema_boundaries() {
    let principal = "0123456789abcdef0123456789abcdef";
    for (pointer, replacement) in [
        ("", json!([
            SCHEMA, PROFILE, 501, 20, 0o100640,
            {"seconds": -1, "nanoseconds": 999_999_999},
            {"seconds": -2, "nanoseconds": 123},
            0, [], {"state": "absent"}
        ])),
        ("/mtime", json!([-1, 999_999_999])),
        ("/birthtime", json!([-2, 123])),
        ("/xattrs/0", json!(["61", "a5"])),
        ("/acl", json!(["absent"])),
        ("/acl", json!(["present", 0, []])),
        ("/acl/entries/0", json!([principal, "allow", 0, 2])),
    ] {
        let mut document = value();
        *document.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            matches!(parse_value(&document), Err(PropertyError::Malformed(_))),
            "{pointer}"
        );
        assert!(
            contracts::validate(Contract::ExecutionMacosCopyProperties, &document).is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn shape_inspection_does_not_erase_duplicate_fields_before_typed_decoding() {
    let document = serde_json::to_string(&value()).unwrap();
    // Each replacement has a valid last value, so Value's last-key-wins
    // representation cannot alone detect these ambiguous original bytes.
    for (needle, replacement) in [
        ("\"uid\":501", "\"uid\":0,\"uid\":501"),
        ("\"seconds\":-1", "\"seconds\":0,\"seconds\":-1"),
        ("\"state\":\"present\"", "\"state\":\"absent\",\"state\":\"present\""),
        ("\"permissions\":2", "\"permissions\":0,\"permissions\":2"),
        ("\"value_hex\":\"a5a5\"", "\"value_hex\":\"\",\"value_hex\":\"a5a5\""),
    ] {
        assert!(document.contains(needle), "fixture must exercise {needle}");
        let duplicated = document.replacen(needle, replacement, 1);
        assert!(matches!(parse(duplicated.as_bytes()), Err(PropertyError::Malformed(_))));
    }
    let duplicate_acl = document.replacen(
        "\"acl\":",
        "\"acl\":{\"state\":\"absent\"},\"acl\":",
        1,
    );
    assert!(matches!(parse(duplicate_acl.as_bytes()), Err(PropertyError::Malformed(_))));
}

#[test]
fn hex_patterns_and_the_parser_reject_trailing_newlines_and_unicode() {
    for pointer in [
        "/xattrs/0/name_hex",
        "/xattrs/0/value_hex",
        "/acl/entries/0/principal_hex",
    ] {
        for suffix in ["\n", "\r\n", "\u{2028}", "é"] {
            let mut document = value();
            let encoded = format!("{}{suffix}", document.pointer(pointer).unwrap().as_str().unwrap());
            *document.pointer_mut(pointer).unwrap() = json!(encoded);
            assert!(parse_value(&document).is_err(), "{pointer}");
            assert!(
                contracts::validate(Contract::ExecutionMacosCopyProperties, &document).is_err(),
                "{pointer}"
            );
        }
    }
}

#[test]
fn absent_and_present_empty_acls_keep_distinct_canonical_identities() {
    let mut absent = snapshot();
    absent.acl = AclSnapshot::Absent {};
    let mut empty = snapshot();
    empty.acl = AclSnapshot::Present {
        flags: 0,
        entries: vec![],
    };
    let absent = review(absent).unwrap();
    let empty = review(empty).unwrap();
    assert_ne!(absent.fingerprint(), empty.fingerprint());
    assert_eq!(
        serde_json::to_value(&absent.snapshot().acl).unwrap(),
        json!({"state": "absent"})
    );
    assert_eq!(
        serde_json::to_value(&empty.snapshot().acl).unwrap(),
        json!({"state": "present", "flags": 0, "entries": []})
    );
}

#[test]
fn acl_entry_order_and_duplicates_are_retained_and_change_the_fingerprint() {
    let allow = ace(AceKind::Allow, "ffffffffffffffffffffffffffffffff");
    let deny = ace(AceKind::Deny, "00000000000000000000000000000000");
    let mut fingerprints = Vec::new();
    for entries in [
        vec![allow.clone(), deny.clone()],
        vec![deny.clone(), allow.clone()],
        vec![allow.clone(), deny.clone(), allow.clone()],
    ] {
        let mut input = snapshot();
        input.acl = AclSnapshot::Present { flags: 0, entries };
        let original = serde_json::to_value(&input).unwrap();
        let reviewed = review(input).unwrap();
        assert_eq!(serde_json::to_value(reviewed.snapshot()).unwrap(), original);
        assert!(!fingerprints.iter().any(|previous| previous == reviewed.fingerprint()));
        fingerprints.push(reviewed.fingerprint().to_owned());
    }
}

#[test]
fn opaque_acl_principals_are_preserved_without_name_or_identity_resolution() {
    let mut input = snapshot();
    input.acl = AclSnapshot::Present {
        flags: 0x0002_ffff,
        entries: vec![Ace {
            principal_hex: "ffffffffffffffffffffffffffffffff".to_owned(),
            kind: AceKind::Deny,
            flags: 0x1f0,
            permissions: 0x01f0_3ffe,
        }],
    };
    let expected = serde_json::to_value(&input).unwrap();
    assert_eq!(
        serde_json::to_value(review(input).unwrap().snapshot()).unwrap(),
        expected
    );
    for principal in [
        "501",
        "owner@example.invalid",
        "0123456789ABCDEF0123456789ABCDEF",
        "0000000000000000000000000000000",
        "gggggggggggggggggggggggggggggggg",
    ] {
        let mut document = value();
        document["acl"]["entries"][0]["principal_hex"] = json!(principal);
        assert!(matches!(parse_value(&document), Err(PropertyError::NonCanonical(_))));
    }
    let mut named = value();
    named["acl"]["entries"][0]["principal_name"] = json!("local-user");
    assert!(matches!(parse_value(&named), Err(PropertyError::Malformed(_))));
}

#[test]
fn acl_entry_count_is_bounded_without_deduplicating_entries() {
    let mut input = snapshot();
    input.acl = AclSnapshot::Present {
        flags: 0,
        entries: vec![ace(AceKind::Allow, "00000000000000000000000000000000"); 128],
    };
    let reviewed = review(input.clone()).unwrap();
    match &reviewed.snapshot().acl {
        AclSnapshot::Present { entries, .. } => assert_eq!(entries.len(), 128),
        AclSnapshot::Absent {} => panic!("present ACL must remain present"),
    }
    if let AclSnapshot::Present { entries, .. } = &mut input.acl {
        entries.push(ace(AceKind::Allow, "00000000000000000000000000000000"));
    }
    assert!(matches!(review(input), Err(PropertyError::Bounds(_))));
}

#[test]
fn unsupported_bsd_acl_and_ace_bits_are_not_silently_masked() {
    for (pointer, bits) in [
        ("/bsd_flags", 1),
        ("/bsd_flags", 0x8000_0000_u32),
        ("/acl/flags", 0x0001_0000),
        ("/acl/flags", 0x0004_0000),
        ("/acl/entries/0/flags", 1),
        ("/acl/entries/0/flags", 0x200),
        ("/acl/entries/0/permissions", 1),
        ("/acl/entries/0/permissions", 0x0000_4000),
        ("/acl/entries/0/permissions", 0x8000_0000),
    ] {
        let mut document = value();
        *document.pointer_mut(pointer).unwrap() = json!(bits);
        assert!(matches!(parse_value(&document), Err(PropertyError::Unsupported(_))), "{pointer}={bits:#x}");
    }
}

#[test]
fn negative_epochs_are_exact_but_nanoseconds_and_file_modes_are_constrained() {
    for mode in [0o100000, 0o107777] {
        let mut input = snapshot();
        input.mode = mode;
        input.mtime.seconds = i64::MIN;
        input.birthtime.seconds = i64::MAX;
        let reviewed = review(input).unwrap();
        assert_eq!(reviewed.snapshot().mtime.seconds, i64::MIN);
        assert_eq!(reviewed.snapshot().birthtime.seconds, i64::MAX);
        assert_eq!(reviewed.snapshot().mtime.nanoseconds, 999_999_999);
    }
    for field in ["mtime", "birthtime"] {
        let mut document = value();
        document[field]["nanoseconds"] = json!(1_000_000_000);
        assert!(matches!(parse_value(&document), Err(PropertyError::NonCanonical(_))));
        document[field]["nanoseconds"] = json!(-1);
        assert!(matches!(parse_value(&document), Err(PropertyError::Malformed(_))));
    }
    for mode in [0o644, 0o040755, 0o077777, 0o110000, 0o120777] {
        let mut input = snapshot();
        input.mode = mode;
        assert!(matches!(review(input), Err(PropertyError::Unsupported(_))));
    }
}

#[test]
fn xattrs_must_arrive_sorted_and_unique_and_are_never_reordered_or_merged() {
    let mut input = snapshot();
    input.xattrs = vec![attribute(0, 0), attribute(1, 3)];
    let original = serde_json::to_value(&input).unwrap();
    assert_eq!(serde_json::to_value(review(input.clone()).unwrap().snapshot()).unwrap(), original);
    input.xattrs.reverse();
    assert!(matches!(review(input), Err(PropertyError::NonCanonical(_))));
    let mut duplicate = snapshot();
    duplicate.xattrs = vec![attribute(0, 0), attribute(0, 3)];
    // Structural validation cannot establish byte ordering or uniqueness by name.
    contracts::validate(Contract::ExecutionMacosCopyProperties, &duplicate).unwrap();
    assert!(matches!(review(duplicate), Err(PropertyError::NonCanonical(_))));
}

#[test]
fn xattr_names_require_lowercase_even_nul_free_hex_with_a_decoded_length_bound() {
    for name in ["", "00", "610062", "a", "GG", "AB"] {
        let mut document = value();
        document["xattrs"][0]["name_hex"] = json!(name);
        assert!(matches!(parse_value(&document), Err(PropertyError::NonCanonical(_))));
    }
    let mut input = snapshot();
    input.xattrs[0].name_hex = "ff".repeat(255);
    assert!(review(input.clone()).is_ok());
    input.xattrs[0].name_hex.push_str("ff");
    assert!(matches!(review(input), Err(PropertyError::Bounds(_))));
}

#[test]
fn xattr_values_are_raw_bytes_in_canonical_hex_and_can_be_empty() {
    for encoded in ["", "00ff80", "aabbcc"] {
        let mut input = snapshot();
        input.xattrs[0].value_hex = encoded.to_owned();
        let reviewed = review(input).unwrap();
        assert_eq!(reviewed.snapshot().xattrs[0].value_hex, encoded);
    }
    for encoded in ["0", "FF", "0g", "a a"] {
        let mut document = value();
        document["xattrs"][0]["value_hex"] = json!(encoded);
        assert!(matches!(parse_value(&document), Err(PropertyError::NonCanonical(_))));
    }
}

#[test]
fn xattr_count_per_value_and_aggregate_limits_are_independent() {
    let mut input = snapshot();
    input.xattrs = (0..128).map(|index| attribute(index, 0)).collect();
    assert_eq!(review(input.clone()).unwrap().snapshot().xattrs.len(), 128);
    input.xattrs.push(attribute(128, 0));
    assert!(matches!(review(input), Err(PropertyError::Bounds(_))));

    let mut input = snapshot();
    input.xattrs = vec![attribute(0, 65_536)];
    assert!(review(input.clone()).is_ok());
    input.xattrs[0].value_hex.push_str("a5");
    assert!(matches!(review(input), Err(PropertyError::Bounds(_))));

    let mut input = snapshot();
    input.xattrs = (0..16).map(|index| attribute(index, 65_536)).collect();
    assert!(review(input.clone()).is_ok());
    input.xattrs.push(attribute(16, 1));
    assert!(matches!(review(input), Err(PropertyError::Bounds(_))));
}

#[test]
fn every_preserved_metadata_component_participates_in_the_fingerprint() {
    let original = fingerprint(snapshot());
    for (pointer, replacement) in [
        ("/uid", json!(502)),
        ("/gid", json!(21)),
        ("/mode", json!(0o100600)),
        ("/mtime/seconds", json!(0)),
        ("/mtime/nanoseconds", json!(1)),
        ("/birthtime/seconds", json!(-3)),
        ("/birthtime/nanoseconds", json!(124)),
        ("/xattrs/0/name_hex", json!(hex(b"user.other"))),
        ("/xattrs/0/value_hex", json!("a5a4")),
        ("/acl/flags", json!(0x0002_0000)),
        ("/acl/entries/0/principal_hex", json!("ffffffffffffffffffffffffffffffff")),
        ("/acl/entries/0/kind", json!("deny")),
        ("/acl/entries/0/flags", json!(0x10)),
        ("/acl/entries/0/permissions", json!(4)),
    ] {
        let mut document = value();
        *document.pointer_mut(pointer).unwrap() = replacement;
        assert_ne!(parse_value(&document).unwrap().fingerprint(), original, "{pointer}");
    }
}

#[test]
fn json_key_order_and_whitespace_do_not_change_reviewed_identity() {
    let document = value();
    let object = document.as_object().unwrap();
    let reversed = object
        .iter()
        .rev()
        .map(|(key, value)| {
            format!(
                "{} : {}",
                serde_json::to_string(key).unwrap(),
                serde_json::to_string_pretty(value).unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let reordered = format!(" \n {{\n{reversed}\n}}\t\n");
    let compact = serde_json::to_vec(&document).unwrap();
    assert_ne!(compact.as_slice(), reordered.as_bytes());
    let first = parse(&compact).unwrap();
    let second = parse(reordered.as_bytes()).unwrap();
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(
        serde_json::to_value(first.snapshot()).unwrap(),
        serde_json::to_value(second.snapshot()).unwrap()
    );
}

#[test]
fn raw_document_limit_is_checked_before_json_deserialization() {
    let mut at_limit = serde_json::to_vec(&snapshot()).unwrap();
    at_limit.resize(3 * 1024 * 1024, b' ');
    assert!(parse(&at_limit).is_ok());
    at_limit.push(b' ');
    assert!(matches!(parse(&at_limit), Err(PropertyError::Bounds(_))));
    let oversized_invalid = vec![b'!'; 3 * 1024 * 1024 + 1];
    assert!(matches!(parse(&oversized_invalid), Err(PropertyError::Bounds(_))));
    assert!(matches!(parse(b"!"), Err(PropertyError::Malformed(_))));
}

#[test]
fn typed_review_cannot_bypass_semantic_refusals_to_obtain_a_fingerprint() {
    let mut wrong_profile = snapshot();
    wrong_profile.profile = "macos-apfs/v1".to_owned();
    assert!(matches!(review(wrong_profile), Err(PropertyError::Unsupported(_))));
    let mut bad_time = snapshot();
    bad_time.mtime.nanoseconds = 1_000_000_000;
    assert!(matches!(review(bad_time), Err(PropertyError::NonCanonical(_))));
    let mut bad_hex = snapshot();
    bad_hex.xattrs[0].value_hex = "FF".to_owned();
    assert!(matches!(review(bad_hex), Err(PropertyError::NonCanonical(_))));
    let mut deferred_acl = snapshot();
    deferred_acl.acl = AclSnapshot::Present {
        flags: 0x0001_0000,
        entries: vec![],
    };
    assert!(matches!(review(deferred_acl), Err(PropertyError::Unsupported(_))));
}
