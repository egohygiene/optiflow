//! Emit an operator lock for the checked-in process-extension example.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use optiflow::contracts::{self, Contract};
use optiflow::domain::NativePath;
use optiflow::extensions::{
    Digest, EXTENSION_LOCK_SCHEMA, ExtensionEffect, ExtensionLockV1, ExtensionManifestV1,
    LockedConfiguration, LockedProcess, TrustMode, fingerprint,
};
use serde_json::json;

fn main() -> Result<()> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() != 3 {
        bail!("usage: create_reference_lock MANIFEST ABSOLUTE_EXECUTABLE ABSOLUTE_WORK_DIRECTORY");
    }
    let manifest_path = PathBuf::from(&arguments[0]);
    let executable = PathBuf::from(&arguments[1]);
    let working_directory = PathBuf::from(&arguments[2]);
    if !executable.is_absolute() || !working_directory.is_absolute() {
        bail!("executable and working directory must be absolute");
    }
    let manifest_bytes = std::fs::read(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let document: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).context("invalid extension manifest JSON")?;
    contracts::validate(Contract::ExtensionManifest, &document)
        .context("manifest failed the public contract")?;
    let manifest: ExtensionManifestV1 =
        serde_json::from_value(document).context("invalid extension manifest")?;
    let values = json!({});
    let configurations = manifest
        .configuration_schemas
        .iter()
        .map(|schema| {
            Ok((
                schema.schema_id.clone(),
                LockedConfiguration {
                    values: values.clone(),
                    fingerprint: fingerprint(&values)?,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut lock = ExtensionLockV1 {
        schema: EXTENSION_LOCK_SCHEMA.to_owned(),
        lock_id: "example.reference-inspector.local".to_owned(),
        extension_id: manifest.extension_id,
        extension_version: manifest.version,
        manifest_digest: Digest::blake3(&manifest_bytes),
        enabled: true,
        trust: TrustMode::TrustedProcess,
        granted_effects: manifest.requested_effects,
        grants_digest: Digest::blake3(b"pending"),
        precedence: 100,
        allowed_replacements: Vec::new(),
        allowed_fallbacks: Vec::new(),
        selected_execution_mode: "json-stdio-v1".to_owned(),
        process: Some(LockedProcess {
            executable: NativePath::from_path(&executable),
            executable_digest: digest_file(&executable)?,
            working_directory: NativePath::from_path(&working_directory),
        }),
        configurations,
    };
    if !lock.granted_effects.contains(&ExtensionEffect::Subprocess) {
        bail!("reference manifest does not request process execution");
    }
    lock.grants_digest = lock.expected_grants_digest()?;
    println!("{}", serde_json::to_string_pretty(&lock)?);
    Ok(())
}

fn digest_file(path: &Path) -> Result<Digest> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        bail!("{} is not a non-symlink regular file", path.display());
    }
    Ok(Digest::blake3(&std::fs::read(path).with_context(|| {
        format!("failed to read {}", path.display())
    })?))
}
