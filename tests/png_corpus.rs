//! Offline PNG bytes from the versioned #65 media corpus.
//!
//! These are validator fixtures, not OxiPNG execution or accepted artifacts.
//! The generator's `--check` owns SHA-256 provenance/drift verification;
//! this test exercises each checked-in pair through the typed byte API.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use optiflow::png_validation::{ByteRefusal, ByteValidationLimits, validate_png_pair};
use serde::Deserialize;

const MAX_ENCODED_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
struct Catalog {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Index {
    schema: String,
    files: Vec<IndexedFile>,
    cases: Vec<IndexedCase>,
}

#[derive(Deserialize)]
struct IndexedFile {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Deserialize)]
struct IndexedCase {
    id: String,
    source_sha256: String,
    candidate_sha256: String,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    source: String,
    candidate: String,
    expected: Expected,
    #[serde(default)]
    limits: Option<CaseLimits>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseLimits {
    decoded_bytes_per_image: usize,
}

#[derive(Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
enum Expected {
    Valid { logical_reduction_bytes: u64 },
    Refused { reason: String },
}

fn limits(case: &Case) -> ByteValidationLimits {
    let mut limits = ByteValidationLimits {
        source_bytes: MAX_ENCODED_BYTES,
        candidate_bytes: MAX_ENCODED_BYTES,
        decoded_bytes_per_image: 1024 * 1024,
        chunks_per_image: 128,
        decoder_allocation_bytes: 1024 * 1024,
    };
    if let Some(override_limits) = &case.limits {
        assert!(
            (1..=limits.decoded_bytes_per_image).contains(&override_limits.decoded_bytes_per_image),
            "{} requested an invalid decoded-byte ceiling",
            case.id
        );
        limits.decoded_bytes_per_image = override_limits.decoded_bytes_per_image;
    }
    limits
}

fn refusal(reason: &str) -> ByteRefusal {
    match reason {
        "LimitExceeded" => ByteRefusal::LimitExceeded,
        "InvalidStructure" => ByteRefusal::InvalidStructure,
        "InvalidChecksum" => ByteRefusal::InvalidChecksum,
        "UnsupportedPng" => ByteRefusal::UnsupportedPng,
        "IncompleteImageStream" => ByteRefusal::IncompleteImageStream,
        "DecodeFailed" => ByteRefusal::DecodeFailed,
        "PreservationMismatch" => ByteRefusal::PreservationMismatch,
        "NotSmaller" => ByteRefusal::NotSmaller,
        _ => panic!("unknown PNG corpus refusal: {reason}"),
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn fixture_path(root: &Path, generated: &Path, relative: &str) -> PathBuf {
    let path = Path::new(relative);
    assert!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "corpus fixture path must be a plain relative path: {relative}"
    );
    let path = root.join(path);
    let metadata = fs::symlink_metadata(&path).expect("committed corpus fixture must exist");
    assert!(
        metadata.file_type().is_file(),
        "corpus fixture must be a regular file"
    );
    assert!(
        metadata.len() <= MAX_ENCODED_BYTES as u64,
        "corpus fixture exceeds encoded byte ceiling"
    );
    let canonical = path.canonicalize().expect("corpus fixture can be resolved");
    assert!(
        canonical.starts_with(generated),
        "corpus fixture must stay inside the generated PNG directory"
    );
    canonical
}

#[test]
fn versioned_png_pairs_have_typed_byte_outcomes_without_mutating_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let generated = root
        .join("tests/fixtures/media/generated")
        .canonicalize()
        .expect("generated PNG fixture directory");
    let catalog: Catalog = serde_json::from_slice(
        &fs::read(root.join("tests/fixtures/media/cases.json")).expect("PNG corpus catalog"),
    )
    .expect("typed PNG corpus catalog");
    assert!(!catalog.cases.is_empty(), "PNG corpus cannot be empty");

    // The Python drift check recomputes SHA-256 over recipes and bytes.
    // Here, bind every tested pair to its indexed path and length and compare
    // the indexed per-case digests with the indexed file digests.
    let index: Index = serde_json::from_slice(
        &fs::read(root.join("tests/fixtures/media/index.json")).expect("PNG corpus index"),
    )
    .expect("typed PNG corpus index");
    assert_eq!(index.schema, "optiflow.png-corpus-index.v1");
    let mut indexed_files = BTreeMap::new();
    for file in index.files {
        assert!(is_sha256(&file.sha256), "invalid indexed PNG SHA-256");
        assert!(
            indexed_files
                .insert(file.path, (file.bytes, file.sha256))
                .is_none(),
            "duplicate indexed PNG path"
        );
    }
    let mut indexed_cases = BTreeMap::new();
    for case in index.cases {
        assert!(
            indexed_cases
                .insert(case.id, (case.source_sha256, case.candidate_sha256))
                .is_none(),
            "duplicate indexed PNG case"
        );
    }
    assert_eq!(indexed_cases.len(), catalog.cases.len());

    let mut ids = BTreeSet::new();
    let mut used_files = BTreeSet::new();
    let mut valid = 0;
    let mut refused = 0;
    for case in catalog.cases {
        assert!(
            ids.insert(case.id.clone()),
            "duplicate PNG corpus ID: {}",
            case.id
        );
        assert!(
            case.id.starts_with("png65-"),
            "unexpected PNG fixture namespace"
        );
        let source_path = fixture_path(root, &generated, &case.source);
        let candidate_path = fixture_path(root, &generated, &case.candidate);
        let source = fs::read(&source_path).expect("source PNG fixture");
        let candidate = fs::read(&candidate_path).expect("candidate PNG fixture");
        if case.id == "png65-gray-complete" {
            // This is a complete unsupported input, unlike header/chunk
            // sentinels that the narrow profile can reject before decoding.
            let mut reader = png::Decoder::new(std::io::Cursor::new(&candidate))
                .read_info()
                .expect("well-formed grayscale PNG header");
            let mut samples = vec![0; reader.output_buffer_size().expect("bounded frame size")];
            let frame = reader
                .next_frame(&mut samples)
                .expect("complete gray frame");
            assert_eq!(frame.color_type, png::ColorType::Grayscale);
            assert_eq!((frame.width, frame.height), (16, 8));
            assert_eq!(samples, vec![11; 16 * 8]);
        }
        let (indexed_source, indexed_candidate) = indexed_cases
            .get(&case.id)
            .unwrap_or_else(|| panic!("missing indexed PNG case: {}", case.id));
        for (relative, bytes, indexed_digest) in [
            (&case.source, &source, indexed_source),
            (&case.candidate, &candidate, indexed_candidate),
        ] {
            used_files.insert(relative.clone());
            let (indexed_bytes, file_digest) = indexed_files
                .get(relative)
                .unwrap_or_else(|| panic!("missing indexed PNG file: {relative}"));
            assert_eq!(*indexed_bytes, bytes.len() as u64, "{}", case.id);
            assert_eq!(file_digest, indexed_digest, "{}", case.id);
        }
        let result = validate_png_pair(&source, &candidate, limits(&case));

        match case.expected {
            Expected::Valid {
                logical_reduction_bytes,
            } => {
                valid += 1;
                let evidence = result.unwrap_or_else(|reason| {
                    panic!("{} unexpectedly refused: {reason:?}", case.id)
                });
                assert!(
                    logical_reduction_bytes > 0,
                    "{} must reduce encoded bytes",
                    case.id
                );
                assert_eq!(
                    evidence.encoded_byte_reduction(),
                    logical_reduction_bytes,
                    "{}",
                    case.id
                );
                assert_eq!(
                    evidence.source().logical_bytes,
                    source.len() as u64,
                    "{}",
                    case.id
                );
                assert_eq!(
                    evidence.candidate().logical_bytes,
                    candidate.len() as u64,
                    "{}",
                    case.id
                );
                assert_eq!(
                    evidence.source().digest.value,
                    blake3::hash(&source).to_hex().to_string(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    evidence.candidate().digest.value,
                    blake3::hash(&candidate).to_hex().to_string(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    evidence.source_png(),
                    evidence.candidate_png(),
                    "{}",
                    case.id
                );
                assert!(evidence.source_png().complete_decode, "{}", case.id);
                assert!(!evidence.source_png().animation_chunks, "{}", case.id);
            }
            Expected::Refused { reason } => {
                refused += 1;
                assert_eq!(result.unwrap_err(), refusal(&reason), "{}", case.id);
            }
        }

        assert_eq!(
            fs::read(source_path).expect("reread source"),
            source,
            "{} source changed",
            case.id
        );
        assert_eq!(
            fs::read(candidate_path).expect("reread candidate"),
            candidate,
            "{} candidate changed",
            case.id
        );
    }
    assert!(
        valid > 0 && refused > 0,
        "corpus needs positive and refusal cases"
    );
    assert_eq!(used_files, indexed_files.into_keys().collect());
}
