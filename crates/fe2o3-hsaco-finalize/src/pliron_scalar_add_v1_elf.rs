//! Checked ELF closure for the measured Pliron scalar-add V1 image.

use std::collections::BTreeSet;

use object::elf;

use crate::{
    DEVICE_DESCRIPTOR_SECTION_NAME, PLIRON_SCALAR_ADD_V1_DESCRIPTOR, PLIRON_SCALAR_ADD_V1_KERNEL,
    PlironScalarAddV1ElfField, PlironScalarAddV1InspectionError,
};

type Result<T> = std::result::Result<T, PlironScalarAddV1InspectionError>;

const ELF_HEADER_SIZE: u64 = 64;
const PROGRAM_HEADER_SIZE: u64 = 56;
const SECTION_HEADER_SIZE: u64 = 64;
const PROGRAM_HEADER_COUNT: u64 = 8;
const SECTION_HEADER_COUNT: u64 = 15;
const SECTION_NAME_TABLE_INDEX: usize = 13;
const SYMBOL_SIZE: u64 = 24;
const DYNAMIC_SIZE: u64 = 16;
const SCALAR_ENTRY_SIZE: u64 = 56;
const TEXT_SIZE: u64 = 0x440;

const PT_GNU_STACK: u32 = 0x6474_e551;
const PT_GNU_RELRO: u32 = 0x6474_e552;
const SHT_GNU_HASH: u32 = 0x6fff_fff6;
const SHT_ANDROID_REL: u32 = 0x6000_0001;
const SHT_ANDROID_RELA: u32 = 0x6000_0002;
const SHT_ANDROID_RELR: u32 = 0x6fff_ff00;

const SHF_WRITE: u64 = 1;
const SHF_ALLOC: u64 = 2;
const SHF_EXECINSTR: u64 = 4;
const SHF_MERGE: u64 = 0x10;
const SHF_STRINGS: u64 = 0x20;
const SHN_UNDEF: u16 = 0;
const SHN_ABS: u16 = 0xfff1;
const SHN_COMMON: u16 = 0xfff2;
const STB_LOCAL: u8 = 0;
const STB_GLOBAL: u8 = 1;
const STT_NOTYPE: u8 = 0;
const STT_OBJECT: u8 = 1;
const STT_FUNC: u8 = 2;
const STV_DEFAULT: u8 = 0;
const STV_HIDDEN: u8 = 2;
const STV_PROTECTED: u8 = 3;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

const NOTE: usize = 1;
const DYNSYM: usize = 2;
const GNU_HASH: usize = 3;
const HASH: usize = 4;
const DYNSTR: usize = 5;
const RODATA: usize = 6;
const TEXT: usize = 7;
const DYNAMIC: usize = 8;
const RELRO_PADDING: usize = 9;
const GPR_MAXIMUMS: usize = 10;
const COMMENT: usize = 11;
const SYMTAB: usize = 12;
const SHSTRTAB: usize = 13;
const STRTAB: usize = 14;

#[derive(Clone, Copy, Debug)]
struct Header {
    program_offset: u64,
    section_offset: u64,
}

#[derive(Clone, Copy, Debug)]
struct Program {
    program_type: u32,
    flags: u32,
    offset: u64,
    virtual_address: u64,
    physical_address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
}

#[derive(Clone, Copy, Debug)]
struct Section {
    name_offset: u32,
    section_type: u32,
    flags: u64,
    address: u64,
    offset: u64,
    size: u64,
    link: u32,
    info: u32,
    alignment: u64,
    entry_size: u64,
}

#[derive(Clone, Debug)]
struct Symbol {
    name: String,
    info: u8,
    other: u8,
    section: u16,
    value: u64,
    size: u64,
}

struct ElfView<'a> {
    bytes: &'a [u8],
    header: Header,
    programs: Vec<Program>,
    sections: Vec<Section>,
    names: Vec<String>,
}

pub(crate) fn validate_scalar_add_v1_elf(bytes: &[u8]) -> Result<()> {
    let view = ElfView::parse(bytes)?;
    view.validate_section_profile()?;
    view.validate_program_profile()?;
    view.validate_dynamic_profile()?;
    view.validate_symbol_profile()?;
    view.validate_executable_profile()
}

impl<'a> ElfView<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self> {
        let header = parse_header(bytes)?;
        let programs = parse_programs(bytes, header)?;
        let sections = parse_sections(bytes, header)?;
        for section in &sections {
            if is_relocation_section(section.section_type) {
                return Err(reject(PlironScalarAddV1ElfField::Relocations));
            }
            if section.section_type != elf::SHT_NOBITS {
                checked_slice(bytes, section.offset, section.size).ok_or_else(object_error)?;
            } else {
                checked_end(section.address, section.size).ok_or_else(object_error)?;
                if section.offset > bytes.len() as u64 {
                    return Err(object_error());
                }
            }
        }
        for program in &programs {
            checked_slice(bytes, program.offset, program.file_size).ok_or_else(object_error)?;
            checked_end(program.virtual_address, program.memory_size).ok_or_else(object_error)?;
        }

        let shstr = sections
            .get(SECTION_NAME_TABLE_INDEX)
            .ok_or_else(object_error)?;
        if shstr.section_type != elf::SHT_STRTAB {
            return Err(object_error());
        }
        let shstr_bytes =
            checked_slice(bytes, shstr.offset, shstr.size).ok_or_else(object_error)?;
        validate_string_table(shstr_bytes)?;
        let names = sections
            .iter()
            .map(|section| read_string(shstr_bytes, u64::from(section.name_offset)))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(object_error)?;
        if names
            .iter()
            .any(|name| name == DEVICE_DESCRIPTOR_SECTION_NAME)
        {
            return Err(reject(
                PlironScalarAddV1ElfField::CanonicalDescriptorSection,
            ));
        }

        let view = Self {
            bytes,
            header,
            programs,
            sections,
            names,
        };
        view.validate_file_layout()?;
        Ok(view)
    }

    fn validate_file_layout(&self) -> Result<()> {
        let section_table_size = SECTION_HEADER_COUNT
            .checked_mul(SECTION_HEADER_SIZE)
            .ok_or_else(object_error)?;
        if checked_end(self.header.section_offset, section_table_size)
            != Some(self.bytes.len() as u64)
            || !self.header.section_offset.is_multiple_of(8)
        {
            return Err(object_error());
        }
        let mut ranges = self
            .sections
            .iter()
            .enumerate()
            .filter(|(_, section)| section.section_type != elf::SHT_NOBITS && section.size != 0)
            .map(|(index, section)| {
                checked_end(section.offset, section.size)
                    .map(|end| (section.offset, end, index))
                    .ok_or_else(object_error)
            })
            .collect::<Result<Vec<_>>>()?;
        ranges.sort_unstable();
        for adjacent in ranges.windows(2) {
            if adjacent[0].1 > adjacent[1].0 {
                return Err(object_error());
            }
        }
        if ranges
            .last()
            .is_some_and(|(_, end, _)| *end > self.header.section_offset)
        {
            return Err(object_error());
        }
        Ok(())
    }

    fn validate_section_profile(&self) -> Result<()> {
        const EXPECTED: [(&str, u32, u64, u32, u32, u64, u64); 15] = [
            ("", elf::SHT_NULL, 0, 0, 0, 0, 0),
            (".note", elf::SHT_NOTE, SHF_ALLOC, 0, 0, 4, 0),
            (
                ".dynsym",
                elf::SHT_DYNSYM,
                SHF_ALLOC,
                DYNSTR as u32,
                1,
                8,
                SYMBOL_SIZE,
            ),
            (".gnu.hash", SHT_GNU_HASH, SHF_ALLOC, DYNSYM as u32, 0, 8, 0),
            (".hash", elf::SHT_HASH, SHF_ALLOC, DYNSYM as u32, 0, 4, 4),
            (".dynstr", elf::SHT_STRTAB, SHF_ALLOC, 0, 0, 1, 0),
            (".rodata", elf::SHT_PROGBITS, SHF_ALLOC, 0, 0, 64, 0),
            (
                ".text",
                elf::SHT_PROGBITS,
                SHF_ALLOC | SHF_EXECINSTR,
                0,
                0,
                256,
                0,
            ),
            (
                ".dynamic",
                elf::SHT_DYNAMIC,
                SHF_WRITE | SHF_ALLOC,
                DYNSTR as u32,
                0,
                8,
                DYNAMIC_SIZE,
            ),
            (
                ".relro_padding",
                elf::SHT_NOBITS,
                SHF_WRITE | SHF_ALLOC,
                0,
                0,
                1,
                0,
            ),
            (".AMDGPU.gpr_maximums", elf::SHT_PROGBITS, 0, 0, 0, 1, 0),
            (
                ".comment",
                elf::SHT_PROGBITS,
                SHF_MERGE | SHF_STRINGS,
                0,
                0,
                1,
                1,
            ),
            (
                ".symtab",
                elf::SHT_SYMTAB,
                0,
                STRTAB as u32,
                10,
                8,
                SYMBOL_SIZE,
            ),
            (".shstrtab", elf::SHT_STRTAB, 0, 0, 0, 1, 0),
            (".strtab", elf::SHT_STRTAB, 0, 0, 0, 1, 0),
        ];
        if self.sections.len() != EXPECTED.len() || self.names.len() != EXPECTED.len() {
            return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
        }
        let mut unique = BTreeSet::new();
        for (index, (name, section_type, flags, link, info, alignment, entry_size)) in
            EXPECTED.into_iter().enumerate()
        {
            let section = self.sections[index];
            if self.names[index] != name
                || !unique.insert(self.names[index].as_str())
                || section.section_type != section_type
                || section.flags != flags
                || section.link != link
                || section.info != info
                || section.alignment != alignment
                || section.entry_size != entry_size
            {
                return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
            }
        }
        let null = self.sections[0];
        if null.name_offset != 0 || null.address != 0 || null.offset != 0 || null.size != 0 {
            return Err(object_error());
        }
        for &index in &[GPR_MAXIMUMS, COMMENT, SYMTAB, SHSTRTAB, STRTAB] {
            if self.sections[index].address != 0 {
                return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
            }
        }
        if self.sections[NOTE].size == 0
            || self.sections[DYNSYM].size != 3 * SYMBOL_SIZE
            || self.sections[GNU_HASH].size != 36
            || self.sections[HASH].size != 32
            || self.sections[DYNSTR].size != 26
            || self.sections[RODATA].size != 64
            || self.sections[TEXT].size != TEXT_SIZE
            || self.sections[DYNAMIC].size != 8 * DYNAMIC_SIZE
            || self.sections[RELRO_PADDING].size == 0
            || self.sections[GPR_MAXIMUMS].size != 0
            || self.sections[COMMENT].size == 0
            || self.sections[SYMTAB].size != 12 * SYMBOL_SIZE
        {
            return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
        }
        Ok(())
    }

    fn validate_program_profile(&self) -> Result<()> {
        if self.programs.len() != PROGRAM_HEADER_COUNT as usize {
            return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
        }
        let note = self.sections[NOTE];
        let rodata = self.sections[RODATA];
        let text = self.sections[TEXT];
        let dynamic = self.sections[DYNAMIC];
        let padding = self.sections[RELRO_PADDING];
        let phdr_size = PROGRAM_HEADER_COUNT
            .checked_mul(PROGRAM_HEADER_SIZE)
            .ok_or_else(object_error)?;
        let read_end = checked_end(rodata.offset, rodata.size).ok_or_else(object_error)?;
        let dynamic_end = checked_end(dynamic.address, dynamic.size).ok_or_else(object_error)?;
        let relro_size = dynamic
            .size
            .checked_add(padding.size)
            .ok_or_else(object_error)?;

        require_program(
            self.programs[0],
            elf::PT_PHDR,
            PF_R,
            self.header.program_offset,
            self.header.program_offset,
            phdr_size,
            phdr_size,
            8,
        )?;
        require_program(
            self.programs[1],
            elf::PT_LOAD,
            PF_R,
            0,
            0,
            text.offset,
            text.offset,
            0x1000,
        )?;
        if read_end > text.offset {
            return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
        }
        require_program(
            self.programs[2],
            elf::PT_LOAD,
            PF_R | PF_X,
            text.offset,
            text.address,
            text.size,
            text.size,
            0x1000,
        )?;
        require_program(
            self.programs[3],
            elf::PT_LOAD,
            PF_R | PF_W,
            dynamic.offset,
            dynamic.address,
            dynamic.size,
            relro_size,
            0x1000,
        )?;
        require_program(
            self.programs[4],
            elf::PT_DYNAMIC,
            PF_R | PF_W,
            dynamic.offset,
            dynamic.address,
            dynamic.size,
            dynamic.size,
            8,
        )?;
        require_program(
            self.programs[5],
            PT_GNU_RELRO,
            PF_R,
            dynamic.offset,
            dynamic.address,
            dynamic.size,
            relro_size,
            1,
        )?;
        require_program(self.programs[6], PT_GNU_STACK, PF_R | PF_W, 0, 0, 0, 0, 0)?;
        require_program(
            self.programs[7],
            elf::PT_NOTE,
            PF_R,
            note.offset,
            note.address,
            note.size,
            note.size,
            4,
        )?;

        if text.address.checked_sub(text.offset) != Some(0x1000)
            || dynamic.address.checked_sub(dynamic.offset) != Some(0x2000)
            || padding.offset != checked_end(dynamic.offset, dynamic.size).unwrap_or(u64::MAX)
            || padding.address != dynamic_end
            || checked_end(dynamic.address, relro_size).is_none_or(|end| end % 0x1000 != 0)
        {
            return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
        }
        for (index, section) in self.sections.iter().copied().enumerate() {
            if section.flags & SHF_ALLOC == 0 || section.size == 0 {
                continue;
            }
            let expected_flags = if index == TEXT {
                PF_R | PF_X
            } else if index == DYNAMIC || index == RELRO_PADDING {
                PF_R | PF_W
            } else {
                PF_R
            };
            let matching = self.programs[1..=3]
                .iter()
                .filter(|program| section_matches_load(section, program))
                .count();
            if matching != 1
                || !self.programs[1..=3].iter().any(|program| {
                    program.flags == expected_flags && section_matches_load(section, program)
                })
            {
                return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
            }
        }
        Ok(())
    }

    fn validate_dynamic_profile(&self) -> Result<()> {
        let dynstr = self.section_bytes(DYNSTR)?;
        if dynstr != b"\0scalar_add\0scalar_add.kd\0" {
            return Err(reject(PlironScalarAddV1ElfField::DefinedSymbols));
        }

        let hash = self.section_bytes(HASH)?;
        let expected_hash = [3_u32, 3, 0, 1, 2, 0, 0, 0];
        if hash.len() != expected_hash.len() * 4
            || hash
                .chunks_exact(4)
                .zip(expected_hash)
                .any(|(word, expected)| read_word(word) != Some(expected))
        {
            return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
        }

        let gnu_hash = self.section_bytes(GNU_HASH)?;
        let entry_hash = gnu_symbol_hash(PLIRON_SCALAR_ADD_V1_KERNEL.as_bytes());
        let descriptor_hash = gnu_symbol_hash(PLIRON_SCALAR_ADD_V1_DESCRIPTOR.as_bytes());
        let bloom = (1_u64 << (entry_hash % 64))
            | (1_u64 << ((entry_hash >> 26) % 64))
            | (1_u64 << (descriptor_hash % 64))
            | (1_u64 << ((descriptor_hash >> 26) % 64));
        if read_u32(gnu_hash, 0) != Some(1)
            || read_u32(gnu_hash, 4) != Some(1)
            || read_u32(gnu_hash, 8) != Some(1)
            || read_u32(gnu_hash, 12) != Some(26)
            || read_u64(gnu_hash, 16) != Some(bloom)
            || read_u32(gnu_hash, 24) != Some(1)
            || read_u32(gnu_hash, 28) != Some(entry_hash & !1)
            || read_u32(gnu_hash, 32) != Some(descriptor_hash | 1)
        {
            return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
        }

        let dynamic = self.section_bytes(DYNAMIC)?;
        const TAGS: [i64; 8] = [
            elf::DT_FLAGS,
            elf::DT_SYMTAB,
            elf::DT_SYMENT,
            elf::DT_STRTAB,
            elf::DT_STRSZ,
            elf::DT_GNU_HASH,
            elf::DT_HASH,
            elf::DT_NULL,
        ];
        let values = [
            elf::DF_SYMBOLIC as u64,
            self.sections[DYNSYM].address,
            SYMBOL_SIZE,
            self.sections[DYNSTR].address,
            self.sections[DYNSTR].size,
            self.sections[GNU_HASH].address,
            self.sections[HASH].address,
            0,
        ];
        if dynamic.len() != TAGS.len() * DYNAMIC_SIZE as usize {
            return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
        }
        for (index, (tag, expected_value)) in TAGS.into_iter().zip(values).enumerate() {
            let offset = index
                .checked_mul(DYNAMIC_SIZE as usize)
                .ok_or_else(object_error)?;
            let observed_tag = read_i64(dynamic, offset).ok_or_else(object_error)?;
            let observed_value =
                read_u64(dynamic, checked_usize_add(offset, 8)?).ok_or_else(object_error)?;
            if observed_tag == elf::DT_NEEDED {
                return Err(reject(PlironScalarAddV1ElfField::UndefinedSymbols));
            }
            if is_relocation_dynamic_tag(observed_tag) {
                return Err(reject(PlironScalarAddV1ElfField::Relocations));
            }
            if observed_tag != tag || observed_value != expected_value {
                return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
            }
        }
        Ok(())
    }

    fn validate_symbol_profile(&self) -> Result<()> {
        let static_symbols = self.symbols(SYMTAB, STRTAB)?;
        let dynamic_symbols = self.symbols(DYNSYM, DYNSTR)?;
        if static_symbols.len() != 12 || dynamic_symbols.len() != 3 {
            return Err(reject(PlironScalarAddV1ElfField::DefinedSymbols));
        }
        validate_null_symbol(&static_symbols[0])?;
        validate_null_symbol(&dynamic_symbols[0])?;
        for symbol in static_symbols[1..].iter().chain(&dynamic_symbols[1..]) {
            if symbol.section == SHN_UNDEF {
                return Err(reject(PlironScalarAddV1ElfField::UndefinedSymbols));
            }
            if symbol.section == SHN_COMMON {
                return Err(reject(PlironScalarAddV1ElfField::DefinedSymbols));
            }
        }

        const RESOURCES: [(&str, u64); 8] = [
            ("scalar_add.private_seg_size", 0),
            ("scalar_add.num_vgpr", 2),
            ("scalar_add.num_agpr", 0),
            ("scalar_add.numbered_sgpr", 8),
            ("scalar_add.uses_vcc", 0),
            ("scalar_add.uses_flat_scratch", 0),
            ("scalar_add.has_dyn_sized_stack", 0),
            ("scalar_add.has_recursion", 0),
        ];
        for (symbol, (name, value)) in static_symbols[1..9].iter().zip(RESOURCES) {
            require_symbol(
                symbol,
                name,
                STB_LOCAL,
                STT_NOTYPE,
                STV_DEFAULT,
                SHN_ABS,
                value,
                0,
            )?;
        }
        require_symbol(
            &static_symbols[9],
            "_DYNAMIC",
            STB_LOCAL,
            STT_NOTYPE,
            STV_HIDDEN,
            DYNAMIC as u16,
            self.sections[DYNAMIC].address,
            0,
        )?;
        require_symbol(
            &static_symbols[10],
            PLIRON_SCALAR_ADD_V1_KERNEL,
            STB_GLOBAL,
            STT_FUNC,
            STV_PROTECTED,
            TEXT as u16,
            self.sections[TEXT].address,
            SCALAR_ENTRY_SIZE,
        )?;
        require_symbol(
            &static_symbols[11],
            PLIRON_SCALAR_ADD_V1_DESCRIPTOR,
            STB_GLOBAL,
            STT_OBJECT,
            STV_DEFAULT,
            RODATA as u16,
            self.sections[RODATA].address,
            64,
        )?;
        require_symbol(
            &dynamic_symbols[1],
            PLIRON_SCALAR_ADD_V1_KERNEL,
            STB_GLOBAL,
            STT_FUNC,
            STV_PROTECTED,
            TEXT as u16,
            self.sections[TEXT].address,
            SCALAR_ENTRY_SIZE,
        )?;
        require_symbol(
            &dynamic_symbols[2],
            PLIRON_SCALAR_ADD_V1_DESCRIPTOR,
            STB_GLOBAL,
            STT_OBJECT,
            STV_DEFAULT,
            RODATA as u16,
            self.sections[RODATA].address,
            64,
        )?;
        Ok(())
    }

    fn validate_executable_profile(&self) -> Result<()> {
        let text = self.sections[TEXT];
        let bytes = self.section_bytes(TEXT)?;
        let entry_size = usize::try_from(SCALAR_ENTRY_SIZE).map_err(|_| object_error())?;
        let padding = bytes
            .get(entry_size..)
            .ok_or_else(|| reject(PlironScalarAddV1ElfField::ExecutableRange))?;
        if text.address == 0
            || bytes.get(..entry_size).is_none()
            || !padding.len().is_multiple_of(4)
            || padding
                .chunks_exact(4)
                .any(|word| word != [0, 0, 0x80, 0xbf])
            || self
                .sections
                .iter()
                .enumerate()
                .any(|(index, section)| index != TEXT && section.flags & SHF_EXECINSTR != 0)
        {
            return Err(reject(PlironScalarAddV1ElfField::ExecutableRange));
        }
        Ok(())
    }

    fn section_bytes(&self, index: usize) -> Result<&'a [u8]> {
        let section = self.sections.get(index).ok_or_else(object_error)?;
        checked_slice(self.bytes, section.offset, section.size).ok_or_else(object_error)
    }

    fn symbols(&self, table_index: usize, strings_index: usize) -> Result<Vec<Symbol>> {
        let table = self.sections[table_index];
        let strings = self.section_bytes(strings_index)?;
        validate_string_table(strings)?;
        if table.entry_size != SYMBOL_SIZE || !table.size.is_multiple_of(SYMBOL_SIZE) {
            return Err(reject(PlironScalarAddV1ElfField::DefinedSymbols));
        }
        let bytes = self.section_bytes(table_index)?;
        let count = table.size / SYMBOL_SIZE;
        let capacity = usize::try_from(count).map_err(|_| object_error())?;
        let mut result = Vec::with_capacity(capacity);
        for index in 0..count {
            let offset = index.checked_mul(SYMBOL_SIZE).ok_or_else(object_error)?;
            let name_offset = read_u32_u64(bytes, offset)?;
            let info = read_u8_u64(bytes, checked_end(offset, 4).ok_or_else(object_error)?)?;
            let other = read_u8_u64(bytes, checked_end(offset, 5).ok_or_else(object_error)?)?;
            let section = read_u16_u64(bytes, checked_end(offset, 6).ok_or_else(object_error)?)?;
            let value = read_u64_u64(bytes, checked_end(offset, 8).ok_or_else(object_error)?)?;
            let size = read_u64_u64(bytes, checked_end(offset, 16).ok_or_else(object_error)?)?;
            let name = read_string(strings, u64::from(name_offset))
                .ok_or_else(|| reject(PlironScalarAddV1ElfField::DefinedSymbols))?;
            result.push(Symbol {
                name,
                info,
                other,
                section,
                value,
                size,
            });
        }
        Ok(result)
    }
}

fn parse_header(bytes: &[u8]) -> Result<Header> {
    let ident = checked_slice(bytes, 0, 16).ok_or_else(object_error)?;
    if ident.get(..9) != Some(b"\x7fELF\x02\x01\x01\x40\x04")
        || ident
            .get(9..16)
            .is_none_or(|padding| padding.iter().any(|byte| *byte != 0))
        || read_u16_u64(bytes, 16)? != elf::ET_DYN
        || read_u16_u64(bytes, 18)? != elf::EM_AMDGPU
        || read_u32_u64(bytes, 20)? != 1
        || read_u64_u64(bytes, 24)? != 0
        || read_u32_u64(bytes, 48)? != 0x64c
        || read_u16_u64(bytes, 52)? != ELF_HEADER_SIZE as u16
        || read_u16_u64(bytes, 54)? != PROGRAM_HEADER_SIZE as u16
        || read_u16_u64(bytes, 56)? != PROGRAM_HEADER_COUNT as u16
        || read_u16_u64(bytes, 58)? != SECTION_HEADER_SIZE as u16
        || read_u16_u64(bytes, 60)? != SECTION_HEADER_COUNT as u16
        || read_u16_u64(bytes, 62)? != SECTION_NAME_TABLE_INDEX as u16
    {
        return Err(object_error());
    }
    let program_offset = read_u64_u64(bytes, 32)?;
    let section_offset = read_u64_u64(bytes, 40)?;
    if program_offset != ELF_HEADER_SIZE {
        return Err(object_error());
    }
    let program_size = PROGRAM_HEADER_COUNT
        .checked_mul(PROGRAM_HEADER_SIZE)
        .ok_or_else(object_error)?;
    checked_slice(bytes, program_offset, program_size).ok_or_else(object_error)?;
    let section_size = SECTION_HEADER_COUNT
        .checked_mul(SECTION_HEADER_SIZE)
        .ok_or_else(object_error)?;
    checked_slice(bytes, section_offset, section_size).ok_or_else(object_error)?;
    Ok(Header {
        program_offset,
        section_offset,
    })
}

fn parse_programs(bytes: &[u8], header: Header) -> Result<Vec<Program>> {
    let mut programs = Vec::with_capacity(PROGRAM_HEADER_COUNT as usize);
    for index in 0..PROGRAM_HEADER_COUNT {
        let offset = record_offset(header.program_offset, index, PROGRAM_HEADER_SIZE)?;
        programs.push(Program {
            program_type: read_u32_u64(bytes, offset)?,
            flags: read_u32_u64(bytes, checked_end(offset, 4).ok_or_else(object_error)?)?,
            offset: read_u64_u64(bytes, checked_end(offset, 8).ok_or_else(object_error)?)?,
            virtual_address: read_u64_u64(
                bytes,
                checked_end(offset, 16).ok_or_else(object_error)?,
            )?,
            physical_address: read_u64_u64(
                bytes,
                checked_end(offset, 24).ok_or_else(object_error)?,
            )?,
            file_size: read_u64_u64(bytes, checked_end(offset, 32).ok_or_else(object_error)?)?,
            memory_size: read_u64_u64(bytes, checked_end(offset, 40).ok_or_else(object_error)?)?,
            alignment: read_u64_u64(bytes, checked_end(offset, 48).ok_or_else(object_error)?)?,
        });
    }
    Ok(programs)
}

fn parse_sections(bytes: &[u8], header: Header) -> Result<Vec<Section>> {
    let mut sections = Vec::with_capacity(SECTION_HEADER_COUNT as usize);
    for index in 0..SECTION_HEADER_COUNT {
        let offset = record_offset(header.section_offset, index, SECTION_HEADER_SIZE)?;
        sections.push(Section {
            name_offset: read_u32_u64(bytes, offset)?,
            section_type: read_u32_u64(bytes, checked_end(offset, 4).ok_or_else(object_error)?)?,
            flags: read_u64_u64(bytes, checked_end(offset, 8).ok_or_else(object_error)?)?,
            address: read_u64_u64(bytes, checked_end(offset, 16).ok_or_else(object_error)?)?,
            offset: read_u64_u64(bytes, checked_end(offset, 24).ok_or_else(object_error)?)?,
            size: read_u64_u64(bytes, checked_end(offset, 32).ok_or_else(object_error)?)?,
            link: read_u32_u64(bytes, checked_end(offset, 40).ok_or_else(object_error)?)?,
            info: read_u32_u64(bytes, checked_end(offset, 44).ok_or_else(object_error)?)?,
            alignment: read_u64_u64(bytes, checked_end(offset, 48).ok_or_else(object_error)?)?,
            entry_size: read_u64_u64(bytes, checked_end(offset, 56).ok_or_else(object_error)?)?,
        });
    }
    Ok(sections)
}

#[allow(clippy::too_many_arguments)]
fn require_program(
    program: Program,
    program_type: u32,
    flags: u32,
    offset: u64,
    address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
) -> Result<()> {
    if program.program_type != program_type
        || program.flags != flags
        || program.offset != offset
        || program.virtual_address != address
        || program.physical_address != address
        || program.file_size != file_size
        || program.memory_size != memory_size
        || program.alignment != alignment
    {
        return Err(reject(PlironScalarAddV1ElfField::DynamicLoader));
    }
    Ok(())
}

fn section_matches_load(section: Section, program: &Program) -> bool {
    if program.program_type != elf::PT_LOAD || section.address < program.virtual_address {
        return false;
    }
    let Some(delta) = section.address.checked_sub(program.virtual_address) else {
        return false;
    };
    let Some(memory_end) = checked_end(delta, section.size) else {
        return false;
    };
    if memory_end > program.memory_size {
        return false;
    }
    if section.section_type == elf::SHT_NOBITS {
        return true;
    }
    let Some(expected_offset) = program.offset.checked_add(delta) else {
        return false;
    };
    let Some(file_end) = checked_end(delta, section.size) else {
        return false;
    };
    expected_offset == section.offset && file_end <= program.file_size
}

#[allow(clippy::too_many_arguments)]
fn require_symbol(
    symbol: &Symbol,
    name: &str,
    binding: u8,
    symbol_type: u8,
    visibility: u8,
    section: u16,
    value: u64,
    size: u64,
) -> Result<()> {
    if symbol.name != name
        || symbol.info != (binding << 4) | symbol_type
        || symbol.other != visibility
        || symbol.section != section
        || symbol.value != value
        || symbol.size != size
    {
        return Err(reject(PlironScalarAddV1ElfField::DefinedSymbols));
    }
    Ok(())
}

fn validate_null_symbol(symbol: &Symbol) -> Result<()> {
    if !symbol.name.is_empty()
        || symbol.info != 0
        || symbol.other != 0
        || symbol.section != SHN_UNDEF
        || symbol.value != 0
        || symbol.size != 0
    {
        return Err(reject(PlironScalarAddV1ElfField::DefinedSymbols));
    }
    Ok(())
}

fn validate_string_table(bytes: &[u8]) -> Result<()> {
    if bytes.first() != Some(&0) || bytes.last() != Some(&0) {
        return Err(object_error());
    }
    Ok(())
}

fn read_string(bytes: &[u8], offset: u64) -> Option<String> {
    let start = usize::try_from(offset).ok()?;
    let tail = bytes.get(start..)?;
    let length = tail.iter().position(|byte| *byte == 0)?;
    let value = tail.get(..length)?;
    if value.iter().any(|byte| !byte.is_ascii_graphic()) {
        return None;
    }
    Some(std::str::from_utf8(value).ok()?.to_owned())
}

fn is_relocation_section(section_type: u32) -> bool {
    matches!(
        section_type,
        elf::SHT_REL
            | elf::SHT_RELA
            | elf::SHT_RELR
            | elf::SHT_CREL
            | SHT_ANDROID_REL
            | SHT_ANDROID_RELA
            | SHT_ANDROID_RELR
    )
}

fn is_relocation_dynamic_tag(tag: i64) -> bool {
    matches!(
        tag,
        elf::DT_PLTRELSZ
            | elf::DT_RELA
            | elf::DT_RELASZ
            | elf::DT_RELAENT
            | elf::DT_REL
            | elf::DT_RELSZ
            | elf::DT_RELENT
            | elf::DT_PLTREL
            | elf::DT_JMPREL
            | elf::DT_TEXTREL
            | elf::DT_RELRSZ
            | elf::DT_RELR
            | elf::DT_RELRENT
            | elf::DT_ANDROID_REL
            | elf::DT_ANDROID_RELSZ
            | elf::DT_ANDROID_RELA
            | elf::DT_ANDROID_RELASZ
            | elf::DT_ANDROID_RELR
            | elf::DT_ANDROID_RELRSZ
            | elf::DT_ANDROID_RELRENT
            | elf::DT_RELACOUNT
            | elf::DT_RELCOUNT
    )
}

fn gnu_symbol_hash(name: &[u8]) -> u32 {
    name.iter().fold(5381_u32, |hash, byte| {
        hash.wrapping_mul(33).wrapping_add(u32::from(*byte))
    })
}

fn read_word(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn checked_slice(bytes: &[u8], offset: u64, size: u64) -> Option<&[u8]> {
    let start = usize::try_from(offset).ok()?;
    let size = usize::try_from(size).ok()?;
    let end = start.checked_add(size)?;
    bytes.get(start..end)
}

fn checked_end(start: u64, size: u64) -> Option<u64> {
    start.checked_add(size)
}

fn checked_usize_add(start: usize, size: usize) -> Result<usize> {
    start.checked_add(size).ok_or_else(object_error)
}

fn record_offset(base: u64, index: u64, size: u64) -> Result<u64> {
    index
        .checked_mul(size)
        .and_then(|delta| base.checked_add(delta))
        .ok_or_else(object_error)
}

fn read_u8_u64(bytes: &[u8], offset: u64) -> Result<u8> {
    let index = usize::try_from(offset).map_err(|_| object_error())?;
    bytes.get(index).copied().ok_or_else(object_error)
}

fn read_u16_u64(bytes: &[u8], offset: u64) -> Result<u16> {
    let value = checked_slice(bytes, offset, 2).ok_or_else(object_error)?;
    Ok(u16::from_le_bytes(
        value.try_into().map_err(|_| object_error())?,
    ))
}

fn read_u32_u64(bytes: &[u8], offset: u64) -> Result<u32> {
    let value = checked_slice(bytes, offset, 4).ok_or_else(object_error)?;
    Ok(u32::from_le_bytes(
        value.try_into().map_err(|_| object_error())?,
    ))
}

fn read_u64_u64(bytes: &[u8], offset: u64) -> Result<u64> {
    let value = checked_slice(bytes, offset, 8).ok_or_else(object_error)?;
    Ok(u64::from_le_bytes(
        value.try_into().map_err(|_| object_error())?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    Some(u64::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_i64(bytes: &[u8], offset: usize) -> Option<i64> {
    let end = offset.checked_add(8)?;
    Some(i64::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn reject(field: PlironScalarAddV1ElfField) -> PlironScalarAddV1InspectionError {
    PlironScalarAddV1InspectionError::ElfProfile(field)
}

fn object_error() -> PlironScalarAddV1InspectionError {
    reject(PlironScalarAddV1ElfField::Object)
}
