use std::{fmt::Display, str::FromStr};

use crate::{
    default_registry::MEMFLOW_DEFAULT_REGISTRY,
    error::{Error, Result},
};

/// Parses a plugin string into it's path components
///
/// # Supported plugin path formats:
///
/// `coredump` - will just pull latest
/// `coredump:latest` - will also pull latest
/// `coredump:0.2.0` - will pull the newest binary with this specific version
/// `memflow.registry.io/coredump` - pulls from another registry
pub struct PluginUri {
    registry: String,
    image: String,
    version: String,
}

#[allow(unused)]
impl PluginUri {
    pub fn new(plugin_uri: &str) -> Result<Self> {
        Self::with_defaults(plugin_uri, MEMFLOW_DEFAULT_REGISTRY, "latest")
    }

    pub fn with_defaults(
        plugin_uri: &str,
        default_registry: &str,
        default_version: &str,
    ) -> Result<Self> {
        // split up registry and image
        let image = plugin_uri.split('/').collect::<Vec<_>>();
        let (registry, image) = if let Some((image, registry)) = image.split_last() {
            let registry = if !registry.is_empty() {
                // TODO parse url
                Some(registry.join("/"))
            } else {
                None
            };

            (registry, image)
        } else {
            (None, &plugin_uri)
        };

        // default to https - only allow http scheme if explicitly requested
        let mut registry = registry.unwrap_or_else(|| default_registry.to_owned());
        if !registry.starts_with("http://") && !registry.starts_with("https://") {
            registry = format!("https://{}", registry);
        }

        // split up image name and version
        let version = image.split(':').collect::<Vec<_>>();
        let (image, version) = if let Some((image, version)) = version.split_first() {
            if !version.is_empty() {
                (image, Some(version.join(":")))
            } else {
                (image, None)
            }
        } else {
            (image, None)
        };

        Ok(PluginUri {
            registry,
            image: image.to_string(),
            version: version.unwrap_or(default_version.to_owned()),
        })
    }

    #[inline]
    pub fn registry(&self) -> &str {
        &self.registry
    }

    #[inline]
    pub fn image(&self) -> &str {
        &self.image
    }

    #[inline]
    pub fn version(&self) -> &str {
        &self.version
    }
}

impl FromStr for PluginUri {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        PluginUri::new(s)
    }
}

impl Display for PluginUri {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}/{}:{}", self.registry, self.image, self.version)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    pub fn plugin_path_simple() {
        let path: PluginUri = "coredump".parse().unwrap();
        assert_eq!(path.registry(), MEMFLOW_DEFAULT_REGISTRY);
        assert_eq!(path.image(), "coredump");
        assert_eq!(path.version(), "latest");
    }

    #[test]
    pub fn plugin_path_with_version() {
        let path: PluginUri = "coredump:0.2.0".parse().unwrap();
        assert_eq!(path.registry(), MEMFLOW_DEFAULT_REGISTRY);
        assert_eq!(path.image(), "coredump");
        assert_eq!(path.version(), "0.2.0");
    }

    #[test]
    pub fn plugin_path_with_registry() {
        let path: PluginUri = "registry.memflow.xyz/coredump:0.2.0".parse().unwrap();
        assert_eq!(path.registry(), "https://registry.memflow.xyz");
        assert_eq!(path.image(), "coredump");
        assert_eq!(path.version(), "0.2.0");
    }

    #[test]
    pub fn plugin_path_with_registry_http() {
        let path: PluginUri = "http://registry.memflow.xyz/coredump:0.2.0"
            .parse()
            .unwrap();
        assert_eq!(path.registry(), "http://registry.memflow.xyz");
        assert_eq!(path.image(), "coredump");
        assert_eq!(path.version(), "0.2.0");
    }

    #[test]
    pub fn plugin_path_invalid_path() {
        let path: PluginUri = "registry.memflow.xyz/coredump/test1234".parse().unwrap();
        assert_eq!(path.registry(), "https://registry.memflow.xyz/coredump");
        assert_eq!(path.image(), "test1234");
        assert_eq!(path.version(), "latest");
    }

    #[test]
    pub fn plugin_path_invalid_version() {
        let path: PluginUri = "test1234:0.2.0:1.0.0".parse().unwrap();
        assert_eq!(path.registry(), MEMFLOW_DEFAULT_REGISTRY);
        assert_eq!(path.image(), "test1234");
        assert_eq!(path.version(), "0.2.0:1.0.0");
    }

    #[test]
    pub fn plugin_path_custom_defaults() {
        let path = PluginUri::with_defaults("coredump", "test.xyz", "newest").unwrap();
        assert_eq!(path.registry(), "https://test.xyz");
        assert_eq!(path.image(), "coredump");
        assert_eq!(path.version(), "newest");
    }
}
