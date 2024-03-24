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
    name: String,
    version: Option<String>,
}

impl PluginUri {
    #[inline]
    pub fn registry(&self) -> String {
        self.registry
            .as_ref()
            .map(|r| format!("https://{}", r))
            .unwrap_or_else(|| MEMFLOW_DEFAULT_REGISTRY.to_owned())
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn version(&self) -> &str {
        self.version.as_deref().unwrap_or("latest")
    }
}

impl FromStr for PluginUri {
    type Err = Error;

    fn from_str(s: &str) -> std::prelude::v1::Result<Self, Self::Err> {
        let path = s.split('/').collect::<Vec<_>>();
        let version = path
            .get(1)
            .unwrap_or_else(|| &path[0])
            .split(':')
            .collect::<Vec<_>>();
        if path.len() > 2 || version.len() > 2 {
            return Err(Error::Parse(
                "invalid plugin path. format should be {registry}/{plugin_name}:{plugin_version}"
                    .to_owned(),
            ));
        }

        Ok(PluginUri {
            registry: if path.len() > 1 {
                Some(path[0].to_owned())
            } else {
                None
            },
            name: version[0].to_owned(),
            version: version.get(1).map(|&s| s.to_owned()),
        })
    }
}

impl Display for PluginUri {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(registry) = &self.registry {
            write!(f, "{}/", registry)?;
        }
        write!(f, "{}", self.name)?;
        if let Some(version) = &self.version {
            write!(f, ":{}", version)?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::error::Result;

    #[test]
    pub fn plugin_path_simple() {
        let path: PluginUri = "coredump".parse().unwrap();
        assert_eq!(path.registry(), MEMFLOW_DEFAULT_REGISTRY);
        assert_eq!(path.name(), "coredump");
        assert_eq!(path.version(), "latest");
    }

    #[test]
    pub fn plugin_path_with_version() {
        let path: PluginUri = "coredump:0.2.0".parse().unwrap();
        assert_eq!(path.registry(), MEMFLOW_DEFAULT_REGISTRY);
        assert_eq!(path.name(), "coredump");
        assert_eq!(path.version(), "0.2.0");
    }

    #[test]
    pub fn plugin_path_with_registry() {
        let path: PluginUri = "registry.memflow.xyz/coredump:0.2.0".parse().unwrap();
        assert_eq!(path.registry(), "registry.memflow.xyz");
        assert_eq!(path.name(), "coredump");
        assert_eq!(path.version(), "0.2.0");
    }

    #[test]
    pub fn plugin_path_invalid_path() {
        let path: Result<PluginUri> = "registry.memflow.xyz/coredump/test1234".parse();
        assert!(path.is_err())
    }

    #[test]
    pub fn plugin_path_invalid_version() {
        let path: Result<PluginUri> = "test1234:0.2.0:1.0.0".parse();
        assert!(path.is_err())
    }
}
