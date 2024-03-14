mod plugin_analyzer;

/// Metadata attached to each file
pub struct FileMetadata {
    pub plugin: String,
    // TODO: plugin type
    pub tag: String,
    // TODO: memflow version / abi version
    // TODO: plugin version
    // TODO: architecture, os, etc.
}

///
pub struct Storage {}

impl Storage {
    pub fn new() -> Self {
        // TODO: re-index all files
        let paths = std::fs::read_dir("./.storage").unwrap();
        for path in paths.filter_map(|p| p.ok()) {
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
        }

        Self {}
    }
}
