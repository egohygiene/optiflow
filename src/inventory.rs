use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result};

use crate::adapters::ffprobe;
use crate::domain::{
    CachedAnalysis, EvidenceValidity, MediaKind, ObservationStability, ObservationStatus,
};

pub fn analyze(path: &Path, probe_media: bool, ffprobe_signature: Option<&str>) -> CachedAnalysis {
    let mut warnings = Vec::new();

    let detected = match infer::get_from_path(path) {
        Ok(value) => value,
        Err(error) => {
            return CachedAnalysis {
                content_type: None,
                media_kind: MediaKind::Unknown,
                content_hash: None,
                media: None,
                probe_signature: probe_media.then(|| {
                    ffprobe_signature
                        .unwrap_or("ffprobe-unavailable")
                        .to_owned()
                }),
                status: ObservationStatus::Unreadable,
                warnings: vec![format!("content inspection failed: {error}")],
                observation_stability: ObservationStability::Unreadable,
                evidence_validity: EvidenceValidity::Unavailable,
                attempt_count: 1,
            };
        }
    };

    let content_type = detected.map(|kind| kind.mime_type().to_owned());
    let media_kind = content_type
        .as_deref()
        .map(classify_content_type)
        .unwrap_or(MediaKind::Unknown);
    let status = if matches!(media_kind, MediaKind::Unknown | MediaKind::Other) {
        ObservationStatus::Unsupported
    } else {
        ObservationStatus::Analyzed
    };

    let media = if probe_media
        && ffprobe_signature.is_some()
        && matches!(
            media_kind,
            MediaKind::Image | MediaKind::Video | MediaKind::Audio
        ) {
        match ffprobe::inspect(path) {
            Ok(descriptor) => Some(descriptor),
            Err(error) => {
                warnings.push(error.to_string());
                None
            }
        }
    } else {
        None
    };

    if probe_media
        && ffprobe_signature.is_none()
        && !matches!(status, ObservationStatus::Unsupported)
    {
        warnings.push("ffprobe is unavailable; stream metadata was not collected".to_owned());
    }

    CachedAnalysis {
        content_type,
        media_kind,
        content_hash: None,
        media,
        probe_signature: probe_media.then(|| {
            ffprobe_signature
                .unwrap_or("ffprobe-unavailable")
                .to_owned()
        }),
        status,
        warnings,
        observation_stability: ObservationStability::Stable,
        evidence_validity: EvidenceValidity::Current,
        attempt_count: 1,
    }
}

/// Analyze content through an already-opened file handle.
///
/// The handle is rewound before and after each consumer so classification,
/// optional probing, and hashing can share one validated observation window.
pub fn analyze_file(
    file: &mut File,
    display_path: &Path,
    probe_media: bool,
    ffprobe_signature: Option<&str>,
) -> Result<CachedAnalysis> {
    const SNIFF_BYTES: usize = 8192;

    file.seek(SeekFrom::Start(0))
        .context("failed to rewind opened file before content inspection")?;
    let mut prefix = vec![0_u8; SNIFF_BYTES];
    let read = file
        .read(&mut prefix)
        .context("failed to inspect content through opened file handle")?;
    prefix.truncate(read);
    file.seek(SeekFrom::Start(0))
        .context("failed to rewind opened file after content inspection")?;

    let detected = infer::get(&prefix);
    let content_type = detected.map(|kind| kind.mime_type().to_owned());
    let media_kind = content_type
        .as_deref()
        .map(classify_content_type)
        .unwrap_or(MediaKind::Unknown);
    let status = if matches!(media_kind, MediaKind::Unknown | MediaKind::Other) {
        ObservationStatus::Unsupported
    } else {
        ObservationStatus::Analyzed
    };
    let mut warnings = Vec::new();

    let media = if probe_media
        && ffprobe_signature.is_some()
        && matches!(
            media_kind,
            MediaKind::Image | MediaKind::Video | MediaKind::Audio
        ) {
        let result = ffprobe::inspect_file(file, display_path);
        file.seek(SeekFrom::Start(0))
            .context("failed to rewind opened file after ffprobe inspection")?;
        match result {
            Ok(descriptor) => Some(descriptor),
            Err(error) => {
                warnings.push(error.to_string());
                None
            }
        }
    } else {
        None
    };

    if probe_media
        && ffprobe_signature.is_none()
        && !matches!(status, ObservationStatus::Unsupported)
    {
        warnings.push("ffprobe is unavailable; stream metadata was not collected".to_owned());
    }

    Ok(CachedAnalysis {
        content_type,
        media_kind,
        content_hash: None,
        media,
        probe_signature: probe_media.then(|| {
            ffprobe_signature
                .unwrap_or("ffprobe-unavailable")
                .to_owned()
        }),
        status,
        warnings,
        observation_stability: ObservationStability::Stable,
        evidence_validity: EvidenceValidity::Current,
        attempt_count: 1,
    })
}

fn classify_content_type(content_type: &str) -> MediaKind {
    match content_type.split_once('/').map(|(prefix, _)| prefix) {
        Some("image") => MediaKind::Image,
        Some("video") => MediaKind::Video,
        Some("audio") => MediaKind::Audio,
        Some(_) => MediaKind::Other,
        None => MediaKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_prefixes_map_to_media_kinds() {
        assert_eq!(classify_content_type("image/png"), MediaKind::Image);
        assert_eq!(classify_content_type("video/mp4"), MediaKind::Video);
        assert_eq!(classify_content_type("audio/flac"), MediaKind::Audio);
        assert_eq!(classify_content_type("application/pdf"), MediaKind::Other);
    }
}
