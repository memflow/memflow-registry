use std::path::PathBuf;
use std::sync::Arc;

use log::info;
use parking_lot::{lock_api::RwLockReadGuard, RawRwLock, RwLock};
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

    pub async fn download(&self, digest: &str) -> Result<File> {
        let plugin_info = {
            let lock = self.database.read();
            lock.find(PluginDatabaseFindParams {
                digest: Some(digest.to_owned()),
                limit: Some(1),
                ..Default::default()
            })
            .first()
            .ok_or_else(|| Error::NotFound("plugin not found".to_owned()))?
            .to_owned()
        };

        let mut file_name = self.root.clone().join(&plugin_info.digest);
        file_name.set_extension("plugin");
        Ok(File::open(&file_name).await?)
    }

    #[inline]
    pub fn database(&self) -> RwLockReadGuard<RawRwLock, PluginDatabase> {
        self.database.read()
    }
}

pub struct PluginDatabase {
    plugins: Vec<PluginEntry>,
}

#[derive(Clone, Serialize)]
pub struct PluginEntry {
    descriptor: PluginDescriptor,
    digest: String,
    tag: String,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PluginDatabaseFindParams {
    pub plugin_name: Option<String>,
    pub plugin_version: Option<i32>,
    pub file_type: Option<PluginFileType>,
    pub architecture: Option<PluginArchitecture>,
    pub digest: Option<String>,
    pub tag: Option<String>,

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

    pub fn insert(&mut self, descriptor: &PluginDescriptor, digest: &str) -> Result<()> {
        info!(
            "adding plugin variant to db: descriptor={:?}, digest={}",
            descriptor, digest
        );

        let digest_short = &digest[..7];

        // TODO: check for duplicate entries?
        self.plugins.push(PluginEntry {
            descriptor: descriptor.clone(),
            digest: digest.to_owned(),
            tag: digest_short.to_owned(),
        });

        Ok(())
    }

    pub fn find(&self, params: PluginDatabaseFindParams) -> Vec<PluginEntry> {
        self.plugins
            .iter()
            .skip(params.skip.unwrap_or(0))
            .filter(|p| {
                if let Some(plugin_name) = &params.plugin_name {
                    if *plugin_name != p.descriptor.name {
                        return false;
                    }
                }

                if let Some(plugin_version) = params.plugin_version {
                    if plugin_version != p.descriptor.plugin_version {
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

                if let Some(tag) = &params.tag {
                    if *tag != p.tag {
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
