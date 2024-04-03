pub const MEMFLOW_DEFAULT_REGISTRY: &str = "https://registry.memflow.io";
pub const MEMFLOW_DEFAULT_REGISTRY_VERIFYING_KEY: &str = "-----BEGIN PUBLIC KEY-----
MFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEwFb+pnLXeLpW3n1utc0a18PfnV1fMaxq
wt3vkyIaLMfMcmIUISq9c51CsNLKPYxzOGS7PW3Nyd0NQWjCvR74mQ==
-----END PUBLIC KEY-----";

pub mod error;
pub mod pki;
pub mod plugin_uri;
pub mod structs;

pub use error::{Error, Result};
pub use pki::{SignatureGenerator, SignatureVerifier};
pub use plugin_uri::PluginUri;
pub use structs::{PluginInfo, PluginVariant, PluginsAllResponse};
