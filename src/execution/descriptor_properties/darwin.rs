//! Native held-file metadata over an explicitly supplied safe ACL bridge.
//! No native dependency adoption, path reopening, unsafe shim or dispatch change.
use super::super::{filesystem, mutation};
use super::*;
use property_evidence::{AclSnapshot, ExtendedAttribute};
use std::{
    os::{macos::fs::FileTimesExt, unix::ffi::OsStrExt},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Integration must supply the reviewed safe descriptor ACL API. A successful
/// call is not native qualification; no CLI/document can instantiate this port.
pub trait DescriptorAclBackend {
    fn identity(&self) -> BackendIdentity;
    fn observe_acl(&self, file: &File) -> Result<AclSnapshot>;
    fn replace_acl(&self, file: &File, acl: &AclSnapshot) -> Result<()>;
}
pub struct DarwinProperties<A> {
    acl: A,
    acl_identity: BackendIdentity,
    identity: BackendIdentity,
}
impl<A: DescriptorAclBackend> DarwinProperties<A> {
    /// identity names the reviewed composite implementation, including its ACL
    /// dependency. It is a supplied pin, never a qualification certificate.
    pub fn new(acl: A, identity: BackendIdentity) -> Result<Self> {
        identity.validate()?;
        let acl_identity = acl.identity();
        acl_identity.validate()?;
        ensure!(
            identity.implementation_id != "unavailable"
                && acl_identity.implementation_id != "unavailable",
            "an actual selected safe ACL integration is required"
        );
        Ok(Self {
            acl,
            acl_identity,
            identity,
        })
    }
    fn check_backend(&self) -> Result<()> {
        ensure!(
            self.acl.identity() == self.acl_identity,
            "selected descriptor ACL backend changed"
        );
        Ok(())
    }
}
fn native<T>(value: super::super::Result<T>) -> Result<T> {
    value.map_err(|e| anyhow::anyhow!("native descriptor property operation refused: {e:?}"))
}
fn encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn decode(text: &str) -> Result<Vec<u8>> {
    ensure!(
        text.len() % 2 == 0
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "canonical property hex required"
    );
    text.as_bytes()
        .chunks_exact(2)
        .map(|v| Ok(u8::from_str_radix(std::str::from_utf8(v)?, 16)?))
        .collect()
}
fn time(value: &Timestamp) -> Result<SystemTime> {
    ensure!(
        value.nanoseconds < 1_000_000_000,
        "normalized native file time required"
    );
    let fraction = Duration::from_nanos(u64::from(value.nanoseconds));
    if value.seconds >= 0 {
        UNIX_EPOCH
            .checked_add(
                Duration::from_secs(value.seconds as u64)
                    .checked_add(fraction)
                    .context("native timestamp overflow")?,
            )
            .context("native timestamp is not representable")
    } else {
        UNIX_EPOCH
            .checked_sub(
                Duration::from_secs(value.seconds.unsigned_abs())
                    .checked_sub(fraction)
                    .context("native negative timestamp overflow")?,
            )
            .context("native negative timestamp is not representable")
    }
}
impl<A: DescriptorAclBackend> DescriptorPropertyBackend for DarwinProperties<A> {
    fn identity(&self) -> BackendIdentity {
        self.identity.clone()
    }
    fn observe(&self, file: &File) -> Result<MacApfsCrossCopyPropertiesV1> {
        self.check_backend()?;
        native(filesystem::require_apfs(file))?;
        let p = native(mutation::properties(file))?;
        ensure!(
            p.flags == 0,
            "strong Darwin copy profile refuses nonzero BSD flags"
        );
        let properties = MacApfsCrossCopyPropertiesV1 {
            schema: property_evidence::SCHEMA.into(),
            profile: property_evidence::PROFILE.into(),
            uid: p.uid,
            gid: p.gid,
            mode: p.mode,
            mtime: Timestamp {
                seconds: p.mtime.0,
                nanoseconds: p.mtime.1.try_into()?,
            },
            birthtime: Timestamp {
                seconds: p.birthtime.0,
                nanoseconds: p.birthtime.1.try_into()?,
            },
            bsd_flags: p.flags,
            xattrs: p
                .xattrs
                .into_iter()
                .map(|(name, value)| ExtendedAttribute {
                    name_hex: encode(&name),
                    value_hex: encode(&value),
                })
                .collect(),
            acl: self.acl.observe_acl(file)?,
        };
        self.check_backend()?;
        property_evidence::review(properties.clone())?;
        Ok(properties)
    }
    fn install(
        &self,
        source: &File,
        destination: &File,
        properties: &MacApfsCrossCopyPropertiesV1,
    ) -> Result<()> {
        use rustix::{
            fs::{Mode, XattrFlags, fchmod, fchown, fsetxattr},
            process::{Gid, Uid},
        };
        self.check_backend()?;
        property_evidence::review(properties.clone())?;
        native(filesystem::require_apfs(source))?;
        native(filesystem::require_apfs(destination))?;
        let a = signature(source)?;
        let b = signature(destination)?;
        ensure!(
            (a.device_id, a.inode) != (b.device_id, b.inode)
                && a.byte_count == b.byte_count
                && self.observe(source)? == *properties,
            "copy property installation requires distinct bound source/destination and current source properties"
        );
        let before = native(mutation::properties(destination))?;
        ensure!(before.flags == 0, "destination BSD flags unsupported");
        let attrs = properties
            .xattrs
            .iter()
            .map(|a| Ok((decode(&a.name_hex)?, decode(&a.value_hex)?)))
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            before
                .xattrs
                .iter()
                .all(|(name, _)| attrs.iter().any(|(expected, _)| name == expected)),
            "unknown destination-only xattrs retained; no metadata stripping"
        );
        let modified = time(&properties.mtime)?;
        let created = time(&properties.birthtime)?;
        // Set once. Any failure can leave changed destination metadata; retain
        // it for inspection and never remove/retry the original.
        fchown(
            destination,
            Some(Uid::from_raw(properties.uid)),
            Some(Gid::from_raw(properties.gid)),
        )?;
        fchmod(destination, Mode::from_raw_mode(properties.mode))?;
        for (name, value) in &attrs {
            fsetxattr(
                destination,
                std::ffi::OsStr::from_bytes(name),
                value,
                XattrFlags::empty(),
            )?;
        }
        destination.set_times(
            std::fs::FileTimes::new()
                .set_modified(modified)
                .set_created(created),
        )?;
        self.acl.replace_acl(destination, &properties.acl)?;
        self.check_backend()?;
        ensure!(
            self.observe(destination)? == *properties && self.observe(source)? == *properties,
            "Darwin property readback differs after installation"
        );
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_pre_epoch_file_times_preserve_fraction() {
        let t = time(&Timestamp {
            seconds: -1,
            nanoseconds: 500_000_000,
        })
        .unwrap();
        assert_eq!(
            UNIX_EPOCH.duration_since(t).unwrap(),
            Duration::from_millis(500)
        );
        assert!(
            time(&Timestamp {
                seconds: 0,
                nanoseconds: 1_000_000_000
            })
            .is_err()
        );
    }
}
