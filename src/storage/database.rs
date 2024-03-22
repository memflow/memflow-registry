use std::cmp::Ordering;

use chrono::NaiveDateTime;
use log::info;
use serde::{Deserialize, Serialize};

use crate::error::Result;

use super::{
    plugin_analyzer::{PluginArchitecture, PluginDescriptor, PluginFileType},
    PluginMetadata,
};

pub struct PluginDatabase {
    plugins: Vec<PluginEntry>,
}

#[derive(Clone, Serialize)]
pub struct PluginEntry {
    pub digest: String,
    pub signature: String,
    pub created_at: NaiveDateTime,
    pub descriptor: PluginDescriptor,
}

#[derive(Clone, Serialize)]
pub struct PluginName {
    name: String,
    description: String,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PluginDatabaseFindParams {
    pub version: Option<String>,
    pub memflow_plugin_version: Option<i32>,
    pub file_type: Option<PluginFileType>,
    pub architecture: Option<PluginArchitecture>,
    pub digest: Option<String>,
    pub digest_short: Option<String>,

    // pagination parameters
    pub skip: Option<usize>,
    pub limit: Option<usize>,
}

impl PluginDatabase {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Inserts all plugin variants of this file into the database
    pub fn insert_all(&mut self, metadata: &PluginMetadata) -> Result<()> {
        for descriptor in metadata.descriptors.iter() {
            info!(
                "adding plugin variant to db: digest={}; created_at={}; descriptor={:?}",
                metadata.digest, metadata.created_at, descriptor
            );

            // TODO: check for duplicate entries?
            self.plugins.push(PluginEntry {
                digest: metadata.digest.clone(),
                signature: metadata.signature.clone(),
                created_at: metadata.created_at,
                descriptor: descriptor.clone(),
            });
        }

        // sort plugins by created_at timestamp to show the newest ones first
        self.plugins.sort_by(|a, b| {
            b.created_at
                .partial_cmp(&a.created_at)
                .unwrap_or(Ordering::Equal)
        });

        Ok(())
    }

    /// Returns a list of all plugin names and their descriptions.
    pub fn plugins(&self) -> Vec<PluginName> {
        let mut plugins = self
            .plugins
            .iter()
            .map(|entry| PluginName {
                name: entry.descriptor.name.clone(),
                description: entry.descriptor.description.clone(),
            })
            .collect::<Vec<_>>();
        plugins.sort_by(|a, b| a.name.cmp(&b.name));
        plugins.dedup_by(|a, b| a.name == b.name);
        plugins
    }

    /// Retrieves a specific digest
    pub fn find_by_digest(&self, digest: &str) -> Option<PluginEntry> {
        self.plugins.iter().find(|p| p.digest == digest).cloned()
    }

    /// Retrieves a list of variants for a specific plugin.
    /// Additional search parameters can be specified.
    pub fn plugin_variants(
        &self,
        plugin_name: &str,
        params: PluginDatabaseFindParams,
    ) -> Vec<PluginEntry> {
        self.plugins
            .iter()
            .skip(params.skip.unwrap_or(0))
            .filter(|p| p.descriptor.name == plugin_name)
            .filter(|p| {
                if let Some(version) = &params.version {
                    if *version != p.descriptor.version {
                        return false;
                    }
                }

                if let Some(memflow_plugin_version) = params.memflow_plugin_version {
                    if memflow_plugin_version != p.descriptor.plugin_version {
                        return false;
                    }
                }

                if let Some(file_type) = params.file_type {
                    if file_type != p.descriptor.file_type {
                        return false;
                    }
                }

                if let Some(architecture) = params.architecture {
                    if architecture != p.descriptor.architecture {
                        return false;
                    }
                }

                if let Some(digest) = &params.digest {
                    if *digest != p.digest {
                        return false;
                    }
                }

                if let Some(digest_short) = &params.digest_short {
                    if *digest_short != p.digest[..7] {
                        return false;
                    }
                }

                true
            })
            .take(params.limit.unwrap_or(50).min(50))
            .cloned()
            .collect::<Vec<_>>()
    }
}
