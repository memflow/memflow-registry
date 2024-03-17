use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use crate::error::Result;

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
                    println!("parsing file: {:?}", path.path());
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
        let id = Uuid::new_v4();

        // write plugin
        let mut file_name = self.root.clone().join(id.to_string());
        file_name.set_extension("plugin");
        let mut plugin_file = File::create(&file_name).await?;
        let n = plugin_file.write(bytes).await?;
        println!("Wrote the first {} bytes of 'some bytes'.", n);

        // write metadata
        let metadata = PluginMetadata {
            descriptors: descriptors.clone(),
        };
        file_name.set_extension("meta");
        let mut metadata_file = File::create(&file_name).await?;
        let n = metadata_file
            .write(serde_json::to_string(&metadata).unwrap().as_bytes())
            .await?;
        println!("Wrote the first {} bytes of 'some bytes'.", n);

        // add to database
        let mut database = self.database.write();
        for descriptor in descriptors.iter() {
            database.insert(descriptor, &id.to_string())?;
        }

        Ok(())
    }
}

struct PluginDatabase {
    // plugin_name -> plugin_versions -> os -> architectures -> info
    plugins: HashMap<String, PluginVersions>,
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
    architectures: HashMap<PluginArchitecture, PluginInfo>,
}

// TODO: multiple tags/versions
struct PluginInfo {
    descriptor: PluginDescriptor,
    file_name: String,
}

impl PluginDatabase {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn insert(&mut self, descriptor: &PluginDescriptor, file_name: &str) -> Result<()> {
        let plugin_versions = self.plugins.entry(descriptor.name.clone()).or_default();
        let plugin_type = plugin_versions
            .versions
            .entry(descriptor.plugin_version)
            .or_default();
        let plugin_architecture = plugin_type.types.entry(descriptor.file_type).or_default();
        let plugin_info = plugin_architecture
            .architectures
            .entry(descriptor.architecture)
            .or_insert_with(|| PluginInfo {
                descriptor: descriptor.clone(),
                file_name: file_name.to_owned(),
            });
        Ok(())
    }
}
