use std::ops::Range;

use goblin::{
    elf::{section_header::SHN_XINDEX, Elf},
    mach::{Mach, MachO, SingleArch},
    pe::{self, options::ParseOptions, PE},
    Object,
};
use memflow::dataview::Pod;
use num_traits::{NumCast, WrappingAdd, WrappingSub, Zero};

use crate::error::{Error, Result};

static MEMFLOW_EXPORT_PREFIX: &str = "MEMFLOW_";

/// Adapted from PluginDescriptor<T: Loadable>
#[repr(C, align(4))]
struct PluginDescriptor32 {
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
struct PluginDescriptor64 {
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

#[derive(Debug)]
pub enum PluginArchitecture {
    Unknown(u32),
    X86,
    X86_64,
    Arm,
    Arm64,
}

#[derive(Debug)]
pub struct PluginDescriptor {
    pub architecture: PluginArchitecture,
    pub plugin_version: i32,
    pub name: String,
    pub version: String,
    pub description: String,
}

/// Parses and returns all descriptors found in the binary.
/// This function tries to guess the binary type.
pub fn parse_descriptors(bytes: &[u8]) -> Result<Vec<PluginDescriptor>> {
    let object = Object::parse(bytes)?;
    match object {
        Object::PE(pe) => pe_parse_descriptors(bytes, &pe),
        Object::Mach(mach) => mach_parse_descriptors(bytes, &mach),
        Object::Elf(elf) => elf_parse_descriptors(bytes, &elf),
        _ => Err(Error::Parse("unknown binary format".to_owned())),
    }
}

/// Parses the descriptors in a PE binary.
/// This function currently supports x86 and x86_64 binaries.
fn pe_parse_descriptors(bytes: &[u8], pe: &PE) -> Result<Vec<PluginDescriptor>> {
    let mut ret = vec![];

    for export in pe.exports.iter() {
        if let Some(name) = export.name {
            if name.starts_with(MEMFLOW_EXPORT_PREFIX) {
                if let Some(offset) = export.offset {
                    use memflow::dataview::DataView;
                    let data_view = DataView::from(bytes);

                    if pe.is_64 {
                        let raw_desc = data_view.read::<PluginDescriptor64>(offset);
                        #[rustfmt::skip]
                    ret.push(PluginDescriptor {
                        architecture: pe_architecture(pe),
                        plugin_version: raw_desc.plugin_version,
                        name: pe_read_sliceref(bytes, pe, raw_desc.name, raw_desc.name_length as usize)?,
                        version: pe_read_sliceref(bytes, pe, raw_desc.version, raw_desc.version_length as usize)?,
                        description: pe_read_sliceref(bytes, pe, raw_desc.description, raw_desc.description_length as usize)?,
                    });
                    } else {
                        let raw_desc = data_view.read::<PluginDescriptor32>(offset);
                        #[rustfmt::skip]
                    ret.push(PluginDescriptor {
                        architecture: pe_architecture(pe),
                        plugin_version: raw_desc.plugin_version,
                        name: pe_read_sliceref(bytes, pe, raw_desc.name as u64, raw_desc.name_length as usize)?,
                        version: pe_read_sliceref(bytes, pe, raw_desc.version as u64, raw_desc.version_length as usize)?,
                        description: pe_read_sliceref(bytes, pe, raw_desc.description as u64, raw_desc.description_length as usize)?,
                    });
                    }
                }
            }
        }
    }

    Ok(ret)
}

fn pe_architecture(pe: &PE) -> PluginArchitecture {
    // https://learn.microsoft.com/en-us/windows/win32/debug/pe-format#machine-types
    match pe.header.coff_header.machine {
        0x14c => PluginArchitecture::X86,
        0x8664 => PluginArchitecture::X86_64,
        0x1c0 | 0x1c4 => PluginArchitecture::Arm,
        0xAA64 => PluginArchitecture::Arm64,
        _ => PluginArchitecture::Unknown(pe.header.coff_header.machine as u32),
    }
}

fn pe_read_sliceref(bytes: &[u8], pe: &PE, ptr: u64, len: usize) -> Result<String> {
    if ptr == 0 {
        return Err(Error::Parse(
            "unable to read referenced string in binary".to_owned(),
        ));
    }

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
    .ok_or_else(|| {
        Error::Parse("could not find any section containing the referenced string".to_owned())
    })?;

    // TODO: bounds check instead of panic
    let mut buffer = vec![0u8; len];
    buffer.copy_from_slice(&bytes[offset..offset + len]);

    Ok(std::str::from_utf8(&buffer[..])?.to_owned())
}

fn mach_parse_descriptors(bytes: &[u8], mach: &Mach) -> Result<Vec<PluginDescriptor>> {
    let mut ret = vec![];

    match mach {
        Mach::Binary(macho) => {
            let mut descriptors = macho_parse_descriptors(bytes, macho)?;
            ret.append(&mut descriptors);
        }
        Mach::Fat(multiarch) => {
            for (index, fatarch) in multiarch.arches()?.iter().enumerate() {
                if let Ok(arch) = multiarch.get(index) {
                    match arch {
                        SingleArch::MachO(macho) => {
                            let offset = fatarch.offset as usize;
                            let len = fatarch.size as usize;
                            let mut descriptors =
                                macho_parse_descriptors(&bytes[offset..offset + len], &macho)?;
                            ret.append(&mut descriptors);
                        }
                        SingleArch::Archive(_) => {
                            return Err(Error::NotImplemented(
                                "mach archives are not supported yet".to_owned(),
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(ret)
}

fn macho_parse_descriptors(bytes: &[u8], macho: &MachO) -> Result<Vec<PluginDescriptor>> {
    let mut ret = vec![];

    if !macho.little_endian {
        return Err(Error::NotImplemented(
            "big endian binaries are not supported yet".to_owned(),
        ));
    }

    if let Ok(exports) = macho.exports() {
        for export in exports.iter() {
            // TODO: append at compile time MEMFLOW_EXPORT_PREFIX
            if export.name.starts_with("_MEMFLOW_") {
                let offset = export.offset;

                use memflow::dataview::DataView;
                let data_view = DataView::from(bytes);

                if macho.is_64 {
                    let raw_desc = data_view.read::<PluginDescriptor64>(offset as usize);
                    #[rustfmt::skip]
                    ret.push(PluginDescriptor{
                        architecture: macho_architecture(macho),
                        plugin_version: raw_desc.plugin_version,
                        name: macho_read_sliceref(bytes, raw_desc.name, raw_desc.name_length as usize)?,
                        version: macho_read_sliceref(bytes, raw_desc.version, raw_desc.version_length as usize)?,
                        description: macho_read_sliceref(bytes, raw_desc.description, raw_desc.description_length as usize)?,
                    });
                } else {
                    let raw_desc = data_view.read::<PluginDescriptor32>(offset as usize);
                    #[rustfmt::skip]
                    ret.push(PluginDescriptor{
                        architecture: macho_architecture(macho),
                        plugin_version: raw_desc.plugin_version,
                        name: macho_read_sliceref(bytes, raw_desc.name as u64, raw_desc.name_length as usize)?,
                        version: macho_read_sliceref(bytes, raw_desc.version as u64, raw_desc.version_length as usize)?,
                        description: macho_read_sliceref(bytes, raw_desc.description as u64, raw_desc.description_length as usize)?,
                    });
                }
            }
        }
    }

    Ok(ret)
}

fn macho_architecture(macho: &MachO) -> PluginArchitecture {
    // https://crystal-lang.org/api/0.24.0/Debug/MachO/CpuType.html
    match macho.header.cputype {
        7 => PluginArchitecture::X86,
        16777223 => PluginArchitecture::X86_64,
        12 => PluginArchitecture::Arm,
        16777228 => PluginArchitecture::Arm64,
        _ => PluginArchitecture::Unknown(macho.header.cputype),
    }
}

fn macho_read_sliceref(bytes: &[u8], ptr: u64, len: usize) -> Result<String> {
    if ptr == 0 {
        return Err(Error::Parse(
            "unable to read referenced string in binary".to_owned(),
        ));
    }

    // TODO: why is this offset padded so high? is there a vm base somewhere?
    let offset = (ptr & 0xffff_ffff) as usize;

    let mut buffer = vec![0u8; len];
    buffer.copy_from_slice(&bytes[offset..offset + len]);

    Ok(std::str::from_utf8(&buffer[..])?.to_owned())
}

/// Parses the descriptors in an ELF binary.
/// This function currently supports x86, x86_64, aarch64 and armv7.
fn elf_parse_descriptors(bytes: &[u8], elf: &Elf) -> Result<Vec<PluginDescriptor>> {
    let mut ret = vec![];

    if !elf.little_endian {
        return Err(Error::NotImplemented(
            "big endian binaries are not supported yet".to_owned(),
        ));
    }

    let iter = elf
        .dynsyms
        .iter()
        .filter(|s| !s.is_import())
        .filter_map(|s| elf.dynstrtab.get_at(s.st_name).map(|n| (s, n)));

    for (sym, name) in iter {
        if name.starts_with(MEMFLOW_EXPORT_PREFIX) {
            if sym.st_shndx == SHN_XINDEX as usize {
                return Err(Error::Parse(
                    "unsupported elf SHN_XINDEX header flag".to_owned(),
                ));
            }

            // section
            let section = elf
                .program_headers
                .iter()
                .find(|h| h.vm_range().contains(&(sym.st_value as usize)))
                .ok_or_else(|| {
                    Error::Parse(
                        "could not find any section containing the plugin descriptor".to_owned(),
                    )
                })?;

            // compute proper file offset based on section
            let offset = section.p_offset + sym.st_value - section.p_vaddr;

            use memflow::dataview::DataView;
            let data_view = DataView::from(bytes);

            if elf.is_64 {
                let mut raw_desc = data_view.read::<PluginDescriptor64>(offset as usize);
                elf_apply_relocs::<u64, _>(
                    elf,
                    sym.st_value..sym.st_value + sym.st_size,
                    &mut raw_desc,
                )?;
                #[rustfmt::skip]
                ret.push(PluginDescriptor{
                    architecture: elf_architecture(elf),
                    plugin_version: raw_desc.plugin_version,
                    name: elf_read_sliceref(bytes, raw_desc.name, raw_desc.name_length as usize)?,
                    version: elf_read_sliceref(bytes, raw_desc.version, raw_desc.version_length as usize)?,
                    description: elf_read_sliceref(bytes, raw_desc.description, raw_desc.description_length as usize)?,
                });
            } else {
                let mut raw_desc = data_view.read::<PluginDescriptor32>(offset as usize);
                elf_apply_relocs::<u32, _>(
                    elf,
                    sym.st_value..sym.st_value + sym.st_size,
                    &mut raw_desc,
                )?;
                #[rustfmt::skip]
                ret.push(PluginDescriptor{
                    architecture: elf_architecture(elf),
                    plugin_version: raw_desc.plugin_version,
                    name: elf_read_sliceref(bytes, raw_desc.name as u64, raw_desc.name_length as usize)?,
                    version: elf_read_sliceref(bytes, raw_desc.version as u64, raw_desc.version_length as usize)?,
                    description: elf_read_sliceref(bytes, raw_desc.description as u64, raw_desc.description_length as usize)?,
                });
            }
        }
    }

    Ok(ret)
}

fn elf_architecture(elf: &Elf) -> PluginArchitecture {
    // https://refspecs.linuxfoundation.org/elf/gabi4+/ch4.eheader.html
    match elf.header.e_machine {
        3 => PluginArchitecture::X86,
        62 => PluginArchitecture::X86_64,
        40 => PluginArchitecture::Arm,
        183 => PluginArchitecture::Arm64,
        _ => PluginArchitecture::Unknown(elf.header.e_machine as u32),
    }
}

fn elf_apply_relocs<N, T>(elf: &Elf, va_range: Range<u64>, obj: &mut T) -> Result<()>
where
    N: Pod + Eq + Zero + NumCast + WrappingAdd + WrappingSub,
    T: Pod,
{
    for section_relocs in elf.shdr_relocs.iter() {
        for reloc in section_relocs.1.iter() {
            if reloc.r_offset >= va_range.start && reloc.r_offset < va_range.end {
                let field_offset = reloc.r_offset - va_range.start;

                use memflow::dataview::DataView;
                let data_view = DataView::from_mut(obj);
                let value = data_view.read::<N>(field_offset as usize);

                // skip over entries that already contain the proper reference
                if value != N::zero() {
                    continue;
                }

                // https://chromium.googlesource.com/android_tools/+/8301b711a9ac7de56e9a9ff3dee0b2ebfc9a380f/ndk/sources/android/crazy_linker/src/crazy_linker_elf_relocations.cpp#36
                // TODO: generalize this check
                if reloc.r_type != 8 && reloc.r_type != 23 && reloc.r_type != 1027 {
                    return Err(Error::Parse(
                        "only relative relocations are supported right now".to_owned(),
                    ));
                }

                // simulate a `wrapping_add_signed`
                let addend = reloc.r_addend.unwrap_or_default();
                let value = if addend > 0 {
                    value.wrapping_add(&(num_traits::cast(addend).unwrap()))
                } else {
                    value.wrapping_sub(&(num_traits::cast(-addend).unwrap()))
                };
                data_view.write::<N>(field_offset as usize, &value);
            }
        }
    }
    Ok(())
}

fn elf_read_sliceref(bytes: &[u8], ptr: u64, len: usize) -> Result<String> {
    if ptr == 0 {
        return Err(Error::Parse(
            "unable to read referenced string in binary".to_owned(),
        ));
    }

    // for elf no further mangling has to be done here
    let offset = ptr as usize;

    let mut buffer = vec![0u8; len];
    buffer.copy_from_slice(&bytes[offset..offset + len]);

    Ok(std::str::from_utf8(&buffer[..])?.to_owned())
}
