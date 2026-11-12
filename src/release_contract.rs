//! Compiled interface description only. It does not open state, probe tools,
//! qualify mutation, contact a registry or establish publication.
use serde_json::{json, Value};

pub fn describe() -> Value {
    json!({"schema_version":"optiflow.release-contract/v1","tool":"optiflow",
        "version":env!("CARGO_PKG_VERSION"),"library_crate":"optiflow",
        "library_version":env!("CARGO_PKG_VERSION"),"registry_channel":"unavailable",
        "primary_binary":"optiflow","qualification":"not_established_by_contract_description",
        "profiles":[
            {"id":"read-only-inventory-v1","source_mutation":false,"state":"implemented_source; exact published artifact remains separately pinned"},
            {"id":"exact-dedup-transaction-v0.2","source_mutation":true,"state":"requires_existing_native_qualification_and_exact_authority"},
            {"id":"lossless-png-replacement-v1","source_mutation":true,"state":"requires_existing_native_qualification_and_exact_authority"},
            {"id":"video-candidates-v2","source_mutation":false,"state":"source_candidate; runtime_and_release_evidence_required"},
            {"id":"video-folder-preview-v1","source_mutation":false,"state":"read_only_source_discovery; no_provider_or_run_authority"},
            {"id":"local-explanations-v1","source_mutation":false,"state":"optional_advisory; no_mutation_authority"}],
        "native_packaging_targets":["x86_64-unknown-linux-gnu","aarch64-apple-darwin","x86_64-apple-darwin"],
        "retained_target_objectives":["x86_64-pc-windows-msvc"],
        "schemas":[
            {"name":"video-folder-preview-request-v1.schema.json","document":include_str!("../schemas/video-folder-preview-request-v1.schema.json")},
            {"name":"command-result.schema.json","document":include_str!("../schemas/command-result.schema.json")},
            {"name":"extension-lock-v1.schema.json","document":include_str!("../schemas/extension-lock-v1.schema.json")}]
    })
}
