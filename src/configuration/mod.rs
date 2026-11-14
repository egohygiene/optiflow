mod document;
mod policy;
mod resolver;
pub mod mp4;

pub use document::{CONFIG_SCHEMA, ConfigDocumentV1};
pub use policy::{
    ConfigurationSourceRecord, ConfigurationSourceStatus, EffectivePolicyV1, PolicyDigest,
    PolicySourceKind, PolicyProvenance, RuntimePolicy, ShadowedValue, validate_fingerprints,
};
pub use resolver::{ConfigurationResolution, SettingExplanation, resolve};
