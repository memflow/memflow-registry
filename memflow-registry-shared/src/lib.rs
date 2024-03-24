pub const MEMFLOW_DEFAULT_REGISTRY: &str = "https://registry.memflow.io";

pub mod error;
pub mod pki;
pub mod plugin_analyzer;
pub mod plugin_uri;
pub mod structs;

pub use error::{Error, Result};
pub use pki::SignatureVerifier;
pub use plugin_analyzer::{PluginArchitecture, PluginDescriptor, PluginFileType};
pub use plugin_uri::PluginUri;
pub use structs::{PluginInfo, PluginVariant, PluginsAllResponse};
