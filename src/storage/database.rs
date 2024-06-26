use std::{cmp::Reverse, collections::HashMap};

use chrono::NaiveDateTime;
use log::info;
use memflow::plugins::plugin_analyzer::{PluginArchitecture, PluginDescriptorInfo, PluginFileType};
use serde::{Deserialize, Serialize};

use crate::{error::Result, rest::models::PluginInfo};

use super::PluginMetadata;

const DEFAULT_PLUGIN_VARIANTS: usize = 5;
const MAX_PLUGIN_VARIANTS: usize = 50;

#[derive(Default)]
pub struct PluginDatabase {
    plugins: HashMap<String, Vec<PluginVariant>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PluginVariant {
    pub digest: String,
    pub signature: String,
    pub created_at: NaiveDateTime,
    pub descriptor: PluginDescriptorInfo,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PluginDatabaseFindParams {
    pub version: Option<String>,
    pub memflow_plugin_version: Option<i32>,
    pub file_type: Option<PluginFileType>,
    pub architecture: Option<PluginArchitecture>,

    // pagination parameters
    pub skip: Option<usize>,
    pub limit: Option<usize>,
}

impl PluginDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts all plugin variants of this file into the database
    pub fn insert_all(&mut self, metadata: &PluginMetadata) -> Result<()> {
        for descriptor in metadata.descriptors.iter() {
            info!(
                "adding plugin variant to db: digest={}; created_at={}; descriptor={:?}",
                metadata.digest, metadata.created_at, descriptor
            );

            let entry = self.plugins.entry(descriptor.name.clone()).or_default();

            // sort by plugin_version and created_at
            // metadata is guaranteed to contain at least one descriptor and the plugin_version is identical for all connectors of a file.
            let search_key = (
                metadata.descriptors.first().unwrap().plugin_version,
                metadata.created_at,
            );
            match entry.binary_search_by_key(&Reverse(search_key), |entry| {
                Reverse((entry.descriptor.plugin_version, entry.created_at))
            }) {
                Ok(_) => unreachable!(), // element already in vector @ `pos` // TODO: check for duplicate entries
                Err(pos) => entry.insert(
                    pos,
                    PluginVariant {
                        digest: metadata.digest.clone(),
                        signature: metadata.signature.clone(),
                        created_at: metadata.created_at,
                        descriptor: descriptor.clone(),
                    },
                ),
            }
        }

        Ok(())
    }

    /// Returns a list of all plugin names and their descriptions.
    pub fn plugins(&self) -> Vec<PluginInfo> {
        let mut plugins = self
            .plugins
            .iter()
            .flat_map(|(key, variants)| {
                variants.iter().map(|variant| PluginInfo {
                    name: key.to_owned(),
                    description: variant.descriptor.description.clone(),
                })
            })
            .collect::<Vec<_>>();
        plugins.sort_by(|a, b| a.name.cmp(&b.name));
        plugins.dedup_by(|a, b| a.name == b.name);
        plugins
    }

    /// Retrieves a specific digest
    #[allow(unused)]
    pub fn find_by_digest(&self, digest: &str) -> Option<PluginVariant> {
        self.plugins
            .iter()
            .find_map(|(_, variants)| variants.iter().find(|variant| variant.digest == digest))
            .cloned()
    }

    /// Removes all entries with the specified digest from the database
    pub fn delete_by_digest(&mut self, digest: &str) {
        for plugin in self.plugins.iter_mut() {
            plugin.1.retain(|variant| variant.digest != digest);
        }
    }

    /// Retrieves a list of variants for a specific plugin.
    /// Additional search parameters can be specified.
    pub fn plugin_variants(
        &self,
        plugin_name: &str,
        params: PluginDatabaseFindParams,
    ) -> Vec<PluginVariant> {
        self.plugins
            .get(plugin_name)
            .map(|variants| {
                variants
                    .iter()
                    .skip(params.skip.unwrap_or(0))
                    .filter(|p| p.descriptor.name == plugin_name)
                    .filter(|p| {
                        if let Some(version) = &params.version {
                            // version can match the version directly or the corresponding digest
                            if *version != p.descriptor.version
                                && *version != p.digest[..version.len()]
                            {
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

                        true
                    })
                    .take(
                        params
                            .limit
                            .unwrap_or(DEFAULT_PLUGIN_VARIANTS)
                            .min(MAX_PLUGIN_VARIANTS),
                    )
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }
}
