//! Safe extension SDK and explicit provider boundary.
//!
//! Extensions are inert declarations until an operator lock selects exact
//! provider bytes and grants a subset of their requested effects. Embedded
//! providers register typed handlers. Process providers are invoked only by
//! explicit absolute path through the bounded subprocess runner.

mod catalog;
mod model;
mod process;
mod registry;

pub use catalog::{
    CatalogError, ExtensionCatalog, ExtensionDoctorReport, ExtensionInspection, ExtensionListEntry,
    ExtensionResolution, ExtensionResolutionStatus, LoadedExtension,
};
pub use model::*;
pub use process::{ProcessExtensionClient, ProcessExtensionError};
pub use registry::{
    Analyzer, EmbeddedExtension, ExportProvider, ExtensionContext, ExtensionFailure,
    ExtensionRegistry, Inspector, LifecycleObserver, NormalizationPolicyContributor, Planner,
    PolicyContributor, RegistryError, ReportProvider, Validator,
};
