use std::collections::BTreeMap;

use crate::domain::{
    DuplicateGroup, DuplicateMember, EvidenceValidity, ExactDuplicateEvidence, ExtentSharingStatus,
    FileObservation, PhysicalReclaimability, ReclaimabilityReasonCode, ReclaimabilityStatus,
};
use crate::hashing::HASH_ALGORITHM;

/// Build exact-duplicate groups from `observations`.
///
/// Groups are keyed by `(logical_size_bytes, complete_content_hash)`.
///
/// Hard-link aliases – observations that share the same stable filesystem
/// identity – are folded into a single `DuplicateMember` entry rather than
/// counted as independent objects.  This prevents over-counting reclaimable
/// bytes when multiple directory entries reference the same inode.
///
/// Physical reclaimability is marked `unknown` unless all members have known
/// allocation metadata and no unobserved hard links.
pub fn exact_groups(observations: &[FileObservation]) -> Vec<DuplicateGroup> {
    // --- Step 1: resolve hard-link aliases ---------------------------------
    // Map stable identity key → (primary observation_id, [all paths]).
    // Observations without stable identity are each treated as independent.
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    enum IdentityKey {
        Known(String, String), // (filesystem_id, file_id)
        Anonymous(String),     // observation_id used as a fallback key
    }

    let mut identity_to_primary: BTreeMap<IdentityKey, String> = BTreeMap::new();
    // primary_observation_id → all alias paths (sorted later)
    let mut primary_to_aliases: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for obs in observations {
        let key = match &obs.filesystem_identity {
            Some(id) => IdentityKey::Known(id.filesystem_id.clone(), id.file_id.clone()),
            None => IdentityKey::Anonymous(obs.observation_id.clone()),
        };

        let primary_id = identity_to_primary
            .entry(key)
            .or_insert_with(|| obs.observation_id.clone())
            .clone();

        primary_to_aliases
            .entry(primary_id)
            .or_default()
            .push(obs.path.clone());
    }

    // --- Step 2: build a map from primary_observation_id → observation ------
    let obs_by_id: BTreeMap<&str, &FileObservation> = observations
        .iter()
        .map(|o| (o.observation_id.as_str(), o))
        .collect();

    // --- Step 3: group by (size, hash) over unique objects only ------------
    // Only observations with current (stable) evidence may participate.
    let mut candidates: BTreeMap<(u64, String), Vec<&FileObservation>> = BTreeMap::new();

    for primary_id in primary_to_aliases.keys() {
        if let Some(obs) = obs_by_id.get(primary_id.as_str()) {
            if obs.evidence_validity != EvidenceValidity::Current {
                // Unstable evidence must not form exact-duplicate groups.
                continue;
            }
            if let Some(content_hash) = &obs.content_hash {
                candidates
                    .entry((obs.size_bytes, content_hash.clone()))
                    .or_default()
                    .push(obs);
            }
        }
    }

    // --- Step 4: emit groups with corrected accounting ---------------------
    candidates
        .into_iter()
        .filter(|(_, members)| members.len() > 1)
        .map(|((size_bytes, content_hash), mut members)| {
            members.sort_by(|l, r| l.path.cmp(&r.path));

            let group_seed = format!("{size_bytes}:{content_hash}");
            let group_hash = blake3::hash(group_seed.as_bytes()).to_hex().to_string();
            let group_id = format!("exact-{}", &group_hash[..16]);

            let member_count = u64::try_from(members.len()).unwrap_or(u64::MAX);
            let observed_path_count: u64 = members
                .iter()
                .map(|obs| {
                    let alias_count = primary_to_aliases
                        .get(obs.observation_id.as_str())
                        .map(|v| v.len())
                        .unwrap_or(1);
                    u64::try_from(alias_count).unwrap_or(u64::MAX)
                })
                .fold(0u64, u64::saturating_add);

            // Reclaimable = unique-object size × (unique objects − 1).
            let reclaimable_bytes = size_bytes.saturating_mul(member_count.saturating_sub(1));

            // Assess physical reclaimability.
            let physical_reclaimability = assess_reclaimability(&members);

            DuplicateGroup {
                group_id,
                classification: "exact".to_owned(),
                evidence: ExactDuplicateEvidence {
                    algorithm: HASH_ALGORITHM.to_owned(),
                    complete_content_hash: content_hash,
                    identical_size_bytes: size_bytes,
                    member_count,
                    observed_path_count,
                },
                reclaimable_bytes,
                physical_reclaimability,
                members: members
                    .into_iter()
                    .map(|obs| {
                        let mut aliases: Vec<String> = primary_to_aliases
                            .get(obs.observation_id.as_str())
                            .cloned()
                            .unwrap_or_default();
                        aliases.sort();
                        // The primary path is the lexicographically first path.
                        let primary_path =
                            aliases.first().cloned().unwrap_or_else(|| obs.path.clone());
                        let alias_paths: Vec<String> =
                            aliases.into_iter().filter(|p| p != &primary_path).collect();
                        DuplicateMember {
                            path: primary_path,
                            observation_id: obs.observation_id.clone(),
                            alias_paths,
                        }
                    })
                    .collect(),
            }
        })
        .collect()
}

fn assess_reclaimability(members: &[&FileObservation]) -> PhysicalReclaimability {
    let mut reason_codes = Vec::new();

    // Check for missing identity.
    if members.iter().any(|obs| obs.filesystem_identity.is_none()) {
        reason_codes.push(ReclaimabilityReasonCode::FilesystemIdentityUnavailable);
    }

    // Check for missing allocation metadata.
    if members.iter().any(|obs| {
        obs.storage_allocation
            .as_ref()
            .is_none_or(|a| a.allocated_size_bytes.is_none())
    }) {
        reason_codes.push(ReclaimabilityReasonCode::AllocationMetadataUnavailable);
    }

    // Check for unobserved hard links.
    if members.iter().any(|obs| {
        obs.filesystem_identity
            .as_ref()
            .and_then(|id| id.link_count)
            .is_some_and(|lc| lc > 1)
    }) {
        reason_codes.push(ReclaimabilityReasonCode::UnobservedHardLinks);
    }

    // Extent sharing is always unknown on the v2 initial release (no platform
    // API for reliable clone detection has been integrated yet).
    if members.iter().any(|obs| {
        obs.storage_allocation
            .as_ref()
            .is_some_and(|a| a.extent_sharing_status == ExtentSharingStatus::Unknown)
    }) {
        reason_codes.push(ReclaimabilityReasonCode::ExtentSharingUnknown);
    }

    if reason_codes.is_empty() {
        // No known reason to be uncertain; give an estimate.
        PhysicalReclaimability {
            status: ReclaimabilityStatus::Estimated,
            reason_codes: Vec::new(),
        }
    } else {
        PhysicalReclaimability {
            status: ReclaimabilityStatus::Unknown,
            reason_codes,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{FilesystemIdentity, MediaKind, ObservationStatus, StorageAllocation};
    use crate::filesystem::identity::{AllocationSource, ExtentSharingStatus};

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
            filesystem_identity: None,
            storage_allocation: None,
            observation_stability: crate::domain::ObservationStability::Stable,
            evidence_validity: crate::domain::EvidenceValidity::Current,
            attempt_count: 1,
        }
    }

    fn observation_with_identity(
        path: &str,
        hash: &str,
        filesystem_id: &str,
        file_id: &str,
        link_count: u64,
    ) -> FileObservation {
        let mut obs = observation(path, hash);
        obs.filesystem_identity = Some(FilesystemIdentity {
            platform: "linux".to_owned(),
            filesystem_id: filesystem_id.to_owned(),
            file_id: file_id.to_owned(),
            link_count: Some(link_count),
        });
        obs.storage_allocation = Some(StorageAllocation {
            logical_size_bytes: 10,
            allocated_size_bytes: Some(4096),
            allocation_source: AllocationSource::StBlocks512,
            extent_sharing_status: ExtentSharingStatus::Unknown,
        });
        obs
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

    #[test]
    fn hard_link_aliases_are_not_counted_as_independent_members() {
        // /a and /b share the same inode – they are aliases of one object.
        // /c is a second independent object with the same content.
        let observations = vec![
            observation_with_identity("/a", "hash1", "dev1", "ino1", 2),
            observation_with_identity("/b", "hash1", "dev1", "ino1", 2),
            observation_with_identity("/c", "hash1", "dev1", "ino2", 1),
        ];

        let groups = exact_groups(&observations);
        assert_eq!(groups.len(), 1, "one group expected");
        // Only 2 unique objects (ino1 and ino2), not 3 paths.
        assert_eq!(groups[0].evidence.member_count, 2);
        // 3 total observed paths.
        assert_eq!(groups[0].evidence.observed_path_count, 3);
        // Reclaimable = 10 × (2−1) = 10, not 20.
        assert_eq!(groups[0].reclaimable_bytes, 10);
    }

    #[test]
    fn pure_alias_group_is_not_a_duplicate_group() {
        // Both paths point to the same inode – no independent duplicate.
        let observations = vec![
            observation_with_identity("/x", "hashX", "dev1", "ino1", 2),
            observation_with_identity("/y", "hashX", "dev1", "ino1", 2),
        ];

        let groups = exact_groups(&observations);
        assert_eq!(
            groups.len(),
            0,
            "aliases alone must not form a duplicate group"
        );
    }

    #[test]
    fn unstable_observations_are_excluded_from_duplicate_groups() {
        // /a has stale evidence (changed during hash); /b and /c are stable and
        // identical.  Only /b and /c should form a group.
        let mut unstable = observation("/a", "same");
        unstable.evidence_validity = crate::domain::EvidenceValidity::Stale;
        unstable.observation_stability = crate::domain::ObservationStability::ChangedDuringHash;

        let stable_b = observation("/b", "same");
        let stable_c = observation("/c", "same");

        let groups = exact_groups(&[unstable, stable_b, stable_c]);
        assert_eq!(groups.len(), 1, "one group from stable observations");
        assert_eq!(
            groups[0].evidence.member_count, 2,
            "only stable observations counted"
        );
        // Ensure /a is not a member of the group.
        let paths: Vec<&str> = groups[0].members.iter().map(|m| m.path.as_str()).collect();
        assert!(!paths.contains(&"/a"), "/a must not appear in the group");
    }
}
