use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use crate::error::Result;

pub mod plugin_analyzer;
use plugin_analyzer::PluginDescriptor;

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
    database: PluginDatabase,
}

impl Storage {
    pub fn new() -> Self {
        // TODO: re-index all files
        // TODO: create path if not exists
        let paths = std::fs::read_dir("./.storage").unwrap();
        /*for path in paths.filter_map(|p| p.ok()) {
            println!("parsing file: {:?}", path.path());
            // TODO: filter by filename
            // TODO: filter by size
            let bytes = std::fs::read(path.path()).unwrap();

            let descriptors = plugin_analyzer::parse_descriptors(&bytes[..]).unwrap();
            for descriptor in descriptors.iter() {
                if descriptor.version.is_empty() {
                    panic!();
                }
                println!("architecture: {:?}", descriptor.architecture);
                println!("plugin_version: {}", descriptor.plugin_version);
                println!("name: {}", descriptor.name);
                println!("version: {}", descriptor.version);
                println!("description: {}", descriptor.description);
            }
        }*/

        Self {
            root: "./.storage".into(),
            database: PluginDatabase::new(),
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
        let metadata = PluginMetadata { descriptors };
        file_name.set_extension("meta");
        let mut metadata_file = File::create(&file_name).await?;
        let n = metadata_file
            .write(serde_json::to_string(&metadata).unwrap().as_bytes())
            .await?;
        println!("Wrote the first {} bytes of 'some bytes'.", n);

        // add to database

        Ok(())
    }
}

#[derive(Clone)]
struct PluginDatabase {}

impl PluginDatabase {
    pub fn new() -> Self {
        Self {}
    }
}
