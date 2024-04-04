use chrono::NaiveDateTime;
use memflow::plugins::plugin_analyzer::PluginDescriptorInfo;
use serde::{Deserialize, Serialize};

/// Health status of the service
#[derive(Debug, Serialize, Deserialize)]
pub enum HealthResponse {
    Ok,
    Error(String),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PluginVariant {
    pub digest: String,
    pub signature: String,
    pub created_at: NaiveDateTime,
    pub descriptor: PluginDescriptorInfo,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PluginsAllResponse {
    pub plugins: Vec<PluginInfo>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PluginsFindResponse {
    pub plugins: Vec<PluginVariant>,
    pub skip: usize,
}

/// Result of an upload request
#[derive(Debug, Serialize, Deserialize)]
pub enum PluginUploadResponse {
    Added,
    AlreadyExists,
}
