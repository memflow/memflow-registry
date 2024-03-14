use std::ops::Range;

use goblin::{
    elf::{section_header::SHN_XINDEX, Elf},
    mach::{Mach, MachO, SingleArch},
    pe::{self, options::ParseOptions, PE},
    Object,
};
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
    // TODO: architecture, os, etc.
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

    /// Parses and returns all descriptors found in the binary.
    /// This function tries to guess the binary type.
    pub fn descriptors(&self) -> Vec<Descriptor> {
        match &self.object {
            Object::PE(pe) => self.descriptors_pe(pe),
            Object::Elf(elf) => self.descriptors_elf(elf),
            Object::Mach(mach) => self.descriptors_mach(mach),
            _ => todo!(),
        }
    }

    /// Parses the descriptors in a PE binary.
    /// This function currently supports x86 and x86_64 binaries.
    fn descriptors_pe(&self, pe: &PE) -> Vec<Descriptor> {
        let mut ret = vec![];

        for export in pe.exports.iter() {
            if let Some(name) = export.name {
                if name.starts_with("MEMFLOW_") {
                    let offset = export.offset.unwrap();

                    use memflow::dataview::DataView;
                    let data_view = DataView::from(self.bytes);

                    let descriptor = if pe.is_64 {
                        PluginDescriptor::Bits64(data_view.read::<PluginDescriptor64>(offset))
                    } else {
                        PluginDescriptor::Bits32(data_view.read::<PluginDescriptor32>(offset))
                    };

                    ret.push(Descriptor {
                        bytes: self.bytes,
                        object: &self.object,
                        plugin_descriptor: descriptor,
                    });
                }
            }
        }

        ret
    }

    /// Parses the descriptors in an ELF binary.
    /// This function currently supports x86, x86_64, aarch64 and armv7.
    fn descriptors_elf(&self, elf: &Elf) -> Vec<Descriptor> {
        let mut ret = vec![];

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
                if sym.st_shndx == SHN_XINDEX as usize {
                    todo!()
                }

                // section
                let section = elf
                    .program_headers
                    .iter()
                    .find(|h| h.vm_range().contains(&(sym.st_value as usize)))
                    .unwrap();

                // compute proper file offset based on section
                let offset = section.p_offset + sym.st_value - section.p_vaddr;

                use memflow::dataview::DataView;
                let data_view = DataView::from(self.bytes);

                let descriptor = if elf.is_64 {
                    let mut descriptor64 = data_view.read::<PluginDescriptor64>(offset as usize);
                    self._elf_apply_relocs64(
                        elf,
                        sym.st_value..sym.st_value + sym.st_size,
                        &mut descriptor64,
                    );
                    PluginDescriptor::Bits64(descriptor64)
                } else {
                    let mut descriptor32 = data_view.read::<PluginDescriptor32>(offset as usize);
                    self._elf_apply_relocs32(
                        elf,
                        sym.st_value..sym.st_value + sym.st_size,
                        &mut descriptor32,
                    );
                    PluginDescriptor::Bits32(descriptor32)
                };

                ret.push(Descriptor {
                    bytes: self.bytes,
                    object: &self.object,
                    plugin_descriptor: descriptor,
                });
            }
        }

        ret
    }

    fn _elf_apply_relocs64<T: Pod>(&self, elf: &Elf, va_range: Range<u64>, obj: &mut T) {
        for section_relocs in elf.shdr_relocs.iter() {
            for reloc in section_relocs.1.iter() {
                if reloc.r_offset >= va_range.start && reloc.r_offset < va_range.end {
                    let field_offset = reloc.r_offset - va_range.start;

                    use memflow::dataview::DataView;
                    let data_view = DataView::from_mut(obj);
                    let value = data_view.read::<u64>(field_offset as usize);

                    // skip over entries that already contain the proper reference
                    if value != 0 {
                        continue;
                    }

                    // https://chromium.googlesource.com/android_tools/+/8301b711a9ac7de56e9a9ff3dee0b2ebfc9a380f/ndk/sources/android/crazy_linker/src/crazy_linker_elf_relocations.cpp#36
                    // TODO: generalize this check
                    if reloc.r_type != 8 && reloc.r_type != 23 && reloc.r_type != 1027 {
                        todo!("only relative relocations are supported right now");
                    }

                    let value = value.wrapping_add_signed(reloc.r_addend.unwrap_or_default());
                    data_view.write::<u64>(field_offset as usize, &value);
                }
            }
        }
    }

    fn _elf_apply_relocs32<T: Pod>(&self, elf: &Elf, va_range: Range<u64>, obj: &mut T) {
        for section_relocs in elf.shdr_relocs.iter() {
            for reloc in section_relocs.1.iter() {
                if reloc.r_offset >= va_range.start && reloc.r_offset < va_range.end {
                    let field_offset = reloc.r_offset - va_range.start;

                    use memflow::dataview::DataView;
                    let data_view = DataView::from_mut(obj);
                    let value = data_view.read::<u32>(field_offset as usize);

                    // skip over entries that already contain the proper reference
                    if value != 0 {
                        continue;
                    }

                    // https://chromium.googlesource.com/android_tools/+/8301b711a9ac7de56e9a9ff3dee0b2ebfc9a380f/ndk/sources/android/crazy_linker/src/crazy_linker_elf_relocations.cpp#36
                    // TODO: generalize this check
                    if reloc.r_type != 8 && reloc.r_type != 23 && reloc.r_type != 1027 {
                        todo!("only relative relocations are supported right now");
                    }

                    let value =
                        value.wrapping_add_signed(reloc.r_addend.unwrap_or_default() as i32);
                    data_view.write::<u32>(field_offset as usize, &value);
                }
            }
        }
    }

    fn descriptors_mach(&self, mach: &Mach) -> Vec<Descriptor> {
        let mut ret = vec![];

        match mach {
            Mach::Fat(multiarch) => {
                // TODO: loop + recurse
                for arch in multiarch.into_iter().filter_map(|a| a.ok()) {
                    match arch {
                        SingleArch::MachO(macho) => {
                            let mut descriptors = self.descriptors_macho(&macho);
                            ret.append(&mut descriptors);
                        }
                        SingleArch::Archive(_) => {
                            panic!("mac archive not implemented");
                        }
                    }
                }
            }
            Mach::Binary(macho) => {
                let mut descriptors = self.descriptors_macho(macho);
                ret.append(&mut descriptors);
            }
        }

        ret
    }

    fn descriptors_macho(&self, macho: &MachO) -> Vec<Descriptor> {
        let mut ret = vec![];

        if !macho.little_endian {
            panic!("no big endian support")
        }

        if let Ok(exports) = macho.exports() {
            for export in exports.iter() {
                if export.name.starts_with("_MEMFLOW_") {
                    let offset = export.offset;

                    use memflow::dataview::DataView;
                    let data_view = DataView::from(self.bytes);

                    let descriptor = if macho.is_64 {
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

                    let file_alignment = pe
                        .header
                        .optional_header
                        .map(|h| h.windows_fields.file_alignment)
                        .unwrap_or(512);
                    let offset = pe::utils::find_offset(
                        offset_va,
                        &pe.sections,
                        file_alignment,
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
            Object::Mach(_) => {
                if ptr != 0 {
                    // TODO: why is this offset padded so high? is there a vm base somewhere?
                    let offset = (ptr & 0xffff_ffff) as usize;

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
                if descriptor.version().is_empty() {
                    panic!();
                }
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
