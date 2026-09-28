//! Deterministic synthetic provider/host declaration recipes for the future
//! PNG candidate coordinator. No provider is launched and no file is touched.

use std::collections::BTreeSet;

use optiflow::png_candidate::{
    CandidateContract, ContractRefusal, check_candidate_contract, parse_contract,
    request_fingerprint,
};
use serde::Deserialize;
use serde_json::Value;

const BASELINE: &str = include_str!("../examples/png-candidate-contract-v1.json");
const CATALOG: &str = include_str!("fixtures/media/provider_cases.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema: String,
    license: String,
    seed: u64,
    generator: String,
    baseline: String,
    scope: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    validity_class: String,
    provenance: String,
    contract: String,
    recipe: Recipe,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recipe {
    mutations: Vec<Mutation>,
    #[serde(default)]
    rebind_request: bool,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Mutation {
    Replace { path: String, value: Value },
    Copy { from: String, path: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    refusal: String,
}

fn expected_refusal(name: &str) -> ContractRefusal {
    match name {
        "RequestMismatch" => ContractRefusal::RequestMismatch,
        "ProviderUnsuccessful" => ContractRefusal::ProviderUnsuccessful,
        "MissingHostEvidence" => ContractRefusal::MissingHostEvidence,
        "ProviderIdentityMismatch" => ContractRefusal::ProviderIdentityMismatch,
        "SourceChanged" => ContractRefusal::SourceChanged,
        "CandidateMismatch" => ContractRefusal::CandidateMismatch,
        "SourceAlias" => ContractRefusal::SourceAlias,
        "ValidationIncomplete" => ContractRefusal::ValidationIncomplete,
        "UnsupportedPng" => ContractRefusal::UnsupportedPng,
        "PreservationMismatch" => ContractRefusal::PreservationMismatch,
        "LimitExceeded" => ContractRefusal::LimitExceeded,
        "NotSmaller" => ContractRefusal::NotSmaller,
        _ => panic!("unknown typed refusal in provider corpus: {name}"),
    }
}

fn apply_recipe(packet: &mut Value, recipe: &Recipe, id: &str) {
    for mutation in &recipe.mutations {
        let (path, value) = match mutation {
            Mutation::Replace { path, value } => (path, value.clone()),
            Mutation::Copy { from, path } => (
                path,
                packet
                    .pointer(from)
                    .unwrap_or_else(|| panic!("{id}: missing copy source {from}"))
                    .clone(),
            ),
        };
        *packet
            .pointer_mut(path)
            .unwrap_or_else(|| panic!("{id}: missing mutation target {path}")) = value;
    }

    if recipe.rebind_request {
        let typed: CandidateContract =
            serde_json::from_value(packet.clone()).expect("synthetic recipe shape");
        let fingerprint =
            serde_json::to_value(request_fingerprint(&typed.request)).expect("fingerprint JSON");
        packet["provider_result"]["request_fingerprint"] = fingerprint.clone();
        packet["host_evidence"]["request_fingerprint"] = fingerprint;
    }
}

#[test]
fn provider_and_host_declarations_follow_named_contract_refusals() {
    let corpus: Corpus = serde_json::from_str(CATALOG).expect("provider corpus catalog");
    assert_eq!(corpus.schema, "optiflow.png-provider-corpus.v1");
    assert_eq!(corpus.license, "MIT");
    assert_eq!(corpus.seed, 0); // Fixed recipe selection; no randomness.
    assert_eq!(corpus.generator, "tests/png_provider_corpus.rs");
    assert_eq!(corpus.baseline, "examples/png-candidate-contract-v1.json");
    assert!(corpus.scope.contains("no provider is launched"));

    let baseline: Value = serde_json::from_str(BASELINE).expect("synthetic baseline JSON");
    let typed = parse_contract(&baseline).expect("synthetic baseline contract");
    assert_eq!(
        check_candidate_contract(&typed)
            .expect("synthetic baseline consistency")
            .reported_encoded_byte_reduction,
        224
    );

    let mut ids = BTreeSet::new();
    assert!(!corpus.cases.is_empty());
    for case in &corpus.cases {
        assert!(
            ids.insert(case.id.as_str()),
            "duplicate case ID: {}",
            case.id
        );
        assert!(
            case.id.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "unstable case ID: {}",
            case.id
        );
        assert_eq!(case.validity_class, "contract_refusal", "{}", case.id);
        assert!(!case.provenance.is_empty(), "{}", case.id);
        assert_eq!(
            case.contract, "optiflow.png-candidate-contract.v1",
            "{}",
            case.id
        );
        assert!(!case.recipe.mutations.is_empty(), "{}", case.id);

        let mut packet = baseline.clone();
        apply_recipe(&mut packet, &case.recipe, &case.id);
        // These are schema-valid declarations. The pure checker must apply
        // its own semantic safety boundary, including separate host claims.
        let typed = parse_contract(&packet)
            .unwrap_or_else(|reason| panic!("{}: invalid recipe shape: {reason:?}", case.id));
        assert_eq!(
            check_candidate_contract(&typed),
            Err(expected_refusal(&case.expected.refusal)),
            "{}",
            case.id
        );
    }
}
