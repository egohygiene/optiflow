//! Product-owned implementation of the federated metadata-only support profile.
//! No environment discovery, subscriber, raw-log reader, provider or network.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX: u64 = 65_536;
const CONTRACTS: &[&str] = &[
    "egohygiene.diagnostics-request/v1",
    "egohygiene.diagnostics-bundle/v1",
    "egohygiene.observability-health/v1",
];
const EXCLUSIONS: &[&str] = &[
    "media_documents",
    "prompts_lyrics_captions_transcripts",
    "environment_credentials_headers",
    "argv_child_captures",
    "private_paths_hostnames",
    "raw_logs_traces_metrics_backtraces",
    "durable_run_artifacts",
];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Unsupported,
    Cancelled,
    Changed,
    Exists,
    Io,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self{Self::Invalid=>"invalid or oversized support metadata",Self::Unsupported=>"support capability or digest algorithm unavailable",Self::Cancelled=>"support operation cancelled or its cooperative deadline expired; preserve any created file",Self::Changed=>"support metadata changed during observation",Self::Exists=>"support destination already exists; preserve it",Self::Io=>"support I/O failed; preserve partial output"})
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Product {
    Flow,
    Aniflow,
    Optiflow,
    Renderflow,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Algorithm {
    #[serde(rename = "sha256")]
    Sha256,
    #[serde(rename = "blake3-256")]
    Blake3,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Digest {
    pub algorithm: Algorithm,
    pub value: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Producer {
    pub tool: Product,
    pub version: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Build,
    Platform,
    Contracts,
    Health,
}
impl Category {
    fn name(self) -> &'static str {
        match self {
            Self::Build => "build.json",
            Self::Platform => "platform.json",
            Self::Contracts => "contracts.json",
            Self::Health => "health.json",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema_version: String,
    pub categories: Vec<Category>,
}
impl Request {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 4096 {
            return Err(Error::Invalid);
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<()> {
        if self.schema_version != "egohygiene.diagnostics-request/v1"
            || self.categories.is_empty()
            || self.categories.len() > 4
            || self.categories.iter().collect::<BTreeSet<_>>().len() != self.categories.len()
        {
            Err(Error::Invalid)
        } else {
            Ok(())
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Health {
    pub schema_version: String,
    pub producer: Producer,
    pub logging_compiled: bool,
    pub logging_runtime_observed: bool,
    pub subscriber_initialized_by_diagnostics: bool,
    pub exporters_compiled: bool,
    pub collector_state: String,
    pub queue_records: Option<u64>,
    pub dropped_records: Option<u64>,
    pub flush_state: String,
    pub file_identity_supported: bool,
    pub network_contacted: bool,
    pub domain_evidence: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "category", rename_all = "snake_case", deny_unknown_fields)]
enum Payload {
    Build {
        producer: Producer,
    },
    Platform {
        compiled_os: String,
        compiled_arch: String,
    },
    Contracts {
        identifiers: Vec<String>,
    },
    Health {
        observation: Health,
    },
}
impl Payload {
    fn category(&self) -> Category {
        match self {
            Self::Build { .. } => Category::Build,
            Self::Platform { .. } => Category::Platform,
            Self::Contracts { .. } => Category::Contracts,
            Self::Health { .. } => Category::Health,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    name: String,
    digest: Digest,
    byte_count: u64,
    payload: Payload,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Archive {
    schema_version: String,
    producer: Producer,
    created_unix_ms: u64,
    redaction_policy: String,
    exclusions: Vec<String>,
    domain_evidence: bool,
    members: Vec<Member>,
}
#[derive(Debug, Serialize)]
pub struct Preview {
    pub schema_version: String,
    pub producer: Producer,
    pub categories: Vec<Category>,
    pub member_names: Vec<String>,
    pub member_count: usize,
    pub estimated_archive_bytes: u64,
    pub maximum_archive_bytes: u64,
    pub exclusions: Vec<&'static str>,
    pub redaction_policy: &'static str,
    pub sensitivity_warnings: Vec<&'static str>,
    pub maximum_member_bytes: u64,
    pub maximum_members: usize,
    pub file_creation_available: bool,
    pub files_written: bool,
    pub network_contacted: bool,
    pub domain_evidence: bool,
}
#[derive(Debug, Serialize)]
pub struct Inspection {
    pub schema_version: String,
    pub producer: Producer,
    pub digest: Digest,
    pub archive_bytes: u64,
    pub categories: Vec<Category>,
    pub member_count: usize,
    pub claims_authenticated: bool,
    pub network_contacted: bool,
    pub domain_evidence: bool,
}
pub struct Owner {
    product: Product,
    version: &'static str,
    algorithm: Algorithm,
    hash: fn(&[u8]) -> String,
    reader: fn(&Path) -> std::io::Result<File>,
    logging_compiled: bool,
}
struct Guard<'a> {
    started: Instant,
    cancel: &'a dyn Fn() -> bool,
}
impl Guard<'_> {
    fn check(&self) -> Result<()> {
        if (self.cancel)() || self.started.elapsed() >= Duration::from_secs(5) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}
fn encoded<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| Error::Invalid)
}
fn atom(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
}
fn pin(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn producer_valid(value: &Producer) -> bool {
    atom(&value.version, 64)
        && value.version.split('.').take(3).count() == 3
        && value.version.as_bytes()[0].is_ascii_digit()
}
fn normalized(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
}
fn same(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    if !before.is_file()
        || !after.is_file()
        || before.len() != after.len()
        || before.modified().ok().is_none()
        || before.modified().ok() != after.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        before.dev() == after.dev()
            && before.ino() == after.ino()
            && before.nlink() == 1
            && after.nlink() == 1
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec()
            && before.mode() == after.mode()
            && before.uid() == after.uid()
            && before.gid() == after.gid()
    }
    #[cfg(not(unix))]
    {
        false
    }
}
impl Owner {
    fn producer(&self) -> Producer {
        Producer {
            tool: self.product,
            version: self.version.into(),
        }
    }
    fn digest(&self, data: &[u8]) -> Result<Digest> {
        let value = (self.hash)(data);
        if !pin(&value) {
            return Err(Error::Invalid);
        }
        Ok(Digest {
            algorithm: self.algorithm,
            value,
        })
    }
    /// Compiled capability facts only; unsupported/unknown are never zero-valued
    /// runtime measurements. Libraries do not discover telemetry configuration.
    pub fn health(&self) -> Health {
        Health {
            schema_version: "egohygiene.observability-health/v1".into(),
            producer: self.producer(),
            logging_compiled: self.logging_compiled,
            logging_runtime_observed: false,
            subscriber_initialized_by_diagnostics: false,
            exporters_compiled: false,
            collector_state: "not_applicable_no_exporter".into(),
            queue_records: None,
            dropped_records: None,
            flush_state: "not_applicable_no_exporter".into(),
            file_identity_supported: cfg!(unix),
            network_contacted: false,
            domain_evidence: false,
        }
    }
    fn archive(&self, request: &Request) -> Result<Archive> {
        request.validate()?;
        let producer = self.producer();
        if !producer_valid(&producer) {
            return Err(Error::Invalid);
        }
        let mut members = vec![];
        for category in &request.categories {
            let payload = match category {
                Category::Build => Payload::Build {
                    producer: producer.clone(),
                },
                Category::Platform => Payload::Platform {
                    compiled_os: std::env::consts::OS.into(),
                    compiled_arch: std::env::consts::ARCH.into(),
                },
                Category::Contracts => Payload::Contracts {
                    identifiers: CONTRACTS.iter().map(|s| (*s).into()).collect(),
                },
                Category::Health => Payload::Health {
                    observation: self.health(),
                },
            };
            let data = encoded(&payload)?;
            members.push(Member {
                name: category.name().into(),
                digest: self.digest(&data)?,
                byte_count: data.len() as u64,
                payload,
            });
        }
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Invalid)?
            .as_millis();
        let value = Archive {
            schema_version: "egohygiene.diagnostics-bundle/v1".into(),
            producer,
            created_unix_ms: u64::try_from(created).map_err(|_| Error::Invalid)?,
            redaction_policy: "generated_allowlist.v1".into(),
            exclusions: EXCLUSIONS.iter().map(|s| (*s).into()).collect(),
            domain_evidence: false,
            members,
        };
        self.validate(&value)?;
        Ok(value)
    }
    fn validate(&self, value: &Archive) -> Result<()> {
        if value.schema_version != "egohygiene.diagnostics-bundle/v1"
            || !producer_valid(&value.producer)
            || value.redaction_policy != "generated_allowlist.v1"
            || !value
                .exclusions
                .iter()
                .map(String::as_str)
                .eq(EXCLUSIONS.iter().copied())
            || value.domain_evidence
            || value.members.is_empty()
            || value.members.len() > 4
        {
            return Err(Error::Invalid);
        }
        let mut seen = BTreeSet::new();
        for member in &value.members {
            if member.digest.algorithm != self.algorithm {
                return Err(Error::Unsupported);
            }
            let valid = match &member.payload {
                Payload::Build { producer } => producer == &value.producer,
                Payload::Platform {
                    compiled_os,
                    compiled_arch,
                } => atom(compiled_os, 64) && atom(compiled_arch, 64),
                Payload::Contracts { identifiers } => identifiers
                    .iter()
                    .map(String::as_str)
                    .eq(CONTRACTS.iter().copied()),
                Payload::Health { observation: h } => {
                    h.schema_version == "egohygiene.observability-health/v1"
                        && h.producer == value.producer
                        && !h.logging_runtime_observed
                        && !h.subscriber_initialized_by_diagnostics
                        && !h.exporters_compiled
                        && h.collector_state == "not_applicable_no_exporter"
                        && h.queue_records.is_none()
                        && h.dropped_records.is_none()
                        && h.flush_state == "not_applicable_no_exporter"
                        && !h.network_contacted
                        && !h.domain_evidence
                }
            };
            let data = encoded(&member.payload)?;
            if !valid
                || !seen.insert(member.payload.category())
                || member.name != member.payload.category().name()
                || data.len() > 4096
                || member.byte_count != data.len() as u64
                || member.digest != self.digest(&data)?
            {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    }
    fn document(&self, value: &Archive) -> Result<Vec<u8>> {
        let mut data = serde_json::to_vec_pretty(value).map_err(|_| Error::Invalid)?;
        data.push(b'\n');
        if data.len() as u64 > MAX {
            return Err(Error::Invalid);
        }
        Ok(data)
    }
    pub fn preview(&self, request: &Request, cancel: &dyn Fn() -> bool) -> Result<Preview> {
        let guard = Guard {
            started: Instant::now(),
            cancel,
        };
        guard.check()?;
        let value = self.archive(request)?;
        let size = self.document(&value)?.len() as u64;
        guard.check()?;
        Ok(Preview {
            schema_version: "egohygiene.diagnostics-preview/v1".into(),
            producer: self.producer(),
            categories: request.categories.clone(),
            member_names: request.categories.iter().map(|c| c.name().into()).collect(),
            member_count: value.members.len(),
            estimated_archive_bytes: size,
            maximum_archive_bytes: MAX,
            exclusions: EXCLUSIONS.to_vec(),
            redaction_policy: "generated_allowlist.v1",
            sensitivity_warnings: vec![
                "Build/platform metadata can fingerprint an installation; review before sharing",
                "Unsigned support metadata is not domain or recovery evidence",
            ],
            maximum_member_bytes: 4096,
            maximum_members: 4,
            file_creation_available: cfg!(unix),
            files_written: false,
            network_contacted: false,
            domain_evidence: false,
        })
    }
    fn read(&self, path: &Path, maximum: u64, guard: &Guard<'_>) -> Result<Vec<u8>> {
        guard.check()?;
        if !cfg!(unix) {
            return Err(Error::Unsupported);
        }
        if !normalized(path) || path.canonicalize().ok().as_deref() != Some(path) {
            return Err(Error::Invalid);
        }
        let before = fs::symlink_metadata(path).map_err(|_| Error::Io)?;
        if before.len() == 0 || before.len() > maximum || !same(&before, &before) {
            return Err(Error::Invalid);
        }
        let mut file = (self.reader)(path).map_err(|_| Error::Io)?;
        if !same(&before, &file.metadata().map_err(|_| Error::Io)?) {
            return Err(Error::Changed);
        }
        let mut data = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            guard.check()?;
            let count = file.read(&mut chunk).map_err(|_| Error::Io)?;
            if count == 0 {
                break;
            }
            if data.len() + count > maximum as usize {
                return Err(Error::Invalid);
            }
            data.extend_from_slice(&chunk[..count]);
        }
        if data.len() as u64 != before.len()
            || !same(&before, &file.metadata().map_err(|_| Error::Io)?)
            || !same(
                &before,
                &fs::symlink_metadata(path).map_err(|_| Error::Changed)?,
            )
        {
            return Err(Error::Changed);
        }
        guard.check()?;
        Ok(data)
    }
    pub fn read_request(&self, path: &Path, cancel: &dyn Fn() -> bool) -> Result<Request> {
        let guard = Guard {
            started: Instant::now(),
            cancel,
        };
        Request::from_json_slice(&self.read(path, 4096, &guard)?)
    }
    fn inspect(&self, path: &Path, guard: &Guard<'_>) -> Result<Inspection> {
        let data = self.read(path, MAX, guard)?;
        let value: Archive = serde_json::from_slice(&data).map_err(|_| Error::Invalid)?;
        self.validate(&value)?;
        guard.check()?;
        Ok(Inspection {
            schema_version: "egohygiene.diagnostics-inspection/v1".into(),
            producer: value.producer,
            digest: self.digest(&data)?,
            archive_bytes: data.len() as u64,
            categories: value.members.iter().map(|m| m.payload.category()).collect(),
            member_count: value.members.len(),
            claims_authenticated: false,
            network_contacted: false,
            domain_evidence: false,
        })
    }
    pub fn inspect_bundle(&self, path: &Path, cancel: &dyn Fn() -> bool) -> Result<Inspection> {
        self.inspect(
            path,
            &Guard {
                started: Instant::now(),
                cancel,
            },
        )
    }
    pub fn create_bundle(
        &self,
        request: &Request,
        output: &Path,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Inspection> {
        let guard = Guard {
            started: Instant::now(),
            cancel,
        };
        guard.check()?;
        if !cfg!(unix) {
            return Err(Error::Unsupported);
        }
        let parent = output.parent().ok_or(Error::Invalid)?;
        if !normalized(output)
            || parent.canonicalize().ok().as_deref() != Some(parent)
            || !fs::metadata(parent).map_err(|_| Error::Io)?.is_dir()
        {
            return Err(Error::Invalid);
        }
        let value = self.archive(request)?;
        let data = self.document(&value)?;
        guard.check()?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(output).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Error::Exists
            } else {
                Error::Io
            }
        })?;
        guard.check()?;
        file.write_all(&data)
            .and_then(|_| file.sync_all())
            .map_err(|_| Error::Io)?;
        guard.check()?;
        let inspection = self.inspect(output, &guard)?;
        if inspection.digest != self.digest(&data)?
            || !same(
                &file.metadata().map_err(|_| Error::Io)?,
                &fs::symlink_metadata(output).map_err(|_| Error::Changed)?,
            )
        {
            return Err(Error::Changed);
        }
        Ok(inspection)
    }
}

fn product_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
fn product_reader(path: &Path) -> std::io::Result<File> {
    #[cfg(unix)]
    {
        rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .map_err(std::io::Error::from)
    }
    #[cfg(not(unix))]
    {
        OpenOptions::new().read(true).open(path)
    }
}
#[must_use]
pub fn owner() -> Owner {
    Owner {
        product: Product::Optiflow,
        version: env!("CARGO_PKG_VERSION"),
        algorithm: Algorithm::Blake3,
        hash: product_hash,
        reader: product_reader,
        logging_compiled: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Request {
        Request {
            schema_version: "egohygiene.diagnostics-request/v1".into(),
            categories: vec![
                Category::Build,
                Category::Platform,
                Category::Contracts,
                Category::Health,
            ],
        }
    }
    #[test]
    fn malformed_or_unbounded_requests_are_refused() {
        assert!(Request::from_json_slice(&[0xff, 0xfe]).is_err());
        assert!(Request::from_json_slice(&vec![b' '; 4097]).is_err());
        let mut duplicate = request();
        duplicate.categories = vec![Category::Build, Category::Build];
        assert!(duplicate.validate().is_err());
        let mut future = request();
        future.schema_version = "egohygiene.diagnostics-request/v2".into();
        assert!(future.validate().is_err());
    }
    #[test]
    fn input_categories_cannot_select_arbitrary_logs_or_content() {
        let mut raw = serde_json::to_value(request()).unwrap();
        raw["logs"] = serde_json::json!("PRIVATE_DIAGNOSTIC_CANARY");
        assert!(Request::from_json_slice(&serde_json::to_vec(&raw).unwrap()).is_err());
        let owner = owner();
        let document = owner.document(&owner.archive(&request()).unwrap()).unwrap();
        assert!(
            !String::from_utf8(document)
                .unwrap()
                .contains("PRIVATE_DIAGNOSTIC_CANARY")
        );
        let preview = owner.preview(&request(), &|| false).unwrap();
        assert_eq!(preview.member_count, 4);
        assert!(!preview.files_written && !preview.network_contacted && !preview.domain_evidence);
    }
    #[test]
    fn unknown_or_tampered_members_cannot_be_inspected_as_valid() {
        let owner = owner();
        let value = owner.archive(&request()).unwrap();
        let mut forged = value.clone();
        forged.members[0].name = "../private".into();
        assert!(owner.validate(&forged).is_err());
        let mut forged = value.clone();
        forged.members[1] = forged.members[0].clone();
        assert!(owner.validate(&forged).is_err());
        let mut forged = value.clone();
        forged.members[0].digest.value = "0".repeat(64);
        assert!(owner.validate(&forged).is_err());
        let mut forged = value;
        forged.domain_evidence = true;
        assert!(owner.validate(&forged).is_err());
    }
    #[test]
    fn unsupported_exporter_health_is_not_zero_or_verified() {
        let health = owner().health();
        assert!(health.queue_records.is_none() && health.dropped_records.is_none());
        assert!(
            !health.exporters_compiled
                && !health.logging_runtime_observed
                && !health.subscriber_initialized_by_diagnostics
                && !health.domain_evidence
                && !health.network_contacted
        );
    }
    #[test]
    fn cancellation_prevents_support_creation() {
        assert!(matches!(
            owner().preview(&request(), &|| true),
            Err(Error::Cancelled)
        ));
    }
    #[cfg(unix)]
    #[test]
    fn fresh_bundle_roundtrip_preserves_existing_output_and_refuses_aliases() {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "support-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let path = root.join("support.json");
        let owner = owner();
        let first = owner.create_bundle(&request(), &path, &|| false).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(matches!(
            owner.create_bundle(&request(), &path, &|| false),
            Err(Error::Exists)
        ));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(
            owner.inspect_bundle(&path, &|| false).unwrap().digest,
            first.digest
        );
        let alias = root.join("alias.json");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        assert!(owner.inspect_bundle(&alias, &|| false).is_err());
        fs::remove_file(&alias).unwrap();
        fs::hard_link(&path, &alias).unwrap();
        assert!(owner.inspect_bundle(&path, &|| false).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
