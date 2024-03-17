use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use log::{info, warn};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::error::{Error, Result};

pub mod plugin_analyzer;
use plugin_analyzer::PluginDescriptor;

use self::plugin_analyzer::{PluginArchitecture, PluginFileType};

/// Metadata attached to each file
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginMetadata {
    // pub plugin: String,
    // // TODO: plugin type
    // pub tag: String,
    // TODO: do we need more?
    /// The sha256sum of the binary file
    pub digest: String,

    /// The plugin descriptor
    pub descriptors: Vec<PluginDescriptor>,
}

///
#[derive(Clone)]
pub struct Storage {
    root: PathBuf,
    database: Arc<RwLock<PluginDatabase>>,
}

impl Storage {
    pub fn new() -> Self {
        // TODO: create path if not exists
        let mut database = PluginDatabase::new();

        let paths = std::fs::read_dir("./.storage").unwrap();
        for path in paths.filter_map(|p| p.ok()) {
            if let Some(extension) = path.path().extension() {
                if extension.to_str().unwrap_or_default() == "meta" {
                    let metadata: PluginMetadata =
                        serde_json::from_str(&std::fs::read_to_string(path.path()).unwrap())
                            .unwrap();
                    for descriptor in metadata.descriptors.iter() {
                        database
                            .insert(
                                descriptor,
                                path.path().file_stem().unwrap().to_str().unwrap(),
                            )
                            .unwrap();
                    }
                }
            }
        }

        Self {
            root: "./.storage".into(),
            database: Arc::new(RwLock::new(database)),
        }
    }

    /// Writes the specified connector into the path and adds it into the database.
    pub async fn upload(&self, bytes: &[u8]) -> Result<()> {
        let descriptors = plugin_analyzer::parse_descriptors(bytes)?;

        // TODO: reuse original file extension
        let digest = sha256::digest(bytes);

        // TODO: check if digest exist, so we do not add duplicate files

        // write plugin
        let mut file_name = self.root.clone().join(&digest);
        file_name.set_extension("plugin");
        let mut plugin_file = File::create(&file_name).await?;
        plugin_file.write_all(bytes).await?;

        // write metadata
        let metadata = PluginMetadata {
            digest: digest.clone(),
            descriptors: descriptors.clone(),
        };
        file_name.set_extension("meta");
        let mut metadata_file = File::create(&file_name).await?;
        metadata_file
            .write_all(serde_json::to_string(&metadata).unwrap().as_bytes())
            .await?;

        // add to database
        let mut database = self.database.write();
        for descriptor in descriptors.iter() {
            database.insert(descriptor, &digest)?;
        }

        Ok(())
    }

    pub async fn download(
        &self,
        plugin_name: &str,
        plugin_version: i32,
        file_type: &PluginFileType,
        architecture: &PluginArchitecture,
        tag: &str,
    ) -> Result<File> {
        let plugin_info = {
            let lock = self.database.read();
            lock.find(plugin_name, plugin_version, file_type, architecture, tag)
                .ok_or_else(|| Error::NotFound("plugin not found".to_owned()))?
                .clone()
        };

        let mut file_name = self.root.clone().join(&plugin_info.digest);
        file_name.set_extension("plugin");
        Ok(File::open(&file_name).await?)
    }
}

struct PluginDatabase {
    // plugin_name -> plugin_versions -> os -> architectures -> tags -> info
    plugins_by_name: HashMap<String, PluginVersions>,
    plugins_by_digest: HashMap<String, PluginInfo>,
}

#[derive(Default)]
struct PluginVersions {
    versions: HashMap<i32, PluginTypes>,
}

#[derive(Default)]
struct PluginTypes {
    types: HashMap<PluginFileType, PluginArchitectures>,
}

#[derive(Default)]
struct PluginArchitectures {
    architectures: HashMap<PluginArchitecture, PluginTags>,
}

#[derive(Default)]
struct PluginTags {
    tags: HashMap<String, PluginInfo>,
}

// TODO: multiple tags/versions
#[derive(Clone)]
struct PluginInfo {
    pub descriptor: PluginDescriptor,
    pub digest: String,
}

impl PluginDatabase {
    pub fn new() -> Self {
        Self {
            plugins_by_name: HashMap::new(),
            plugins_by_digest: HashMap::new(),
        }
    }

    pub fn insert(&mut self, descriptor: &PluginDescriptor, digest: &str) -> Result<()> {
        info!(
            "adding plugin variant to db: descriptor={:?}, digest={}",
            descriptor, digest
        );

        let plugin_versions = self
            .plugins_by_name
            .entry(descriptor.name.clone())
            .or_default();
        let plugin_type = plugin_versions
            .versions
            .entry(descriptor.plugin_version)
            .or_default();
        let plugin_architecture = plugin_type.types.entry(descriptor.file_type).or_default();
        let plugin_tags = plugin_architecture
            .architectures
            .entry(descriptor.architecture)
            .or_default();

        let plugin_info = PluginInfo {
            descriptor: descriptor.clone(),
            digest: digest.to_owned(),
        };

        let digest_short = &digest[..7];

        let mut replace = false;

        replace = replace
            || plugin_tags
                .tags
                .insert("latest".to_owned(), plugin_info.clone())
                .is_some();
        replace = replace
            || plugin_tags
                .tags
                .insert(digest_short.to_owned(), plugin_info.clone())
                .is_some();
        replace = replace
            || self
                .plugins_by_digest
                .insert(digest.to_owned(), plugin_info)
                .is_some();

        if replace {
            warn!("replaced previous connector release");
        }

        Ok(())
    }

    fn find(
        &self,
        plugin_name: &str,
        plugin_version: i32,
        file_type: &PluginFileType,
        architecture: &PluginArchitecture,
        tag: &str,
    ) -> Option<&PluginInfo> {
        let plugin_versions = self.plugins_by_name.get(plugin_name)?;
        let plugin_type = plugin_versions.versions.get(&plugin_version)?;
        let plugin_architecture = plugin_type.types.get(file_type)?;
        let plugin_tags = plugin_architecture.architectures.get(architecture)?;
        plugin_tags.tags.get(tag)
    }
}
