use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{NaiveDateTime, Utc};
use log::warn;
use parking_lot::{lock_api::RwLockReadGuard, RawRwLock, RwLock};
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use memflow_registry_shared::{
    plugin_analyzer, Error, PluginDescriptor, Result, SignatureVerifier,
};

pub mod database;
use database::PluginDatabase;

/// Metadata attached to each file
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// The sha256sum of the binary file
    pub digest: String,
    /// File signature of this binary
    pub signature: String,
    /// Timestamp at which the file was added
    pub created_at: NaiveDateTime,
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

        let paths = std::fs::read_dir(&root)?;
        for path in paths.filter_map(|p| p.ok()) {
            if let Some(extension) = path.path().extension() {
                if extension.to_str().unwrap_or_default() == "meta" {
                    let metadata: PluginMetadata =
                        serde_json::from_str(&std::fs::read_to_string(path.path())?)?;
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
        let mut file_name = self.root.clone().join(digest);
        file_name.set_extension("plugin");
        Ok(File::open(&file_name).await?)
    }

    /// Deletes the file with the given digest from the database.
    pub async fn delete(&self, digest: &str) -> Result<()> {
        // check if file exists
        let mut file_name = self.root.clone().join(digest);
        file_name.set_extension("plugin");
        if !file_name.exists() {
            return Err(Error::NotFound("digest was not found".to_owned()));
        }

        // lock and remove from database
        {
            let mut database = self.database.write();
            database.delete_by_digest(digest);
        }

        // try to remove the file
        tokio::fs::remove_file(file_name).await?;

        Ok(())
    }

    #[inline]
    pub fn database(&self) -> RwLockReadGuard<RawRwLock, PluginDatabase> {
        self.database.read()
    }
}