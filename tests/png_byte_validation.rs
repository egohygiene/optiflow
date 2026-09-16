use std::io::Write;

use flate2::{Compression, write::ZlibEncoder};
use optiflow::png_validation::{ByteRefusal, ByteValidationLimits, validate_png_pair};
use proptest::prelude::*;

type Chunk = ([u8; 4], Vec<u8>);

fn limits() -> ByteValidationLimits {
    ByteValidationLimits {
        source_bytes: 1 << 20,
        candidate_bytes: 1 << 20,
        decoded_bytes_per_image: 1 << 20,
        chunks_per_image: 128,
        decoder_allocation_bytes: 1 << 20,
    }
}

fn compress(bytes: &[u8], level: Compression) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), level);
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn samples(color: u8) -> Vec<u8> {
    let pixel: &[u8] = if color == 2 {
        &[11, 22, 33]
    } else {
        &[11, 22, 33, 0]
    };
    pixel.repeat(16 * 8)
}

fn fixture(color: u8, pixels: &[u8], level: Compression, sub_filter: bool) -> Vec<Chunk> {
    fixture_size(color, pixels, level, sub_filter, 16, 8)
}

fn fixture_size(
    color: u8,
    pixels: &[u8],
    level: Compression,
    sub_filter: bool,
    width: u32,
    height: u32,
) -> Vec<Chunk> {
    let channels = if color == 2 { 3 } else { 4 };
    let mut filtered = Vec::new();
    for row in pixels.chunks_exact(width as usize * channels) {
        filtered.push(u8::from(sub_filter));
        for (i, sample) in row.iter().enumerate() {
            filtered.push(if sub_filter && i >= channels {
                sample.wrapping_sub(row[i - channels])
            } else {
                *sample
            });
        }
    }
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, color, 0, 0, 0]);
    vec![
        (*b"IHDR", ihdr),
        (*b"IDAT", compress(&filtered, level)),
        (*b"IEND", vec![]),
    ]
}

fn pair(color: u8) -> (Vec<Chunk>, Vec<Chunk>) {
    (
        fixture(color, &samples(color), Compression::none(), false),
        fixture(color, &samples(color), Compression::best(), true),
    )
}

fn encode(chunks: &[Chunk]) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, data) in chunks {
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = bytes.len();
        bytes.extend_from_slice(kind);
        bytes.extend_from_slice(data);
        let crc = crc32fast::hash(&bytes[start..]);
        bytes.extend_from_slice(&crc.to_be_bytes());
    }
    bytes
}

fn refuse(source: &[Chunk], candidate: &[Chunk], reason: ByteRefusal) {
    assert_eq!(
        validate_png_pair(&encode(source), &encode(candidate), limits()).unwrap_err(),
        reason
    );
}

#[test]
fn real_rgb_rgba_recompression_observes_exact_content_and_samples() {
    for color in [2, 6] {
        let (source, candidate) = pair(color);
        let source = encode(&source);
        let candidate = encode(&candidate);
        let original = source.clone();
        let result = validate_png_pair(&source, &candidate, limits()).unwrap();
        assert_eq!(source, original);
        assert_eq!(
            result.source().digest.value,
            blake3::hash(&source).to_hex().to_string()
        );
        assert_eq!(
            result.candidate().digest.value,
            blake3::hash(&candidate).to_hex().to_string()
        );
        assert_eq!(
            result.source_png().decoded_samples_digest.value,
            blake3::hash(&samples(color)).to_hex().to_string()
        );
        assert_eq!(result.source_png(), result.candidate_png());
        assert_eq!(
            result.source_png().decoded_bytes,
            samples(color).len() as u64
        );
        assert_eq!(result.source_png().width, 16);
        assert_eq!(result.source_png().height, 8);
        assert_eq!(
            result.encoded_byte_reduction(),
            (source.len() - candidate.len()) as u64
        );
        let repeat = validate_png_pair(&source, &candidate, limits()).unwrap();
        assert_eq!(result.candidate_png(), repeat.candidate_png());
    }
}

#[test]
fn complete_stream_check_spans_multiple_scratch_buffers() {
    let pixels: Vec<u8> = (0..65 * 257 * 4).map(|index| (index % 251) as u8).collect();
    let source = fixture_size(6, &pixels, Compression::none(), false, 65, 257);
    let candidate = fixture_size(6, &pixels, Compression::best(), true, 65, 257);
    let result = validate_png_pair(&encode(&source), &encode(&candidate), limits()).unwrap();
    assert_eq!(
        result.source_png().decoded_samples_digest.value,
        blake3::hash(&pixels).to_hex().to_string()
    );
    let mut extra_row = candidate.clone();
    extra_row[1].1 = compress(&vec![0; (65 * 4 + 1) * 258], Compression::best());
    refuse(&source, &extra_row, ByteRefusal::IncompleteImageStream);
}

#[test]
fn metadata_transparency_palette_and_private_safe_chunks_are_preserved() {
    let (mut source, mut candidate) = pair(2);
    let metadata = vec![
        (*b"gAMA", 45455_u32.to_be_bytes().to_vec()),
        (*b"sRGB", vec![0]),
        (*b"PLTE", vec![11, 22, 33]),
        (*b"tRNS", vec![0, 11, 0, 22, 0, 33]),
        (*b"pHYs", vec![0, 0, 1, 0, 0, 0, 1, 0, 1]),
        (*b"tEXt", b"Author\0Optiflow synthetic fixture".to_vec()),
        (*b"raNd", vec![0, 1, 2, 255]),
    ];
    source.splice(1..1, metadata.clone());
    candidate.splice(1..1, metadata);
    source.insert(source.len() - 1, (*b"tEXt", b"After\0IDAT".to_vec()));
    candidate.insert(candidate.len() - 1, (*b"tEXt", b"After\0IDAT".to_vec()));
    let result = validate_png_pair(&encode(&source), &encode(&candidate), limits()).unwrap();
    // tRNS must not silently expand RGB to RGBA or normalize transparent RGB.
    assert_eq!(result.source_png().color_type, 2);
    assert_eq!(result.source_png().decoded_bytes, 16 * 8 * 3);
}

#[test]
fn idat_repartitioning_and_empty_chunks_do_not_change_preservation_identity() {
    let (source, mut candidate) = pair(6);
    let idat = candidate.remove(1).1;
    let middle = idat.len() / 2;
    candidate.splice(
        1..1,
        [
            (*b"IDAT", idat[..middle].to_vec()),
            (*b"IDAT", vec![]),
            (*b"IDAT", idat[middle..].to_vec()),
        ],
    );
    validate_png_pair(&encode(&source), &encode(&candidate), limits()).unwrap();
    candidate.insert(2, (*b"raNd", vec![]));
    refuse(&source, &candidate, ByteRefusal::InvalidStructure);
}

#[test]
fn changed_visible_samples_alpha_and_invisible_rgb_are_refused() {
    for (color, changed_index) in [(2, 0), (6, 3), (6, 0)] {
        let (source, _) = pair(color);
        let mut pixels = samples(color);
        pixels[changed_index] += 1;
        let candidate = fixture(color, &pixels, Compression::best(), true);
        refuse(&source, &candidate, ByteRefusal::PreservationMismatch);
    }
}

#[test]
fn metadata_add_remove_change_order_and_idat_placement_are_refused() {
    let (mut source, mut candidate) = pair(2);
    let metadata = [
        (*b"tEXt", b"One\0first".to_vec()),
        (*b"tEXt", b"Two\0second".to_vec()),
    ];
    source.splice(1..1, metadata.clone());
    candidate.splice(1..1, metadata);
    for mutation in 0..5 {
        let mut changed = candidate.clone();
        match mutation {
            0 => {
                changed.insert(1, (*b"raNd", vec![]));
            }
            1 => {
                changed.remove(1);
            }
            2 => {
                changed[1].1.push(b'!');
            }
            3 => {
                changed.swap(1, 2);
            }
            _ => {
                let chunk = changed.remove(1);
                changed.insert(3, chunk);
            }
        }
        refuse(&source, &changed, ByteRefusal::PreservationMismatch);
    }
}

#[test]
fn unchanged_or_larger_candidates_are_not_reduction_evidence() {
    let (source, candidate) = pair(2);
    refuse(&source, &source, ByteRefusal::NotSmaller);
    refuse(&candidate, &source, ByteRefusal::NotSmaller);
}

#[test]
fn checksums_are_verified_on_every_chunk_including_ancillary() {
    let (mut source, candidate) = pair(2);
    source.insert(1, (*b"tEXt", b"Key\0value".to_vec()));
    let good = encode(&source);
    let mut offset = 8;
    for (_, data) in &source {
        let mut corrupt = good.clone();
        corrupt[offset + data.len() + 8] ^= 1;
        assert_eq!(
            validate_png_pair(&corrupt, &encode(&candidate), limits()).unwrap_err(),
            ByteRefusal::InvalidChecksum
        );
        offset += data.len() + 12;
    }
}

#[test]
fn complete_zlib_consumption_is_required_even_with_correct_chunk_crcs() {
    let (source, candidate) = pair(2);
    let valid = &candidate[1].1;
    let mut bad_adler = valid.clone();
    *bad_adler.last_mut().unwrap() ^= 1;
    let mut trailing = valid.clone();
    trailing.push(0);
    let mut two_streams = valid.clone();
    two_streams.extend_from_slice(valid);
    let mut cases = vec![
        bad_adler,
        trailing,
        two_streams,
        vec![],
        compress(&vec![0; (16 * 3 + 1) * 8 + 1], Compression::best()),
        compress(&vec![0; (16 * 3 + 1) * 8 - 1], Compression::best()),
    ];
    for truncate in 1..=valid.len() {
        cases.push(valid[..valid.len() - truncate].to_vec());
    }
    for data in cases {
        let mut changed = candidate.clone();
        changed[1].1 = data;
        refuse(&source, &changed, ByteRefusal::IncompleteImageStream);
    }
}

#[test]
fn invalid_filters_are_not_valid_samples() {
    let (source, mut candidate) = pair(2);
    let mut filtered = vec![0; (16 * 3 + 1) * 8];
    filtered[0] = 5;
    candidate[1].1 = compress(&filtered, Compression::best());
    refuse(&source, &candidate, ByteRefusal::DecodeFailed);
}

#[test]
fn structural_damage_and_trailing_file_bytes_are_refused() {
    let (source, candidate) = pair(2);
    let good = encode(&candidate);
    for length in 0..good.len() {
        assert!(validate_png_pair(&encode(&source), &good[..length], limits()).is_err());
    }
    let mut trailing = good;
    trailing.push(0);
    assert_eq!(
        validate_png_pair(&encode(&source), &trailing, limits()).unwrap_err(),
        ByteRefusal::InvalidStructure
    );
    let mut cases = Vec::new();
    let mut duplicate = candidate.clone();
    duplicate.insert(1, duplicate[0].clone());
    cases.push(duplicate);
    let mut missing_idat = candidate.clone();
    missing_idat.remove(1);
    cases.push(missing_idat);
    let mut nonempty_iend = candidate.clone();
    nonempty_iend[2].1.push(1);
    cases.push(nonempty_iend);
    for kind in [*b"rAnd", *b"ra1d"] {
        let mut changed = candidate.clone();
        changed.insert(1, (kind, vec![]));
        cases.push(changed);
    }
    for changed in cases {
        refuse(&source, &changed, ByteRefusal::InvalidStructure);
    }
    let mut huge_chunk = encode(&candidate);
    huge_chunk[8..12].copy_from_slice(&0xffff_ffff_u32.to_be_bytes());
    assert_eq!(
        validate_png_pair(&encode(&source), &huge_chunk, limits()).unwrap_err(),
        ByteRefusal::InvalidStructure
    );
}

#[test]
fn unsupported_encodings_animation_and_metadata_are_explicit_refusals() {
    let (source, candidate) = pair(2);
    for (index, value) in [(8, 16), (9, 0), (9, 3), (9, 4), (12, 1)] {
        let mut changed = candidate.clone();
        changed[0].1[index] = value;
        refuse(&source, &changed, ByteRefusal::UnsupportedPng);
    }
    for kind in [
        *b"acTL", *b"fcTL", *b"fdAT", *b"iCCP", *b"zTXt", *b"iTXt", *b"cHRM", *b"eXIf", *b"raND",
        *b"RaNd", *b"rANd",
    ] {
        for position in [1, 2] {
            let mut changed = candidate.clone();
            changed.insert(position, (kind, vec![]));
            refuse(&source, &changed, ByteRefusal::UnsupportedPng);
        }
    }
}

#[test]
fn supported_metadata_has_strict_shape_order_and_multiplicity() {
    let (source, candidate) = pair(2);
    for chunk in [
        (*b"gAMA", vec![0; 4]),
        (*b"sRGB", vec![4]),
        (*b"pHYs", vec![0; 8]),
        (*b"PLTE", vec![1]),
        (*b"tRNS", vec![1; 5]),
        (*b"tEXt", b"No separator".to_vec()),
        (*b"tEXt", b" Key\0text".to_vec()),
        (*b"tEXt", b"K  ey\0text".to_vec()),
        (*b"tEXt", b"Key\0text\0".to_vec()),
    ] {
        let mut changed = candidate.clone();
        changed.insert(1, chunk);
        refuse(&source, &changed, ByteRefusal::InvalidStructure);
    }
    for chunk in [
        (*b"gAMA", 45455_u32.to_be_bytes().to_vec()),
        (*b"sRGB", vec![0]),
        (*b"pHYs", vec![0; 9]),
        (*b"PLTE", vec![1, 2, 3]),
        (*b"tRNS", vec![0; 6]),
    ] {
        let mut changed = candidate.clone();
        changed.insert(2, chunk.clone());
        refuse(&source, &changed, ByteRefusal::InvalidStructure);
        changed = candidate.clone();
        changed.splice(1..1, [chunk.clone(), chunk]);
        refuse(&source, &changed, ByteRefusal::InvalidStructure);
    }
    let (rgba_source, mut rgba) = pair(6);
    rgba.insert(1, (*b"tRNS", vec![0; 6]));
    refuse(&rgba_source, &rgba, ByteRefusal::InvalidStructure);
    for chunks in [
        vec![(*b"PLTE", vec![1, 2, 3]), (*b"sRGB", vec![0])],
        vec![(*b"tRNS", vec![0; 6]), (*b"PLTE", vec![1, 2, 3])],
    ] {
        let mut changed = candidate.clone();
        changed.splice(1..1, chunks);
        refuse(&source, &changed, ByteRefusal::InvalidStructure);
    }
}

#[test]
fn metadata_integer_ranges_and_gamma_srgb_consistency_are_validated() {
    let (source, candidate) = pair(2);
    let gamma = |value: u32| (*b"gAMA", value.to_be_bytes().to_vec());
    let density = |x: u32, y: u32| {
        let mut bytes = x.to_be_bytes().to_vec();
        bytes.extend_from_slice(&y.to_be_bytes());
        bytes.push(1);
        (*b"pHYs", bytes)
    };
    for chunks in [
        vec![gamma(0x8000_0000)],
        vec![density(0x8000_0000, 1)],
        vec![density(1, 0x8000_0000)],
        vec![gamma(1), (*b"sRGB", vec![0])],
        vec![(*b"sRGB", vec![0]), gamma(1)],
    ] {
        let mut bad_source = source.clone();
        let mut bad_candidate = candidate.clone();
        bad_source.splice(1..1, chunks.clone());
        bad_candidate.splice(1..1, chunks);
        refuse(&bad_source, &bad_candidate, ByteRefusal::InvalidStructure);
    }
    for chunks in [
        vec![gamma(0x7fff_ffff), density(0x7fff_ffff, 0x7fff_ffff)],
        vec![(*b"sRGB", vec![0]), gamma(45455)],
        // PNG3 requires unused high tRNS bits to be masked by readers; identity
        // samples stay RGB and the original transparency bytes stay unchanged.
        vec![(*b"tRNS", vec![255, 11, 128, 22, 1, 33])],
    ] {
        let mut valid_source = source.clone();
        let mut valid_candidate = candidate.clone();
        valid_source.splice(1..1, chunks.clone());
        valid_candidate.splice(1..1, chunks);
        validate_png_pair(&encode(&valid_source), &encode(&valid_candidate), limits()).unwrap();
    }
}

#[test]
fn dimensions_and_explicit_budgets_are_checked_before_large_allocation() {
    let (source_chunks, candidate_chunks) = pair(6);
    let source = encode(&source_chunks);
    let candidate = encode(&candidate_chunks);
    let exact = ByteValidationLimits {
        source_bytes: source.len(),
        candidate_bytes: candidate.len(),
        decoded_bytes_per_image: samples(6).len(),
        chunks_per_image: 3,
        ..limits()
    };
    validate_png_pair(&source, &candidate, exact).unwrap();
    for constrained in [
        ByteValidationLimits {
            source_bytes: source.len() - 1,
            ..exact
        },
        ByteValidationLimits {
            candidate_bytes: candidate.len() - 1,
            ..exact
        },
        ByteValidationLimits {
            decoded_bytes_per_image: samples(6).len() - 1,
            ..exact
        },
        ByteValidationLimits {
            chunks_per_image: 2,
            ..exact
        },
        ByteValidationLimits {
            decoder_allocation_bytes: 1,
            ..exact
        },
    ] {
        assert_eq!(
            validate_png_pair(&source, &candidate, constrained).unwrap_err(),
            ByteRefusal::LimitExceeded
        );
    }
    for (dimension, reason) in [
        (0_u32, ByteRefusal::InvalidStructure),
        (0x8000_0000, ByteRefusal::InvalidStructure),
        (0x7fff_ffff, ByteRefusal::LimitExceeded),
    ] {
        let mut huge = candidate_chunks.clone();
        huge[0].1[..4].copy_from_slice(&dimension.to_be_bytes());
        refuse(&source_chunks, &huge, reason);
    }
}

proptest! {
    #[test]
    fn arbitrary_bytes_never_manufacture_validated_evidence(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let (source, _) = pair(2);
        prop_assert!(validate_png_pair(&encode(&source), &bytes, limits()).is_err());
    }
}
