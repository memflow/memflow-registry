use std::ffi::CStr;

use axum::extract::ConnectInfo;
use goblin::{
    container::Ctx,
    elf::{dynamic, Dynamic, Elf, ProgramHeader, RelocSection, Symtab},
    mach::{exports::ExportInfo as MachExportInfo, Mach, MachO},
    pe::{options::ParseOptions, PE},
    strtab::Strtab,
    Object,
};
use memflow::cglue::CSliceRef;
use memflow::plugins::ConnectorDescriptor;

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

impl Storage {
    pub fn new() -> Self {
        // TODO: re-index all files
        let paths = std::fs::read_dir("./.storage").unwrap();
        for path in paths {
            let bytes = std::fs::read(path.unwrap().path()).unwrap();
            let obj = Object::parse(&bytes).unwrap();
            match obj {
                Object::PE(pe) => {
                    for export in pe.exports.iter() {
                        if let Some(name) = export.name {
                            if name.starts_with("MEMFLOW_CONNECTOR_")
                                || name.starts_with("MEMFLOW_OS_")
                            {
                                let offset = export.offset.unwrap();

                                use memflow::dataview::DataView;
                                let data_view = DataView::from(&bytes[..]);
                                let descriptor = data_view.read::<ConnectorDescriptor>(offset);

                                println!("plugin_version: {}", descriptor.plugin_version);
                                println!(
                                    "name: {}",
                                    pe_read_sliceref(&bytes, &pe, &descriptor.name)
                                );
                                println!(
                                    "version: {}",
                                    pe_read_sliceref(&bytes, &pe, &descriptor.version)
                                );
                                println!(
                                    "description: {}",
                                    pe_read_sliceref(&bytes, &pe, &descriptor.description)
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Self {}
    }
}

fn pe_read_sliceref(bytes: &[u8], pe: &PE, str: &CSliceRef<u8>) -> String {
    let offset_va = str.as_ptr() as usize - pe.image_base;
    let offset =
        goblin::pe::utils::find_offset(offset_va, &pe.sections, 8, &ParseOptions::default())
            .unwrap();

    let mut buffer = vec![0u8; str.len()];
    buffer.copy_from_slice(&bytes[offset..offset + str.len()]);

    std::str::from_utf8(&buffer[..]).unwrap().to_owned()
}
