mod document;
pub mod mp4;
mod policy;
mod resolver;

pub use document::{CONFIG_SCHEMA, ConfigDocumentV1};
pub use policy::{
    ConfigurationSourceRecord, ConfigurationSourceStatus, EffectivePolicyV1, PolicyDigest,
    PolicyProvenance, PolicySourceKind, RuntimePolicy, ShadowedValue, validate_fingerprints,
};
pub use resolver::{ConfigurationResolution, SettingExplanation, resolve};
