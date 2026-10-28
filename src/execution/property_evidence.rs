//! Structural review of declared macOS cross-copy properties.
//!
//! This host-neutral contract performs no filesystem observation or mutation.
//! A reviewed declaration is not proof that a descriptor was observed, that
//! metadata was preserved, or that a caller may copy or remove a file. Native
//! observation, object binding, freshness, authorization and durability remain
//! separate requirements of any future transaction consumer.
//!
//! The fingerprint binds this schema and profile together with every declared
//! property. Its bare hexadecimal value must never be stored in an existing
//! v3 recovery event as if it used the historical Linux or `macos-apfs/v1`
//! property encoding. This module introduces no journal format or migration.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Standalone property declaration schema; not an execution or recovery event.
pub const SCHEMA: &str = "optiflow.execution-properties-macos-cross-copy.v1";
/// Initial conservative APFS cross-copy property profile.
pub const PROFILE: &str = "macos-apfs-cross-copy/v1";

const MAX_INPUT_BYTES: usize = 3 * 1024 * 1024;
const MAX_XATTRS: usize = 128;
const MAX_XATTR_NAME_BYTES: usize = 255;
const MAX_XATTR_VALUE_BYTES: usize = 65_536;
const MAX_XATTR_TOTAL_VALUE_BYTES: usize = 1_048_576;
const MAX_ACES: usize = 128;
const MAX_ACL_ENCODED_BYTES: usize = 65_536;
const ACL_FLAGS: u32 = 0x0003_ffff;
const ACL_DEFER_INHERIT: u32 = 0x0001_0000;
const ACE_FLAGS: u32 = 0x0000_01f0;
const ACE_PERMISSIONS: u32 = 0x01f0_3ffe;

/// A declared timestamp with normalized nonnegative nanoseconds.
///
/// Signed seconds preserve pre-epoch declarations. Acceptance here does not
/// establish that a native filesystem setter can represent the timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timestamp {
    pub seconds: i64,
    pub nanoseconds: u32,
}

/// Complete raw extended-attribute name and value, encoded as lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtendedAttribute {
    pub name_hex: String,
    pub value_hex: String,
}

/// The two ACE kinds admitted by this profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AceKind {
    Allow,
    Deny,
}

/// One ordered Darwin ACL entry, without principal-name resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ace {
    /// Exactly 16 raw UUID bytes, represented by 32 lowercase hex digits.
    pub principal_hex: String,
    pub kind: AceKind,
    /// Entry inheritance bits, excluding the ACE kind nibble.
    pub flags: u32,
    /// Explicit supported rights and generic rights bits.
    pub permissions: u32,
}

/// Declared ACL state. An absent ACL differs from a present, empty ACL.
///
/// There is deliberately no default, unknown or unreadable variant. Failed
/// observation cannot supply evidence for either supported state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum AclSnapshot {
    Absent {},
    Present {
        /// Preserve private low 16 bits and supported public ACL flags.
        flags: u32,
        /// Native order and duplicates are significant and never normalized.
        entries: Vec<Ace>,
    },
}

/// Provisional, standalone declaration for an initial APFS cross-copy profile.
///
/// All fields, including ACL state, are required in serialized input. This
/// profile omits atime, ctime and inode identity from preserved properties:
/// reads may advance atime, and copying creates a different object. A future
/// consumer must bind object identities independently of this declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacApfsCrossCopyPropertiesV1 {
    pub schema: String,
    pub profile: String,
    pub uid: u32,
    pub gid: u32,
    /// Regular-file type plus permission/special mode bits.
    pub mode: u32,
    pub mtime: Timestamp,
    pub birthtime: Timestamp,
    /// The initial profile admits only zero BSD flags.
    pub bsd_flags: u32,
    /// Strictly increasing raw name bytes; every name occurs exactly once.
    pub xattrs: Vec<ExtendedAttribute>,
    pub acl: AclSnapshot,
}

/// A declaration whose structure, bounds and profile have been reviewed.
///
/// The private fields prevent safe callers from changing the reviewed value
/// without reviewing it again. This is not a native observation token, an
/// authenticated receipt, or source-removal authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedProperties {
    snapshot: MacApfsCrossCopyPropertiesV1,
    fingerprint: String,
}

impl ReviewedProperties {
    /// Return the complete immutable declaration, including its schema/profile.
    #[must_use]
    pub fn snapshot(&self) -> &MacApfsCrossCopyPropertiesV1 {
        &self.snapshot
    }

    /// Return the domain-bound BLAKE3 fingerprint of the canonical declaration.
    ///
    /// A consumer must carry the schema/profile alongside this value. The hash
    /// is incompatible with legacy v3 recovery property fingerprints.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// Why a property declaration could not be reviewed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyError {
    /// Invalid JSON, missing/unknown fields, or invalid field representation.
    Malformed(String),
    /// The declaration names a different schema version.
    Schema(String),
    /// A profile, object kind, flag or right is outside this profile.
    Unsupported(String),
    /// An input, count or byte bound was exceeded.
    Bounds(String),
    /// Hex, name order, name bytes or timestamp normalization is invalid.
    NonCanonical(String),
}

impl fmt::Display for PropertyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (category, detail) = match self {
            Self::Malformed(detail) => ("malformed property declaration", detail),
            Self::Schema(detail) => ("unsupported property schema", detail),
            Self::Unsupported(detail) => ("unsupported property declaration", detail),
            Self::Bounds(detail) => ("property declaration bound exceeded", detail),
            Self::NonCanonical(detail) => ("noncanonical property declaration", detail),
        };
        write!(formatter, "{category}: {detail}")
    }
}

impl Error for PropertyError {}

/// Decode and structurally review one complete bounded JSON declaration.
///
/// The three-MiB input bound is checked before deserialization. Whitespace and
/// object-key order do not affect the fingerprint. This function performs no
/// native observation and does not accept legacy fingerprints as ACL evidence.
pub fn parse(bytes: &[u8]) -> Result<ReviewedProperties, PropertyError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(PropertyError::Bounds("input exceeds three MiB".to_owned()));
    }
    // Serde's derived structs also accept positional sequences. The wire
    // contract permits objects only at record boundaries, so inspect those
    // shapes before decoding the typed declaration. Do not deserialize the
    // typed value from this intermediate Value: it has collapsed duplicate
    // object keys. Reading the original bytes again retains duplicate-field
    // rejection from the closed typed representation.
    let shapes: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| PropertyError::Malformed(error.to_string()))?;
    require_object_shapes(&shapes)?;
    drop(shapes);
    let snapshot = serde_json::from_slice(bytes)
        .map_err(|error| PropertyError::Malformed(error.to_string()))?;
    review(snapshot)
}

fn require_object_shapes(value: &serde_json::Value) -> Result<(), PropertyError> {
    require_object(value, "property declaration")?;
    require_object(&value["mtime"], "mtime")?;
    require_object(&value["birthtime"], "birthtime")?;
    require_object(&value["acl"], "ACL")?;
    let attributes = value["xattrs"].as_array().ok_or_else(|| {
        PropertyError::Malformed("xattrs must be an array of objects".to_owned())
    })?;
    for attribute in attributes {
        require_object(attribute, "extended attribute")?;
    }
    if let Some(entries) = value["acl"].get("entries") {
        let entries = entries.as_array().ok_or_else(|| {
            PropertyError::Malformed("ACL entries must be an array of objects".to_owned())
        })?;
        for entry in entries {
            require_object(entry, "ACL entry")?;
        }
    }
    Ok(())
}

fn require_object(value: &serde_json::Value, name: &str) -> Result<(), PropertyError> {
    if !value.is_object() {
        return Err(PropertyError::Malformed(format!("{name} must be an object")));
    }
    Ok(())
}

/// Review an owned declaration and fingerprint it only after all checks pass.
///
/// Callers constructing this value directly own its prior allocation costs;
/// use [`parse`] for bounded byte input. No ACL entries are sorted or removed,
/// and noncanonical xattr ordering is refused rather than repaired.
pub fn review(
    snapshot: MacApfsCrossCopyPropertiesV1,
) -> Result<ReviewedProperties, PropertyError> {
    if snapshot.schema != SCHEMA {
        return Err(PropertyError::Schema("expected the standalone v1 schema".to_owned()));
    }
    if snapshot.profile != PROFILE {
        return Err(PropertyError::Unsupported("expected macos-apfs-cross-copy/v1".to_owned()));
    }
    if !(0o100000..=0o107777).contains(&snapshot.mode) {
        return Err(PropertyError::Unsupported("mode must describe a regular file".to_owned()));
    }
    if snapshot.bsd_flags != 0 {
        return Err(PropertyError::Unsupported("nonzero BSD flags are outside this profile".to_owned()));
    }
    for (name, timestamp) in [("mtime", &snapshot.mtime), ("birthtime", &snapshot.birthtime)] {
        if timestamp.nanoseconds >= 1_000_000_000 {
            return Err(PropertyError::NonCanonical(format!("{name} nanoseconds must be below one billion")));
        }
    }
    review_xattrs(&snapshot.xattrs)?;
    review_acl(&snapshot.acl)?;
    // Bound the canonical ACL object itself, including the state discriminator
    // and all entry fields. Entry limits already make this a conservative cap.
    if canonical_bytes(&snapshot.acl)?.len() > MAX_ACL_ENCODED_BYTES {
        return Err(PropertyError::Bounds("encoded ACL exceeds 64 KiB".to_owned()));
    }

    // All declaration fields, including SCHEMA and PROFILE, participate.
    // Explicit recursive ordering also works with serde_json preserve_order.
    let canonical = canonical_bytes(&snapshot)?;
    let fingerprint = blake3::hash(&canonical).to_hex().to_string();
    Ok(ReviewedProperties { snapshot, fingerprint })
}

fn review_xattrs(attributes: &[ExtendedAttribute]) -> Result<(), PropertyError> {
    if attributes.len() > MAX_XATTRS {
        return Err(PropertyError::Bounds("more than 128 extended attributes".to_owned()));
    }
    let mut previous: Option<&str> = None;
    let mut total = 0usize;
    for attribute in attributes {
        review_hex(&attribute.name_hex, MAX_XATTR_NAME_BYTES, "xattr name")?;
        if attribute.name_hex.is_empty()
            || attribute.name_hex.as_bytes().chunks_exact(2).any(|pair| pair == b"00")
        {
            return Err(PropertyError::NonCanonical("xattr names must be nonempty and NUL-free".to_owned()));
        }
        // Lowercase hex is order-preserving for byte strings, so no decoding
        // or allocation is needed to compare canonical raw names.
        if previous.is_some_and(|name| name >= attribute.name_hex.as_str()) {
            return Err(PropertyError::NonCanonical("xattr names must be strictly increasing and unique".to_owned()));
        }
        previous = Some(&attribute.name_hex);
        let length = review_hex(&attribute.value_hex, MAX_XATTR_VALUE_BYTES, "xattr value")?;
        total = total.checked_add(length)
            .ok_or_else(|| PropertyError::Bounds("xattr value byte count overflow".to_owned()))?;
        if total > MAX_XATTR_TOTAL_VALUE_BYTES {
            return Err(PropertyError::Bounds("xattr values exceed one MiB in total".to_owned()));
        }
    }
    Ok(())
}

fn review_acl(acl: &AclSnapshot) -> Result<(), PropertyError> {
    let AclSnapshot::Present { flags, entries } = acl else {
        return Ok(());
    };
    if flags & !ACL_FLAGS != 0 {
        return Err(PropertyError::Unsupported("unknown ACL header flags".to_owned()));
    }
    if flags & ACL_DEFER_INHERIT != 0 {
        return Err(PropertyError::Unsupported("deferred ACL inheritance is outside this profile".to_owned()));
    }
    if entries.len() > MAX_ACES {
        return Err(PropertyError::Bounds("more than 128 ACL entries".to_owned()));
    }
    for entry in entries {
        if entry.principal_hex.len() != 32 {
            return Err(PropertyError::NonCanonical("ACL principal must contain exactly 16 raw UUID bytes".to_owned()));
        }
        review_hex(&entry.principal_hex, 16, "ACL principal")?;
        if entry.flags & !ACE_FLAGS != 0 {
            return Err(PropertyError::Unsupported("unknown ACL entry flags".to_owned()));
        }
        if entry.permissions & !ACE_PERMISSIONS != 0 {
            return Err(PropertyError::Unsupported("unknown ACL rights".to_owned()));
        }
    }
    Ok(())
}

fn review_hex(value: &str, maximum_bytes: usize, name: &str) -> Result<usize, PropertyError> {
    if value.len() > maximum_bytes * 2 {
        return Err(PropertyError::Bounds(format!("{name} exceeds its decoded byte bound")));
    }
    if value.len() % 2 != 0
        || !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PropertyError::NonCanonical(format!("{name} must use complete lowercase hex byte pairs")));
    }
    Ok(value.len() / 2)
}

#[derive(Serialize)]
#[serde(untagged)]
enum CanonicalValue<'a> {
    Object(BTreeMap<&'a str, CanonicalValue<'a>>),
    Array(Vec<CanonicalValue<'a>>),
    Scalar(&'a serde_json::Value),
}

impl<'a> From<&'a serde_json::Value> for CanonicalValue<'a> {
    fn from(value: &'a serde_json::Value) -> Self {
        match value {
            serde_json::Value::Object(entries) => Self::Object(
                entries.iter()
                    .map(|(key, value)| (key.as_str(), Self::from(value)))
                    .collect(),
            ),
            serde_json::Value::Array(entries) => {
                Self::Array(entries.iter().map(Self::from).collect())
            }
            _ => Self::Scalar(value),
        }
    }
}

fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, PropertyError> {
    let value = serde_json::to_value(value)
        .map_err(|error| PropertyError::Malformed(error.to_string()))?;
    serde_json::to_vec(&CanonicalValue::from(&value))
        .map_err(|error| PropertyError::Malformed(error.to_string()))
}
