use serde::{Deserialize, Serialize};

use crate::storage::database::PluginVariant;

#[derive(Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub description: String,
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
