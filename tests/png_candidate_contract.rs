//! Synthetic contract fixtures only: these tests neither decode nor create PNGs.

use optiflow::contracts::{self, Contract};
use optiflow::png_candidate::{
    CandidateContract, ContractRefusal, check_candidate_contract, parse_contract,
    request_fingerprint,
};
use serde_json::{Value, json};

fn example() -> Value {
    serde_json::from_str(include_str!("../examples/png-candidate-contract-v1.json"))
        .expect("checked-in synthetic example")
}

fn assess(value: &Value) -> Result<u64, ContractRefusal> {
    let packet = parse_contract(value)?;
    check_candidate_contract(&packet).map(|review| review.reported_encoded_byte_reduction)
}

fn rebind(value: &mut Value) {
    let packet: CandidateContract = serde_json::from_value(value.clone()).expect("typed fixture");
    let fingerprint = serde_json::to_value(request_fingerprint(&packet.request)).unwrap();
    value["provider_result"]["request_fingerprint"] = fingerprint.clone();
    if !value["host_evidence"].is_null() {
        value["host_evidence"]["request_fingerprint"] = fingerprint;
    }
}

#[test]
fn complete_synthetic_packet_is_consistent_but_not_an_accepted_artifact() {
    let value = example();
    contracts::validate(Contract::PngCandidateContract, &value).unwrap();
    assert_eq!(assess(&value), Ok(224));
    let packet = parse_contract(&value).unwrap();
    assert_eq!(
        request_fingerprint(&packet.request),
        packet.provider_result.request_fingerprint
    );
    assert_eq!(serde_json::to_value(&packet).unwrap(), value);
    let round_trip = parse_contract(&serde_json::to_value(&packet).unwrap()).unwrap();
    assert_eq!(
        request_fingerprint(&packet.request),
        request_fingerprint(&round_trip.request)
    );
}

#[test]
fn provider_claims_cannot_supply_or_bypass_host_evidence() {
    let mut value = example();
    value["host_evidence"] = Value::Null;
    assert_eq!(assess(&value), Err(ContractRefusal::MissingHostEvidence));
    value["provider_result"]["validated"] = json!(true);
    assert_eq!(assess(&value), Err(ContractRefusal::InvalidShape));
    for status in ["failed", "timed_out", "cancelled"] {
        let mut value = example();
        value["provider_result"]["status"] = json!(status);
        assert_eq!(assess(&value), Err(ContractRefusal::ProviderUnsuccessful));
    }
}

#[test]
fn schema_rejects_unsupported_versions_fields_policies_and_shapes() {
    for (pointer, replacement) in [
        ("/schema", json!("optiflow.png-candidate-contract.v2")),
        ("/request/profile", json!("lossy")),
        (
            "/request/source/content/digest/algorithm",
            json!("metadata_fingerprint"),
        ),
        (
            "/request/source/content/digest/value",
            json!("not-a-full-hash"),
        ),
        ("/request/source/regular_file", json!(false)),
        ("/host_evidence/candidate/symlink", json!(true)),
        ("/request/limits/input_bytes", json!(0)),
        ("/host_evidence/checks/0/status", json!("skipped")),
        ("/host_evidence/checks/0/kind", json!("provider_exit_zero")),
    ] {
        let mut value = example();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert_eq!(
            assess(&value),
            Err(ContractRefusal::InvalidShape),
            "{pointer}"
        );
    }
    for pointer in [
        "",
        "/request",
        "/request/source",
        "/request/producer",
        "/request/source/path",
        "/request/source/filesystem_identity",
        "/request/source/content/digest",
        "/host_evidence",
    ] {
        let mut value = example();
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".to_owned(), json!(true));
        assert_eq!(
            assess(&value),
            Err(ContractRefusal::InvalidShape),
            "{pointer}"
        );
    }
    let mut value = example();
    value["request"]["source"]
        .as_object_mut()
        .unwrap()
        .remove("content");
    assert_eq!(assess(&value), Err(ContractRefusal::InvalidShape));
}

#[test]
fn request_binding_changes_with_material_inputs_and_rejects_replay() {
    let original = parse_contract(&example()).unwrap();
    for (pointer, replacement) in [
        (
            "/request/source/content/digest/value",
            json!("b".repeat(64)),
        ),
        (
            "/request/effective_policy_fingerprint/value",
            json!("c".repeat(64)),
        ),
        (
            "/request/producer/binary_digest/value",
            json!("d".repeat(64)),
        ),
        (
            "/request/producer/invocation_fingerprint/value",
            json!("e".repeat(64)),
        ),
        ("/request/validator/version", json!("synthetic-2")),
        ("/request/limits/elapsed_ms", json!(500)),
    ] {
        let mut value = example();
        *value.pointer_mut(pointer).unwrap() = replacement;
        let changed = parse_contract(&value).unwrap();
        assert_ne!(
            request_fingerprint(&original.request),
            request_fingerprint(&changed.request)
        );
        assert_eq!(
            assess(&value),
            Err(ContractRefusal::RequestMismatch),
            "{pointer}"
        );
    }
    let mut value = example();
    value["host_evidence"]["request_fingerprint"]["value"] = json!("f".repeat(64));
    assert_eq!(assess(&value), Err(ContractRefusal::RequestMismatch));
}

#[test]
fn changed_sources_candidates_and_executables_are_refused() {
    for field in ["source_before", "source_after"] {
        let mut value = example();
        value["host_evidence"][field]["content"]["digest"]["value"] = json!("b".repeat(64));
        assert_eq!(assess(&value), Err(ContractRefusal::SourceChanged));
    }
    for field in [
        "producer_before",
        "producer_after",
        "validator_before",
        "validator_after",
    ] {
        let mut value = example();
        value["host_evidence"][field]["binary_digest"]["value"] = json!("c".repeat(64));
        assert_eq!(
            assess(&value),
            Err(ContractRefusal::ProviderIdentityMismatch)
        );
    }
    let mut value = example();
    value["provider_result"]["candidate"]["digest"]["value"] = json!("b".repeat(64));
    assert_eq!(assess(&value), Err(ContractRefusal::CandidateMismatch));
    let mut value = example();
    value["provider_result"]["candidate"] = Value::Null;
    assert_eq!(assess(&value), Err(ContractRefusal::CandidateMismatch));
}

#[test]
fn candidate_aliases_and_ambiguous_paths_are_refused() {
    for field in ["path", "filesystem_identity"] {
        let mut value = example();
        value["host_evidence"]["candidate"][field] = value["request"]["source"][field].clone();
        assert_eq!(assess(&value), Err(ContractRefusal::SourceAlias));
    }
    for path in [
        "relative.png",
        "/synthetic/../source.png",
        "/synthetic/./candidate.png",
        "/synthetic//candidate.png",
        "/synthetic/\0candidate",
    ] {
        let mut value = example();
        value["host_evidence"]["candidate"]["path"]["value"] = json!(path);
        assert_eq!(
            assess(&value),
            Err(ContractRefusal::InvalidPath),
            "{path:?}"
        );
    }
}

#[test]
fn required_checks_cannot_be_missing_duplicated_or_nonpassing() {
    for index in 0..8 {
        for status in ["failed", "unsupported", "not_run"] {
            let mut value = example();
            value["host_evidence"]["checks"][index]["status"] = json!(status);
            assert_eq!(assess(&value), Err(ContractRefusal::ValidationIncomplete));
        }
        let mut value = example();
        value["host_evidence"]["checks"]
            .as_array_mut()
            .unwrap()
            .remove(index);
        assert_eq!(assess(&value), Err(ContractRefusal::ValidationIncomplete));
    }
    let mut value = example();
    value["host_evidence"]["checks"][1] = value["host_evidence"]["checks"][0].clone();
    assert_eq!(assess(&value), Err(ContractRefusal::ValidationIncomplete));
}

#[test]
fn png_dimensions_cannot_exceed_the_ihdr_integer_range() {
    for dimension in ["width", "height"] {
        let mut value = example();
        for facts in ["source_png", "candidate_png"] {
            value["host_evidence"][facts][dimension] = json!(2147483648_u64);
        }
        assert_eq!(assess(&value), Err(ContractRefusal::InvalidShape));
    }
}

#[test]
fn preservation_and_decode_facts_are_required() {
    // Structural success from a provider is insufficient when facts contradict it.
    for (field, replacement) in [
        ("complete_decode", json!(false)),
        ("animation_chunks", json!(true)),
        ("unknown_unsafe_to_copy_chunks", json!(true)),
        ("frames", json!(2)),
        ("bit_depth", json!(16)),
        ("color_type", json!(3)),
        ("decoded_bytes", json!(255)),
    ] {
        let mut value = example();
        value["host_evidence"]["candidate_png"][field] = replacement;
        assert_eq!(
            assess(&value),
            Err(ContractRefusal::UnsupportedPng),
            "{field}"
        );
    }
    for field in [
        "ihdr_digest",
        "decoded_samples_digest",
        "non_idat_chunks_digest",
    ] {
        let mut value = example();
        value["host_evidence"]["candidate_png"][field]["value"] = json!("b".repeat(64));
        assert_eq!(
            assess(&value),
            Err(ContractRefusal::PreservationMismatch),
            "{field}"
        );
    }
    let mut value = example();
    value["host_evidence"]["candidate_png"]["width"] = json!(4);
    value["host_evidence"]["candidate_png"]["decoded_bytes"] = json!(128);
    assert_eq!(assess(&value), Err(ContractRefusal::PreservationMismatch));
}

#[test]
fn limits_and_reported_reduction_are_checked_without_physical_savings_claims() {
    for (field, limit) in [
        ("input_bytes", 1023),
        ("candidate_bytes", 799),
        ("decoded_bytes", 255),
        ("temporary_bytes", 799),
        ("memory_bytes", 100),
        ("elapsed_ms", 9),
    ] {
        let mut value = example();
        value["request"]["limits"][field] = json!(limit);
        rebind(&mut value);
        assert_eq!(
            assess(&value),
            Err(ContractRefusal::LimitExceeded),
            "{field}"
        );
    }
    for bytes in [1024, 1025] {
        let mut value = example();
        value["provider_result"]["candidate"]["logical_bytes"] = json!(bytes);
        value["host_evidence"]["candidate"]["content"]["logical_bytes"] = json!(bytes);
        value["host_evidence"]["resource_usage"]["peak_temporary_bytes"] = json!(bytes);
        assert_eq!(assess(&value), Err(ContractRefusal::NotSmaller));
    }
}
