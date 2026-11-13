//! Closed-structure synthetic cases; payload bytes are not native-qualified FFV1.
use optiflow::video_container::{compare_stream_copy, inspect};
fn element(id: u64, data: &[u8]) -> Vec<u8> {
    let encoded = id.to_be_bytes();
    let start = encoded.iter().position(|v| *v != 0).unwrap();
    let mut out = encoded[start..].to_vec();
    let size = data.len() as u64;
    let width = (1..=8).find(|w| size < (1_u64 << (7 * w)) - 1).unwrap();
    let value = size | (1_u64 << (7 * width));
    out.extend_from_slice(&value.to_be_bytes()[8 - width..]);
    out.extend_from_slice(data);
    out
}
fn uint(id: u64, n: u64) -> Vec<u8> {
    let bytes = n.to_be_bytes();
    let start = bytes.iter().position(|v| *v != 0).unwrap_or(7);
    element(id, &bytes[start..])
}
fn joined(parts: Vec<Vec<u8>>) -> Vec<u8> {
    parts.concat()
}
fn fixture(payload: u8, padding: usize, extra: Option<Vec<u8>>) -> Vec<u8> {
    let color = element(
        0x55b0,
        &joined(vec![
            uint(0x55b1, 1),
            uint(0x55b9, 1),
            uint(0x55ba, 1),
            uint(0x55bb, 1),
        ]),
    );
    let video = element(0xe0, &joined(vec![uint(0xb0, 2), uint(0xba, 2), color]));
    let mut private = vec![0; 40];
    private[16..20].copy_from_slice(b"FFV1");
    let track = element(
        0xae,
        &joined(vec![
            uint(0xd7, 1),
            uint(0x73c5, 7),
            uint(0x83, 1),
            uint(0x23e383, 40_000_000),
            uint(0x88, 0),
            uint(0x9c, 0),
            element(0x22b59c, b"und"),
            element(0x86, b"V_MS/VFW/FOURCC"),
            element(0x63a2, &private),
            video,
        ]),
    );
    let block = element(0xa3, &[0x81, 0, 0, 0x80, payload]);
    let cluster = element(0x1f43b675, &joined(vec![uint(0xe7, 0), block]));
    let mut segment = joined(vec![
        element(0x1549a966, &uint(0x2ad7b1, 1_000_000)),
        element(0x1654ae6b, &track),
        cluster,
    ]);
    if padding > 0 {
        segment.extend(element(0xec, &vec![0; padding]));
    }
    if let Some(extra) = extra {
        segment.extend(extra);
    }
    joined(vec![
        element(0x1a45dfa3, &element(0x4282, b"matroska")),
        element(0x18538067, &segment),
    ])
}
#[test]
fn independent_packet_view_allows_padding_change_but_binds_payload_and_clock() {
    let source = inspect(&fixture(10, 512, None)).unwrap();
    let candidate = inspect(&fixture(10, 0, None)).unwrap();
    assert_eq!(source.void_bytes, 512);
    assert_eq!(candidate.void_bytes, 0);
    compare_stream_copy(&source, &candidate).unwrap();
    let changed = inspect(&fixture(11, 0, None)).unwrap();
    assert!(compare_stream_copy(&source, &changed).is_err());
}
#[test]
fn unknown_private_or_uninspected_container_features_are_refused() {
    for id in [0xbf, 0x1941a469, 0x1043a770, 0x7ba9, 0xabcdef] {
        assert!(inspect(&fixture(1, 0, Some(element(id, b"private")))).is_err());
    }
}
#[test]
fn truncation_oversize_and_unknown_lengths_cannot_be_claimed_complete() {
    let bytes = fixture(1, 0, None);
    for end in [0, 1, bytes.len() - 1] {
        assert!(inspect(&bytes[..end]).is_err());
    }
    assert!(inspect(&vec![0; 16_777_217]).is_err());
    assert!(inspect(&[0x1a, 0x45, 0xdf, 0xa3, 0xff]).is_err());
}
#[test]
fn extra_tracks_or_payload_flag_changes_are_not_stream_copy_equivalence() {
    let bytes = fixture(1, 0, Some(element(0x1654ae6b, &[])));
    assert!(inspect(&bytes).is_err());
    let source = inspect(&fixture(1, 0, None)).unwrap();
    let mut changed = source.clone();
    changed.packets[0].pts_milliseconds = 40;
    assert!(compare_stream_copy(&source, &changed).is_err());
    changed = source.clone();
    changed.packets[0].keyframe = false;
    assert!(compare_stream_copy(&source, &changed).is_err());
}
