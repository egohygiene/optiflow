use std::collections::BTreeMap;

use crate::domain::{DuplicateGroup, DuplicateMember, ExactDuplicateEvidence, FileObservation};
use crate::hashing::HASH_ALGORITHM;

pub fn exact_groups(observations: &[FileObservation]) -> Vec<DuplicateGroup> {
    let mut candidates: BTreeMap<(u64, String), Vec<&FileObservation>> = BTreeMap::new();

    for observation in observations {
        if let Some(content_hash) = &observation.content_hash {
            candidates
                .entry((observation.size_bytes, content_hash.clone()))
                .or_default()
                .push(observation);
        }
    }

    candidates
        .into_iter()
        .filter(|(_, members)| members.len() > 1)
        .map(|((size_bytes, content_hash), mut members)| {
            members.sort_by(|left, right| left.path.cmp(&right.path));
            let group_seed = format!("{size_bytes}:{content_hash}");
            let group_hash = blake3::hash(group_seed.as_bytes()).to_hex().to_string();
            let group_id = format!("exact-{}", &group_hash[..16]);
            let member_count = u64::try_from(members.len()).unwrap_or(u64::MAX);
            DuplicateGroup {
                group_id,
                classification: "exact".to_owned(),
                evidence: ExactDuplicateEvidence {
                    algorithm: HASH_ALGORITHM.to_owned(),
                    complete_content_hash: content_hash,
                    identical_size_bytes: size_bytes,
                    member_count,
                },
                reclaimable_bytes: size_bytes.saturating_mul(member_count.saturating_sub(1)),
                members: members
                    .into_iter()
                    .map(|observation| DuplicateMember {
                        path: observation.path.clone(),
                        observation_id: observation.observation_id.clone(),
                    })
                    .collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::domain::{MediaKind, ObservationStatus};

    use super::*;

    fn observation(path: &str, hash: &str) -> FileObservation {
        FileObservation {
            observation_id: path.to_owned(),
            run_id: "run".to_owned(),
            path: path.to_owned(),
            size_bytes: 10,
            modified_unix_ns: None,
            device_id: None,
            inode: None,
            content_type: None,
            media_kind: MediaKind::Unknown,
            content_hash: Some(hash.to_owned()),
            hash_algorithm: Some(HASH_ALGORITHM.to_owned()),
            media: None,
            status: ObservationStatus::Unsupported,
            cache_hit: false,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn groups_only_matching_size_and_hash() {
        let groups = exact_groups(&[
            observation("/a", "same"),
            observation("/b", "same"),
            observation("/c", "different"),
        ]);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].members.len(), 2);
        assert_eq!(groups[0].reclaimable_bytes, 10);
    }
}
