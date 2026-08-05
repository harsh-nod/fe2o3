use core::str;

use object::elf;

use crate::{
    CodeObjectVersion, InspectedHsaco, InspectedKernel, KernelBindingError, MAX_ELF_SECTIONS,
    MAX_ELF_SEGMENTS, MAX_ELF_SYMBOLS, MAX_HSACO_BYTES, MAX_MESSAGEPACK_STRING_BYTES,
};

const ELF64_HEADER_BYTES: usize = 64;
const ELF64_PROGRAM_HEADER_BYTES: usize = 56;
const ELF64_SECTION_HEADER_BYTES: usize = 64;
const ELF64_SYMBOL_BYTES: usize = 24;
const AMDHSA_KERNEL_DESCRIPTOR_BYTES: usize = 64;
const AMDHSA_KERNEL_ENTRY_ALIGNMENT: u64 = 256;
const SHF_ALLOC: u64 = 0x2;
const SHF_EXECINSTR: u64 = 0x4;

const PF_X: u32 = 0x1;
const PF_W: u32 = 0x2;
const PF_R: u32 = 0x4;

const STT_OBJECT: u8 = 1;
const STT_FUNC: u8 = 2;
const STB_GLOBAL: u8 = 1;
const STB_WEAK: u8 = 2;
const SHN_LORESERVE: u16 = 0xff00;
const SHN_XINDEX: u16 = 0xffff;

const PROPERTY_ENABLE_SGPR_PRIVATE_SEGMENT_BUFFER: u16 = 1 << 0;
const PROPERTY_ENABLE_SGPR_FLAT_SCRATCH_INIT: u16 = 1 << 5;
const PROPERTY_WAVEFRONT_SIZE32: u16 = 1 << 10;
const PROPERTY_USES_DYNAMIC_STACK: u16 = 1 << 11;
const PROPERTY_RESERVED_MASK: u16 = 0xf380;

/// Raw fields from one pinned-layout 64-byte AMDHSA kernel descriptor.
///
/// The fields are descriptive evidence from the input bytes. This value is not
/// a launch token and does not attest the compiler or code object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmdhsaKernelDescriptor {
    group_segment_fixed_size: u32,
    private_segment_fixed_size: u32,
    kernarg_size: u32,
    kernel_code_entry_byte_offset: i64,
    compute_pgm_rsrc3: u32,
    compute_pgm_rsrc1: u32,
    compute_pgm_rsrc2: u32,
    kernel_code_properties: u16,
    kernarg_preload: u16,
}

impl AmdhsaKernelDescriptor {
    pub const fn group_segment_fixed_size(self) -> u32 {
        self.group_segment_fixed_size
    }

    pub const fn private_segment_fixed_size(self) -> u32 {
        self.private_segment_fixed_size
    }

    pub const fn kernarg_size(self) -> u32 {
        self.kernarg_size
    }

    pub const fn kernel_code_entry_byte_offset(self) -> i64 {
        self.kernel_code_entry_byte_offset
    }

    pub const fn compute_pgm_rsrc3(self) -> u32 {
        self.compute_pgm_rsrc3
    }

    pub const fn compute_pgm_rsrc1(self) -> u32 {
        self.compute_pgm_rsrc1
    }

    pub const fn compute_pgm_rsrc2(self) -> u32 {
        self.compute_pgm_rsrc2
    }

    pub const fn kernel_code_properties(self) -> u16 {
        self.kernel_code_properties
    }

    pub const fn kernarg_preload(self) -> u16 {
        self.kernarg_preload
    }

    pub const fn wavefront_size(self) -> u32 {
        if self.kernel_code_properties & PROPERTY_WAVEFRONT_SIZE32 != 0 {
            32
        } else {
            64
        }
    }

    pub const fn uses_dynamic_stack(self) -> bool {
        self.kernel_code_properties & PROPERTY_USES_DYNAMIC_STACK != 0
    }

    pub const fn private_segment_enabled(self) -> bool {
        self.compute_pgm_rsrc2 & 1 != 0
    }
}

/// One exact metadata-name-to-ELF-symbol binding.
///
/// Bindings are returned in metadata kernel order and carry no load or launch
/// authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelDescriptorBinding {
    kernel_index: usize,
    descriptor_address: u64,
    descriptor_file_offset: u64,
    entry_address: u64,
    entry_file_offset: u64,
    entry_size: u64,
    descriptor: AmdhsaKernelDescriptor,
}

impl KernelDescriptorBinding {
    pub const fn kernel_index(self) -> usize {
        self.kernel_index
    }

    pub const fn descriptor_address(self) -> u64 {
        self.descriptor_address
    }

    pub const fn descriptor_file_offset(self) -> u64 {
        self.descriptor_file_offset
    }

    pub const fn entry_address(self) -> u64 {
        self.entry_address
    }

    pub const fn entry_file_offset(self) -> u64 {
        self.entry_file_offset
    }

    pub const fn entry_size(self) -> u64 {
        self.entry_size
    }

    pub const fn descriptor(self) -> AmdhsaKernelDescriptor {
        self.descriptor
    }
}

/// Metadata inspection plus explicit descriptive ELF kernel bindings.
///
/// This result cannot load a module or authorize a dispatch. A later sealed
/// artifact layer must bind it to compiler identity, payload identity, target
/// compatibility, and runtime module state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectedKernelBindings {
    inspection: InspectedHsaco,
    bindings: Vec<KernelDescriptorBinding>,
}

impl InspectedKernelBindings {
    pub fn inspection(&self) -> &InspectedHsaco {
        &self.inspection
    }

    pub fn bindings(&self) -> &[KernelDescriptorBinding] {
        &self.bindings
    }
}

#[derive(Clone, Copy)]
struct Section {
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

#[derive(Clone, Copy)]
struct LoadSegment {
    flags: u32,
    offset: u64,
    address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
}

#[derive(Clone, Copy)]
struct Symbol<'a> {
    name: &'a [u8],
    binding: u8,
    symbol_type: u8,
    other: u8,
    section_index: u16,
    value: u64,
    size: u64,
}

pub(crate) fn bind(
    bytes: &[u8],
    inspection: InspectedHsaco,
) -> Result<InspectedKernelBindings, KernelBindingError> {
    if inspection.kernels().is_empty() {
        return Ok(InspectedKernelBindings {
            inspection,
            bindings: Vec::new(),
        });
    }

    let sections = parse_sections(bytes)?;
    let symbols = parse_symbols(bytes, &sections)?;
    let loads = parse_load_segments(bytes)?;
    let mut bindings = Vec::with_capacity(inspection.kernels().len());

    for (kernel_index, kernel) in inspection.kernels().iter().enumerate() {
        bindings.push(bind_kernel(
            bytes,
            &sections,
            &loads,
            &symbols,
            inspection.code_object_version(),
            inspection.target().processor(),
            kernel_index,
            kernel,
        )?);
    }

    Ok(InspectedKernelBindings {
        inspection,
        bindings,
    })
}

fn parse_sections(bytes: &[u8]) -> Result<Vec<Section>, KernelBindingError> {
    if bytes.len() < ELF64_HEADER_BYTES || bytes.len() > MAX_HSACO_BYTES {
        return Err(KernelBindingError::InvalidSymbolTable(
            "invalid ELF byte length",
        ));
    }
    let offset = read_u64(bytes, 40)?;
    let entry_size = usize::from(read_u16(bytes, 58)?);
    let count = usize::from(read_u16(bytes, 60)?);
    if count == 0 {
        return Err(KernelBindingError::InvalidSymbolTable(
            "section table is missing",
        ));
    }
    if count > MAX_ELF_SECTIONS || entry_size != ELF64_SECTION_HEADER_BYTES {
        return Err(KernelBindingError::InvalidSymbolTable(
            "invalid section table dimensions",
        ));
    }
    let offset = usize_from_u64(offset, "section table offset overflows usize")?;
    checked_table_range(
        bytes.len(),
        offset,
        entry_size,
        count,
        "section table is out of bounds",
    )?;
    let section_zero = bytes
        .get(offset..offset + ELF64_SECTION_HEADER_BYTES)
        .ok_or(KernelBindingError::InvalidSymbolTable(
            "section header zero is out of bounds",
        ))?;
    if section_zero.iter().any(|byte| *byte != 0) {
        return Err(KernelBindingError::InvalidSymbolTable(
            "section header zero is not an all-zero SHT_NULL record",
        ));
    }

    let mut sections = Vec::with_capacity(count);
    for index in 0..count {
        let base = offset
            .checked_add(index.checked_mul(entry_size).ok_or(
                KernelBindingError::InvalidSymbolTable("section index overflow"),
            )?)
            .ok_or(KernelBindingError::InvalidSymbolTable(
                "section offset overflow",
            ))?;
        sections.push(Section {
            section_type: read_u32(bytes, base + 4)?,
            flags: read_u64(bytes, base + 8)?,
            address: read_u64(bytes, base + 16)?,
            offset: read_u64(bytes, base + 24)?,
            size: read_u64(bytes, base + 32)?,
            link: read_u32(bytes, base + 40)?,
            info: read_u32(bytes, base + 44)?,
            alignment: read_u64(bytes, base + 48)?,
            entry_size: read_u64(bytes, base + 56)?,
        });
    }
    Ok(sections)
}

fn parse_load_segments(bytes: &[u8]) -> Result<Vec<LoadSegment>, KernelBindingError> {
    let offset = usize_from_u64(read_u64(bytes, 32)?, "program table offset overflows usize")?;
    let entry_size = usize::from(read_u16(bytes, 54)?);
    let count = usize::from(read_u16(bytes, 56)?);
    if count == 0 || count > MAX_ELF_SEGMENTS || entry_size != ELF64_PROGRAM_HEADER_BYTES {
        return Err(KernelBindingError::InvalidLoadMapping(
            "invalid program table dimensions",
        ));
    }
    checked_table_range(
        bytes.len(),
        offset,
        entry_size,
        count,
        "program table is out of bounds",
    )
    .map_err(|_| KernelBindingError::InvalidLoadMapping("program table is out of bounds"))?;

    let mut loads = Vec::new();
    for index in 0..count {
        let base = offset
            .checked_add(index.checked_mul(entry_size).ok_or(
                KernelBindingError::InvalidLoadMapping("program index overflow"),
            )?)
            .ok_or(KernelBindingError::InvalidLoadMapping(
                "program offset overflow",
            ))?;
        if read_u32(bytes, base)? != elf::PT_LOAD {
            continue;
        }
        let segment = LoadSegment {
            flags: read_u32(bytes, base + 4)?,
            offset: read_u64(bytes, base + 8)?,
            address: read_u64(bytes, base + 16)?,
            file_size: read_u64(bytes, base + 32)?,
            memory_size: read_u64(bytes, base + 40)?,
            alignment: read_u64(bytes, base + 48)?,
        };
        validate_load(bytes, segment)?;
        loads.push(segment);
    }
    if loads.is_empty() {
        return Err(KernelBindingError::InvalidLoadMapping(
            "PT_LOAD segment is missing",
        ));
    }
    Ok(loads)
}

fn validate_load(bytes: &[u8], segment: LoadSegment) -> Result<(), KernelBindingError> {
    if segment.flags & !(PF_R | PF_W | PF_X) != 0 {
        return Err(KernelBindingError::InvalidLoadMapping(
            "PT_LOAD has unsupported permission bits",
        ));
    }
    if segment.file_size > segment.memory_size {
        return Err(KernelBindingError::InvalidLoadMapping(
            "PT_LOAD file size exceeds memory size",
        ));
    }
    checked_u64_range(
        bytes.len(),
        segment.offset,
        segment.file_size,
        KernelBindingError::InvalidLoadMapping("PT_LOAD file range is out of bounds"),
    )?;
    segment.address.checked_add(segment.memory_size).ok_or(
        KernelBindingError::InvalidLoadMapping("PT_LOAD address range overflows"),
    )?;
    if segment.alignment > 1
        && (!segment.alignment.is_power_of_two()
            || segment.offset % segment.alignment != segment.address % segment.alignment)
    {
        return Err(KernelBindingError::InvalidLoadMapping(
            "PT_LOAD alignment is invalid",
        ));
    }
    Ok(())
}

fn parse_symbols<'a>(
    bytes: &'a [u8],
    sections: &[Section],
) -> Result<Vec<Symbol<'a>>, KernelBindingError> {
    let mut symbols = Vec::new();
    let mut found_table = false;

    for section in sections {
        if section.section_type != elf::SHT_SYMTAB {
            continue;
        }
        found_table = true;
        if section.entry_size != ELF64_SYMBOL_BYTES as u64
            || section.size == 0
            || section.size % section.entry_size != 0
        {
            return Err(KernelBindingError::InvalidSymbolTable(
                "invalid symbol entry size or count",
            ));
        }
        if section.alignment != 8 || !section.offset.is_multiple_of(8) {
            return Err(KernelBindingError::InvalidSymbolTable(
                "invalid symbol table alignment",
            ));
        }
        let count = usize_from_u64(
            section.size / section.entry_size,
            "symbol count overflows usize",
        )?;
        let new_total = symbols
            .len()
            .checked_add(count)
            .ok_or(KernelBindingError::TooManySymbols)?;
        if new_total > MAX_ELF_SYMBOLS {
            return Err(KernelBindingError::TooManySymbols);
        }
        let table_offset = checked_u64_range(
            bytes.len(),
            section.offset,
            section.size,
            KernelBindingError::InvalidSymbolTable("symbol table is out of bounds"),
        )?;
        let string_index = usize::try_from(section.link).map_err(|_| {
            KernelBindingError::InvalidSymbolTable("symbol string-table link overflows usize")
        })?;
        let string_section =
            sections
                .get(string_index)
                .ok_or(KernelBindingError::InvalidSymbolTable(
                    "symbol string-table link is out of bounds",
                ))?;
        if string_section.section_type != elf::SHT_STRTAB {
            return Err(KernelBindingError::InvalidSymbolTable(
                "symbol table does not link to SHT_STRTAB",
            ));
        }
        if string_section.entry_size != 0 || string_section.alignment != 1 {
            return Err(KernelBindingError::InvalidSymbolTable(
                "invalid symbol string-table layout",
            ));
        }
        let string_offset = checked_u64_range(
            bytes.len(),
            string_section.offset,
            string_section.size,
            KernelBindingError::InvalidSymbolTable("symbol string table is out of bounds"),
        )?;
        let string_size = usize_from_u64(string_section.size, "string table size overflows usize")?;
        let strings = bytes
            .get(string_offset..string_offset + string_size)
            .ok_or(KernelBindingError::InvalidSymbolTable(
                "symbol string table is out of bounds",
            ))?;
        if strings.first() != Some(&0) {
            return Err(KernelBindingError::InvalidSymbolTable(
                "symbol string table lacks its initial NUL",
            ));
        }
        if section.info == 0 || u64::from(section.info) > count as u64 {
            return Err(KernelBindingError::InvalidSymbolTable(
                "invalid first non-local symbol index",
            ));
        }

        for index in 0..count {
            let base = table_offset
                .checked_add(index.checked_mul(ELF64_SYMBOL_BYTES).ok_or(
                    KernelBindingError::InvalidSymbolTable("symbol index overflow"),
                )?)
                .ok_or(KernelBindingError::InvalidSymbolTable(
                    "symbol offset overflow",
                ))?;
            let name_offset = read_u32(bytes, base)?;
            let name = symbol_name(strings, name_offset)?;
            let info = *bytes
                .get(base + 4)
                .ok_or(KernelBindingError::InvalidSymbolTable(
                    "truncated symbol info",
                ))?;
            let section_index = read_u16(bytes, base + 6)?;
            let symbol = Symbol {
                name,
                binding: info >> 4,
                symbol_type: info & 0x0f,
                other: read_u8(bytes, base + 5)?,
                section_index,
                value: read_u64(bytes, base + 8)?,
                size: read_u64(bytes, base + 16)?,
            };
            if section_index == SHN_XINDEX {
                return Err(KernelBindingError::InvalidSymbolTable(
                    "extended symbol section indexes are unsupported",
                ));
            }
            if section_index != 0
                && section_index < SHN_LORESERVE
                && usize::from(section_index) >= sections.len()
            {
                return Err(KernelBindingError::InvalidSymbolTable(
                    "symbol section index is out of bounds",
                ));
            }
            if (index < section.info as usize) != (symbol.binding == 0) {
                return Err(KernelBindingError::InvalidSymbolTable(
                    "local symbols do not match sh_info",
                ));
            }
            if index == 0
                && (name_offset != 0
                    || !symbol.name.is_empty()
                    || info != 0
                    || symbol.other != 0
                    || symbol.section_index != 0
                    || symbol.value != 0
                    || symbol.size != 0)
            {
                return Err(KernelBindingError::InvalidSymbolTable(
                    "first symbol is not the null symbol",
                ));
            }
            symbols.push(symbol);
        }
    }

    if !found_table {
        return Err(KernelBindingError::InvalidSymbolTable(
            "SHT_SYMTAB is missing",
        ));
    }
    Ok(symbols)
}

fn symbol_name(strings: &[u8], offset: u32) -> Result<&[u8], KernelBindingError> {
    let offset = usize::try_from(offset).map_err(|_| {
        KernelBindingError::InvalidSymbolTable("symbol name offset overflows usize")
    })?;
    let tail = strings
        .get(offset..)
        .ok_or(KernelBindingError::InvalidSymbolTable(
            "symbol name offset is out of bounds",
        ))?;
    let bounded_length = tail.len().min(MAX_MESSAGEPACK_STRING_BYTES + 1);
    let Some(length) = tail[..bounded_length].iter().position(|byte| *byte == 0) else {
        if tail.len() > MAX_MESSAGEPACK_STRING_BYTES {
            return Err(KernelBindingError::InvalidSymbolTable(
                "symbol name is too long",
            ));
        }
        return Err(KernelBindingError::InvalidSymbolTable(
            "symbol name is not NUL-terminated",
        ));
    };
    if length > MAX_MESSAGEPACK_STRING_BYTES {
        return Err(KernelBindingError::InvalidSymbolTable(
            "symbol name is too long",
        ));
    }
    let name = &tail[..length];
    str::from_utf8(name)
        .map_err(|_| KernelBindingError::InvalidSymbolTable("symbol name is not UTF-8"))?;
    Ok(name)
}

#[allow(clippy::too_many_arguments)]
fn bind_kernel(
    bytes: &[u8],
    sections: &[Section],
    loads: &[LoadSegment],
    symbols: &[Symbol<'_>],
    code_object_version: CodeObjectVersion,
    processor: &str,
    kernel_index: usize,
    kernel: &InspectedKernel,
) -> Result<KernelDescriptorBinding, KernelBindingError> {
    let descriptor_symbol = unique_symbol(
        symbols,
        kernel.symbol().as_bytes(),
        KernelBindingError::MissingDescriptorSymbol,
        KernelBindingError::AmbiguousDescriptorSymbol,
    )?;
    validate_descriptor_symbol(descriptor_symbol, sections)?;
    let descriptor_file_offset = section_file_mapping(
        bytes,
        sections,
        descriptor_symbol,
        AMDHSA_KERNEL_DESCRIPTOR_BYTES as u64,
        KernelBindingError::InvalidDescriptorSymbol("descriptor is outside its section"),
    )?;
    let load_descriptor_offset = load_file_mapping(
        bytes,
        loads,
        descriptor_symbol.value,
        AMDHSA_KERNEL_DESCRIPTOR_BYTES as u64,
        PF_R,
        PF_W | PF_X,
    )?;
    if descriptor_file_offset != load_descriptor_offset {
        return Err(KernelBindingError::InvalidLoadMapping(
            "descriptor section and PT_LOAD mappings disagree",
        ));
    }
    let descriptor_bytes = bytes
        .get(descriptor_file_offset..descriptor_file_offset + AMDHSA_KERNEL_DESCRIPTOR_BYTES)
        .ok_or(KernelBindingError::InvalidKernelDescriptor(
            "descriptor bytes are truncated",
        ))?;
    let descriptor = parse_descriptor(descriptor_bytes)?;

    let entry_symbol = unique_symbol(
        symbols,
        kernel.name().as_bytes(),
        KernelBindingError::MissingEntrySymbol,
        KernelBindingError::AmbiguousEntrySymbol,
    )?;
    validate_entry_symbol(entry_symbol, sections)?;
    validate_symbol_relationship(descriptor_symbol, entry_symbol)?;
    let entry_file_offset = section_file_mapping(
        bytes,
        sections,
        entry_symbol,
        entry_symbol.size,
        KernelBindingError::InvalidEntrySymbol("entry is outside its section"),
    )?;
    let load_entry_offset = load_file_mapping(
        bytes,
        loads,
        entry_symbol.value,
        entry_symbol.size,
        PF_R | PF_X,
        PF_W,
    )?;
    if entry_file_offset != load_entry_offset {
        return Err(KernelBindingError::InvalidLoadMapping(
            "entry section and PT_LOAD mappings disagree",
        ));
    }
    let descriptor_end = descriptor_file_offset + AMDHSA_KERNEL_DESCRIPTOR_BYTES;
    let entry_end =
        entry_file_offset
            .checked_add(usize::try_from(entry_symbol.size).map_err(|_| {
                KernelBindingError::InvalidEntrySymbol("entry size overflows usize")
            })?)
            .ok_or(KernelBindingError::InvalidEntrySymbol(
                "entry file range overflows",
            ))?;
    if descriptor_file_offset < entry_end && entry_file_offset < descriptor_end {
        return Err(KernelBindingError::InvalidLoadMapping(
            "descriptor and entry file ranges overlap",
        ));
    }

    let computed_entry = checked_signed_add(
        descriptor_symbol.value,
        descriptor.kernel_code_entry_byte_offset,
    )?;
    if computed_entry != entry_symbol.value {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "entry offset does not resolve to the function symbol",
        ));
    }
    validate_descriptor_against_metadata(descriptor, code_object_version, processor, kernel)?;

    Ok(KernelDescriptorBinding {
        kernel_index,
        descriptor_address: descriptor_symbol.value,
        descriptor_file_offset: descriptor_file_offset as u64,
        entry_address: entry_symbol.value,
        entry_file_offset: entry_file_offset as u64,
        entry_size: entry_symbol.size,
        descriptor,
    })
}

fn unique_symbol<'a>(
    symbols: &'a [Symbol<'a>],
    name: &[u8],
    missing: KernelBindingError,
    ambiguous: KernelBindingError,
) -> Result<Symbol<'a>, KernelBindingError> {
    let mut matches = symbols.iter().copied().filter(|symbol| symbol.name == name);
    let symbol = matches.next().ok_or(missing)?;
    if matches.next().is_some() {
        return Err(ambiguous);
    }
    Ok(symbol)
}

fn validate_descriptor_symbol(
    symbol: Symbol<'_>,
    sections: &[Section],
) -> Result<(), KernelBindingError> {
    if !is_supported_symbol_binding(symbol.binding) {
        return Err(KernelBindingError::InvalidDescriptorSymbol(
            "symbol binding is not STB_GLOBAL or STB_WEAK",
        ));
    }
    if symbol.symbol_type != STT_OBJECT {
        return Err(KernelBindingError::InvalidDescriptorSymbol(
            "symbol type is not STT_OBJECT",
        ));
    }
    if symbol.size != AMDHSA_KERNEL_DESCRIPTOR_BYTES as u64 {
        return Err(KernelBindingError::InvalidDescriptorSymbol(
            "symbol size is not 64 bytes",
        ));
    }
    if !symbol
        .value
        .is_multiple_of(AMDHSA_KERNEL_DESCRIPTOR_BYTES as u64)
    {
        return Err(KernelBindingError::InvalidDescriptorSymbol(
            "symbol address is not 64-byte aligned",
        ));
    }
    let section = ordinary_symbol_section(symbol, sections, true)?;
    if section.section_type != elf::SHT_PROGBITS || section.flags != SHF_ALLOC {
        return Err(KernelBindingError::InvalidDescriptorSymbol(
            "descriptor section is not uncompressed read-only allocated PROGBITS",
        ));
    }
    if section.alignment < AMDHSA_KERNEL_DESCRIPTOR_BYTES as u64
        || !section.alignment.is_power_of_two()
    {
        return Err(KernelBindingError::InvalidDescriptorSymbol(
            "descriptor section alignment is less than 64 bytes",
        ));
    }
    Ok(())
}

fn validate_entry_symbol(
    symbol: Symbol<'_>,
    sections: &[Section],
) -> Result<(), KernelBindingError> {
    if !is_supported_symbol_binding(symbol.binding) {
        return Err(KernelBindingError::InvalidEntrySymbol(
            "symbol binding is not STB_GLOBAL or STB_WEAK",
        ));
    }
    if symbol.symbol_type != STT_FUNC {
        return Err(KernelBindingError::InvalidEntrySymbol(
            "symbol type is not STT_FUNC",
        ));
    }
    if symbol.size == 0 {
        return Err(KernelBindingError::InvalidEntrySymbol(
            "function symbol has zero size",
        ));
    }
    if !symbol.value.is_multiple_of(AMDHSA_KERNEL_ENTRY_ALIGNMENT) {
        return Err(KernelBindingError::InvalidEntrySymbol(
            "function address is not 256-byte aligned",
        ));
    }
    let section = ordinary_symbol_section(symbol, sections, false)?;
    if section.section_type != elf::SHT_PROGBITS || section.flags != SHF_ALLOC | SHF_EXECINSTR {
        return Err(KernelBindingError::InvalidEntrySymbol(
            "entry section is not uncompressed read-only executable PROGBITS",
        ));
    }
    if section.alignment == 0 || !section.alignment.is_power_of_two() {
        return Err(KernelBindingError::InvalidEntrySymbol(
            "entry section alignment is invalid",
        ));
    }
    Ok(())
}

const fn is_supported_symbol_binding(binding: u8) -> bool {
    matches!(binding, STB_GLOBAL | STB_WEAK)
}

fn validate_symbol_relationship(
    descriptor: Symbol<'_>,
    entry: Symbol<'_>,
) -> Result<(), KernelBindingError> {
    if descriptor.binding != entry.binding {
        return Err(KernelBindingError::InvalidDescriptorSymbol(
            "descriptor and entry symbol bindings differ",
        ));
    }
    if descriptor.other & !0x3 != 0 || entry.other & !0x3 != 0 {
        return Err(KernelBindingError::InvalidDescriptorSymbol(
            "descriptor or entry symbol has unsupported st_other bits",
        ));
    }
    let descriptor_visibility = descriptor.other & 0x3;
    let entry_visibility = entry.other & 0x3;
    if descriptor_visibility != entry_visibility
        && !(descriptor_visibility == 0 && entry_visibility == 3)
    {
        return Err(KernelBindingError::InvalidDescriptorSymbol(
            "descriptor and entry symbol visibility is inconsistent",
        ));
    }
    Ok(())
}

fn ordinary_symbol_section<'a>(
    symbol: Symbol<'_>,
    sections: &'a [Section],
    descriptor: bool,
) -> Result<&'a Section, KernelBindingError> {
    if symbol.section_index == 0 || symbol.section_index >= SHN_LORESERVE {
        return Err(if descriptor {
            KernelBindingError::InvalidDescriptorSymbol("symbol is not section-defined")
        } else {
            KernelBindingError::InvalidEntrySymbol("symbol is not section-defined")
        });
    }
    sections
        .get(usize::from(symbol.section_index))
        .ok_or(if descriptor {
            KernelBindingError::InvalidDescriptorSymbol("symbol section is out of bounds")
        } else {
            KernelBindingError::InvalidEntrySymbol("symbol section is out of bounds")
        })
}

fn section_file_mapping(
    bytes: &[u8],
    sections: &[Section],
    symbol: Symbol<'_>,
    size: u64,
    error: KernelBindingError,
) -> Result<usize, KernelBindingError> {
    let section = sections
        .get(usize::from(symbol.section_index))
        .ok_or(error)?;
    let relative = symbol.value.checked_sub(section.address).ok_or(error)?;
    let relative_end = relative.checked_add(size).ok_or(error)?;
    if relative_end > section.size {
        return Err(error);
    }
    let file_offset = section.offset.checked_add(relative).ok_or(error)?;
    checked_u64_range(bytes.len(), file_offset, size, error)
}

fn load_file_mapping(
    bytes: &[u8],
    loads: &[LoadSegment],
    address: u64,
    size: u64,
    required_flags: u32,
    forbidden_flags: u32,
) -> Result<usize, KernelBindingError> {
    let requested_end = address
        .checked_add(size)
        .ok_or(KernelBindingError::InvalidLoadMapping(
            "requested virtual range overflows",
        ))?;
    let mut mapping = None;
    for segment in loads {
        let memory_end = segment.address.checked_add(segment.memory_size).ok_or(
            KernelBindingError::InvalidLoadMapping("PT_LOAD address range overflows"),
        )?;
        if address >= memory_end || segment.address >= requested_end {
            continue;
        }
        if segment.flags & required_flags != required_flags || segment.flags & forbidden_flags != 0
        {
            return Err(KernelBindingError::InvalidLoadMapping(
                "mapped PT_LOAD has inappropriate permissions",
            ));
        }
        if mapping.is_some() {
            return Err(KernelBindingError::InvalidLoadMapping(
                "address has ambiguous PT_LOAD memory mappings",
            ));
        }
        if segment.address > address || memory_end < requested_end {
            return Err(KernelBindingError::InvalidLoadMapping(
                "requested virtual range only partially intersects PT_LOAD",
            ));
        }
        let relative = address - segment.address;
        let relative_end =
            relative
                .checked_add(size)
                .ok_or(KernelBindingError::InvalidLoadMapping(
                    "mapped virtual range overflows",
                ))?;
        if relative_end > segment.file_size {
            return Err(KernelBindingError::InvalidLoadMapping(
                "address is not file-backed by PT_LOAD",
            ));
        }
        let file_offset =
            segment
                .offset
                .checked_add(relative)
                .ok_or(KernelBindingError::InvalidLoadMapping(
                    "mapped file offset overflows",
                ))?;
        let file_offset = checked_u64_range(
            bytes.len(),
            file_offset,
            size,
            KernelBindingError::InvalidLoadMapping("mapped file range is out of bounds"),
        )?;
        mapping = Some(file_offset);
    }
    mapping.ok_or(KernelBindingError::InvalidLoadMapping(
        "address is not file-backed by PT_LOAD",
    ))
}

fn parse_descriptor(bytes: &[u8]) -> Result<AmdhsaKernelDescriptor, KernelBindingError> {
    if bytes.len() != AMDHSA_KERNEL_DESCRIPTOR_BYTES {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "descriptor size is not 64 bytes",
        ));
    }
    if bytes[12..16]
        .iter()
        .chain(&bytes[24..44])
        .chain(&bytes[60..64])
        .any(|byte| *byte != 0)
    {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "reserved descriptor bytes are nonzero",
        ));
    }
    let properties = read_u16(bytes, 56)?;
    if properties & PROPERTY_RESERVED_MASK != 0 {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "reserved kernel-code-property bits are nonzero",
        ));
    }
    let kernarg_preload = read_u16(bytes, 58)?;
    if kernarg_preload != 0 {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "kernarg preload is unsupported",
        ));
    }
    Ok(AmdhsaKernelDescriptor {
        group_segment_fixed_size: read_u32(bytes, 0)?,
        private_segment_fixed_size: read_u32(bytes, 4)?,
        kernarg_size: read_u32(bytes, 8)?,
        kernel_code_entry_byte_offset: read_i64(bytes, 16)?,
        compute_pgm_rsrc3: read_u32(bytes, 44)?,
        compute_pgm_rsrc1: read_u32(bytes, 48)?,
        compute_pgm_rsrc2: read_u32(bytes, 52)?,
        kernel_code_properties: properties,
        kernarg_preload,
    })
}

fn validate_descriptor_against_metadata(
    descriptor: AmdhsaKernelDescriptor,
    code_object_version: CodeObjectVersion,
    processor: &str,
    kernel: &InspectedKernel,
) -> Result<(), KernelBindingError> {
    compare_u64(
        u64::from(descriptor.group_segment_fixed_size),
        kernel.group_segment_fixed_size(),
        ".group_segment_fixed_size",
    )?;
    compare_u64(
        u64::from(descriptor.private_segment_fixed_size),
        kernel.private_segment_fixed_size(),
        ".private_segment_fixed_size",
    )?;
    compare_u64(
        u64::from(descriptor.kernarg_size),
        kernel.kernarg_segment_size(),
        ".kernarg_segment_size",
    )?;
    if descriptor.wavefront_size() != kernel.wavefront_size() {
        return Err(KernelBindingError::MetadataMismatch(".wavefront_size"));
    }
    if code_object_version != CodeObjectVersion::V4
        && descriptor.uses_dynamic_stack() != kernel.uses_dynamic_stack()
    {
        return Err(KernelBindingError::MetadataMismatch(".uses_dynamic_stack"));
    }
    let expected_private =
        descriptor.private_segment_fixed_size != 0 || descriptor.uses_dynamic_stack();
    if descriptor.private_segment_enabled() != expected_private {
        return Err(KernelBindingError::MetadataMismatch(
            "private-segment enablement",
        ));
    }

    validate_resource_bits(descriptor, processor, kernel)?;
    validate_register_capacity(descriptor, processor, kernel)?;
    Ok(())
}

fn validate_resource_bits(
    descriptor: AmdhsaKernelDescriptor,
    processor: &str,
    kernel: &InspectedKernel,
) -> Result<(), KernelBindingError> {
    let generation = generation(processor)?;
    let rsrc1 = descriptor.compute_pgm_rsrc1;
    let rsrc2 = descriptor.compute_pgm_rsrc2;
    let rsrc3 = descriptor.compute_pgm_rsrc3;

    if has_architected_flat_scratch(processor)
        && descriptor.kernel_code_properties
            & (PROPERTY_ENABLE_SGPR_PRIVATE_SEGMENT_BUFFER | PROPERTY_ENABLE_SGPR_FLAT_SCRATCH_INIT)
            != 0
    {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "architected flat scratch forbids private-buffer and flat-scratch-init properties",
        ));
    }

    if generation < 10 && descriptor.kernel_code_properties & PROPERTY_WAVEFRONT_SIZE32 != 0 {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "wave32 property is reserved before GFX10",
        ));
    }

    let compiler_fixed_zero_rsrc1 = (0b11 << 10) | (1 << 20) | (1 << 22) | (0b11 << 24) | (1 << 28);
    if rsrc1 & compiler_fixed_zero_rsrc1 != 0 {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "reserved or unsupported COMPUTE_PGM_RSRC1 bits are nonzero",
        ));
    }
    let target_reserved_rsrc1 = if generation <= 8 {
        (1 << 26) | (1 << 27) | (0b111 << 29)
    } else if generation == 9 {
        (1 << 27) | (0b111 << 29)
    } else if is_gfx125(processor) {
        0
    } else {
        1 << 27
    };
    if rsrc1 & target_reserved_rsrc1 != 0 {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "target-reserved COMPUTE_PGM_RSRC1 bits are nonzero",
        ));
    }
    if generation >= 10 && rsrc1 & (0xf << 6) != 0 {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "GFX10+ SGPR block field is nonzero",
        ));
    }
    if generation >= 10 && rsrc1 & (1 << 30) == 0 {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "pinned LLVM MEM_ORDERED bit is zero",
        ));
    }
    if let Some(wgp_mode) = kernel.workgroup_processor_mode()
        && (rsrc1 & (1 << 29) != 0) != wgp_mode
    {
        return Err(KernelBindingError::MetadataMismatch(
            ".workgroup_processor_mode",
        ));
    }

    if rsrc2 & 0xffff_e000 != 0 {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "HSA-fixed or reserved COMPUTE_PGM_RSRC2 bits are nonzero",
        ));
    }
    if generation <= 11 && rsrc2 & (1 << 6) != 0 {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "HSA trap-handler bit is nonzero",
        ));
    }
    if generation >= 13 && rsrc2 & (1 << 6) != 0 {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "target-reserved COMPUTE_PGM_RSRC2 bit is nonzero",
        ));
    }

    let allowed_rsrc3 = if is_gfx90a_family(processor) {
        0x0001_003f
    } else {
        match generation {
            6..=9 => 0,
            10 => 0x0000_000f,
            11 => 0x8000_0fff,
            12 if is_gfx125(processor) => 0x803f_eff0,
            12..=13 => 0x8000_2ff0,
            _ => 0,
        }
    };
    if rsrc3 & !allowed_rsrc3 != 0 {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "target-reserved COMPUTE_PGM_RSRC3 bits are nonzero",
        ));
    }
    if is_gfx90a_family(processor) {
        let agpr_count = kernel
            .agpr_count()
            .ok_or(KernelBindingError::MetadataMismatch(".agpr_count"))?;
        let total_vgpr_count = u32::from(kernel.vgpr_count());
        let aligned_arch_vgpr_count = if agpr_count == 0 {
            total_vgpr_count.max(1).div_ceil(4) * 4
        } else {
            let count = total_vgpr_count
                .checked_sub(agpr_count)
                .ok_or(KernelBindingError::MetadataMismatch(".vgpr_count"))?;
            if !count.is_multiple_of(4) {
                return Err(KernelBindingError::MetadataMismatch(".vgpr_count"));
            }
            count
        };
        let expected_accum_offset = aligned_arch_vgpr_count.div_ceil(4).saturating_sub(1);
        if rsrc3 & 0x3f != expected_accum_offset {
            return Err(KernelBindingError::MetadataMismatch(
                "GFX90A accumulator offset",
            ));
        }
    }
    Ok(())
}

fn validate_register_capacity(
    descriptor: AmdhsaKernelDescriptor,
    processor: &str,
    kernel: &InspectedKernel,
) -> Result<(), KernelBindingError> {
    let vgpr_granule = if is_gfx90a_family(processor) {
        8
    } else if is_gfx125(processor) {
        if kernel.wavefront_size() == 32 { 16 } else { 8 }
    } else if kernel.wavefront_size() == 32 {
        8
    } else {
        4
    };
    let vgpr_capacity = ((descriptor.compute_pgm_rsrc1 & 0x3f) + 1) * vgpr_granule;
    if vgpr_capacity < u32::from(kernel.vgpr_count()) {
        return Err(KernelBindingError::MetadataMismatch(".vgpr_count"));
    }

    match generation(processor)? {
        6..=9 => {
            let encoded_blocks = (descriptor.compute_pgm_rsrc1 >> 6) & 0xf;
            if encoded_blocks > 13 {
                return Err(KernelBindingError::InvalidKernelDescriptor(
                    "pre-GFX10 SGPR block field exceeds the pinned 112-register limit",
                ));
            }
            let sgpr_capacity = (encoded_blocks + 1) * 8;
            if sgpr_capacity < u32::from(kernel.sgpr_count()) {
                return Err(KernelBindingError::MetadataMismatch(".sgpr_count"));
            }
        }
        10..=12 => {
            if kernel.sgpr_count() > 128 {
                return Err(KernelBindingError::MetadataMismatch(".sgpr_count"));
            }
        }
        _ => {
            return Err(KernelBindingError::InvalidKernelDescriptor(
                "target SGPR capacity is not pinned",
            ));
        }
    }
    Ok(())
}

fn compare_u64(actual: u64, expected: u64, field: &'static str) -> Result<(), KernelBindingError> {
    if actual != expected {
        return Err(KernelBindingError::MetadataMismatch(field));
    }
    Ok(())
}

fn generation(processor: &str) -> Result<u8, KernelBindingError> {
    let generation = if processor.starts_with("gfx6") {
        6
    } else if processor.starts_with("gfx7") {
        7
    } else if processor.starts_with("gfx8") {
        8
    } else if processor.starts_with("gfx9") {
        9
    } else if processor.starts_with("gfx10") {
        10
    } else if processor.starts_with("gfx11") {
        11
    } else if processor.starts_with("gfx12") {
        12
    } else if processor.starts_with("gfx13") {
        13
    } else {
        return Err(KernelBindingError::InvalidKernelDescriptor(
            "target generation is unsupported",
        ));
    };
    Ok(generation)
}

fn is_gfx90a_family(processor: &str) -> bool {
    matches!(processor, "gfx90a" | "gfx942" | "gfx950")
}

fn is_gfx125(processor: &str) -> bool {
    matches!(processor, "gfx1250" | "gfx1251")
}

// This is the exact intersection of fe2o3-amd-target's concrete processor
// table and FeatureArchitectedFlatScratch in the pinned LLVM AMDGPU.td feature
// sets. Keep it explicit so accepting a new processor requires a source audit.
fn has_architected_flat_scratch(processor: &str) -> bool {
    matches!(
        processor,
        "gfx942"
            | "gfx950"
            | "gfx1100"
            | "gfx1101"
            | "gfx1102"
            | "gfx1103"
            | "gfx1150"
            | "gfx1151"
            | "gfx1152"
            | "gfx1153"
            | "gfx1154"
            | "gfx1170"
            | "gfx1171"
            | "gfx1172"
            | "gfx1200"
            | "gfx1201"
            | "gfx1250"
            | "gfx1251"
            | "gfx1310"
    )
}

fn checked_signed_add(base: u64, offset: i64) -> Result<u64, KernelBindingError> {
    if offset >= 0 {
        base.checked_add(offset as u64)
    } else {
        base.checked_sub(offset.unsigned_abs())
    }
    .ok_or(KernelBindingError::InvalidKernelDescriptor(
        "entry address arithmetic overflows",
    ))
}

fn checked_table_range(
    input_length: usize,
    offset: usize,
    entry_size: usize,
    count: usize,
    reason: &'static str,
) -> Result<(), KernelBindingError> {
    let size = entry_size
        .checked_mul(count)
        .ok_or(KernelBindingError::InvalidSymbolTable(reason))?;
    let end = offset
        .checked_add(size)
        .ok_or(KernelBindingError::InvalidSymbolTable(reason))?;
    if end > input_length {
        return Err(KernelBindingError::InvalidSymbolTable(reason));
    }
    Ok(())
}

fn checked_u64_range(
    input_length: usize,
    offset: u64,
    size: u64,
    error: KernelBindingError,
) -> Result<usize, KernelBindingError> {
    let offset = usize::try_from(offset).map_err(|_| error)?;
    let size = usize::try_from(size).map_err(|_| error)?;
    let end = offset.checked_add(size).ok_or(error)?;
    if end > input_length {
        return Err(error);
    }
    Ok(offset)
}

fn usize_from_u64(value: u64, reason: &'static str) -> Result<usize, KernelBindingError> {
    usize::try_from(value).map_err(|_| KernelBindingError::InvalidSymbolTable(reason))
}

fn read_u8(bytes: &[u8], offset: usize) -> Result<u8, KernelBindingError> {
    bytes
        .get(offset)
        .copied()
        .ok_or(KernelBindingError::InvalidSymbolTable(
            "truncated ELF field",
        ))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, KernelBindingError> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, KernelBindingError> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, KernelBindingError> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_i64(bytes: &[u8], offset: usize) -> Result<i64, KernelBindingError> {
    Ok(i64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], KernelBindingError> {
    let end = offset
        .checked_add(N)
        .ok_or(KernelBindingError::InvalidSymbolTable(
            "ELF field offset overflow",
        ))?;
    bytes
        .get(offset..end)
        .ok_or(KernelBindingError::InvalidSymbolTable(
            "truncated ELF field",
        ))?
        .try_into()
        .map_err(|_| KernelBindingError::InvalidSymbolTable("invalid ELF field width"))
}
