use std::path::Path;
use std::sync::Arc;
use std::{cmp::Ordering, path::PathBuf};

use chrono::{NaiveDateTime, Utc};
use log::{info, warn};
use parking_lot::{lock_api::RwLockReadGuard, RawRwLock, RwLock};
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::error::{Error, Result};

pub mod plugin_analyzer;
use plugin_analyzer::{PluginArchitecture, PluginDescriptor, PluginFileType};

pub mod pki;
use pki::SignatureVerifier;

/// Metadata attached to each file
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginMetadata {
    // pub plugin: String,
    // // TODO: plugin type
    // pub tag: String,
    // TODO: do we need more?
    /// The sha256sum of the binary file
    pub digest: String,

    /// File signature of this binary
    pub signature: String,

    /// Timestamp at which the file was added
    pub created_at: NaiveDateTime,

    /// Timestamp at when this file was added
    // TODO: can we simply use file timestamp?

    /// The plugin descriptor
    pub descriptors: Vec<PluginDescriptor>,
}

///
#[derive(Clone)]
pub struct Storage {
    root: PathBuf,
    database: Arc<RwLock<PluginDatabase>>,
    signature_verifier: Option<SignatureVerifier>,
}

impl Storage {
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self> {
        // TODO: create path if not exists
        let mut database = PluginDatabase::new();

        let paths = std::fs::read_dir(&root).unwrap();
        for path in paths.filter_map(|p| p.ok()) {
            if let Some(extension) = path.path().extension() {
                if extension.to_str().unwrap_or_default() == "meta" {
                    let metadata: PluginMetadata =
                        serde_json::from_str(&std::fs::read_to_string(path.path()).unwrap())
                            .unwrap();
                    database.insert_all(&metadata)?;
                }
            }
        }

        Ok(Self {
            root: root.as_ref().to_path_buf(),
            database: Arc::new(RwLock::new(database)),
            signature_verifier: None,
        })
    }

    /// Adds the given SignatureVerifier to the file store.
    pub fn with_signature_verifier(mut self, verifier: SignatureVerifier) -> Self {
        self.signature_verifier = Some(verifier);
        self
    }

    /// Writes the specified connector into the path and adds it into the database.
    pub async fn upload(&self, bytes: &[u8], signature: &str) -> Result<()> {
        // TODO: what happens with old signatures in case we change the signing key?
        if let Some(verifier) = &self.signature_verifier {
            if let Err(err) = verifier.is_valid(bytes, signature) {
                warn!("invalid file signature for uploaded binary: {}", err);
                return Err(Error::Signature("file signature is invalid".to_owned()));
            }
        }

        // parse descriptors
        let descriptors = plugin_analyzer::parse_descriptors(bytes)?;

        // generate sha256 digest
        let digest = sha256::digest(bytes);

        // plugin path: {digest}.plugin
        let mut file_name = self.root.clone().join(&digest);
        file_name.set_extension("plugin");

        // check if digest is already existent
        if file_name.exists() {
            return Err(Error::AlreadyExists(
                "plugin with the same digest was already added".to_owned(),
            ));
        }

        // write plugin
        let mut plugin_file = File::create(&file_name).await?;
        plugin_file.write_all(bytes).await?;

        // metadata path: {digest}.meta
        let metadata = PluginMetadata {
            digest: digest.clone(),
            signature: signature.to_owned(),
            created_at: Utc::now().naive_utc(),
            descriptors: descriptors.clone(),
        };
        file_name.set_extension("meta");
        let mut metadata_file = File::create(&file_name).await?;
        metadata_file
            .write_all(serde_json::to_string(&metadata).unwrap().as_bytes())
            .await?;

        // add to database
        let mut database = self.database.write();
        database.insert_all(&metadata)?;

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
    digest: String,
    signature: String,
    created_at: NaiveDateTime,
    descriptor: PluginDescriptor,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PluginDatabaseFindParams {
    pub name: Option<String>,
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

    pub fn find(&self, params: PluginDatabaseFindParams) -> Vec<PluginEntry> {
        self.plugins
            .iter()
            .skip(params.skip.unwrap_or(0))
            .filter(|p| {
                if let Some(name) = &params.name {
                    if *name != p.descriptor.name {
                        return false;
                    }
                }

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
