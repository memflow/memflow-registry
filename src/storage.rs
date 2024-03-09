use std::{ffi::CStr, marker::PhantomData, path::Path};

use axum::extract::ConnectInfo;
use goblin::{
    container::Ctx,
    elf::{dynamic, Dynamic, Elf, ProgramHeader, RelocSection, Symtab},
    mach::{exports::ExportInfo as MachExportInfo, Mach, MachO},
    pe::{options::ParseOptions, PE},
    strtab::Strtab,
    Object,
};
use memflow::plugins::{ConnectorDescriptor, LoadableConnector, PluginDescriptor};
use memflow::{cglue::CSliceRef, plugins::MEMFLOW_PLUGIN_VERSION};

/// Metadata attached to each file
pub struct FileMetadata {
    pub plugin: String,
    // TODO: plugin type
    pub tag: String,
    // TODO: memflow version / abi version
    // TODO: plugin version
}

///
pub struct Storage {}

pub struct DescriptorFile<'a> {
    bytes: &'a [u8],
    object: Object<'a>,
}

pub struct Descriptor<'a> {
    bytes: &'a [u8],
    object: &'a Object<'a>,
    plugin_descriptor: ConnectorDescriptor,
}

impl<'a> DescriptorFile<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        let object = Object::parse(&bytes[..]).unwrap();
        Self { bytes, object }
    }

    pub fn descriptors(&self) -> Vec<Descriptor> {
        let mut ret = vec![];
        match &self.object {
            Object::PE(pe) => {
                for export in pe.exports.iter() {
                    if let Some(name) = export.name {
                        if name.starts_with("MEMFLOW_CONNECTOR_") || name.starts_with("MEMFLOW_OS_")
                        {
                            let offset = export.offset.unwrap();

                            use memflow::dataview::DataView;
                            let data_view = DataView::from(self.bytes);
                            let descriptor = data_view.read::<ConnectorDescriptor>(offset);

                            ret.push(Descriptor {
                                bytes: self.bytes,
                                object: &self.object,
                                plugin_descriptor: descriptor,
                            });
                        }
                    }
                }
            }
            _ => {}
        }

        ret
    }
}

impl<'a> Descriptor<'a> {
    #[inline]
    pub fn plugin_version(&self) -> i32 {
        self.plugin_descriptor.plugin_version
    }

    #[inline]
    pub fn is_compatible(&self) -> bool {
        self.plugin_version() == MEMFLOW_PLUGIN_VERSION
    }

    #[inline]
    pub fn name(&self) -> String {
        self.read_sliceref(&self.plugin_descriptor.name)
    }

    #[inline]
    pub fn version(&self) -> String {
        let obj = Object::parse(self.bytes).unwrap();
        let descriptors = self.descriptor(&obj);
        let descriptor = descriptors.first().unwrap();
        self.read_sliceref(&descriptor.version)
    }

    #[inline]
    pub fn description(&self) -> String {
        let obj = Object::parse(self.bytes).unwrap();
        let descriptors = self.descriptor(&obj);
        let descriptor = descriptors.first().unwrap();
        self.read_sliceref(&descriptor.description)
    }

    fn descriptor(&self, obj: &goblin::Object) -> Vec<ConnectorDescriptor> {
        match obj {
            Object::PE(pe) => {
                for export in pe.exports.iter() {
                    if let Some(name) = export.name {
                        if name.starts_with("MEMFLOW_") {
                            let offset = export.offset.unwrap();

                            use memflow::dataview::DataView;
                            let data_view = DataView::from(self.bytes);
                            let descriptor = data_view.read::<ConnectorDescriptor>(offset);
                            return vec![descriptor];
                        }
                    }
                }
            }
            _ => {}
        }

        vec![]
    }

    fn read_sliceref(&self, str: &CSliceRef<u8>) -> String {
        match self.object {
            Object::PE(pe) => {
                let offset_va = str.as_ptr() as usize - pe.image_base;
                let offset = goblin::pe::utils::find_offset(
                    offset_va,
                    &pe.sections,
                    8,
                    &ParseOptions::default(),
                )
                .unwrap();

                let mut buffer = vec![0u8; str.len()];
                buffer.copy_from_slice(&self.bytes[offset..offset + str.len()]);

                std::str::from_utf8(&buffer[..]).unwrap().to_owned()
            }
            _ => "".to_owned(),
        }
    }
}

impl Storage {
    pub fn new() -> Self {
        // TODO: re-index all files
        let paths = std::fs::read_dir("./.storage").unwrap();
        for path in paths.filter_map(|p| p.ok()) {
            let bytes = std::fs::read(path.path()).unwrap();
            let file = DescriptorFile::new(&bytes[..]);
            let descriptors = file.descriptors();
            for descriptor in descriptors.iter() {
                println!("plugin_version: {}", descriptor.plugin_version());
                println!("name: {}", descriptor.name());
                println!("version: {}", descriptor.version());
                println!("description: {}", descriptor.description());
            }
        }

        Self {}
    }
}
