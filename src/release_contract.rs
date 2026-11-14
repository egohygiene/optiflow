//! Compiled interface description only. It does not open state, probe tools,
//! qualify mutation, contact a registry or establish publication.
use serde_json::{Value, json};

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
            {"id":"optiflow.mp4-h264-aac-delivery.v1","source_mutation":false,"state":"unqualified_source_candidate; lossy_video_requires_human_quality_review","provider_execution":"explicit_linux_guard_only","replacement_authority":false,"upload_authority":false,"capabilities":crate::mp4_delivery::handoff::capabilities()},
            {"id":"local-explanations-v1","source_mutation":false,"state":"optional_advisory; no_mutation_authority"}],
        "native_packaging_targets":["x86_64-unknown-linux-gnu","aarch64-apple-darwin","x86_64-apple-darwin"],
        "retained_target_objectives":["x86_64-pc-windows-msvc"],
        "schemas":[
            {"name":"mp4-input-reference-v1.schema.json","document":include_str!("../schemas/mp4-input-reference-v1.schema.json")},
            {"name":"mp4-consumer-v1.schema.json","document":include_str!("../schemas/mp4-consumer-v1.schema.json")},
            {"name":"mp4-handoff-preparation-v1.schema.json","document":include_str!("../schemas/mp4-handoff-preparation-v1.schema.json")},
            {"name":"mp4-delivery-handoff-v1.schema.json","document":include_str!("../schemas/mp4-delivery-handoff-v1.schema.json")},
            {"name":"mp4-handoff-result-v1.schema.json","document":include_str!("../schemas/mp4-handoff-result-v1.schema.json")},
            {"name":"mp4-delivery-capabilities-v1.schema.json","document":include_str!("../schemas/mp4-delivery-capabilities-v1.schema.json")},
            {"name":"mp4-folder-comparison-v1.schema.json","document":include_str!("../schemas/mp4-folder-comparison-v1.schema.json")},
            {"name":"mp4-folder-report-v2.schema.json","document":include_str!("../schemas/mp4-folder-report-v2.schema.json")},
            {"name":"mp4-progress-v1.schema.json","document":include_str!("../schemas/mp4-progress-v1.schema.json")},
            {"name":"mp4-preflight-v1.schema.json","document":include_str!("../schemas/mp4-preflight-v1.schema.json")},
            {"name":"mp4-delivery-selection-v1.schema.json","document":include_str!("../schemas/mp4-delivery-selection-v1.schema.json")},
            {"name":"mp4-folder-selection-v1.schema.json","document":include_str!("../schemas/mp4-folder-selection-v1.schema.json")},
            {"name":"mp4-settings-v1.schema.json","document":include_str!("../schemas/mp4-settings-v1.schema.json")},
            {"name":"mp4-delivery-request-v1.schema.json","document":include_str!("../schemas/mp4-delivery-request-v1.schema.json")},
            {"name":"mp4-delivery-plan-v1.schema.json","document":include_str!("../schemas/mp4-delivery-plan-v1.schema.json")},
            {"name":"mp4-delivery-artifact-pin-v1.schema.json","document":include_str!("../schemas/mp4-delivery-artifact-pin-v1.schema.json")},
            {"name":"mp4-delivery-evidence-v1.schema.json","document":include_str!("../schemas/mp4-delivery-evidence-v1.schema.json")},
            {"name":"mp4-folder-request-v1.schema.json","document":include_str!("../schemas/mp4-folder-request-v1.schema.json")},
            {"name":"video-folder-preview-request-v1.schema.json","document":include_str!("../schemas/video-folder-preview-request-v1.schema.json")},
            {"name":"command-result.schema.json","document":include_str!("../schemas/command-result.schema.json")},
            {"name":"extension-lock-v1.schema.json","document":include_str!("../schemas/extension-lock-v1.schema.json")}]
    })
}
