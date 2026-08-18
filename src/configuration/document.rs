use serde::{Deserialize, Serialize};

use crate::cli::OutputFormat;

pub const CONFIG_SCHEMA: &str = "optiflow.config.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigDocumentV1 {
    pub schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputConfigDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<StateConfigDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan: Option<ScanConfigDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputConfigDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateConfigDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScanConfigDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_symlinks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_hidden: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cross_filesystems: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe_media: Option<bool>,
}
