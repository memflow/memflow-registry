use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::PluginDescriptor;

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
