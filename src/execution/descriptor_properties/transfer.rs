//! Strong-property copy receipts have an independent recovery identity.
//! They cannot be interpreted as historical mutation/finalization v1–v4.
use super::*;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    QuarantineCopy,
    RestoreCopy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferReceipt {
    pub schema: String,
    pub transfer_id: Uuid,
    pub direction: Direction,
    pub source: Evidence,
    pub destination: Evidence,
    pub destination_data_synchronized: bool,
    pub source_retained_at_observation: bool,
    pub namespace_commit_observed: bool,
    pub native_qualified: bool,
    pub source_removal_authority: bool,
    pub receipt_fingerprint: String,
}
impl TransferReceipt {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= 8_388_608, "transfer receipt exceeds 8 MiB");
        let raw: serde_json::Value = serde_json::from_slice(bytes)?;
        ensure!(raw.is_object(), "transfer receipt must be an object");
        for name in ["source", "destination"] {
            Evidence::from_json_slice(&serde_json::to_vec(&raw[name])?)?;
        }
        let record: Self = serde_json::from_slice(bytes)?;
        record.validate()?;
        Ok(record)
    }
    pub fn validate(&self) -> Result<()> {
        self.source.validate()?;
        self.destination.validate()?;
        let mut copy = self.clone();
        copy.receipt_fingerprint.clear();
        ensure!(
            self.schema == "optiflow.strong-property-copy-receipt.v1"
                && !self.transfer_id.is_nil()
                && self.receipt_fingerprint == fingerprint(&copy)?
                && self.destination_data_synchronized
                && self.source_retained_at_observation
                && !self.namespace_commit_observed
                && !self.native_qualified
                && !self.source_removal_authority,
            "copy receipt identity or scope differs"
        );
        equivalent(&self.source, &self.destination)
    }
}
fn equivalent(source: &Evidence, destination: &Evidence) -> Result<()> {
    ensure!(
        source.backend == destination.backend
            && source.property_fingerprint == destination.property_fingerprint
            && source.properties == destination.properties
            && source.subject.content_blake3 == destination.subject.content_blake3
            && source.subject.byte_count == destination.subject.byte_count
            && (source.subject.device_id, source.subject.inode)
                != (destination.subject.device_id, destination.subject.inode),
        "copy receipt needs distinct exact-content subjects and identical strong properties"
    );
    Ok(())
}

/// Complete the property/data boundary of an ALREADY COPIED private file.
/// Caller still owns namespace durability, journal commitment and original
/// retention. Synchronization uses the existing platform-specific primitive.
/// A failed setter/sync/readback leaves an inspectable destination; no retry,
/// source removal, publication or fallback to weaker evidence occurs here.
pub fn finish_copy(
    source: &File,
    destination: &File,
    backend: &impl DescriptorPropertyBackend,
    expected: &Observation,
    direction: Direction,
    maximum: u64,
    cancel: &impl Fn() -> bool,
) -> Result<TransferReceipt> {
    let copied = preserve_on_copy(source, destination, backend, expected, maximum, cancel)?;
    check(cancel)?;
    super::super::filesystem::sync_file(destination)
        .map_err(|e| anyhow::anyhow!("destination synchronization refused: {e:?}"))?;
    recheck(source, backend, expected, maximum, cancel)?;
    recheck(destination, backend, &copied, maximum, cancel)?;
    let mut record = TransferReceipt {
        schema: "optiflow.strong-property-copy-receipt.v1".into(),
        transfer_id: Uuid::now_v7(),
        direction,
        source: expected.evidence()?,
        destination: copied.evidence()?,
        destination_data_synchronized: true,
        source_retained_at_observation: true,
        namespace_commit_observed: false,
        native_qualified: false,
        source_removal_authority: false,
        receipt_fingerprint: String::new(),
    };
    record.receipt_fingerprint = fingerprint(&record)?;
    record.validate()?;
    Ok(record)
}

/// Read-only recovery comparison. A saved receipt is never a live token or a
/// permission to unlink, finalize, retry an uncertain copy, or upgrade an old run.
pub fn inspect_retained_pair(
    source: &File,
    destination: &File,
    backend: &impl DescriptorPropertyBackend,
    record: &TransferReceipt,
    maximum: u64,
    cancel: &impl Fn() -> bool,
) -> Result<()> {
    record.validate()?;
    let current_source = observe(source, backend, maximum, cancel)?.evidence()?;
    let current_destination = observe(destination, backend, maximum, cancel)?.evidence()?;
    ensure!(
        current_source == record.source && current_destination == record.destination,
        "retained copy pair changed; no recovery action is authorized"
    );
    equivalent(&current_source, &current_destination)
}
