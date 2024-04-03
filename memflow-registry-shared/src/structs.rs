use chrono::NaiveDateTime;
use memflow::plugins::plugin_analyzer::PluginDescriptor;
use serde::{Deserialize, Serialize};

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
    pub descriptor: PluginDescriptor,
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
