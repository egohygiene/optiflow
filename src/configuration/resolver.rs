use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use directories::{BaseDirs, ProjectDirs};
use serde::Serialize;
use serde_json::Value;

use super::document::{CONFIG_SCHEMA, ConfigDocumentV1};
use super::policy::{
    ConfigurationSourceRecord, ConfigurationSourceStatus, EffectivePolicyV1, EvidencePolicy,
    ExactGroupingProfile, PolicyProvenance, PolicySourceKind, RuntimePolicy, ShadowedValue,
    StabilityRequirement, UnstableObservationPolicy, build_effective_policy,
};
use crate::cli::{Cli, Command, OutputFormat};
use crate::contracts::{self, Contract};
use crate::domain::{ScanOptions, SerializedPath};
use crate::hashing::DEFAULT_MAX_OBSERVATION_ATTEMPTS;
use crate::outcome::{
    Diagnostic, DiagnosticClassification, DiagnosticCode, DiagnosticImpact, DiagnosticSeverity,
};

const PROJECT_CONFIG_NAME: &str = "optiflow.toml";
const CONFIG_READ_ATTEMPTS: usize = 2;

#[derive(Debug, Clone, Serialize)]
pub struct ConfigurationResolution {
    pub policy: EffectivePolicyV1,
    pub sources: Vec<ConfigurationSourceRecord>,
    pub shadowed_values: Vec<ShadowedValue>,
    #[serde(skip)]
    pub runtime: RuntimePolicy,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingExplanation {
    pub setting: String,
    pub value: Value,
    pub provenance: PolicyProvenance,
    pub shadowed_values: Vec<ShadowedValue>,
    pub category: String,
    pub affects_evidence: bool,
    pub locked: bool,
    pub constraints: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_variable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_argument: Option<String>,
}

#[derive(Clone)]
struct ResolvedValue<T> {
    value: T,
    provenance: PolicyProvenance,
}

struct WorkingPolicy {
    output_format: ResolvedValue<OutputFormat>,
    state_directory: ResolvedValue<PathBuf>,
    follow_symlinks: ResolvedValue<bool>,
    include_hidden: ResolvedValue<bool>,
    cross_filesystems: ResolvedValue<bool>,
    probe_media: ResolvedValue<bool>,
    shadowed: Vec<ShadowedValue>,
}

struct CapturedEnvironment {
    config: Option<OsString>,
    state_directory: Option<OsString>,
    output_format: Option<OsString>,
    follow_symlinks: Option<OsString>,
    include_hidden: Option<OsString>,
    cross_filesystems: Option<OsString>,
    probe_media: Option<OsString>,
    xdg_state_home: Option<OsString>,
}

pub fn resolve(cli: &Cli) -> Result<ConfigurationResolution, Box<Diagnostic>> {
    let initial_working_directory = std::env::current_dir().map_err(|error| {
        diagnostic(
            DiagnosticCode::ConfigurationDiscoveryFailed,
            format!("the initial working directory could not be captured: {error}"),
        )
    })?;
    let environment = capture_environment();
    if cli.no_config && cli.config.is_some() {
        return Err(diagnostic(
            DiagnosticCode::ConfigurationSelectorConflict,
            "--config and --no-config cannot be used together",
        ));
    }

    let default_state_directory = default_state_directory(environment.xdg_state_home.as_deref())
        .map_err(|message| {
            diagnostic(DiagnosticCode::ConfigurationPathResolutionFailed, message)
        })?;
    let default = |detail: &str| PolicyProvenance {
        source: PolicySourceKind::CompiledDefault,
        detail: detail.to_owned(),
        path: None,
    };
    let mut working = WorkingPolicy {
        output_format: ResolvedValue {
            value: OutputFormat::Human,
            provenance: default("compiled output default"),
        },
        state_directory: ResolvedValue {
            value: default_state_directory,
            provenance: default("platform state-directory default"),
        },
        follow_symlinks: ResolvedValue {
            value: false,
            provenance: default("conservative traversal default"),
        },
        include_hidden: ResolvedValue {
            value: false,
            provenance: default("hidden paths excluded by default"),
        },
        cross_filesystems: ResolvedValue {
            value: false,
            provenance: default("filesystem boundaries preserved by default"),
        },
        probe_media: ResolvedValue {
            value: true,
            provenance: default("optional media probing enabled by default"),
        },
        shadowed: Vec::new(),
    };

    let mut sources = Vec::new();
    apply_file_sources(
        cli,
        &environment,
        &initial_working_directory,
        &mut working,
        &mut sources,
    )?;
    apply_environment(&environment, &initial_working_directory, &mut working)?;
    apply_command_line(cli, &initial_working_directory, &mut working)?;

    let evidence_policy = EvidencePolicy {
        follow_symlinks: working.follow_symlinks.value,
        include_hidden: working.include_hidden.value,
        cross_filesystems: working.cross_filesystems.value,
        probe_media: working.probe_media.value,
        exact_grouping_profile: ExactGroupingProfile::SizeAndBlake3_256,
        observation_stability: StabilityRequirement::Required,
        unstable_observations: UnstableObservationPolicy::Exclude,
        maximum_observation_attempts: DEFAULT_MAX_OBSERVATION_ATTEMPTS,
    };
    let mut provenance = BTreeMap::from([
        (
            "presentation_policy.output_format".to_owned(),
            working.output_format.provenance.clone(),
        ),
        (
            "operational_policy.state_directory".to_owned(),
            working.state_directory.provenance.clone(),
        ),
        (
            "evidence_policy.follow_symlinks".to_owned(),
            working.follow_symlinks.provenance.clone(),
        ),
        (
            "evidence_policy.include_hidden".to_owned(),
            working.include_hidden.provenance.clone(),
        ),
        (
            "evidence_policy.cross_filesystems".to_owned(),
            working.cross_filesystems.provenance.clone(),
        ),
        (
            "evidence_policy.probe_media".to_owned(),
            working.probe_media.provenance.clone(),
        ),
    ]);
    for setting in [
        "evidence_policy.exact_grouping_profile",
        "evidence_policy.observation_stability",
        "evidence_policy.unstable_observations",
        "evidence_policy.maximum_observation_attempts",
        "safety_invariants.source_mutation",
        "safety_invariants.apply_enabled",
        "safety_invariants.shell_execution",
        "safety_invariants.artifact_validation_required",
        "safety_invariants.atomic_artifact_commit_required",
        "safety_invariants.current_stability_required_for_exact_groups",
    ] {
        provenance.insert(
            setting.to_owned(),
            PolicyProvenance {
                source: PolicySourceKind::LockedInvariant,
                detail: "locked by the OptiFlow safety contract".to_owned(),
                path: None,
            },
        );
    }

    let policy = build_effective_policy(
        evidence_policy,
        &working.state_directory.value,
        working.output_format.value,
        provenance,
    )
    .map_err(|error| {
        internal_diagnostic(
            DiagnosticCode::EffectivePolicySerializationFailed,
            format!("the effective policy could not be materialized: {error}"),
        )
    })?;
    contracts::validate(Contract::EffectivePolicy, &policy).map_err(|error| {
        internal_diagnostic(
            DiagnosticCode::EffectivePolicyInvalid,
            format!("the generated effective policy failed validation: {error}"),
        )
    })?;

    let runtime = RuntimePolicy {
        state_directory: working.state_directory.value,
        scan: ScanOptions {
            follow_symlinks: working.follow_symlinks.value,
            include_hidden: working.include_hidden.value,
            cross_filesystems: working.cross_filesystems.value,
            probe_media: working.probe_media.value,
        },
        output_format: working.output_format.value,
    };
    Ok(ConfigurationResolution {
        policy,
        sources,
        shadowed_values: working.shadowed,
        runtime,
    })
}

impl ConfigurationResolution {
    pub fn explain(&self, requested: &str) -> Result<SettingExplanation, Box<Diagnostic>> {
        let canonical = canonical_setting(requested).ok_or_else(|| {
            let mut diagnostic = diagnostic(
                DiagnosticCode::ConfigurationSettingUnknown,
                "the requested configuration setting is unknown",
            );
            diagnostic.context.setting = Some(requested.to_owned());
            diagnostic
        })?;
        let document = serde_json::to_value(&self.policy).map_err(|error| {
            internal_diagnostic(
                DiagnosticCode::EffectivePolicySerializationFailed,
                format!("the effective policy could not be inspected: {error}"),
            )
        })?;
        let pointer = format!("/{}", canonical.replace('.', "/"));
        let value = document.pointer(&pointer).cloned().ok_or_else(|| {
            internal_diagnostic(
                DiagnosticCode::EffectivePolicyInvalid,
                format!("the effective policy omitted {canonical}"),
            )
        })?;
        let provenance = self
            .policy
            .provenance
            .get(canonical)
            .cloned()
            .ok_or_else(|| {
                internal_diagnostic(
                    DiagnosticCode::EffectivePolicyInvalid,
                    format!("the effective policy omitted provenance for {canonical}"),
                )
            })?;
        let (category, affects_evidence, locked, constraints, environment_variable, cli_argument) =
            setting_metadata(canonical);
        Ok(SettingExplanation {
            setting: canonical.to_owned(),
            value,
            provenance,
            shadowed_values: self
                .shadowed_values
                .iter()
                .filter(|value| value.setting == canonical)
                .cloned()
                .collect(),
            category: category.to_owned(),
            affects_evidence,
            locked,
            constraints: constraints.to_owned(),
            environment_variable: environment_variable.map(str::to_owned),
            cli_argument: cli_argument.map(str::to_owned),
        })
    }
}

fn apply_file_sources(
    cli: &Cli,
    environment: &CapturedEnvironment,
    cwd: &Path,
    working: &mut WorkingPolicy,
    sources: &mut Vec<ConfigurationSourceRecord>,
) -> Result<(), Box<Diagnostic>> {
    if cli.no_config {
        for source in [
            PolicySourceKind::UserConfiguration,
            PolicySourceKind::ProjectConfiguration,
            PolicySourceKind::ExplicitConfiguration,
        ] {
            sources.push(ConfigurationSourceRecord {
                source,
                status: ConfigurationSourceStatus::Suppressed,
                path: None,
            });
        }
        return Ok(());
    }

    let explicit = cli
        .config
        .clone()
        .map(|path| resolve_relative(&path, cwd))
        .or_else(|| {
            environment
                .config
                .as_ref()
                .map(|value| resolve_relative(Path::new(value), cwd))
        });
    if let Some(path) = explicit {
        let document = load_document(&path, true)?;
        apply_document(
            &document,
            PolicySourceKind::ExplicitConfiguration,
            &path,
            working,
        )?;
        sources.push(source_record(
            PolicySourceKind::ExplicitConfiguration,
            ConfigurationSourceStatus::Loaded,
            Some(&path),
        ));
        sources.push(source_record(
            PolicySourceKind::UserConfiguration,
            ConfigurationSourceStatus::Suppressed,
            None,
        ));
        sources.push(source_record(
            PolicySourceKind::ProjectConfiguration,
            ConfigurationSourceStatus::Suppressed,
            None,
        ));
        return Ok(());
    }

    let user_path = user_config_path();
    match user_path {
        Some(path) => match fs::symlink_metadata(&path) {
            Ok(_) => {
                let document = load_document(&path, false)?;
                apply_document(
                    &document,
                    PolicySourceKind::UserConfiguration,
                    &path,
                    working,
                )?;
                sources.push(source_record(
                    PolicySourceKind::UserConfiguration,
                    ConfigurationSourceStatus::Loaded,
                    Some(&path),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                sources.push(source_record(
                    PolicySourceKind::UserConfiguration,
                    ConfigurationSourceStatus::Absent,
                    Some(&path),
                ));
            }
            Err(error) => {
                return Err(path_diagnostic(
                    DiagnosticCode::ConfigurationDiscoveryFailed,
                    &path,
                    format!("user configuration could not be inspected: {error}"),
                ));
            }
        },
        None => sources.push(source_record(
            PolicySourceKind::UserConfiguration,
            ConfigurationSourceStatus::Absent,
            None,
        )),
    }

    match discover_project_config(cwd)? {
        Some(path) => {
            let document = load_document(&path, false)?;
            apply_document(
                &document,
                PolicySourceKind::ProjectConfiguration,
                &path,
                working,
            )?;
            sources.push(source_record(
                PolicySourceKind::ProjectConfiguration,
                ConfigurationSourceStatus::Loaded,
                Some(&path),
            ));
        }
        None => sources.push(source_record(
            PolicySourceKind::ProjectConfiguration,
            ConfigurationSourceStatus::Absent,
            Some(&cwd.join(PROJECT_CONFIG_NAME)),
        )),
    }
    Ok(())
}

fn apply_document(
    document: &ConfigDocumentV1,
    source: PolicySourceKind,
    path: &Path,
    working: &mut WorkingPolicy,
) -> Result<(), Box<Diagnostic>> {
    let provenance = |detail: &str| PolicyProvenance {
        source,
        detail: detail.to_owned(),
        path: Some(SerializedPath::from_path(path)),
    };
    if let Some(format) = document.output.as_ref().and_then(|output| output.format) {
        replace(
            "presentation_policy.output_format",
            &mut working.output_format,
            format,
            provenance("output.format"),
            &mut working.shadowed,
        );
    }
    if let Some(directory) = document
        .state
        .as_ref()
        .and_then(|state| state.directory.as_deref())
    {
        if directory.is_empty() {
            return Err(setting_diagnostic(
                DiagnosticCode::ConfigurationValueInvalid,
                "state.directory",
                "state.directory cannot be empty",
            ));
        }
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        replace(
            "operational_policy.state_directory",
            &mut working.state_directory,
            resolve_relative(Path::new(directory), base),
            provenance("state.directory"),
            &mut working.shadowed,
        );
    }
    if let Some(scan) = &document.scan {
        for (setting, value, target) in [
            (
                "evidence_policy.follow_symlinks",
                scan.follow_symlinks,
                &mut working.follow_symlinks,
            ),
            (
                "evidence_policy.include_hidden",
                scan.include_hidden,
                &mut working.include_hidden,
            ),
            (
                "evidence_policy.cross_filesystems",
                scan.cross_filesystems,
                &mut working.cross_filesystems,
            ),
            (
                "evidence_policy.probe_media",
                scan.probe_media,
                &mut working.probe_media,
            ),
        ] {
            if let Some(value) = value {
                replace(
                    setting,
                    target,
                    value,
                    provenance(setting),
                    &mut working.shadowed,
                );
            }
        }
    }
    Ok(())
}

fn apply_environment(
    environment: &CapturedEnvironment,
    cwd: &Path,
    working: &mut WorkingPolicy,
) -> Result<(), Box<Diagnostic>> {
    if let Some(value) = &environment.output_format {
        let format = parse_unicode_environment("OPTIFLOW_OUTPUT_FORMAT", value)?;
        let format = match format {
            "human" => OutputFormat::Human,
            "json" => OutputFormat::Json,
            _ => {
                return Err(environment_diagnostic(
                    "OPTIFLOW_OUTPUT_FORMAT",
                    "expected human or json",
                ));
            }
        };
        replace(
            "presentation_policy.output_format",
            &mut working.output_format,
            format,
            environment_provenance("OPTIFLOW_OUTPUT_FORMAT"),
            &mut working.shadowed,
        );
    }
    if let Some(value) = &environment.state_directory {
        if value.is_empty() {
            return Err(environment_diagnostic(
                "OPTIFLOW_STATE_DIRECTORY",
                "path cannot be empty",
            ));
        }
        replace(
            "operational_policy.state_directory",
            &mut working.state_directory,
            resolve_relative(Path::new(value), cwd),
            environment_provenance("OPTIFLOW_STATE_DIRECTORY"),
            &mut working.shadowed,
        );
    }
    for (name, value, setting, target) in [
        (
            "OPTIFLOW_FOLLOW_SYMLINKS",
            &environment.follow_symlinks,
            "evidence_policy.follow_symlinks",
            &mut working.follow_symlinks,
        ),
        (
            "OPTIFLOW_INCLUDE_HIDDEN",
            &environment.include_hidden,
            "evidence_policy.include_hidden",
            &mut working.include_hidden,
        ),
        (
            "OPTIFLOW_CROSS_FILESYSTEMS",
            &environment.cross_filesystems,
            "evidence_policy.cross_filesystems",
            &mut working.cross_filesystems,
        ),
        (
            "OPTIFLOW_PROBE_MEDIA",
            &environment.probe_media,
            "evidence_policy.probe_media",
            &mut working.probe_media,
        ),
    ] {
        if let Some(value) = value {
            replace(
                setting,
                target,
                parse_boolean_environment(name, value)?,
                environment_provenance(name),
                &mut working.shadowed,
            );
        }
    }
    Ok(())
}

fn apply_command_line(
    cli: &Cli,
    cwd: &Path,
    working: &mut WorkingPolicy,
) -> Result<(), Box<Diagnostic>> {
    let cli_provenance = |argument: &str| PolicyProvenance {
        source: PolicySourceKind::CommandLine,
        detail: argument.to_owned(),
        path: None,
    };
    if cli.json {
        replace(
            "presentation_policy.output_format",
            &mut working.output_format,
            OutputFormat::Json,
            cli_provenance("--json"),
            &mut working.shadowed,
        );
    } else if let Some(format) = cli.output_format {
        replace(
            "presentation_policy.output_format",
            &mut working.output_format,
            format,
            cli_provenance("--output-format"),
            &mut working.shadowed,
        );
    }
    if let Some(directory) = &cli.state_directory {
        replace(
            "operational_policy.state_directory",
            &mut working.state_directory,
            resolve_relative(directory, cwd),
            cli_provenance("--state-directory"),
            &mut working.shadowed,
        );
    }
    if let Command::Scan(scan) = &cli.command {
        for (setting, value, argument, target) in [
            (
                "evidence_policy.follow_symlinks",
                scan.follow_symlinks_override(),
                if scan.follow_symlinks {
                    "--follow-symlinks"
                } else {
                    "--no-follow-symlinks"
                },
                &mut working.follow_symlinks,
            ),
            (
                "evidence_policy.include_hidden",
                scan.include_hidden_override(),
                if scan.include_hidden {
                    "--include-hidden"
                } else {
                    "--exclude-hidden"
                },
                &mut working.include_hidden,
            ),
            (
                "evidence_policy.cross_filesystems",
                scan.cross_filesystems_override(),
                if scan.cross_filesystems {
                    "--cross-filesystems"
                } else {
                    "--stay-on-filesystem"
                },
                &mut working.cross_filesystems,
            ),
            (
                "evidence_policy.probe_media",
                scan.probe_media_override(),
                if scan.no_probe {
                    "--no-probe"
                } else {
                    "--probe"
                },
                &mut working.probe_media,
            ),
        ] {
            if let Some(value) = value {
                replace(
                    setting,
                    target,
                    value,
                    cli_provenance(argument),
                    &mut working.shadowed,
                );
            }
        }
    }
    Ok(())
}

fn load_document(
    path: &Path,
    explicitly_selected: bool,
) -> Result<ConfigDocumentV1, Box<Diagnostic>> {
    let text = stable_read(path, explicitly_selected)?;
    let loose: toml::Value =
        toml::from_str(&text).map_err(|error| toml_diagnostic(path, &text, error))?;
    let Some(schema) = loose.get("schema") else {
        return Err(path_diagnostic(
            DiagnosticCode::ConfigurationSchemaMissing,
            path,
            "the configuration document must declare schema",
        ));
    };
    let Some(schema) = schema.as_str() else {
        return Err(path_diagnostic(
            DiagnosticCode::ConfigurationValueInvalid,
            path,
            "configuration schema must be a string",
        ));
    };
    if schema != CONFIG_SCHEMA {
        return Err(path_diagnostic(
            DiagnosticCode::ConfigurationSchemaUnsupported,
            path,
            "the configuration document declares an unsupported schema",
        ));
    }
    if loose.get("safety").is_some() || loose.get("safety_invariants").is_some() {
        return Err(path_diagnostic(
            DiagnosticCode::ConfigurationLockedInvariant,
            path,
            "locked safety invariants cannot be configured",
        ));
    }
    let document: ConfigDocumentV1 =
        toml::from_str(&text).map_err(|error| toml_diagnostic(path, &text, error))?;
    contracts::validate(Contract::Config, &document).map_err(|error| {
        path_diagnostic(
            DiagnosticCode::ConfigurationValueInvalid,
            path,
            format!("configuration failed contract validation: {error}"),
        )
    })?;
    Ok(document)
}

fn stable_read(path: &Path, explicitly_selected: bool) -> Result<String, Box<Diagnostic>> {
    for attempt in 0..CONFIG_READ_ATTEMPTS {
        let before = fs::symlink_metadata(path).map_err(|error| {
            path_diagnostic(
                if explicitly_selected && error.kind() == std::io::ErrorKind::NotFound {
                    DiagnosticCode::ConfigurationNotFound
                } else {
                    DiagnosticCode::ConfigurationUnreadable
                },
                path,
                format!("configuration metadata could not be read: {error}"),
            )
        })?;
        if before.file_type().is_symlink() || !before.is_file() {
            return Err(path_diagnostic(
                DiagnosticCode::ConfigurationNotRegularFile,
                path,
                "configuration must be a regular file and cannot be a symbolic link",
            ));
        }
        let mut file = fs::File::open(path).map_err(|error| {
            path_diagnostic(
                DiagnosticCode::ConfigurationUnreadable,
                path,
                format!("configuration could not be opened: {error}"),
            )
        })?;
        let opened = file.metadata().map_err(|error| {
            path_diagnostic(
                DiagnosticCode::ConfigurationUnreadable,
                path,
                format!("opened configuration metadata could not be read: {error}"),
            )
        })?;
        if !same_file_snapshot(&before, &opened) {
            if attempt + 1 == CONFIG_READ_ATTEMPTS {
                return Err(path_diagnostic(
                    DiagnosticCode::ConfigurationSourceReplacedDuringRead,
                    path,
                    "configuration identity changed before it could be read",
                ));
            }
            continue;
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|error| {
            path_diagnostic(
                DiagnosticCode::ConfigurationUnreadable,
                path,
                format!("configuration could not be read: {error}"),
            )
        })?;
        let opened_after = file.metadata().map_err(|error| {
            path_diagnostic(
                DiagnosticCode::ConfigurationSourceReplacedDuringRead,
                path,
                format!("configuration metadata changed while being read: {error}"),
            )
        })?;
        let after = fs::symlink_metadata(path).map_err(|error| {
            path_diagnostic(
                DiagnosticCode::ConfigurationSourceReplacedDuringRead,
                path,
                format!("configuration disappeared while being read: {error}"),
            )
        })?;
        if same_file_snapshot(&before, &opened_after)
            && same_file_snapshot(&opened_after, &after)
            && u64::try_from(bytes.len()).ok() == Some(opened_after.len())
        {
            return String::from_utf8(bytes).map_err(|error| {
                path_diagnostic(
                    DiagnosticCode::ConfigurationParseFailed,
                    path,
                    format!("configuration is not valid UTF-8 TOML: {error}"),
                )
            });
        }
        if attempt + 1 == CONFIG_READ_ATTEMPTS {
            return Err(path_diagnostic(
                DiagnosticCode::ConfigurationSourceReplacedDuringRead,
                path,
                "configuration changed during both bounded read attempts",
            ));
        }
    }
    unreachable!("bounded configuration read always returns")
}

fn same_file_snapshot(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        before.dev() == after.dev() && before.ino() == after.ino()
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn discover_project_config(cwd: &Path) -> Result<Option<PathBuf>, Box<Diagnostic>> {
    for ancestor in cwd.ancestors() {
        let candidate = ancestor.join(PROJECT_CONFIG_NAME);
        match fs::symlink_metadata(&candidate) {
            Ok(_) => return Ok(Some(candidate)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(path_diagnostic(
                    DiagnosticCode::ConfigurationDiscoveryFailed,
                    &candidate,
                    format!("project configuration could not be inspected: {error}"),
                ));
            }
        }
    }
    Ok(None)
}

fn user_config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "optiflow")
        .map(|directories| directories.config_dir().join(PROJECT_CONFIG_NAME))
}

fn default_state_directory(_xdg_state_home: Option<&std::ffi::OsStr>) -> Result<PathBuf, String> {
    let Some(base_directories) = BaseDirs::new() else {
        return Err("the platform state-directory default is unavailable".to_owned());
    };
    #[cfg(target_os = "macos")]
    {
        Ok(base_directories
            .home_dir()
            .join("Library")
            .join("Application Support")
            .join("optiflow"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(_xdg_state_home.map_or_else(
            || base_directories.home_dir().join(".local/state/optiflow"),
            |path| PathBuf::from(path).join("optiflow"),
        ))
    }
}

fn capture_environment() -> CapturedEnvironment {
    CapturedEnvironment {
        config: std::env::var_os("OPTIFLOW_CONFIG"),
        state_directory: std::env::var_os("OPTIFLOW_STATE_DIRECTORY"),
        output_format: std::env::var_os("OPTIFLOW_OUTPUT_FORMAT"),
        follow_symlinks: std::env::var_os("OPTIFLOW_FOLLOW_SYMLINKS"),
        include_hidden: std::env::var_os("OPTIFLOW_INCLUDE_HIDDEN"),
        cross_filesystems: std::env::var_os("OPTIFLOW_CROSS_FILESYSTEMS"),
        probe_media: std::env::var_os("OPTIFLOW_PROBE_MEDIA"),
        xdg_state_home: std::env::var_os("XDG_STATE_HOME"),
    }
}

fn replace<T: Clone + Serialize>(
    setting: &str,
    target: &mut ResolvedValue<T>,
    value: T,
    provenance: PolicyProvenance,
    shadowed: &mut Vec<ShadowedValue>,
) {
    shadowed.push(ShadowedValue {
        setting: setting.to_owned(),
        value: serde_json::to_value(&target.value).unwrap_or(Value::Null),
        provenance: target.provenance.clone(),
    });
    target.value = value;
    target.provenance = provenance;
}

fn resolve_relative(path: &Path, base: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn parse_unicode_environment<'a>(
    name: &str,
    value: &'a OsString,
) -> Result<&'a str, Box<Diagnostic>> {
    value
        .to_str()
        .ok_or_else(|| environment_diagnostic(name, "value must be Unicode"))
}

fn parse_boolean_environment(name: &str, value: &OsString) -> Result<bool, Box<Diagnostic>> {
    match parse_unicode_environment(name, value)? {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(environment_diagnostic(name, "expected true or false")),
    }
}

fn environment_provenance(name: &str) -> PolicyProvenance {
    PolicyProvenance {
        source: PolicySourceKind::Environment,
        detail: name.to_owned(),
        path: None,
    }
}

fn canonical_setting(requested: &str) -> Option<&'static str> {
    match requested {
        "output.format" | "presentation_policy.output_format" => {
            Some("presentation_policy.output_format")
        }
        "state.directory" | "operational_policy.state_directory" => {
            Some("operational_policy.state_directory")
        }
        "scan.follow_symlinks" | "evidence_policy.follow_symlinks" => {
            Some("evidence_policy.follow_symlinks")
        }
        "scan.include_hidden" | "evidence_policy.include_hidden" => {
            Some("evidence_policy.include_hidden")
        }
        "scan.cross_filesystems" | "evidence_policy.cross_filesystems" => {
            Some("evidence_policy.cross_filesystems")
        }
        "scan.probe_media" | "evidence_policy.probe_media" => Some("evidence_policy.probe_media"),
        "evidence_policy.exact_grouping_profile" => Some("evidence_policy.exact_grouping_profile"),
        "evidence_policy.observation_stability" => Some("evidence_policy.observation_stability"),
        "evidence_policy.unstable_observations" => Some("evidence_policy.unstable_observations"),
        "evidence_policy.maximum_observation_attempts" => {
            Some("evidence_policy.maximum_observation_attempts")
        }
        "safety_invariants.source_mutation" => Some("safety_invariants.source_mutation"),
        "safety_invariants.apply_enabled" => Some("safety_invariants.apply_enabled"),
        "safety_invariants.shell_execution" => Some("safety_invariants.shell_execution"),
        "safety_invariants.artifact_validation_required" => {
            Some("safety_invariants.artifact_validation_required")
        }
        "safety_invariants.atomic_artifact_commit_required" => {
            Some("safety_invariants.atomic_artifact_commit_required")
        }
        "safety_invariants.current_stability_required_for_exact_groups" => {
            Some("safety_invariants.current_stability_required_for_exact_groups")
        }
        _ => None,
    }
}

#[allow(clippy::type_complexity)]
fn setting_metadata(
    setting: &str,
) -> (
    &'static str,
    bool,
    bool,
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
) {
    match setting {
        "presentation_policy.output_format" => (
            "presentation",
            false,
            false,
            "human or json",
            Some("OPTIFLOW_OUTPUT_FORMAT"),
            Some("--output-format / --json"),
        ),
        "operational_policy.state_directory" => (
            "operational",
            false,
            false,
            "non-empty native path",
            Some("OPTIFLOW_STATE_DIRECTORY"),
            Some("--state-directory"),
        ),
        "evidence_policy.follow_symlinks" => (
            "evidence",
            true,
            false,
            "boolean",
            Some("OPTIFLOW_FOLLOW_SYMLINKS"),
            Some("--follow-symlinks / --no-follow-symlinks"),
        ),
        "evidence_policy.include_hidden" => (
            "evidence",
            true,
            false,
            "boolean",
            Some("OPTIFLOW_INCLUDE_HIDDEN"),
            Some("--include-hidden / --exclude-hidden"),
        ),
        "evidence_policy.cross_filesystems" => (
            "evidence",
            true,
            false,
            "boolean",
            Some("OPTIFLOW_CROSS_FILESYSTEMS"),
            Some("--cross-filesystems / --stay-on-filesystem"),
        ),
        "evidence_policy.probe_media" => (
            "evidence",
            true,
            false,
            "boolean",
            Some("OPTIFLOW_PROBE_MEDIA"),
            Some("--probe / --no-probe"),
        ),
        setting if setting.starts_with("safety_invariants.") => {
            ("locked_safety", true, true, "not configurable", None, None)
        }
        _ => ("evidence", true, true, "not configurable", None, None),
    }
}

fn source_record(
    source: PolicySourceKind,
    status: ConfigurationSourceStatus,
    path: Option<&Path>,
) -> ConfigurationSourceRecord {
    ConfigurationSourceRecord {
        source,
        status,
        path: path.map(SerializedPath::from_path),
    }
}

fn diagnostic(code: DiagnosticCode, message: impl Into<String>) -> Box<Diagnostic> {
    Box::new(Diagnostic::new(
        code,
        DiagnosticSeverity::Error,
        DiagnosticClassification::Input,
        DiagnosticImpact::BlocksCommand,
        message,
    ))
}

fn internal_diagnostic(code: DiagnosticCode, message: impl Into<String>) -> Box<Diagnostic> {
    Box::new(Diagnostic::new(
        code,
        DiagnosticSeverity::Fatal,
        DiagnosticClassification::Internal,
        DiagnosticImpact::BlocksCommand,
        message,
    ))
}

fn path_diagnostic(
    code: DiagnosticCode,
    path: &Path,
    message: impl Into<String>,
) -> Box<Diagnostic> {
    Box::new(
        Diagnostic::new(
            code,
            DiagnosticSeverity::Error,
            DiagnosticClassification::Input,
            DiagnosticImpact::BlocksCommand,
            message,
        )
        .with_path(path),
    )
}

fn setting_diagnostic(
    code: DiagnosticCode,
    setting: &str,
    message: impl Into<String>,
) -> Box<Diagnostic> {
    let mut diagnostic = diagnostic(code, message);
    diagnostic.context.setting = Some(setting.to_owned());
    diagnostic
}

fn environment_diagnostic(name: &str, message: &str) -> Box<Diagnostic> {
    let mut diagnostic = diagnostic(
        DiagnosticCode::ConfigurationEnvironmentInvalid,
        format!("{name} is invalid: {message}"),
    );
    diagnostic.context.environment_variable = Some(name.to_owned());
    diagnostic
}

fn toml_diagnostic(path: &Path, text: &str, error: toml::de::Error) -> Box<Diagnostic> {
    let message = error.to_string();
    let code = if message.contains("unknown field") {
        DiagnosticCode::ConfigurationUnknownKey
    } else {
        DiagnosticCode::ConfigurationParseFailed
    };
    let mut diagnostic = path_diagnostic(
        code,
        path,
        format!("configuration TOML is invalid: {message}"),
    );
    if let Some(span) = error.span() {
        let prefix = &text.as_bytes()[..span.start.min(text.len())];
        diagnostic.context.line = Some(
            u64::try_from(prefix.iter().filter(|byte| **byte == b'\n').count() + 1)
                .unwrap_or(u64::MAX),
        );
        let column = prefix
            .iter()
            .rev()
            .take_while(|byte| **byte != b'\n')
            .count()
            + 1;
        diagnostic.context.column = Some(u64::try_from(column).unwrap_or(u64::MAX));
    }
    diagnostic
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_do_not_expand_shell_syntax() {
        let base = Path::new("/workspace/project");
        assert_eq!(
            resolve_relative(Path::new("$HOME/*.db"), base),
            Path::new("/workspace/project/$HOME/*.db")
        );
        assert_eq!(
            resolve_relative(Path::new("~/state"), base),
            Path::new("/workspace/project/~/state")
        );
    }

    #[test]
    fn boolean_environment_is_strict() {
        assert!(parse_boolean_environment("OPTIFLOW_TEST", &OsString::from("true")).unwrap());
        assert!(parse_boolean_environment("OPTIFLOW_TEST", &OsString::from("yes")).is_err());
    }
}
