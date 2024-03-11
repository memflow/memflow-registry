use std::{ffi::CStr, marker::PhantomData, path::Path};

use axum::extract::ConnectInfo;
use goblin::{
    container::Ctx,
    elf::{dynamic, section_header::SHN_XINDEX, Dynamic, Elf, ProgramHeader, RelocSection, Symtab},
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
            Object::Elf(elf) => {
                if !elf.little_endian {
                    panic!("big_endian unsupported");
                }
                if !elf.is_64 {
                    println!("--- NO 64 BIT SUPPORT YET ---");
                    return vec![];
                }

                //                println!("header: {:?}", elf.program_headers);

                let iter = elf
                    .dynsyms
                    .iter()
                    .filter(|s| !s.is_import())
                    .filter_map(|s| elf.dynstrtab.get_at(s.st_name).map(|n| (s, n)));

                for (sym, name) in iter {
                    if name.starts_with("MEMFLOW_") {
                        let section = elf.section_headers.get(sym.st_shndx).unwrap();
                        if section.is_relocation() {
                            todo!()
                        }

                        if sym.st_shndx == SHN_XINDEX as usize {
                            todo!()
                        }

                        // section
                        let section = elf
                            .program_headers
                            .iter()
                            .find(|h| h.vm_range().contains(&(sym.st_value as usize)))
                            .unwrap();

                        let offset = sym.st_value - section.p_align;

                        use memflow::dataview::DataView;
                        let data_view = DataView::from(self.bytes);
                        let descriptor = data_view.read::<ConnectorDescriptor>(offset as usize);
                        println!("descriptor.plugin_version: {}", descriptor.plugin_version);

                        ret.push(Descriptor {
                            bytes: self.bytes,
                            object: &self.object,
                            plugin_descriptor: descriptor,
                        });
                    }
                }
            }
            _ => todo!(),
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
    pub fn accept_input(&self) -> bool {
        self.plugin_descriptor.accept_input
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
        self.read_sliceref(&self.plugin_descriptor.version)
    }

    #[inline]
    pub fn description(&self) -> String {
        self.read_sliceref(&self.plugin_descriptor.description)
    }

    fn read_sliceref(&self, str: &CSliceRef<u8>) -> String {
        match self.object {
            Object::PE(pe) => {
                if !str.as_ptr().is_null() {
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
                } else {
                    String::new()
                }
            }
            Object::Elf(_) => {
                if !str.as_ptr().is_null() {
                    // for elf no further mangling has to be done here
                    let offset = str.as_ptr() as usize;

                    let mut buffer = vec![0u8; str.len()];
                    buffer.copy_from_slice(&self.bytes[offset..offset + str.len()]);

                    std::str::from_utf8(&buffer[..]).unwrap().to_owned()
                } else {
                    String::new()
                }
            }
            _ => todo!(),
        }
    }
}

impl Storage {
    pub fn new() -> Self {
        // TODO: re-index all files
        let paths = std::fs::read_dir("./.storage").unwrap();
        for path in paths.filter_map(|p| p.ok()) {
            println!("parsing file: {:?}", path.path());
            // TODO: filter by filename
            // TODO: filter by size
            let bytes = std::fs::read(path.path()).unwrap();
            let file = DescriptorFile::new(&bytes[..]);
            let descriptors = file.descriptors();
            for descriptor in descriptors.iter() {
                println!("plugin_version: {}", descriptor.plugin_version());
                println!("accept_input: {}", descriptor.accept_input());
                println!("name: {}", descriptor.name());
                println!("version: {}", descriptor.version());
                println!("description: {}", descriptor.description());
            }
        }

        Self {}
    }
}
