use std::{cmp::Ordering, collections::HashMap};

use log::info;
use serde::Deserialize;

use memflow_registry_shared::{
    PluginArchitecture, PluginFileType, PluginInfo, PluginVariant, Result,
};

use super::PluginMetadata;

const DEFAULT_PLUGIN_VARIANTS: usize = 5;
const MAX_PLUGIN_VARIANTS: usize = 50;

pub struct PluginDatabase {
    plugins: HashMap<String, Vec<PluginVariant>>,
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
            plugins: HashMap::new(),
        }
    }

    /// Inserts all plugin variants of this file into the database
    pub fn insert_all(&mut self, metadata: &PluginMetadata) -> Result<()> {
        for descriptor in metadata.descriptors.iter() {
            info!(
                "adding plugin variant to db: digest={}; created_at={}; descriptor={:?}",
                metadata.digest, metadata.created_at, descriptor
            );

            let entry = self.plugins.entry(descriptor.name.clone()).or_default();

            // sort by plugin_version first, show the highest number first
            // if plugin_version is equal, sort by created_at timestamp to show the newest ones first
            let search_by_plugin_version_and_created_at = |entry: &PluginVariant| {
                // Metadata is guaranteed to contain at least one descriptor and the plugin_version is identical for all connectors of a file.
                let plugin_version = metadata
                    .descriptors
                    .first()
                    .unwrap()
                    .plugin_version
                    .cmp(&entry.descriptor.plugin_version);
                if plugin_version == Ordering::Equal {
                    metadata.created_at.cmp(&entry.created_at)
                } else {
                    plugin_version
                }
            };

            match entry.binary_search_by(search_by_plugin_version_and_created_at) {
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
