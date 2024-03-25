use std::{fmt::Display, str::FromStr};

use crate::{error::Error, MEMFLOW_DEFAULT_REGISTRY};

/// Parses a plugin string into it's path components
///
/// # Supported plugin path formats:
///
/// `coredump` - will just pull latest
/// `coredump:latest` - will also pull latest
/// `coredump:0.2.0` - will pull the newest binary with this specific version
/// `memflow.registry.io/coredump` - pulls from another registry
pub struct PluginUri {
    registry: Option<String>,
    image: String,
    version: Option<String>,
}

impl PluginUri {
    #[inline]
    pub fn registry(&self) -> String {
        self.registry
            .clone()
            .unwrap_or_else(|| MEMFLOW_DEFAULT_REGISTRY.to_owned())
    }

    #[inline]
    pub fn image(&self) -> &str {
        &self.image
    }

    #[inline]
    pub fn version(&self) -> &str {
        self.version.as_deref().unwrap_or("latest")
    }
}

impl FromStr for PluginUri {
    type Err = Error;

    fn from_str(s: &str) -> std::prelude::v1::Result<Self, Self::Err> {
        // split up registry and image
        let image = s.split('/').collect::<Vec<_>>();
        let (registry, image) = if let Some((image, registry)) = image.split_last() {
            let registry = if !registry.is_empty() {
                // TODO parse url
                let registry_url = registry.join("/");
                if registry_url.starts_with("http://") || registry_url.starts_with("https://") {
                    // only allow http scheme if explicitly requested
                    Some(registry_url)
                } else {
                    // prepend https as the default scheme
                    Some(format!("https://{}", registry_url))
                }
            } else {
                None
            };

            (registry, image)
        } else {
            (None, &s)
        };

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
            version,
        })
    }
}

impl Display for PluginUri {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(registry) = &self.registry {
            write!(f, "{}/", registry)?;
        }
        write!(f, "{}", self.image)?;
        if let Some(version) = &self.version {
            write!(f, ":{}", version)?;
        }
        Ok(())
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
}
