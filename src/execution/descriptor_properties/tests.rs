//! Authored/unrun Linux synthetic port cases, not native Darwin qualification.
use super::*;
use property_evidence::{Ace, AceKind, AclSnapshot};
use std::{
    cell::Cell,
    io::{Seek, SeekFrom, Write},
};
struct Backend {
    reads: Cell<usize>,
    writes: Cell<usize>,
    change: bool,
    fail_write: bool,
}
impl Backend {
    fn new() -> Self {
        Self {
            reads: Cell::new(0),
            writes: Cell::new(0),
            change: false,
            fail_write: false,
        }
    }
}
impl DescriptorPropertyBackend for Backend {
    fn identity(&self) -> BackendIdentity {
        BackendIdentity {
            schema: "optiflow.descriptor-property-backend.v1".into(),
            implementation_id: "synthetic-only".into(),
            source_sha256: "a".repeat(64),
        }
    }
    fn observe(&self, file: &File) -> Result<MacApfsCrossCopyPropertiesV1> {
        let s = signature(file)?;
        let n = self.reads.get();
        self.reads.set(n + 1);
        Ok(MacApfsCrossCopyPropertiesV1 {
            schema: property_evidence::SCHEMA.into(),
            profile: property_evidence::PROFILE.into(),
            uid: s.uid,
            gid: s.gid,
            mode: s.mode,
            mtime: s.modified,
            birthtime: Timestamp {
                seconds: 0,
                nanoseconds: 0,
            },
            bsd_flags: 0,
            xattrs: vec![],
            acl: if self.change && n > 0 {
                AclSnapshot::Present {
                    flags: 0,
                    entries: vec![],
                }
            } else {
                AclSnapshot::Absent {}
            },
        })
    }
    fn install(&self, source: &File, dest: &File, _: &MacApfsCrossCopyPropertiesV1) -> Result<()> {
        self.writes.set(self.writes.get() + 1);
        if self.fail_write {
            bail!("synthetic setter failure");
        }
        let modified = source.metadata()?.modified()?;
        dest.set_times(std::fs::FileTimes::new().set_modified(modified))?;
        Ok(())
    }
}
fn fixture() -> (tempfile::TempDir, File, File) {
    let root = tempfile::tempdir().unwrap();
    let mut a = std::fs::OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(root.path().join("a"))
        .unwrap();
    let mut b = std::fs::OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(root.path().join("b"))
        .unwrap();
    a.write_all(b"same bounded content").unwrap();
    b.write_all(b"same bounded content").unwrap();
    (root, a, b)
}
#[test]
fn default_unavailable_backend_never_fabricates_absent_acl() {
    let (_r, a, _b) = fixture();
    assert!(observe(&a, &UnavailableBackend, 1024, &|| false).is_err());
}
#[test]
fn held_descriptor_observation_preserves_cursor_and_has_no_removal_authority() {
    let (_r, mut a, _b) = fixture();
    a.seek(SeekFrom::Start(3)).unwrap();
    let backend = Backend::new();
    let observed = observe(&a, &backend, 1024, &|| false).unwrap();
    assert_eq!(a.stream_position().unwrap(), 3);
    let evidence = observed.evidence().unwrap();
    evidence.validate().unwrap();
    assert!(!evidence.native_qualified && !evidence.source_removal_authority);
    assert_eq!(backend.reads.get(), 2);
}
#[test]
fn changing_acl_window_never_becomes_a_stable_observation() {
    let (_r, a, _b) = fixture();
    let mut backend = Backend::new();
    backend.change = true;
    assert!(observe(&a, &backend, 1024, &|| false).is_err());
}
#[test]
fn changed_contents_invalidate_the_live_token() {
    let (_r, mut a, _b) = fixture();
    let backend = Backend::new();
    let observed = observe(&a, &backend, 1024, &|| false).unwrap();
    a.seek(SeekFrom::Start(0)).unwrap();
    a.write_all(b"CHANGED").unwrap();
    assert!(recheck(&a, &backend, &observed, 1024, &|| false).is_err());
}
#[test]
fn metadata_copy_rechecks_distinct_exact_contents_and_independent_readback() {
    let (_r, a, b) = fixture();
    let backend = Backend::new();
    let expected = observe(&a, &backend, 1024, &|| false).unwrap();
    assert!(preserve_on_copy(&a, &a, &backend, &expected, 1024, &|| false).is_err());
    let out = preserve_on_copy(&a, &b, &backend, &expected, 1024, &|| false).unwrap();
    assert_eq!(expected.properties(), out.properties());
    assert_eq!(backend.writes.get(), 1);
    assert_ne!(expected.subject().inode, out.subject().inode);
}
#[test]
fn setter_failure_is_not_retried_or_converted_to_copy_success() {
    let (_r, a, b) = fixture();
    let mut backend = Backend::new();
    backend.fail_write = true;
    let expected = observe(&a, &backend, 1024, &|| false).unwrap();
    assert!(preserve_on_copy(&a, &b, &backend, &expected, 1024, &|| false).is_err());
    assert_eq!(backend.writes.get(), 1);
    assert_eq!(a.metadata().unwrap().len(), 20);
}
#[test]
fn size_and_cancellation_refuse_before_backend_or_setter_calls() {
    let (_r, a, b) = fixture();
    let backend = Backend::new();
    assert!(observe(&a, &backend, 1, &|| false).is_err());
    assert!(observe(&a, &backend, 1024, &|| true).is_err());
    assert_eq!(backend.reads.get(), 0);
    let expected = observe(&a, &backend, 1024, &|| false).unwrap();
    assert!(preserve_on_copy(&a, &b, &backend, &expected, 1024, &|| true).is_err());
    assert_eq!(backend.writes.get(), 0);
}
#[test]
fn persisted_self_consistency_never_constructs_a_live_observation_token() {
    let (_r, a, _b) = fixture();
    let evidence = observe(&a, &Backend::new(), 1024, &|| false)
        .unwrap()
        .evidence()
        .unwrap();
    let bytes = serde_json::to_vec(&evidence).unwrap();
    Evidence::from_json_slice(&bytes).unwrap();
    let mut changed = evidence.clone();
    changed.subject.content_blake3 = "b".repeat(64);
    assert!(changed.validate().is_err());
    let mut value = serde_json::to_value(evidence).unwrap();
    value["source_removal_authority"] = serde_json::json!(true);
    assert!(Evidence::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn absent_empty_ordered_and_deferred_acl_rules_stay_owned_by_property_contract() {
    let (_r, a, _b) = fixture();
    let base = Backend::new().observe(&a).unwrap();
    let absent = property_evidence::review(base.clone()).unwrap();
    let mut empty = base.clone();
    empty.acl = AclSnapshot::Present {
        flags: 0,
        entries: vec![],
    };
    assert_ne!(
        absent.fingerprint(),
        property_evidence::review(empty).unwrap().fingerprint()
    );
    let mut deferred = base;
    deferred.acl = AclSnapshot::Present {
        flags: 0x10000,
        entries: vec![Ace {
            principal_hex: "a".repeat(32),
            kind: AceKind::Allow,
            flags: 0,
            permissions: 2,
        }],
    };
    assert!(property_evidence::review(deferred).is_err());
}

#[test]
fn synchronized_copy_receipt_has_distinct_version_and_no_finalization_authority() {
    let (_r, a, b) = fixture();
    let backend = Backend::new();
    let token = observe(&a, &backend, 1024, &|| false).unwrap();
    let record = transfer::finish_copy(
        &a,
        &b,
        &backend,
        &token,
        transfer::Direction::RestoreCopy,
        1024,
        &|| false,
    )
    .unwrap();
    assert_eq!(record.schema, "optiflow.strong-property-copy-receipt.v1");
    transfer::inspect_retained_pair(&a, &b, &backend, &record, 1024, &|| false).unwrap();
    assert!(
        !record.source_removal_authority
            && !record.namespace_commit_observed
            && !record.native_qualified
    );
}
#[test]
fn changed_copy_pair_or_legacy_schema_cannot_recover() {
    let (_r, a, mut b) = fixture();
    let backend = Backend::new();
    let token = observe(&a, &backend, 1024, &|| false).unwrap();
    let mut record = transfer::finish_copy(
        &a,
        &b,
        &backend,
        &token,
        transfer::Direction::QuarantineCopy,
        1024,
        &|| false,
    )
    .unwrap();
    record.schema = "optiflow.execution-mutation.v2".into();
    assert!(record.validate().is_err());
    record.schema = "optiflow.strong-property-copy-receipt.v1".into();
    b.seek(SeekFrom::Start(0)).unwrap();
    b.write_all(b"CHANGED").unwrap();
    assert!(transfer::inspect_retained_pair(&a, &b, &backend, &record, 1024, &|| false).is_err());
}
#[test]
fn supplied_receipt_flags_cannot_gain_removal_or_namespace_authority() {
    let (_r, a, b) = fixture();
    let backend = Backend::new();
    let token = observe(&a, &backend, 1024, &|| false).unwrap();
    let record = transfer::finish_copy(
        &a,
        &b,
        &backend,
        &token,
        transfer::Direction::RestoreCopy,
        1024,
        &|| false,
    )
    .unwrap();
    for field in [
        "native_qualified",
        "namespace_commit_observed",
        "source_removal_authority",
    ] {
        let mut value = serde_json::to_value(&record).unwrap();
        value[field] = serde_json::json!(true);
        assert!(
            transfer::TransferReceipt::from_json_slice(&serde_json::to_vec(&value).unwrap())
                .is_err()
        );
    }
}
#[test]
fn staging_copy_preserves_source_and_refuses_nonempty_destinations() {
    let (_r, a, b) = fixture();
    let backend = Backend::new();
    let token = observe(&a, &backend, 1024, &|| false).unwrap();
    assert!(
        transfer::copy_to_staging(
            &a,
            &b,
            &backend,
            &token,
            transfer::Direction::RestoreCopy,
            1024,
            &|| false
        )
        .is_err()
    );
    b.set_len(0).unwrap();
    let record = transfer::copy_to_staging(
        &a,
        &b,
        &backend,
        &token,
        transfer::Direction::RestoreCopy,
        1024,
        &|| false,
    )
    .unwrap();
    transfer::inspect_retained_pair(&a, &b, &backend, &record, 1024, &|| false).unwrap();
    assert_eq!(a.metadata().unwrap().len(), 20);
    assert!(!record.source_removal_authority && !record.namespace_commit_observed);
}
#[test]
fn cancelled_staging_copy_does_not_write_or_install() {
    let (_r, a, b) = fixture();
    b.set_len(0).unwrap();
    let backend = Backend::new();
    let token = observe(&a, &backend, 1024, &|| false).unwrap();
    assert!(
        transfer::copy_to_staging(
            &a,
            &b,
            &backend,
            &token,
            transfer::Direction::QuarantineCopy,
            1024,
            &|| true
        )
        .is_err()
    );
    assert_eq!(b.metadata().unwrap().len(), 0);
    assert_eq!(backend.writes.get(), 0);
}
