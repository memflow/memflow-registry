use goblin::{elf::section_header::SHN_XINDEX, pe::options::ParseOptions, Object};
use memflow::{dataview::Pod, plugins::MEMFLOW_PLUGIN_VERSION};

/// Adapted from PluginDescriptor<T: Loadable>
#[repr(C, align(4))]
pub struct PluginDescriptor32 {
    pub plugin_version: i32,
    pub accept_input: bool,
    pub input_layout: u32,  // &'static TypeLayout
    pub output_layout: u32, // &'static TypeLayout,
    pub name: u32,          // CSliceRef<'static, u8>,
    pub name_length: u32,
    pub version: u32, // CSliceRef<'static, u8>,
    pub version_length: u32,
    pub description: u32, //CSliceRef<'static, u8>,
    pub description_length: u32,
    pub help_callback: u32, // Option<extern "C" fn(callback: HelpCallback) -> ()>,
    pub target_list_callback: u32, // Option<extern "C" fn(callback: TargetCallback) -> i32>,
    pub create: u32,        // CreateFn<T>,
}
const _: [(); std::mem::size_of::<PluginDescriptor32>()] = [(); 0x34];
unsafe impl Pod for PluginDescriptor32 {}

#[repr(C)]
pub struct PluginDescriptor64 {
    pub plugin_version: i32,
    pub accept_input: bool,
    pub input_layout: u64,  // &'static TypeLayout
    pub output_layout: u64, // &'static TypeLayout,
    pub name: u64,          // CSliceRef<'static, u8>,
    pub name_length: u32,
    pub version: u64, // CSliceRef<'static, u8>,
    pub version_length: u32,
    pub description: u64, //CSliceRef<'static, u8>,
    pub description_length: u32,
    pub help_callback: u64, // Option<extern "C" fn(callback: HelpCallback) -> ()>,
    pub target_list_callback: u64, // Option<extern "C" fn(callback: TargetCallback) -> i32>,
    pub create: u64,        // CreateFn<T>,
}
const _: [(); std::mem::size_of::<PluginDescriptor64>()] = [(); 0x60];
unsafe impl Pod for PluginDescriptor64 {}

pub enum PluginDescriptor {
    Bits32(PluginDescriptor32),
    Bits64(PluginDescriptor64),
}

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
    plugin_descriptor: PluginDescriptor,
}

impl<'a> DescriptorFile<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        let object = Object::parse(bytes).unwrap();
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

                            let descriptor = if pe.is_64 {
                                PluginDescriptor::Bits64(
                                    data_view.read::<PluginDescriptor64>(offset),
                                )
                            } else {
                                PluginDescriptor::Bits32(
                                    data_view.read::<PluginDescriptor32>(offset),
                                )
                            };

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

                let iter = elf
                    .dynsyms
                    .iter()
                    .filter(|s| !s.is_import())
                    .filter_map(|s| elf.dynstrtab.get_at(s.st_name).map(|n| (s, n)));

                for (sym, name) in iter {
                    if name.starts_with("MEMFLOW_") {
                        println!("sym: {:?}", sym);
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

                        let descriptor = if elf.is_64 {
                            PluginDescriptor::Bits64(
                                data_view.read::<PluginDescriptor64>(offset as usize),
                            )
                        } else {
                            PluginDescriptor::Bits32(
                                data_view.read::<PluginDescriptor32>(offset as usize),
                            )
                        };

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
        match &self.plugin_descriptor {
            PluginDescriptor::Bits32(descriptor) => descriptor.plugin_version,
            PluginDescriptor::Bits64(descriptor) => descriptor.plugin_version,
        }
    }

    #[inline]
    pub fn accept_input(&self) -> bool {
        match &self.plugin_descriptor {
            PluginDescriptor::Bits32(descriptor) => descriptor.accept_input,
            PluginDescriptor::Bits64(descriptor) => descriptor.accept_input,
        }
    }

    #[inline]
    pub fn is_compatible(&self) -> bool {
        self.plugin_version() == MEMFLOW_PLUGIN_VERSION
    }

    #[inline]
    pub fn name(&self) -> String {
        match &self.plugin_descriptor {
            PluginDescriptor::Bits32(descriptor) => {
                self.read_sliceref(descriptor.name as u64, descriptor.name_length as usize)
            }
            PluginDescriptor::Bits64(descriptor) => {
                self.read_sliceref(descriptor.name, descriptor.name_length as usize)
            }
        }
    }

    #[inline]
    pub fn version(&self) -> String {
        match &self.plugin_descriptor {
            PluginDescriptor::Bits32(descriptor) => self.read_sliceref(
                descriptor.version as u64,
                descriptor.version_length as usize,
            ),
            PluginDescriptor::Bits64(descriptor) => {
                self.read_sliceref(descriptor.version, descriptor.version_length as usize)
            }
        }
    }

    #[inline]
    pub fn description(&self) -> String {
        match &self.plugin_descriptor {
            PluginDescriptor::Bits32(descriptor) => self.read_sliceref(
                descriptor.description as u64,
                descriptor.description_length as usize,
            ),
            PluginDescriptor::Bits64(descriptor) => self.read_sliceref(
                descriptor.description,
                descriptor.description_length as usize,
            ),
        }
    }

    fn read_sliceref(&self, ptr: u64, len: usize) -> String {
        match self.object {
            Object::PE(pe) => {
                if ptr != 0 {
                    let offset_va = ptr as usize - pe.image_base;
                    let offset = goblin::pe::utils::find_offset(
                        offset_va,
                        &pe.sections,
                        8,
                        &ParseOptions::default(),
                    )
                    .unwrap();

                    let mut buffer = vec![0u8; len];
                    buffer.copy_from_slice(&self.bytes[offset..offset + len]);

                    std::str::from_utf8(&buffer[..]).unwrap().to_owned()
                } else {
                    String::new()
                }
            }
            Object::Elf(_) => {
                if ptr != 0 {
                    // for elf no further mangling has to be done here
                    let offset = ptr as usize;

                    let mut buffer = vec![0u8; len];
                    buffer.copy_from_slice(&self.bytes[offset..offset + len]);

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
