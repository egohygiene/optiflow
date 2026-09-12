#![no_main]

use libfuzzer_sys::fuzz_target;
use optiflow::configuration::ConfigDocumentV1;
use optiflow::contracts::{self, Contract};

fuzz_target!(|data: &[u8]| {
    if let Ok(document) = serde_json::from_slice::<ConfigDocumentV1>(data) {
        let _ = contracts::validate(Contract::Config, &document);
    }

    if let Ok(source) = std::str::from_utf8(data) {
        if let Ok(document) = toml::from_str::<ConfigDocumentV1>(source) {
            let _ = contracts::validate(Contract::Config, &document);
        }
    }
});
