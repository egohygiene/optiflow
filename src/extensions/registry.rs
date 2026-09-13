use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use super::catalog::{LoadedExtension, validate_invocation, validate_result};
use super::model::{
    CapabilityKind, ExtensionInvocationV1, ExtensionResultV1, LifecycleEvent, ProgressEvent,
    TrustMode,
};

/// Host services exposed to an embedded extension invocation.
pub struct ExtensionContext<'a> {
    pub invocation: &'a ExtensionInvocationV1,
    is_cancelled: &'a dyn Fn() -> bool,
    progress: &'a dyn Fn(&ProgressEvent),
}

impl<'a> ExtensionContext<'a> {
    pub fn new(
        invocation: &'a ExtensionInvocationV1,
        is_cancelled: &'a dyn Fn() -> bool,
        progress: &'a dyn Fn(&ProgressEvent),
    ) -> Self {
        Self {
            invocation,
            is_cancelled,
            progress,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        (self.is_cancelled)()
    }

    pub fn report_progress(&self, event: &ProgressEvent) {
        (self.progress)(event);
    }
}

/// A typed extension failure which has not crossed the result acceptance boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl ExtensionFailure {
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

impl fmt::Display for ExtensionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ExtensionFailure {}

pub trait Inspector: Send + Sync {
    fn inspect(
        &self,
        context: &ExtensionContext<'_>,
    ) -> Result<ExtensionResultV1, ExtensionFailure>;
}

pub trait Analyzer: Send + Sync {
    fn analyze(
        &self,
        context: &ExtensionContext<'_>,
    ) -> Result<ExtensionResultV1, ExtensionFailure>;
}

pub trait PolicyContributor: Send + Sync {
    fn contribute_policy(
        &self,
        context: &ExtensionContext<'_>,
    ) -> Result<ExtensionResultV1, ExtensionFailure>;
}

/// Marker for policy contributors that emit normalization evidence.
pub trait NormalizationPolicyContributor: PolicyContributor {}

impl<T: PolicyContributor + ?Sized> NormalizationPolicyContributor for T {}

pub trait Planner: Send + Sync {
    fn plan(&self, context: &ExtensionContext<'_>) -> Result<ExtensionResultV1, ExtensionFailure>;
}

pub trait Validator: Send + Sync {
    fn validate(
        &self,
        context: &ExtensionContext<'_>,
    ) -> Result<ExtensionResultV1, ExtensionFailure>;
}

pub trait ReportProvider: Send + Sync {
    fn report(&self, context: &ExtensionContext<'_>)
    -> Result<ExtensionResultV1, ExtensionFailure>;
}

/// Marker for report providers whose report is an export projection.
pub trait ExportProvider: ReportProvider {}

impl<T: ReportProvider + ?Sized> ExportProvider for T {}

/// Lifecycle observers receive immutable event data and cannot return contributions.
pub trait LifecycleObserver: Send + Sync {
    fn observe(
        &self,
        context: &ExtensionContext<'_>,
        event: &LifecycleEvent,
    ) -> Result<(), ExtensionFailure>;
}

#[derive(Clone)]
pub enum EmbeddedExtension {
    Inspector(Arc<dyn Inspector>),
    Analyzer(Arc<dyn Analyzer>),
    PolicyContributor(Arc<dyn PolicyContributor>),
    Planner(Arc<dyn Planner>),
    Validator(Arc<dyn Validator>),
    ReportProvider(Arc<dyn ReportProvider>),
    LifecycleObserver(Arc<dyn LifecycleObserver>),
}

impl EmbeddedExtension {
    fn kind(&self) -> CapabilityKind {
        match self {
            Self::Inspector(_) => CapabilityKind::Inspector,
            Self::Analyzer(_) => CapabilityKind::Analyzer,
            Self::PolicyContributor(_) => CapabilityKind::PolicyContributor,
            Self::Planner(_) => CapabilityKind::Planner,
            Self::Validator(_) => CapabilityKind::Validator,
            Self::ReportProvider(_) => CapabilityKind::ReportProvider,
            Self::LifecycleObserver(_) => CapabilityKind::LifecycleObserver,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    InvalidRegistration {
        capability_id: String,
        message: String,
    },
    DuplicateRegistration {
        capability_id: String,
    },
    MissingRegistration {
        capability_id: String,
    },
    InvocationRejected {
        capability_id: String,
        message: String,
    },
    HandlerFailed {
        capability_id: String,
        failure: ExtensionFailure,
    },
    ResultRejected {
        capability_id: String,
        message: String,
    },
    Cancelled {
        capability_id: String,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRegistration {
                capability_id,
                message,
            } => write!(formatter, "cannot register {capability_id}: {message}"),
            Self::DuplicateRegistration { capability_id } => {
                write!(
                    formatter,
                    "capability {capability_id} is already registered"
                )
            }
            Self::MissingRegistration { capability_id } => {
                write!(formatter, "capability {capability_id} is not registered")
            }
            Self::InvocationRejected {
                capability_id,
                message,
            } => write!(
                formatter,
                "invocation for {capability_id} was rejected: {message}"
            ),
            Self::HandlerFailed {
                capability_id,
                failure,
            } => write!(
                formatter,
                "extension handler {capability_id} failed: {failure}"
            ),
            Self::ResultRejected {
                capability_id,
                message,
            } => write!(
                formatter,
                "result from {capability_id} was rejected: {message}"
            ),
            Self::Cancelled { capability_id } => {
                write!(
                    formatter,
                    "extension invocation {capability_id} was cancelled"
                )
            }
        }
    }
}

impl Error for RegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::HandlerFailed { failure, .. } => Some(failure),
            _ => None,
        }
    }
}

/// Explicit, typed registry for one locked embedded extension.
pub struct ExtensionRegistry {
    extension: LoadedExtension,
    handlers: BTreeMap<String, EmbeddedExtension>,
}

impl ExtensionRegistry {
    pub fn new(extension: LoadedExtension) -> Result<Self, RegistryError> {
        if extension.lock.trust != TrustMode::TrustedEmbedded {
            return Err(RegistryError::InvalidRegistration {
                capability_id: extension.manifest.extension_id.clone(),
                message: "the operator lock does not grant trusted embedded execution".to_owned(),
            });
        }
        if !extension.availability_reasons.is_empty() {
            return Err(RegistryError::InvalidRegistration {
                capability_id: extension.manifest.extension_id.clone(),
                message: extension.availability_reasons.join("; "),
            });
        }
        Ok(Self {
            extension,
            handlers: BTreeMap::new(),
        })
    }

    pub fn extension(&self) -> &LoadedExtension {
        &self.extension
    }

    pub fn register_inspector(
        &mut self,
        capability_id: impl Into<String>,
        handler: Arc<dyn Inspector>,
    ) -> Result<(), RegistryError> {
        self.register(capability_id.into(), EmbeddedExtension::Inspector(handler))
    }

    pub fn register_analyzer(
        &mut self,
        capability_id: impl Into<String>,
        handler: Arc<dyn Analyzer>,
    ) -> Result<(), RegistryError> {
        self.register(capability_id.into(), EmbeddedExtension::Analyzer(handler))
    }

    pub fn register_policy_contributor(
        &mut self,
        capability_id: impl Into<String>,
        handler: Arc<dyn PolicyContributor>,
    ) -> Result<(), RegistryError> {
        self.register(
            capability_id.into(),
            EmbeddedExtension::PolicyContributor(handler),
        )
    }

    pub fn register_planner(
        &mut self,
        capability_id: impl Into<String>,
        handler: Arc<dyn Planner>,
    ) -> Result<(), RegistryError> {
        self.register(capability_id.into(), EmbeddedExtension::Planner(handler))
    }

    pub fn register_validator(
        &mut self,
        capability_id: impl Into<String>,
        handler: Arc<dyn Validator>,
    ) -> Result<(), RegistryError> {
        self.register(capability_id.into(), EmbeddedExtension::Validator(handler))
    }

    pub fn register_report_provider(
        &mut self,
        capability_id: impl Into<String>,
        handler: Arc<dyn ReportProvider>,
    ) -> Result<(), RegistryError> {
        self.register(
            capability_id.into(),
            EmbeddedExtension::ReportProvider(handler),
        )
    }

    pub fn register_lifecycle_observer(
        &mut self,
        capability_id: impl Into<String>,
        handler: Arc<dyn LifecycleObserver>,
    ) -> Result<(), RegistryError> {
        self.register(
            capability_id.into(),
            EmbeddedExtension::LifecycleObserver(handler),
        )
    }

    pub fn invoke(
        &self,
        context: &ExtensionContext<'_>,
    ) -> Result<ExtensionResultV1, RegistryError> {
        let capability_id = context.invocation.capability_id.clone();
        validate_invocation(&self.extension, context.invocation).map_err(|message| {
            RegistryError::InvocationRejected {
                capability_id: capability_id.clone(),
                message,
            }
        })?;
        if context.is_cancelled() {
            return Err(RegistryError::Cancelled { capability_id });
        }
        let handler = self.handlers.get(&capability_id).ok_or_else(|| {
            RegistryError::MissingRegistration {
                capability_id: capability_id.clone(),
            }
        })?;
        let result = catch_unwind(AssertUnwindSafe(|| match handler {
            EmbeddedExtension::Inspector(handler) => handler.inspect(context),
            EmbeddedExtension::Analyzer(handler) => handler.analyze(context),
            EmbeddedExtension::PolicyContributor(handler) => handler.contribute_policy(context),
            EmbeddedExtension::Planner(handler) => handler.plan(context),
            EmbeddedExtension::Validator(handler) => handler.validate(context),
            EmbeddedExtension::ReportProvider(handler) => handler.report(context),
            EmbeddedExtension::LifecycleObserver(_) => Err(ExtensionFailure::new(
                "invalid_observer_invocation",
                "lifecycle observers must be invoked through observe",
                false,
            )),
        }))
        .map_err(|_| RegistryError::HandlerFailed {
            capability_id: capability_id.clone(),
            failure: ExtensionFailure::new(
                "extension_panicked",
                "embedded extension panicked; no result was accepted",
                false,
            ),
        })?
        .map_err(|failure| RegistryError::HandlerFailed {
            capability_id: capability_id.clone(),
            failure,
        })?;
        if context.is_cancelled() {
            return Err(RegistryError::Cancelled { capability_id });
        }
        validate_result(&self.extension, context.invocation, &result).map_err(|message| {
            RegistryError::ResultRejected {
                capability_id,
                message,
            }
        })?;
        Ok(result)
    }

    pub fn observe(
        &self,
        context: &ExtensionContext<'_>,
        event: &LifecycleEvent,
    ) -> Result<(), RegistryError> {
        let capability_id = context.invocation.capability_id.clone();
        validate_invocation(&self.extension, context.invocation).map_err(|message| {
            RegistryError::InvocationRejected {
                capability_id: capability_id.clone(),
                message,
            }
        })?;
        if event.invocation_id != context.invocation.invocation_id
            || context.invocation.lifecycle_event.as_ref() != Some(event)
        {
            return Err(RegistryError::InvocationRejected {
                capability_id,
                message: "lifecycle event is not bound to the invocation".to_owned(),
            });
        }
        if context.is_cancelled() {
            return Err(RegistryError::Cancelled { capability_id });
        }
        let handler = self.handlers.get(&capability_id).ok_or_else(|| {
            RegistryError::MissingRegistration {
                capability_id: capability_id.clone(),
            }
        })?;
        let EmbeddedExtension::LifecycleObserver(handler) = handler else {
            return Err(RegistryError::InvalidRegistration {
                capability_id,
                message: "only a lifecycle observer can receive lifecycle events".to_owned(),
            });
        };
        let declared = self
            .extension
            .manifest
            .observer_hooks
            .iter()
            .any(|hook| hook.read_only && hook.lifecycle_points.contains(&event.phase));
        if !declared {
            return Err(RegistryError::InvocationRejected {
                capability_id,
                message: "the lifecycle phase is not declared by a read-only hook".to_owned(),
            });
        }
        catch_unwind(AssertUnwindSafe(|| handler.observe(context, event)))
            .map_err(|_| RegistryError::HandlerFailed {
                capability_id: capability_id.clone(),
                failure: ExtensionFailure::new(
                    "extension_panicked",
                    "embedded lifecycle observer panicked",
                    false,
                ),
            })?
            .map_err(|failure| RegistryError::HandlerFailed {
                capability_id,
                failure,
            })
    }

    fn register(
        &mut self,
        capability_id: String,
        handler: EmbeddedExtension,
    ) -> Result<(), RegistryError> {
        let Some(capability) = self.extension.capability(&capability_id) else {
            return Err(RegistryError::InvalidRegistration {
                capability_id,
                message: "the capability is not declared in the pinned manifest".to_owned(),
            });
        };
        if capability.kind != handler.kind() {
            return Err(RegistryError::InvalidRegistration {
                capability_id,
                message: "the typed handler role does not match the manifest".to_owned(),
            });
        }
        if !self.extension.is_available_for(capability) {
            return Err(RegistryError::InvalidRegistration {
                capability_id,
                message: "the capability is not enabled, configured, and authorized".to_owned(),
            });
        }
        if self.handlers.contains_key(&capability_id) {
            return Err(RegistryError::DuplicateRegistration { capability_id });
        }
        self.handlers.insert(capability_id, handler);
        Ok(())
    }
}
