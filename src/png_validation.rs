//! Read-only observations of real PNG bytes, independent of provider claims.
//!
//! This deliberately supports a subset of the candidate profile. Success proves
//! byte preservation and encoded reduction, not filesystem stability, provider
//! execution, resource measurements, artifact acceptance, or write authority.

use std::collections::BTreeMap;
use std::io::Cursor;

use flate2::{Decompress, FlushDecompress, Status};

use crate::domain::EvidenceDigest;
use crate::png_candidate::{ContentIdentity, PngFacts};

/// Explicit per-call ceilings. Decoder accounting is best effort, not an RSS
/// limit; caller input, sample buffers and chunk bookkeeping are outside it.
#[derive(Debug, Clone, Copy)]
pub struct ByteValidationLimits {
    pub source_bytes: usize,
    pub candidate_bytes: usize,
    pub decoded_bytes_per_image: usize,
    pub chunks_per_image: usize,
    pub decoder_allocation_bytes: usize,
}

/// Content-free refusals; unsupported does not necessarily mean invalid PNG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteRefusal {
    LimitExceeded,
    InvalidStructure,
    InvalidChecksum,
    UnsupportedPng,
    IncompleteImageStream,
    DecodeFailed,
    PreservationMismatch,
    NotSmaller,
}

/// Constructed only by actual byte validation. No deserialization or public
/// fields: supplied contract declarations cannot manufacture this result.
#[derive(Debug)]
pub struct ValidatedPngPair {
    source: ContentIdentity,
    candidate: ContentIdentity,
    source_png: PngFacts,
    candidate_png: PngFacts,
}

impl ValidatedPngPair {
    pub fn source(&self) -> &ContentIdentity {
        &self.source
    }
    pub fn candidate(&self) -> &ContentIdentity {
        &self.candidate
    }
    pub fn source_png(&self) -> &PngFacts {
        &self.source_png
    }
    pub fn candidate_png(&self) -> &PngFacts {
        &self.candidate_png
    }
    /// Logical encoded bytes only; retaining both files consumes more space.
    pub fn encoded_byte_reduction(&self) -> u64 {
        self.source.logical_bytes - self.candidate.logical_bytes
    }
}

/// Observe two immutable byte slices. This function opens no files, starts no
/// processes and writes no artifacts. See docs/png-candidate-contract.md for
/// the supported metadata subset and the remaining host responsibilities.
pub fn validate_png_pair(
    source: &[u8],
    candidate: &[u8],
    limits: ByteValidationLimits,
) -> Result<ValidatedPngPair, ByteRefusal> {
    if source.len() > limits.source_bytes
        || candidate.len() > limits.candidate_bytes
        || limits.decoded_bytes_per_image == 0
        || limits.chunks_per_image == 0
        || limits.decoder_allocation_bytes == 0
    {
        return Err(ByteRefusal::LimitExceeded);
    }
    let source_image = observe(source, limits)?;
    let candidate_image = observe(candidate, limits)?;
    // Compare complete actual bytes, not just their compact digest records.
    if source_image.preserved != candidate_image.preserved
        || source_image.samples != candidate_image.samples
    {
        return Err(ByteRefusal::PreservationMismatch);
    }
    if candidate.len() >= source.len() {
        return Err(ByteRefusal::NotSmaller);
    }
    Ok(ValidatedPngPair {
        source: identity(source),
        candidate: identity(candidate),
        source_png: source_image.facts,
        candidate_png: candidate_image.facts,
    })
}

struct Observed<'a> {
    // None is the one IDAT-run marker, binding all metadata placement.
    preserved: Vec<Option<&'a [u8]>>,
    samples: Vec<u8>,
    facts: PngFacts,
}

fn observe(bytes: &[u8], limits: ByteValidationLimits) -> Result<Observed<'_>, ByteRefusal> {
    use ByteRefusal::{InvalidStructure, LimitExceeded, UnsupportedPng};
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(InvalidStructure);
    }
    let mut offset = 8_usize;
    let mut count = 0_usize;
    let mut ihdr = None;
    let mut preserved = Vec::new();
    let mut compressed = Vec::new();
    let mut seen = BTreeMap::new();
    let mut idat_started = false;
    let mut idat_ended = false;
    let mut ended = false;
    while offset < bytes.len() {
        count += 1;
        if count > limits.chunks_per_image {
            return Err(LimitExceeded);
        }
        let header = bytes
            .get(offset..offset.checked_add(8).ok_or(InvalidStructure)?)
            .ok_or(InvalidStructure)?;
        let length = be32(&header[..4]) as usize;
        if length > 0x7fff_ffff {
            return Err(InvalidStructure);
        }
        let end = offset
            .checked_add(length)
            .and_then(|n| n.checked_add(12))
            .ok_or(InvalidStructure)?;
        let raw = bytes.get(offset..end).ok_or(InvalidStructure)?;
        let kind: [u8; 4] = header[4..8].try_into().expect("four-byte chunk type");
        if !kind.iter().all(u8::is_ascii_alphabetic) || !kind[2].is_ascii_uppercase() {
            return Err(InvalidStructure);
        }
        if crc32fast::hash(&raw[4..raw.len() - 4]) != be32(&raw[raw.len() - 4..]) {
            return Err(ByteRefusal::InvalidChecksum);
        }
        let data = &raw[8..raw.len() - 4];
        if count == 1 && kind != *b"IHDR" {
            return Err(InvalidStructure);
        }
        if kind != *b"IDAT" && idat_started {
            idat_ended = true;
        }
        match &kind {
            b"IHDR" => {
                if count != 1 || data.len() != 13 {
                    return Err(InvalidStructure);
                }
                let width = be32(&data[..4]);
                let height = be32(&data[4..8]);
                if width == 0 || height == 0 || width > 0x7fff_ffff || height > 0x7fff_ffff {
                    return Err(InvalidStructure);
                }
                if data[8] != 8 || !matches!(data[9], 2 | 6) || data[10..] != [0, 0, 0] {
                    return Err(UnsupportedPng);
                }
                let size = u64::from(width) * u64::from(height) * if data[9] == 2 { 3 } else { 4 };
                if size > limits.decoded_bytes_per_image as u64 {
                    return Err(LimitExceeded);
                }
                ihdr = Some(data);
            }
            b"IDAT" => {
                if idat_ended {
                    return Err(InvalidStructure);
                }
                if !idat_started {
                    preserved.try_reserve(1).map_err(|_| LimitExceeded)?;
                    preserved.push(None);
                    idat_started = true;
                }
                compressed
                    .try_reserve(data.len())
                    .map_err(|_| LimitExceeded)?;
                compressed.extend_from_slice(data);
            }
            b"IEND" => {
                if !idat_started || !data.is_empty() || end != bytes.len() {
                    return Err(InvalidStructure);
                }
                ended = true;
            }
            b"acTL" | b"fcTL" | b"fdAT" => return Err(UnsupportedPng),
            _ => validate_metadata(
                kind,
                data,
                ihdr.ok_or(InvalidStructure)?[9],
                idat_started,
                &mut seen,
            )?,
        }
        if kind != *b"IDAT" {
            preserved.try_reserve(1).map_err(|_| LimitExceeded)?;
            preserved.push(Some(raw));
        }
        offset = end;
    }
    if !ended {
        return Err(InvalidStructure);
    }
    let ihdr = ihdr.ok_or(InvalidStructure)?;
    let width = be32(&ihdr[..4]);
    let height = be32(&ihdr[4..8]);
    let channels = if ihdr[9] == 2 { 3_usize } else { 4 };
    let row_bytes = (width as usize)
        .checked_mul(channels)
        .ok_or(LimitExceeded)?;
    let sample_size = row_bytes
        .checked_mul(height as usize)
        .ok_or(LimitExceeded)?;
    let filtered_size = (sample_size as u64) + u64::from(height);
    check_zlib(&compressed, filtered_size)?;
    drop(compressed);

    let mut options = png::DecodeOptions::default();
    options.set_ignore_checksums(false);
    options.set_skip_ancillary_crc_failures(false);
    // Compressed metadata is refused above; text is checked without allocation.
    options.set_ignore_text_chunk(true);
    options.set_ignore_iccp_chunk(true);
    let mut decoder = png::Decoder::new_with_options(Cursor::new(bytes), options);
    decoder.set_limits(png::Limits {
        bytes: limits.decoder_allocation_bytes,
    });
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().map_err(decode_error)?;
    if reader.output_buffer_size() != Some(sample_size) {
        return Err(ByteRefusal::DecodeFailed);
    }
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(sample_size)
        .map_err(|_| LimitExceeded)?;
    samples.resize(sample_size, 0);
    let output = reader.next_frame(&mut samples).map_err(decode_error)?;
    if output.width != width
        || output.height != height
        || output.bit_depth != png::BitDepth::Eight
        || output.color_type as u8 != ihdr[9]
        || output.line_size != row_bytes
        || output.buffer_size() != sample_size
    {
        return Err(ByteRefusal::DecodeFailed);
    }
    reader.finish().map_err(decode_error)?;
    let mut preservation = blake3::Hasher::new();
    preservation.update(b"optiflow.png-preservation.v1\0");
    for record in &preserved {
        match record {
            Some(raw) => {
                preservation.update(&[0]);
                preservation.update(&(raw.len() as u64).to_be_bytes());
                preservation.update(raw);
            }
            None => {
                preservation.update(&[1]);
            }
        }
    }
    let facts = PngFacts {
        width,
        height,
        bit_depth: 8,
        color_type: ihdr[9],
        frames: 1,
        complete_decode: true,
        animation_chunks: false,
        unknown_unsafe_to_copy_chunks: false,
        decoded_bytes: sample_size as u64,
        ihdr_digest: digest(ihdr),
        decoded_samples_digest: digest(&samples),
        non_idat_chunks_digest: hash_record(preservation.finalize()),
    };
    Ok(Observed {
        preserved,
        samples,
        facts,
    })
}

fn validate_metadata<'a>(
    kind: [u8; 4],
    data: &'a [u8],
    color: u8,
    after_idat: bool,
    seen: &mut BTreeMap<[u8; 4], &'a [u8]>,
) -> Result<(), ByteRefusal> {
    use ByteRefusal::{InvalidStructure, UnsupportedPng};
    let singleton = matches!(&kind, b"PLTE" | b"tRNS" | b"gAMA" | b"sRGB" | b"pHYs");
    if singleton && (after_idat || seen.insert(kind, data).is_some()) {
        return Err(InvalidStructure);
    }
    let valid = match &kind {
        b"PLTE" => {
            !seen.contains_key(b"tRNS")
                && !data.is_empty()
                && data.len() <= 768
                && data.len() % 3 == 0
        }
        b"tRNS" => color == 2 && data.len() == 6,
        b"gAMA" => {
            !seen.contains_key(b"PLTE")
                && data.len() == 4
                && (1..=0x7fff_ffff).contains(&be32(data))
                && (!seen.contains_key(b"sRGB") || be32(data) == 45455)
        }
        b"sRGB" => {
            !seen.contains_key(b"PLTE")
                && data.len() == 1
                && data[0] <= 3
                && seen.get(b"gAMA").is_none_or(|gamma| be32(gamma) == 45455)
        }
        b"pHYs" => {
            data.len() == 9
                && data[8] <= 1
                && be32(&data[..4]) <= 0x7fff_ffff
                && be32(&data[4..8]) <= 0x7fff_ffff
        }
        b"tEXt" => {
            let Some(separator) = data.iter().position(|byte| *byte == 0) else {
                return Err(InvalidStructure);
            };
            let key = &data[..separator];
            (1..=79).contains(&key.len())
                && key.iter().all(|byte| matches!(byte, 32..=126 | 161..=255))
                && key.first() != Some(&b' ')
                && key.last() != Some(&b' ')
                && !key.windows(2).any(|pair| pair == b"  ")
                && !data[separator + 1..].contains(&0)
        }
        // Only unknown *private* safe-to-copy ancillary chunks are admitted as
        // opaque bytes. Other registered/public metadata needs separate support.
        _ if kind[0].is_ascii_lowercase()
            && kind[1].is_ascii_lowercase()
            && kind[3].is_ascii_lowercase() =>
        {
            true
        }
        _ => return Err(UnsupportedPng),
    };
    if valid { Ok(()) } else { Err(InvalidStructure) }
}

/// Require the complete first zlib stream, exact output length and no extra
/// compressed input. This is stricter than PNG's reader tolerance for unused
/// final IDAT bytes; png::Reader alone intentionally tolerates such input.
fn check_zlib(input: &[u8], expected: u64) -> Result<(), ByteRefusal> {
    let mut decoder = Decompress::new(true);
    let mut scratch = [0_u8; 8192];
    loop {
        let before = (decoder.total_in(), decoder.total_out());
        let status = decoder
            .decompress(
                &input[before.0 as usize..],
                &mut scratch,
                FlushDecompress::None,
            )
            .map_err(|_| ByteRefusal::IncompleteImageStream)?;
        let after = (decoder.total_in(), decoder.total_out());
        if after.1 > expected {
            return Err(ByteRefusal::IncompleteImageStream);
        }
        if status == Status::StreamEnd {
            return if after.0 == input.len() as u64 && after.1 == expected {
                Ok(())
            } else {
                Err(ByteRefusal::IncompleteImageStream)
            };
        }
        if before == after {
            return Err(ByteRefusal::IncompleteImageStream);
        }
    }
}

fn decode_error(error: png::DecodingError) -> ByteRefusal {
    match error {
        png::DecodingError::LimitsExceeded => ByteRefusal::LimitExceeded,
        _ => ByteRefusal::DecodeFailed,
    }
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().expect("checked four-byte field"))
}

fn hash_record(hash: blake3::Hash) -> EvidenceDigest {
    EvidenceDigest {
        algorithm: "blake3-256".to_owned(),
        value: hash.to_hex().to_string(),
    }
}

fn digest(bytes: &[u8]) -> EvidenceDigest {
    hash_record(blake3::hash(bytes))
}

fn identity(bytes: &[u8]) -> ContentIdentity {
    ContentIdentity {
        digest: digest(bytes),
        logical_bytes: bytes.len() as u64,
    }
}
