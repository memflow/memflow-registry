use reqwest::{Response, Url};

use memflow_registry_shared::{
    structs::PluginsFindResponse, Error, PluginInfo, PluginUri, PluginVariant, PluginsAllResponse,
    Result, MEMFLOW_DEFAULT_REGISTRY,
};

fn to_http_err<S: ToString>(err: S) -> Error {
    Error::Http(err.to_string())
}

/// Retrieves a list of all plugins and their descriptions.
pub async fn plugins(registry: Option<&str>) -> Result<Vec<PluginInfo>> {
    let mut path: Url = registry
        .unwrap_or(MEMFLOW_DEFAULT_REGISTRY)
        .parse()
        .unwrap();
    path.set_path("plugins");

    let response = reqwest::get(path)
        .await
        .map_err(to_http_err)?
        .json::<PluginsAllResponse>()
        .await
        .map_err(to_http_err)?;

    Ok(response.plugins)
}

pub async fn plugin_versions(
    registry: Option<&str>,
    plugin_name: &str,
    limit: usize,
) -> Result<Vec<PluginVariant>> {
    // construct query path
    let mut path: Url = registry
        .unwrap_or(MEMFLOW_DEFAULT_REGISTRY)
        .parse()
        .unwrap();
    path.set_path(&format!("plugins/{}", plugin_name));

    // setup filtering based on the os memflowup is built for
    {
        let mut query = path.query_pairs_mut();
        query.append_pair("memflow_plugin_version", "1"); // TODO:
        query.append_pair("limit", &limit.to_string());
    }
    append_os_arch_filter(&mut path);

    let response = reqwest::get(path)
        .await
        .map_err(to_http_err)?
        .json::<PluginsFindResponse>()
        .await
        .map_err(to_http_err)?;

    Ok(response.plugins)
}

// Downloads a plugin based on the specified uri
pub async fn find_by_uri(plugin_uri: &PluginUri) -> Result<PluginVariant> {
    // construct query path
    let mut path: Url = plugin_uri.registry().parse().unwrap();
    path.set_path(&format!("plugins/{}", plugin_uri.name()));

    // setup filtering based on the os memflowup is built for
    {
        let mut query = path.query_pairs_mut();
        if plugin_uri.version() != "latest" {
            query.append_pair("version", plugin_uri.version());
        }
        query.append_pair("memflow_plugin_version", "1"); // TODO:
                                                          // limit to the latest entry
        query.append_pair("limit", "1");
    }
    append_os_arch_filter(&mut path);

    let response = reqwest::get(path)
        .await
        .map_err(to_http_err)?
        .json::<PluginsFindResponse>()
        .await
        .map_err(to_http_err)?;

    if let Some(variant) = response.plugins.first() {
        Ok(variant.to_owned())
    } else {
        Err(Error::NotFound(format!(
            "plugin `{}` not found",
            plugin_uri
        )))
    }
}

pub async fn download(plugin_uri: &PluginUri, variant: &PluginVariant) -> Result<Response> {
    let mut path: Url = plugin_uri.registry().parse().unwrap();
    path.set_path(&format!("files/{}", variant.digest));

    let response = reqwest::get(path).await.map_err(to_http_err)?;
    Ok(response)
}

fn append_os_arch_filter(path: &mut Url) {
    let mut query = path.query_pairs_mut();

    // file_type
    #[cfg(target_os = "windows")]
    query.append_pair("file_type", "pe");
    #[cfg(target_os = "linux")]
    query.append_pair("file_type", "elf");
    #[cfg(target_os = "macos")]
    query.append_pair("file_type", "mach");

    // architecture
    #[cfg(target_arch = "x86_64")]
    query.append_pair("architecture", "x86_64");
    #[cfg(target_arch = "x86")]
    query.append_pair("architecture", "x86");
    #[cfg(target_arch = "aarch64")]
    query.append_pair("architecture", "arm64");
    #[cfg(target_arch = "arm")]
    query.append_pair("architecture", "arm");
}
