use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::model::{
    CapabilityDeclaration, CapabilityKind, DeclaredCoverage, Digest, EXTENSION_INVOCATION_SCHEMA,
    EXTENSION_LOCK_SCHEMA, EXTENSION_MANIFEST_SCHEMA, EXTENSION_RESULT_SCHEMA,
    EXTENSION_SDK_CONTRACT, ExecutionModeDeclaration, ExtensionEffect, ExtensionLockV1,
    ExtensionManifestV1, ExtensionResultV1, MAX_EXTENSION_STDERR_BYTES, MAX_EXTENSION_STDIN_BYTES,
    MAX_EXTENSION_STDOUT_BYTES, MAX_EXTENSION_TIMEOUT_MS, TrustMode, fingerprint,
};
use crate::contracts::{self, Contract};
use crate::domain::NativePath;

const MAX_DECLARATION_BYTES: u64 = 4 * 1024 * 1024;
const STABLE_READ_ATTEMPTS: usize = 2;

#[derive(Debug)]
pub enum CatalogError {
    InvalidInput {
        path: PathBuf,
        message: String,
    },
    DuplicateManifest {
        extension_id: String,
        version: String,
    },
    DuplicateLock {
        extension_id: String,
        version: String,
    },
    MissingLock {
        extension_id: String,
        version: String,
    },
    OrphanLock {
        extension_id: String,
        version: String,
    },
    ExtensionNotFound {
        extension_id: String,
    },
}

impl CatalogError {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::InvalidInput { path, .. } => Some(path),
            _ => None,
        }
    }
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { path, message } => {
                write!(formatter, "{}: {message}", path.display())
            }
            Self::DuplicateManifest {
                extension_id,
                version,
            } => write!(
                formatter,
                "duplicate manifest for extension {extension_id} version {version}"
            ),
            Self::DuplicateLock {
                extension_id,
                version,
            } => write!(
                formatter,
                "duplicate lock for extension {extension_id} version {version}"
            ),
            Self::MissingLock {
                extension_id,
                version,
            } => write!(
                formatter,
                "no explicit lock was supplied for extension {extension_id} version {version}"
            ),
            Self::OrphanLock {
                extension_id,
                version,
            } => write!(
                formatter,
                "lock for extension {extension_id} version {version} has no matching manifest"
            ),
            Self::ExtensionNotFound { extension_id } => {
                write!(formatter, "extension {extension_id} was not found")
            }
        }
    }
}

impl Error for CatalogError {}

#[derive(Debug, Clone)]
pub struct LoadedExtension {
    pub(crate) manifest_path: PathBuf,
    pub(crate) lock_path: PathBuf,
    pub(crate) manifest_digest: Digest,
    pub(crate) lock_digest: Digest,
    pub(crate) manifest: ExtensionManifestV1,
    pub(crate) lock: ExtensionLockV1,
    pub(crate) availability_reasons: Vec<String>,
}

impl LoadedExtension {
    pub fn manifest(&self) -> &ExtensionManifestV1 {
        &self.manifest
    }

    pub fn lock(&self) -> &ExtensionLockV1 {
        &self.lock
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    pub fn manifest_digest(&self) -> &Digest {
        &self.manifest_digest
    }

    pub fn lock_digest(&self) -> &Digest {
        &self.lock_digest
    }

    pub fn availability_reasons(&self) -> &[String] {
        &self.availability_reasons
    }

    pub fn capability(&self, capability_id: &str) -> Option<&CapabilityDeclaration> {
        self.manifest
            .capabilities
            .iter()
            .find(|capability| capability.capability_id == capability_id)
    }

    pub fn is_available_for(&self, capability: &CapabilityDeclaration) -> bool {
        self.availability_reasons.is_empty()
            && capability.effects.is_subset(&self.lock.granted_effects)
            && self
                .lock
                .configurations
                .contains_key(&capability.configuration_schema)
    }

    pub fn selected_execution_mode(&self) -> Option<&ExecutionModeDeclaration> {
        self.manifest
            .execution_modes
            .iter()
            .find(|mode| mode.name() == self.lock.selected_execution_mode)
    }
}

#[derive(Debug, Clone)]
pub struct ExtensionCatalog {
    extensions: Vec<LoadedExtension>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionListEntry {
    pub extension_id: String,
    pub version: String,
    pub publisher_id: String,
    pub manifest_digest: Digest,
    pub lock_digest: Digest,
    pub enabled: bool,
    pub trust: TrustMode,
    pub precedence: i32,
    pub execution_mode: String,
    pub capability_ids: Vec<String>,
    pub available: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionInspection {
    pub extension: ExtensionListEntry,
    pub manifest: ExtensionManifestV1,
    pub lock: ExtensionLockV1,
    pub manifest_path: NativePath,
    pub lock_path: NativePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionResolutionStatus {
    Selected,
    Unavailable,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionCandidate {
    pub extension_id: String,
    pub version: String,
    pub precedence: i32,
    pub eligible: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionResolution {
    pub capability_id: String,
    pub status: ExtensionResolutionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_extension_id: Option<String>,
    pub candidates: Vec<ResolutionCandidate>,
    pub fallback_extension_ids: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionDoctorReport {
    pub sdk_contract: String,
    pub extension_count: usize,
    pub available_extension_count: usize,
    pub capability_count: usize,
    pub complete: bool,
    pub extensions: Vec<ExtensionListEntry>,
    pub resolutions: Vec<ExtensionResolution>,
}

impl ExtensionCatalog {
    pub fn load(manifest_paths: &[PathBuf], lock_paths: &[PathBuf]) -> Result<Self, CatalogError> {
        let mut manifests = BTreeMap::new();
        for path in manifest_paths {
            let bytes = stable_read(path)?;
            let manifest: ExtensionManifestV1 =
                decode_contract(path, &bytes, Contract::ExtensionManifest)?;
            validate_manifest(path, &manifest)?;
            let key = (manifest.extension_id.clone(), manifest.version.clone());
            if manifests
                .insert(
                    key.clone(),
                    (path.clone(), Digest::blake3(&bytes), manifest),
                )
                .is_some()
            {
                return Err(CatalogError::DuplicateManifest {
                    extension_id: key.0,
                    version: key.1,
                });
            }
        }

        let mut locks = BTreeMap::new();
        for path in lock_paths {
            let bytes = stable_read(path)?;
            let lock: ExtensionLockV1 = decode_contract(path, &bytes, Contract::ExtensionLock)?;
            let key = (lock.extension_id.clone(), lock.extension_version.clone());
            if locks
                .insert(key.clone(), (path.clone(), Digest::blake3(&bytes), lock))
                .is_some()
            {
                return Err(CatalogError::DuplicateLock {
                    extension_id: key.0,
                    version: key.1,
                });
            }
        }

        let mut extension_ids = BTreeSet::new();
        let mut extensions = Vec::with_capacity(manifests.len());
        for (key, (manifest_path, manifest_digest, manifest)) in manifests {
            if !extension_ids.insert(manifest.extension_id.clone()) {
                return Err(CatalogError::InvalidInput {
                    path: manifest_path,
                    message: "only one explicitly selected version of an extension may be loaded"
                        .to_owned(),
                });
            }
            let Some((lock_path, lock_digest, lock)) = locks.remove(&key) else {
                return Err(CatalogError::MissingLock {
                    extension_id: key.0,
                    version: key.1,
                });
            };
            let availability_reasons = availability_reasons(&manifest, &manifest_digest, &lock);
            extensions.push(LoadedExtension {
                manifest_path,
                lock_path,
                manifest_digest,
                lock_digest,
                manifest,
                lock,
                availability_reasons,
            });
        }
        if let Some(((extension_id, version), _)) = locks.into_iter().next() {
            return Err(CatalogError::OrphanLock {
                extension_id,
                version,
            });
        }
        extensions.sort_by(|left, right| {
            left.manifest
                .extension_id
                .cmp(&right.manifest.extension_id)
                .then_with(|| left.manifest.version.cmp(&right.manifest.version))
        });
        Ok(Self { extensions })
    }

    pub fn extensions(&self) -> &[LoadedExtension] {
        &self.extensions
    }

    pub fn list(&self) -> Vec<ExtensionListEntry> {
        self.extensions.iter().map(list_entry).collect()
    }

    pub fn inspect(&self, extension_id: &str) -> Result<ExtensionInspection, CatalogError> {
        let extension = self
            .extensions
            .iter()
            .find(|extension| extension.manifest.extension_id == extension_id)
            .ok_or_else(|| CatalogError::ExtensionNotFound {
                extension_id: extension_id.to_owned(),
            })?;
        Ok(ExtensionInspection {
            extension: list_entry(extension),
            manifest: extension.manifest.clone(),
            lock: extension.lock.clone(),
            manifest_path: NativePath::from_path(&extension.manifest_path),
            lock_path: NativePath::from_path(&extension.lock_path),
        })
    }

    pub fn resolve(&self, capability_id: &str) -> ExtensionResolution {
        let mut candidates = Vec::new();
        for extension in &self.extensions {
            let Some(capability) = extension.capability(capability_id) else {
                continue;
            };
            let mut reasons = extension.availability_reasons.clone();
            if !capability
                .effects
                .is_subset(&extension.lock.granted_effects)
            {
                reasons.push(
                    "the lock does not grant every effect required by the capability".to_owned(),
                );
            }
            if !extension
                .lock
                .configurations
                .contains_key(&capability.configuration_schema)
            {
                reasons.push(format!(
                    "locked configuration {} is missing",
                    capability.configuration_schema
                ));
            }
            candidates.push(ResolutionCandidate {
                extension_id: extension.manifest.extension_id.clone(),
                version: extension.manifest.version.clone(),
                precedence: extension.lock.precedence,
                eligible: reasons.is_empty(),
                reasons,
            });
        }
        candidates.sort_by(|left, right| {
            right
                .precedence
                .cmp(&left.precedence)
                .then_with(|| left.extension_id.cmp(&right.extension_id))
                .then_with(|| left.version.cmp(&right.version))
        });
        let Some(highest_precedence) = candidates.first().map(|candidate| candidate.precedence)
        else {
            return ExtensionResolution {
                capability_id: capability_id.to_owned(),
                status: ExtensionResolutionStatus::Unavailable,
                selected_extension_id: None,
                candidates,
                fallback_extension_ids: Vec::new(),
                reasons: vec!["no provider declares the requested capability".to_owned()],
            };
        };
        let eligible_at_highest: Vec<_> = candidates
            .iter()
            .filter(|candidate| candidate.precedence == highest_precedence && candidate.eligible)
            .collect();
        if eligible_at_highest.len() > 1 {
            return ExtensionResolution {
                capability_id: capability_id.to_owned(),
                status: ExtensionResolutionStatus::Conflict,
                selected_extension_id: None,
                candidates,
                fallback_extension_ids: Vec::new(),
                reasons: vec![
                    "multiple eligible providers have the same highest precedence".to_owned(),
                ],
            };
        }

        if let Some(selected) = eligible_at_highest.first() {
            let selected_extension_id = selected.extension_id.clone();
            let fallback_extension_ids = candidates
                .iter()
                .filter(|candidate| {
                    candidate.eligible
                        && candidate.precedence < highest_precedence
                        && self.is_authorized_fallback(candidate, &selected_extension_id)
                })
                .map(|candidate| candidate.extension_id.clone())
                .collect();
            return ExtensionResolution {
                capability_id: capability_id.to_owned(),
                status: ExtensionResolutionStatus::Selected,
                selected_extension_id: Some(selected_extension_id),
                candidates,
                fallback_extension_ids,
                reasons: vec![
                    "selected the unique eligible provider at the highest precedence".to_owned(),
                ],
            };
        }

        let highest_tier_count = candidates
            .iter()
            .take_while(|candidate| candidate.precedence == highest_precedence)
            .count();
        if highest_tier_count != 1 {
            return ExtensionResolution {
                capability_id: capability_id.to_owned(),
                status: ExtensionResolutionStatus::Unavailable,
                selected_extension_id: None,
                candidates,
                fallback_extension_ids: Vec::new(),
                reasons: vec![
                    "the highest-precedence tier is unavailable and does not identify one preferred provider for fallback"
                        .to_owned(),
                ],
            };
        }

        let preferred_extension_id = candidates[0].extension_id.clone();
        let authorized_fallbacks: Vec<_> = candidates
            .iter()
            .skip(1)
            .filter(|candidate| {
                candidate.eligible
                    && self.is_authorized_fallback(candidate, &preferred_extension_id)
            })
            .collect();
        let Some(selected_fallback) = authorized_fallbacks.first() else {
            return ExtensionResolution {
                capability_id: capability_id.to_owned(),
                status: ExtensionResolutionStatus::Unavailable,
                selected_extension_id: None,
                candidates,
                fallback_extension_ids: Vec::new(),
                reasons: vec![
                    "the preferred provider is unavailable and no explicitly authorized fallback is available"
                        .to_owned(),
                ],
            };
        };
        if authorized_fallbacks
            .get(1)
            .is_some_and(|candidate| candidate.precedence == selected_fallback.precedence)
        {
            return ExtensionResolution {
                capability_id: capability_id.to_owned(),
                status: ExtensionResolutionStatus::Conflict,
                selected_extension_id: None,
                candidates,
                fallback_extension_ids: Vec::new(),
                reasons: vec![
                    "multiple authorized fallbacks have the same highest precedence".to_owned(),
                ],
            };
        }
        let selected_extension_id = selected_fallback.extension_id.clone();
        let fallback_extension_ids = authorized_fallbacks
            .iter()
            .map(|candidate| candidate.extension_id.clone())
            .collect();
        ExtensionResolution {
            capability_id: capability_id.to_owned(),
            status: ExtensionResolutionStatus::Selected,
            selected_extension_id: Some(selected_extension_id),
            candidates,
            fallback_extension_ids,
            reasons: vec![format!(
                "preferred provider {preferred_extension_id} is unavailable; selected the explicitly authorized fallback"
            )],
        }
    }

    fn is_authorized_fallback(
        &self,
        candidate: &ResolutionCandidate,
        preferred_extension_id: &str,
    ) -> bool {
        self.extensions
            .iter()
            .find(|extension| {
                extension.manifest.extension_id == candidate.extension_id
                    && extension.manifest.version == candidate.version
            })
            .is_some_and(|extension| {
                extension
                    .manifest
                    .replacement
                    .fallbacks_for
                    .iter()
                    .any(|extension_id| extension_id == preferred_extension_id)
                    && extension
                        .lock
                        .allowed_fallbacks
                        .iter()
                        .any(|extension_id| extension_id == preferred_extension_id)
            })
    }

    pub fn selected(&self, capability_id: &str) -> Option<&LoadedExtension> {
        let resolution = self.resolve(capability_id);
        let selected = resolution.selected_extension_id?;
        self.extensions
            .iter()
            .find(|extension| extension.manifest.extension_id == selected)
    }

    pub fn doctor(&self) -> ExtensionDoctorReport {
        let extension_entries = self.list();
        let capability_ids: BTreeSet<String> = self
            .extensions
            .iter()
            .flat_map(|extension| {
                extension
                    .manifest
                    .capabilities
                    .iter()
                    .map(|capability| capability.capability_id.clone())
            })
            .collect();
        let resolutions: Vec<_> = capability_ids
            .iter()
            .map(|capability_id| self.resolve(capability_id))
            .collect();
        let complete = extension_entries.iter().all(|entry| entry.available)
            && resolutions
                .iter()
                .all(|resolution| resolution.status == ExtensionResolutionStatus::Selected);
        ExtensionDoctorReport {
            sdk_contract: EXTENSION_SDK_CONTRACT.to_owned(),
            extension_count: extension_entries.len(),
            available_extension_count: extension_entries
                .iter()
                .filter(|entry| entry.available)
                .count(),
            capability_count: capability_ids.len(),
            complete,
            extensions: extension_entries,
            resolutions,
        }
    }
}

fn list_entry(extension: &LoadedExtension) -> ExtensionListEntry {
    let mut capability_ids: Vec<_> = extension
        .manifest
        .capabilities
        .iter()
        .map(|capability| capability.capability_id.clone())
        .collect();
    capability_ids.sort();
    ExtensionListEntry {
        extension_id: extension.manifest.extension_id.clone(),
        version: extension.manifest.version.clone(),
        publisher_id: extension.manifest.publisher.id.clone(),
        manifest_digest: extension.manifest_digest.clone(),
        lock_digest: extension.lock_digest.clone(),
        enabled: extension.lock.enabled,
        trust: extension.lock.trust,
        precedence: extension.lock.precedence,
        execution_mode: extension.lock.selected_execution_mode.clone(),
        capability_ids,
        available: extension.availability_reasons.is_empty(),
        reasons: extension.availability_reasons.clone(),
    }
}

fn decode_contract<T>(path: &Path, bytes: &[u8], contract: Contract) -> Result<T, CatalogError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    let document: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| CatalogError::InvalidInput {
            path: path.to_path_buf(),
            message: format!("extension document is not valid JSON: {error}"),
        })?;
    contracts::validate(contract, &document).map_err(|error| CatalogError::InvalidInput {
        path: path.to_path_buf(),
        message: format!("extension document failed contract validation: {error}"),
    })?;
    serde_json::from_value(document).map_err(|error| CatalogError::InvalidInput {
        path: path.to_path_buf(),
        message: format!("extension document could not be decoded: {error}"),
    })
}

fn validate_manifest(path: &Path, manifest: &ExtensionManifestV1) -> Result<(), CatalogError> {
    let invalid = |message: String| CatalogError::InvalidInput {
        path: path.to_path_buf(),
        message,
    };
    if manifest.schema != EXTENSION_MANIFEST_SCHEMA {
        return Err(invalid("unsupported extension manifest schema".to_owned()));
    }
    if !valid_qualified_id(&manifest.extension_id) {
        return Err(invalid(
            "extension_id must be a lowercase qualified identifier".to_owned(),
        ));
    }
    if !valid_semver(&manifest.version) {
        return Err(invalid(
            "extension version must be semantic versioning".to_owned(),
        ));
    }
    if manifest.capabilities.is_empty() {
        return Err(invalid(
            "an extension must declare at least one capability".to_owned(),
        ));
    }
    if manifest
        .replacement
        .replaces
        .iter()
        .chain(&manifest.replacement.fallbacks_for)
        .any(|extension_id| extension_id == &manifest.extension_id)
    {
        return Err(invalid(
            "an extension cannot replace or fall back for itself".to_owned(),
        ));
    }

    let mut schema_ids = BTreeSet::new();
    for declaration in &manifest.configuration_schemas {
        if !schema_ids.insert(declaration.schema_id.clone()) {
            return Err(invalid(format!(
                "configuration schema {} is declared more than once",
                declaration.schema_id
            )));
        }
        let expected =
            fingerprint(&declaration.document).map_err(|error| invalid(error.to_string()))?;
        if expected != declaration.schema_digest {
            return Err(invalid(format!(
                "configuration schema {} fingerprint does not match its document",
                declaration.schema_id
            )));
        }
        jsonschema::validator_for(&declaration.document).map_err(|error| {
            invalid(format!(
                "configuration schema {} cannot be compiled: {error}",
                declaration.schema_id
            ))
        })?;
    }

    let prohibited = BTreeSet::from([
        ExtensionEffect::SourceMutation,
        ExtensionEffect::Destructive,
        ExtensionEffect::Sign,
        ExtensionEffect::Publish,
    ]);
    if !manifest.requested_effects.is_disjoint(&prohibited) {
        return Err(invalid(
            "extension SDK v1 cannot request mutation, destructive, signing, or publication authority"
                .to_owned(),
        ));
    }

    let mut capability_ids = BTreeSet::new();
    for capability in &manifest.capabilities {
        if !capability_ids.insert(capability.capability_id.clone()) {
            return Err(invalid(format!(
                "capability {} is declared more than once",
                capability.capability_id
            )));
        }
        if !valid_capability_id(&capability.capability_id) {
            return Err(invalid(format!(
                "capability {} is outside OptiFlow domain ownership",
                capability.capability_id
            )));
        }
        if !valid_semver(&capability.capability_version) {
            return Err(invalid(format!(
                "capability {} has an invalid version",
                capability.capability_id
            )));
        }
        if !schema_ids.contains(&capability.configuration_schema) {
            return Err(invalid(format!(
                "capability {} references an undeclared configuration schema",
                capability.capability_id
            )));
        }
        if !capability.effects.is_subset(&manifest.requested_effects) {
            return Err(invalid(format!(
                "capability {} uses an effect the manifest did not request",
                capability.capability_id
            )));
        }
        if capability.locality.local_only
            && (!capability.locality.network_hosts.is_empty()
                || capability.effects.contains(&ExtensionEffect::Network))
        {
            return Err(invalid(format!(
                "local-only capability {} cannot declare network behavior",
                capability.capability_id
            )));
        }
        if !capability.locality.local_only
            && (capability.locality.network_hosts.is_empty()
                || !capability.effects.contains(&ExtensionEffect::Network))
        {
            return Err(invalid(format!(
                "networked capability {} must declare hosts and the network effect",
                capability.capability_id
            )));
        }
        if capability.resource_needs.gpu != capability.effects.contains(&ExtensionEffect::Gpu) {
            return Err(invalid(format!(
                "capability {} has inconsistent GPU declarations",
                capability.capability_id
            )));
        }
        if capability.content_changes && capability.kind != CapabilityKind::Planner {
            return Err(invalid(format!(
                "only a planner may describe a content-changing operation: {}",
                capability.capability_id
            )));
        }
        if capability.kind == CapabilityKind::LifecycleObserver && capability.content_changes {
            return Err(invalid("lifecycle observers must be read-only".to_owned()));
        }
        if capability.coverage.mode == DeclaredCoverage::PartialByDesign
            && capability.coverage.dimensions.is_empty()
        {
            return Err(invalid(format!(
                "partial-by-design capability {} must name coverage dimensions",
                capability.capability_id
            )));
        }
    }

    let mut mode_names = BTreeSet::new();
    for mode in &manifest.execution_modes {
        if !mode_names.insert(mode.name().to_owned()) {
            return Err(invalid(format!(
                "execution mode {} is declared more than once",
                mode.name()
            )));
        }
        if let ExecutionModeDeclaration::Process {
            protocol, limits, ..
        } = mode
        {
            if !manifest
                .requested_effects
                .contains(&ExtensionEffect::Subprocess)
            {
                return Err(invalid(format!(
                    "process execution mode {} requires the subprocess effect",
                    mode.name()
                )));
            }
            if protocol != EXTENSION_INVOCATION_SCHEMA {
                return Err(invalid(format!(
                    "process execution mode {} uses an unsupported protocol",
                    mode.name()
                )));
            }
            if limits.timeout_ms == 0
                || limits.max_stdin_bytes == 0
                || limits.max_stdout_bytes == 0
                || limits.max_stderr_bytes == 0
            {
                return Err(invalid(format!(
                    "process execution mode {} has a zero resource bound",
                    mode.name()
                )));
            }
            if limits.timeout_ms > MAX_EXTENSION_TIMEOUT_MS
                || limits.max_stdin_bytes > MAX_EXTENSION_STDIN_BYTES
                || limits.max_stdout_bytes > MAX_EXTENSION_STDOUT_BYTES
                || limits.max_stderr_bytes > MAX_EXTENSION_STDERR_BYTES
            {
                return Err(invalid(format!(
                    "process execution mode {} exceeds an SDK v1 host ceiling",
                    mode.name()
                )));
            }
        }
    }
    if mode_names.is_empty() {
        return Err(invalid(
            "an extension must declare an execution mode".to_owned(),
        ));
    }

    let has_observer_capability = manifest
        .capabilities
        .iter()
        .any(|capability| capability.kind == CapabilityKind::LifecycleObserver);
    let mut hook_ids = BTreeSet::new();
    for hook in &manifest.observer_hooks {
        if !hook_ids.insert(hook.hook_id.clone()) {
            return Err(invalid(format!(
                "observer hook {} is declared more than once",
                hook.hook_id
            )));
        }
        if !hook.read_only {
            return Err(invalid(format!(
                "observer hook {} must declare read_only true",
                hook.hook_id
            )));
        }
        if hook.lifecycle_points.is_empty() {
            return Err(invalid(format!(
                "observer hook {} must select a lifecycle point",
                hook.hook_id
            )));
        }
    }
    if has_observer_capability && manifest.observer_hooks.is_empty() {
        return Err(invalid(
            "lifecycle-observer capabilities require a read-only hook declaration".to_owned(),
        ));
    }
    Ok(())
}

fn availability_reasons(
    manifest: &ExtensionManifestV1,
    manifest_digest: &Digest,
    lock: &ExtensionLockV1,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if manifest.sdk_contract != EXTENSION_SDK_CONTRACT {
        reasons.push(format!(
            "SDK contract {} is incompatible with {}",
            manifest.sdk_contract, EXTENSION_SDK_CONTRACT
        ));
    }
    if lock.schema != EXTENSION_LOCK_SCHEMA {
        reasons.push("the lock declares an unsupported schema".to_owned());
    }
    if lock.manifest_digest != *manifest_digest {
        reasons.push("the lock does not pin the exact manifest bytes".to_owned());
    }
    if !lock.enabled || lock.trust == TrustMode::Disabled {
        reasons.push("the extension is disabled by its operator lock".to_owned());
    }
    match lock.expected_grants_digest() {
        Ok(expected) if expected == lock.grants_digest => {}
        Ok(_) => reasons.push("the lock grants fingerprint is inconsistent".to_owned()),
        Err(error) => reasons.push(format!(
            "the lock grants could not be fingerprinted: {error}"
        )),
    }
    if !lock.granted_effects.is_subset(&manifest.requested_effects) {
        reasons.push("the lock grants effects the manifest did not request".to_owned());
    }
    if !lock
        .allowed_replacements
        .iter()
        .all(|extension_id| manifest.replacement.replaces.contains(extension_id))
    {
        reasons.push("the lock authorizes an undeclared replacement".to_owned());
    }
    if !lock
        .allowed_fallbacks
        .iter()
        .all(|extension_id| manifest.replacement.fallbacks_for.contains(extension_id))
    {
        reasons.push("the lock authorizes an undeclared fallback".to_owned());
    }
    for capability in &manifest.capabilities {
        if !lock
            .configurations
            .contains_key(&capability.configuration_schema)
        {
            reasons.push(format!(
                "capability {} has no locked configuration for {}",
                capability.capability_id, capability.configuration_schema
            ));
        }
    }
    let selected_mode = manifest
        .execution_modes
        .iter()
        .find(|mode| mode.name() == lock.selected_execution_mode);
    match selected_mode {
        None => reasons.push("the selected execution mode is not declared".to_owned()),
        Some(ExecutionModeDeclaration::Embedded { .. }) => {
            if lock.trust != TrustMode::TrustedEmbedded {
                reasons.push("embedded execution requires trusted_embedded mode".to_owned());
            }
            if lock.process.is_some() {
                reasons.push("an embedded lock cannot include process execution state".to_owned());
            }
        }
        Some(ExecutionModeDeclaration::Process { .. }) => {
            if lock.trust != TrustMode::TrustedProcess {
                reasons.push("process execution requires trusted_process mode".to_owned());
            }
            if !lock.granted_effects.contains(&ExtensionEffect::Subprocess) {
                reasons.push("process execution requires an explicit subprocess grant".to_owned());
            }
            match &lock.process {
                Some(process) => reasons.extend(process_reasons(process)),
                None => reasons.push("a process lock must pin an executable".to_owned()),
            }
        }
    }

    let configuration_schemas: BTreeMap<_, _> = manifest
        .configuration_schemas
        .iter()
        .map(|declaration| (declaration.schema_id.as_str(), declaration))
        .collect();
    for (schema_id, locked) in &lock.configurations {
        let Some(declaration) = configuration_schemas.get(schema_id.as_str()) else {
            reasons.push(format!(
                "locked configuration {schema_id} has no declared schema"
            ));
            continue;
        };
        match fingerprint(&locked.values) {
            Ok(expected) if expected == locked.fingerprint => {}
            Ok(_) => reasons.push(format!(
                "locked configuration {schema_id} fingerprint is inconsistent"
            )),
            Err(error) => reasons.push(format!(
                "locked configuration {schema_id} could not be fingerprinted: {error}"
            )),
        }
        match jsonschema::validator_for(&declaration.document) {
            Ok(validator) => {
                if let Err(error) = validator.validate(&locked.values) {
                    reasons.push(format!(
                        "locked configuration {schema_id} is invalid at {}: {error}",
                        error.instance_path()
                    ));
                }
            }
            Err(error) => reasons.push(format!(
                "configuration schema {schema_id} could not be compiled: {error}"
            )),
        }
    }
    reasons.sort();
    reasons.dedup();
    reasons
}

fn process_reasons(process: &super::model::LockedProcess) -> Vec<String> {
    let path = process.executable.to_path_buf();
    let working_directory = process.working_directory.to_path_buf();
    let mut reasons = Vec::new();
    if !path.is_absolute() {
        reasons.push("the locked process executable path is not absolute".to_owned());
    }
    if !working_directory.is_absolute() {
        reasons.push("the locked process working directory is not absolute".to_owned());
    }
    match digest_regular_file(&path) {
        Ok(digest) if digest == process.executable_digest => {}
        Ok(_) => reasons.push("the process executable digest does not match the lock".to_owned()),
        Err(error) => reasons.push(format!("the process executable is unavailable: {error}")),
    }
    if !has_executable_permission(&path) {
        reasons.push("the locked process file is not executable".to_owned());
    }
    match fs::symlink_metadata(&working_directory) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            reasons.push("the process working directory is not a non-symlink directory".to_owned())
        }
        Err(error) => reasons.push(format!(
            "the process working directory is unavailable: {error}"
        )),
    }
    reasons
}

#[cfg(unix)]
fn has_executable_permission(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn has_executable_permission(path: &Path) -> bool {
    path.is_file()
}

pub(crate) fn validate_invocation(
    extension: &LoadedExtension,
    invocation: &super::model::ExtensionInvocationV1,
) -> Result<(), String> {
    contracts::validate(Contract::ExtensionInvocation, invocation)
        .map_err(|error| format!("invocation contract is invalid: {error}"))?;
    if invocation.schema != EXTENSION_INVOCATION_SCHEMA {
        return Err("invocation schema is unsupported".to_owned());
    }
    if invocation.extension.extension_id != extension.manifest.extension_id
        || invocation.extension.version != extension.manifest.version
        || invocation.extension.manifest_digest != extension.manifest_digest
    {
        return Err(
            "invocation extension identity does not match the resolved provider".to_owned(),
        );
    }
    let capability = extension
        .capability(&invocation.capability_id)
        .ok_or_else(|| "invocation capability is not declared by the provider".to_owned())?;
    if invocation.capability_kind != capability.kind {
        return Err("invocation capability kind does not match the manifest".to_owned());
    }
    if invocation
        .input_artifacts
        .iter()
        .any(|artifact| !capability.accepts.contains(&artifact.schema))
    {
        return Err(
            "invocation contains an artifact schema the capability does not accept".to_owned(),
        );
    }
    let mut evidence_ids = BTreeSet::new();
    for evidence in &invocation.input_evidence {
        if !evidence_ids.insert(evidence.evidence_id.as_str()) {
            return Err("inline input evidence identifiers must be unique".to_owned());
        }
        let Some(artifact) = invocation
            .input_artifacts
            .iter()
            .find(|artifact| artifact.artifact_id == evidence.evidence_id)
        else {
            return Err("inline input evidence has no matching artifact reference".to_owned());
        };
        if artifact.schema != evidence.schema
            || artifact.content_digest != evidence.fingerprint
            || fingerprint(&evidence.value).map_err(|error| error.to_string())?
                != evidence.fingerprint
        {
            return Err("inline input evidence does not match its immutable reference".to_owned());
        }
    }
    let mut artifact_ids = BTreeSet::new();
    if invocation
        .input_artifacts
        .iter()
        .chain(&invocation.checkpoint_references)
        .any(|artifact| !artifact_ids.insert(artifact.artifact_id.as_str()))
    {
        return Err("invocation artifact identifiers must be unique".to_owned());
    }
    match (capability.kind, &invocation.lifecycle_event) {
        (CapabilityKind::LifecycleObserver, Some(event)) => {
            if event.invocation_id != invocation.invocation_id {
                return Err("lifecycle event is not bound to its invocation".to_owned());
            }
            if event.artifact_references.iter().any(|artifact| {
                !invocation
                    .input_artifacts
                    .iter()
                    .any(|input| input == artifact)
            }) {
                return Err("lifecycle event references evidence outside its invocation".to_owned());
            }
            let declared = extension
                .manifest
                .observer_hooks
                .iter()
                .any(|hook| hook.read_only && hook.lifecycle_points.contains(&event.phase));
            if !declared {
                return Err("lifecycle event phase is not declared by a read-only hook".to_owned());
            }
        }
        (CapabilityKind::LifecycleObserver, None) => {
            return Err("lifecycle observer invocation is missing its event".to_owned());
        }
        (_, Some(_)) => {
            return Err("only a lifecycle observer invocation may contain an event".to_owned());
        }
        (_, None) => {}
    }
    if invocation.configuration.schema_id != capability.configuration_schema {
        return Err("invocation configuration schema does not match the capability".to_owned());
    }
    let Some(selected_mode) = extension.selected_execution_mode() else {
        return Err("the locked execution mode is unavailable".to_owned());
    };
    if let ExecutionModeDeclaration::Process { limits, .. } = selected_mode {
        if invocation.limits != *limits {
            return Err("invocation limits do not match the pinned process limits".to_owned());
        }
    }
    let locked = extension
        .lock
        .configurations
        .get(&invocation.configuration.schema_id)
        .ok_or_else(|| "invocation configuration is not present in the lock".to_owned())?;
    if invocation.configuration.values != locked.values
        || invocation.configuration.fingerprint != locked.fingerprint
    {
        return Err("invocation configuration does not match locked evidence".to_owned());
    }
    if invocation.authorization.lock_id != extension.lock.lock_id
        || invocation.authorization.lock_digest != extension.lock_digest
        || invocation.authorization.grants_digest != extension.lock.grants_digest
        || invocation.authorization.granted_effects != extension.lock.granted_effects
    {
        return Err("invocation authorization does not match the operator lock".to_owned());
    }
    if !capability
        .effects
        .is_subset(&invocation.authorization.granted_effects)
    {
        return Err("invocation lacks an effect required by its capability".to_owned());
    }
    Ok(())
}

pub(crate) fn validate_result(
    extension: &LoadedExtension,
    invocation: &super::model::ExtensionInvocationV1,
    result: &ExtensionResultV1,
) -> Result<(), String> {
    contracts::validate(Contract::ExtensionResult, result)
        .map_err(|error| format!("extension result contract is invalid: {error}"))?;
    if result.schema != EXTENSION_RESULT_SCHEMA {
        return Err("extension result schema is unsupported".to_owned());
    }
    if result.invocation_id != invocation.invocation_id
        || result.extension != invocation.extension
        || result.capability_id != invocation.capability_id
    {
        return Err("extension result identity does not match its invocation".to_owned());
    }
    let capability = extension
        .capability(&result.capability_id)
        .ok_or_else(|| "extension result names an undeclared capability".to_owned())?;
    if result.provenance.extension != invocation.extension
        || result.provenance.capability_id != capability.capability_id
        || result.provenance.capability_version != capability.capability_version
        || result.provenance.configuration_fingerprint != invocation.configuration.fingerprint
        || result.provenance.authorization_lock_id != extension.lock.lock_id
        || result.provenance.authorization_lock_digest != extension.lock_digest
        || result.provenance.grants_digest != extension.lock.grants_digest
        || result.provenance.execution_mode != extension.lock.selected_execution_mode
    {
        return Err("extension result provenance is not bound to its invocation".to_owned());
    }
    let expected_inputs: Vec<_> = invocation
        .input_artifacts
        .iter()
        .map(|artifact| artifact.content_digest.clone())
        .collect();
    if result.provenance.input_fingerprints != expected_inputs {
        return Err("extension result input fingerprints do not match the invocation".to_owned());
    }
    if result.checkpoint_references != invocation.checkpoint_references {
        return Err(
            "extension result checkpoint evidence does not match the invocation".to_owned(),
        );
    }
    let expected_artifact_ids: BTreeSet<_> = invocation
        .input_artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.as_str())
        .collect();
    if result
        .consumed_artifacts
        .iter()
        .any(|artifact_id| !expected_artifact_ids.contains(artifact_id.as_str()))
    {
        return Err("extension result claims an input artifact outside its invocation".to_owned());
    }
    let consumed_artifact_ids: BTreeSet<_> = result
        .consumed_artifacts
        .iter()
        .map(String::as_str)
        .collect();
    if !result.observed_effects.is_subset(&capability.effects)
        || !result
            .observed_effects
            .is_subset(&invocation.authorization.granted_effects)
    {
        return Err("extension result reports an undeclared or unauthorized effect".to_owned());
    }
    match result.coverage.status {
        super::model::ExtensionCoverageStatus::Complete
            if !result.coverage.limitations.is_empty() =>
        {
            return Err("complete extension coverage cannot include limitations".to_owned());
        }
        super::model::ExtensionCoverageStatus::Partial
            if result.coverage.limitations.is_empty() =>
        {
            return Err("partial extension coverage must explain its limitations".to_owned());
        }
        _ => {}
    }
    if capability.coverage.mode == DeclaredCoverage::PartialByDesign
        && result.coverage.status == super::model::ExtensionCoverageStatus::Complete
    {
        return Err("a partial-by-design capability cannot claim complete coverage".to_owned());
    }
    if result.coverage.status == super::model::ExtensionCoverageStatus::Complete
        && consumed_artifact_ids != expected_artifact_ids
    {
        return Err("complete coverage must consume every invoked artifact".to_owned());
    }
    if matches!(
        result.outcome,
        super::model::ExtensionOutcome::Unavailable
            | super::model::ExtensionOutcome::Cancelled
            | super::model::ExtensionOutcome::Failed
    ) && result.coverage.status != super::model::ExtensionCoverageStatus::Partial
    {
        return Err("an unsuccessful extension result must report partial coverage".to_owned());
    }
    if matches!(
        result.outcome,
        super::model::ExtensionOutcome::Unavailable
            | super::model::ExtensionOutcome::Cancelled
            | super::model::ExtensionOutcome::Failed
    ) && (!result.evidence.is_empty()
        || !result.policy_contributions.is_empty()
        || !result.plan_operations.is_empty()
        || !result.validations.is_empty()
        || !result.reports.is_empty()
        || !result.observed_effects.is_empty())
    {
        return Err("an unsuccessful extension result cannot publish contributions".to_owned());
    }

    if !result.evidence.is_empty()
        && !matches!(
            capability.kind,
            CapabilityKind::Inspector | CapabilityKind::Analyzer
        )
    {
        return Err("only inspector or analyzer capabilities may contribute evidence".to_owned());
    }

    let mut contribution_ids = BTreeSet::new();
    for evidence in result.evidence.iter().chain(
        result
            .validations
            .iter()
            .map(|validation| &validation.evidence),
    ) {
        if !contribution_ids.insert(evidence.evidence_id.as_str()) {
            return Err(format!(
                "evidence contribution {} is duplicated",
                evidence.evidence_id
            ));
        }
        if !capability.produces.contains(&evidence.schema) {
            return Err(format!(
                "evidence contribution {} uses an undeclared output schema",
                evidence.evidence_id
            ));
        }
        if fingerprint(&evidence.value).map_err(|error| error.to_string())? != evidence.fingerprint
        {
            return Err(format!(
                "evidence contribution {} has a mismatched fingerprint",
                evidence.evidence_id
            ));
        }
    }
    for report in &result.reports {
        if capability.kind != CapabilityKind::ReportProvider {
            return Err("only report-provider capabilities may contribute reports".to_owned());
        }
        if !contribution_ids.insert(report.report_id.as_str()) {
            return Err(format!(
                "report contribution {} is duplicated",
                report.report_id
            ));
        }
        if !capability.produces.contains(&report.schema) {
            return Err(format!(
                "report contribution {} uses an undeclared output schema",
                report.report_id
            ));
        }
        if fingerprint(&report.value).map_err(|error| error.to_string())? != report.fingerprint {
            return Err(format!(
                "report contribution {} has a mismatched fingerprint",
                report.report_id
            ));
        }
    }
    for policy in &result.policy_contributions {
        if capability.kind != CapabilityKind::PolicyContributor {
            return Err("only policy-contributor capabilities may contribute policy".to_owned());
        }
        if !contribution_ids.insert(policy.policy_id.as_str()) {
            return Err(format!(
                "policy contribution {} is duplicated",
                policy.policy_id
            ));
        }
        if !capability.produces.contains(&policy.schema) {
            return Err(format!(
                "policy contribution {} uses an undeclared output schema",
                policy.policy_id
            ));
        }
        if fingerprint(&policy.value).map_err(|error| error.to_string())? != policy.fingerprint {
            return Err(format!(
                "policy contribution {} has a mismatched fingerprint",
                policy.policy_id
            ));
        }
    }
    for operation in &result.plan_operations {
        if capability.kind != CapabilityKind::Planner {
            return Err("only planner capabilities may contribute plan operations".to_owned());
        }
        if !contribution_ids.insert(operation.operation_id.as_str()) {
            return Err(format!(
                "plan operation {} is duplicated",
                operation.operation_id
            ));
        }
        if operation.input_artifacts.iter().any(|artifact| {
            !invocation
                .input_artifacts
                .iter()
                .any(|expected| expected == artifact)
        }) {
            return Err(format!(
                "plan operation {} references evidence outside its invocation",
                operation.operation_id
            ));
        }
        if capability.content_changes
            && (operation.input_artifacts.is_empty()
                || operation.expected_output_schemas.is_empty())
        {
            return Err(format!(
                "content-changing plan operation {} must bind inputs and outputs",
                operation.operation_id
            ));
        }
        if !operation.preconditions.is_complete()
            || operation.mutates_source
            || !operation.requires_core_execution
        {
            return Err(format!(
                "plan operation {} bypasses a required core safety boundary",
                operation.operation_id
            ));
        }
        if operation
            .expected_output_schemas
            .iter()
            .any(|schema| !capability.produces.contains(schema))
        {
            return Err(format!(
                "plan operation {} uses an undeclared output schema",
                operation.operation_id
            ));
        }
        if operation
            .expected_fingerprint()
            .map_err(|error| error.to_string())?
            != operation.fingerprint
        {
            return Err(format!(
                "plan operation {} has a mismatched fingerprint",
                operation.operation_id
            ));
        }
    }
    if !result.validations.is_empty() && capability.kind != CapabilityKind::Validator {
        return Err("only validator capabilities may contribute validations".to_owned());
    }
    if result.validations.iter().any(|validation| {
        validation.validator_id != capability.capability_id
            || validation.validator_version != capability.capability_version
    }) {
        return Err("validation identity does not match the selected capability".to_owned());
    }
    if capability.kind == CapabilityKind::LifecycleObserver
        && (!result.evidence.is_empty()
            || !result.policy_contributions.is_empty()
            || !result.plan_operations.is_empty()
            || !result.validations.is_empty()
            || !result.reports.is_empty()
            || !result.observed_effects.is_empty()
            || !result.checkpoint_references.is_empty())
    {
        return Err("a lifecycle observer cannot return contributions or effects".to_owned());
    }
    if result.progress.windows(2).any(|pair| {
        pair[0].sequence >= pair[1].sequence
            || pair[0].completed > pair[1].completed
            || matches!((pair[0].total, pair[1].total), (Some(left), Some(right)) if left != right)
    }) {
        return Err(
            "extension progress sequence must increase and counts must remain monotonic".to_owned(),
        );
    }
    if result
        .progress
        .iter()
        .any(|event| event.total.is_some_and(|total| event.completed > total))
    {
        return Err("extension progress cannot exceed its declared total".to_owned());
    }
    Ok(())
}

fn stable_read(path: &Path) -> Result<Vec<u8>, CatalogError> {
    for attempt in 0..STABLE_READ_ATTEMPTS {
        let before = fs::symlink_metadata(path).map_err(|error| CatalogError::InvalidInput {
            path: path.to_path_buf(),
            message: format!("extension document metadata could not be read: {error}"),
        })?;
        if before.file_type().is_symlink() || !before.file_type().is_file() {
            return Err(CatalogError::InvalidInput {
                path: path.to_path_buf(),
                message: "extension documents must be non-symlink regular files".to_owned(),
            });
        }
        if before.len() > MAX_DECLARATION_BYTES {
            return Err(CatalogError::InvalidInput {
                path: path.to_path_buf(),
                message: format!(
                    "extension document exceeds the {MAX_DECLARATION_BYTES} byte limit"
                ),
            });
        }
        let mut file = File::open(path).map_err(|error| CatalogError::InvalidInput {
            path: path.to_path_buf(),
            message: format!("extension document could not be opened: {error}"),
        })?;
        let opened_before = file
            .metadata()
            .map_err(|error| CatalogError::InvalidInput {
                path: path.to_path_buf(),
                message: format!("opened extension metadata could not be read: {error}"),
            })?;
        if !same_file(&before, &opened_before) {
            if attempt + 1 < STABLE_READ_ATTEMPTS {
                continue;
            }
            return Err(CatalogError::InvalidInput {
                path: path.to_path_buf(),
                message: "extension document identity changed before reading".to_owned(),
            });
        }
        let mut bytes = Vec::with_capacity(before.len() as usize);
        file.read_to_end(&mut bytes)
            .map_err(|error| CatalogError::InvalidInput {
                path: path.to_path_buf(),
                message: format!("extension document could not be read: {error}"),
            })?;
        let opened_after = file
            .metadata()
            .map_err(|error| CatalogError::InvalidInput {
                path: path.to_path_buf(),
                message: format!("opened extension metadata could not be re-read: {error}"),
            })?;
        let after = fs::symlink_metadata(path).map_err(|error| CatalogError::InvalidInput {
            path: path.to_path_buf(),
            message: format!("extension document disappeared while reading: {error}"),
        })?;
        if same_file(&before, &opened_after)
            && same_file(&opened_after, &after)
            && bytes.len() as u64 == after.len()
        {
            return Ok(bytes);
        }
        if attempt + 1 == STABLE_READ_ATTEMPTS {
            return Err(CatalogError::InvalidInput {
                path: path.to_path_buf(),
                message: "extension document changed during both bounded read attempts".to_owned(),
            });
        }
    }
    unreachable!("bounded stable read always returns")
}

pub(crate) fn digest_regular_file(path: &Path) -> Result<Digest, String> {
    let before = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if before.file_type().is_symlink() || !before.file_type().is_file() {
        return Err("path is not a non-symlink regular file".to_owned());
    }
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let opened_before = file.metadata().map_err(|error| error.to_string())?;
    if !same_file(&before, &opened_before) {
        return Err("file identity changed before hashing".to_owned());
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let opened_after = file.metadata().map_err(|error| error.to_string())?;
    let after = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !same_file(&before, &opened_after) || !same_file(&opened_after, &after) {
        return Err("file identity or metadata changed while hashing".to_owned());
    }
    Ok(Digest {
        algorithm: "blake3-256".to_owned(),
        value: hasher.finalize().to_hex().to_string(),
    })
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}

#[cfg(not(unix))]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.file_type().is_file() == right.file_type().is_file()
}

fn valid_qualified_id(value: &str) -> bool {
    value.contains('.')
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_lowercase())
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn valid_capability_id(value: &str) -> bool {
    value.strip_prefix("optiflow/").is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix.split('/').all(|segment| {
                !segment.is_empty()
                    && segment
                        .bytes()
                        .next()
                        .is_some_and(|byte| byte.is_ascii_lowercase())
                    && segment.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                    })
            })
    })
}

fn valid_semver(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part == &"0" || !part.starts_with('0'))
        })
}
