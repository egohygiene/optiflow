#![no_main]

use libfuzzer_sys::fuzz_target;
use optiflow::artifact_set::{
    SCAN_MARKER_FILE_NAME, inspect_plan_set, inspect_scan_set, plan_marker_path,
};

fuzz_target!(|data: &[u8]| {
    let directory = tempfile::tempdir().expect("temporary artifact-set directory");

    let scan_directory = directory.path().join("scan");
    std::fs::create_dir(&scan_directory).expect("scan artifact-set directory");
    std::fs::write(scan_directory.join(SCAN_MARKER_FILE_NAME), data)
        .expect("write scan marker input");
    let _ = inspect_scan_set(&scan_directory);

    let plan_path = directory.path().join("plan.json");
    std::fs::write(plan_marker_path(&plan_path), data).expect("write plan marker input");
    let _ = inspect_plan_set(&plan_path);
});
