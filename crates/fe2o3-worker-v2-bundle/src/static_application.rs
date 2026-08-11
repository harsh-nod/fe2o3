use core::fmt;

use sha2::{Digest, Sha256};

const IDENTITY_DOMAIN: &[u8] = b"FE2O3/SEALED-STATIC-APPLICATION-IDENTITY/V1\0";
const ELF_HEADER_BYTES: usize = 64;
const ELF_PROGRAM_HEADER_BYTES: usize = 56;
const ELF_MACHINE_X86_64: u16 = 62;
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;
const PT_NULL: u32 = 0;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PT_NOTE: u32 = 4;
const PT_PHDR: u32 = 6;
const PT_TLS: u32 = 7;
const PT_GNU_EH_FRAME: u32 = 0x6474_e550;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PT_GNU_RELRO: u32 = 0x6474_e552;
const DT_NULL: i64 = 0;
const DT_NEEDED: i64 = 1;
const DT_STRTAB: i64 = 5;
const DT_SYMTAB: i64 = 6;
const DT_RELA: i64 = 7;
const DT_RELASZ: i64 = 8;
const DT_RELAENT: i64 = 9;
const DT_STRSZ: i64 = 10;
const DT_SYMENT: i64 = 11;
const DT_INIT: i64 = 12;
const DT_FINI: i64 = 13;
const DT_SONAME: i64 = 14;
const DT_RPATH: i64 = 15;
const DT_REL: i64 = 17;
const DT_RELSZ: i64 = 18;
const DT_RELENT: i64 = 19;
const DT_DEBUG: i64 = 21;
const DT_TEXTREL: i64 = 22;
const DT_BIND_NOW: i64 = 24;
const DT_INIT_ARRAY: i64 = 25;
const DT_FINI_ARRAY: i64 = 26;
const DT_INIT_ARRAYSZ: i64 = 27;
const DT_FINI_ARRAYSZ: i64 = 28;
const DT_RUNPATH: i64 = 29;
const DT_FLAGS: i64 = 30;
const DT_GNU_HASH: i64 = 0x6fff_fef5;
const DT_RELACOUNT: i64 = 0x6fff_fff9;
const DT_RELCOUNT: i64 = 0x6fff_fffa;
const DT_FLAGS_1: i64 = 0x6fff_fffb;
const DT_DEPAUDIT: i64 = 0x6fff_fefb;
const DT_AUDIT: i64 = 0x6fff_fefc;
const DT_AUXILIARY: i64 = 0x7fff_fffd;
const DT_FILTER: i64 = 0x7fff_ffff;
const DF_BIND_NOW: u64 = 0x8;
const DF_1_NOW: u64 = 0x1;
const DF_1_PIE: u64 = 0x0800_0000;
const R_X86_64_RELATIVE: u32 = 8;
const R_X86_64_IRELATIVE: u32 = 37;
const MAX_PROGRAM_HEADERS: usize = 1_024;
const X86_64_LOAD_PAGE_BYTES: u64 = 4_096;
const X86_64_MAX_USER_ADDRESS: u64 = 0x0000_7fff_ffff_ffff;

/// Rejection while deriving the identity of a loader-independent application image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SealedStaticApplicationErrorV1 {
    InvalidElf,
    UnsupportedElf,
    ProgramHeaderBounds,
    InterpreterPresent,
    UnsupportedProgramHeader,
    SegmentLayout,
    SegmentPermissions,
    RuntimeDependencyPresent,
    DynamicSegmentMalformed,
    UnsupportedDynamicTag,
    RelocationMalformed,
    UnsupportedRelocation,
}

impl fmt::Display for SealedStaticApplicationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidElf => formatter.write_str("application is not a canonical ELF64 image"),
            Self::UnsupportedElf => formatter
                .write_str("application is not a little-endian x86-64 executable or static PIE"),
            Self::ProgramHeaderBounds => {
                formatter.write_str("application ELF program headers are out of bounds")
            }
            Self::InterpreterPresent => formatter.write_str(
                "application has an ELF interpreter and is outside the sealed-static profile",
            ),
            Self::UnsupportedProgramHeader => formatter.write_str(
                "application has a program-header type outside the sealed-static profile",
            ),
            Self::SegmentLayout => formatter.write_str(
                "application has an invalid or overlapping sealed-static segment layout",
            ),
            Self::SegmentPermissions => formatter
                .write_str("application has segment permissions outside the sealed-static profile"),
            Self::RuntimeDependencyPresent => formatter
                .write_str("application has a dependency, search-path, audit, or filter entry"),
            Self::DynamicSegmentMalformed => {
                formatter.write_str("application has malformed static-PIE dynamic metadata")
            }
            Self::UnsupportedDynamicTag => formatter
                .write_str("application has dynamic metadata outside the static-PIE allowlist"),
            Self::RelocationMalformed => {
                formatter.write_str("application has malformed static-PIE relocations")
            }
            Self::UnsupportedRelocation => formatter
                .write_str("application requires a relocation outside the static-PIE allowlist"),
        }
    }
}

impl std::error::Error for SealedStaticApplicationErrorV1 {}

#[derive(Clone, Copy, Debug)]
struct ProgramHeader {
    kind: u32,
    flags: u32,
    offset: u64,
    virtual_address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
}

impl ProgramHeader {
    fn file_end(self) -> Result<u64, SealedStaticApplicationErrorV1> {
        self.offset
            .checked_add(self.file_size)
            .ok_or(SealedStaticApplicationErrorV1::SegmentLayout)
    }

    fn memory_end(self) -> Result<u64, SealedStaticApplicationErrorV1> {
        self.virtual_address
            .checked_add(self.memory_size)
            .ok_or(SealedStaticApplicationErrorV1::SegmentLayout)
    }

    fn contains_file_range(
        self,
        offset: u64,
        size: u64,
    ) -> Result<bool, SealedStaticApplicationErrorV1> {
        let end = offset
            .checked_add(size)
            .ok_or(SealedStaticApplicationErrorV1::SegmentLayout)?;
        Ok(offset >= self.offset && end <= self.file_end()?)
    }

    fn contains_memory_range(
        self,
        address: u64,
        size: u64,
        file_backed: bool,
    ) -> Result<bool, SealedStaticApplicationErrorV1> {
        let end = address
            .checked_add(size)
            .ok_or(SealedStaticApplicationErrorV1::SegmentLayout)?;
        let segment_end = if file_backed {
            self.virtual_address
                .checked_add(self.file_size)
                .ok_or(SealedStaticApplicationErrorV1::SegmentLayout)?
        } else {
            self.memory_end()?
        };
        Ok(address >= self.virtual_address && end <= segment_end)
    }
}

#[derive(Clone, Copy, Debug)]
struct DynamicEntry {
    tag: i64,
    value: u64,
}

pub(crate) fn sealed_static_application_identity_v1(
    bytes: &[u8],
) -> Result<[u8; 32], SealedStaticApplicationErrorV1> {
    validate_sealed_static_elf_v1(bytes)?;
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    Ok(digest.finalize().into())
}

fn validate_sealed_static_elf_v1(bytes: &[u8]) -> Result<(), SealedStaticApplicationErrorV1> {
    let header = bytes
        .get(..ELF_HEADER_BYTES)
        .ok_or(SealedStaticApplicationErrorV1::InvalidElf)?;
    if &header[..7] != b"\x7fELF\x02\x01\x01"
        || !matches!(header[7], 0 | 3)
        || header[8] != 0
        || header[9..16].iter().any(|byte| *byte != 0)
        || read_u16(header, 18)? != ELF_MACHINE_X86_64
        || read_u32(header, 20)? != 1
        || !matches!(read_u16(header, 16)?, ET_EXEC | ET_DYN)
        || read_u32(header, 48)? != 0
        || read_u16(header, 52)? as usize != ELF_HEADER_BYTES
    {
        return Err(SealedStaticApplicationErrorV1::UnsupportedElf);
    }

    let elf_type = read_u16(header, 16)?;
    let entrypoint = read_u64(header, 24)?;
    let program_offset = usize::try_from(read_u64(header, 32)?)
        .map_err(|_| SealedStaticApplicationErrorV1::ProgramHeaderBounds)?;
    let entry_size = read_u16(header, 54)? as usize;
    let entry_count = read_u16(header, 56)? as usize;
    if program_offset < ELF_HEADER_BYTES
        || entry_size != ELF_PROGRAM_HEADER_BYTES
        || entry_count == 0
        || entry_count > MAX_PROGRAM_HEADERS
    {
        return Err(SealedStaticApplicationErrorV1::ProgramHeaderBounds);
    }
    let table_size = entry_size
        .checked_mul(entry_count)
        .ok_or(SealedStaticApplicationErrorV1::ProgramHeaderBounds)?;
    let table_end = program_offset
        .checked_add(table_size)
        .filter(|end| *end <= bytes.len())
        .ok_or(SealedStaticApplicationErrorV1::ProgramHeaderBounds)?;

    let mut programs = Vec::with_capacity(entry_count);
    for index in 0..entry_count {
        let start = program_offset + index * entry_size;
        let program = &bytes[start..start + entry_size];
        let parsed = ProgramHeader {
            kind: read_u32(program, 0)?,
            flags: read_u32(program, 4)?,
            offset: read_u64(program, 8)?,
            virtual_address: read_u64(program, 16)?,
            file_size: read_u64(program, 32)?,
            memory_size: read_u64(program, 40)?,
            alignment: read_u64(program, 48)?,
        };
        validate_program_bounds(bytes, parsed)?;
        programs.push(parsed);
    }

    let loads: Vec<_> = programs
        .iter()
        .copied()
        .filter(|program| program.kind == PT_LOAD)
        .collect();
    validate_load_segments(&loads, program_offset, table_end, entrypoint)?;
    validate_auxiliary_segments(&programs, &loads, program_offset, table_size)?;

    let dynamic: Vec<_> = programs
        .iter()
        .copied()
        .filter(|program| program.kind == PT_DYNAMIC)
        .collect();
    if dynamic.len() > 1 || (elf_type == ET_DYN && dynamic.len() != 1) {
        return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
    }
    if let Some(dynamic) = dynamic.first().copied() {
        validate_dynamic_segment(bytes, &loads, dynamic, elf_type)?;
    }
    Ok(())
}

fn validate_program_bounds(
    bytes: &[u8],
    program: ProgramHeader,
) -> Result<(), SealedStaticApplicationErrorV1> {
    if program.flags & !(PF_R | PF_W | PF_X) != 0
        || program.file_size > program.memory_size
        || program.file_end()? > bytes.len() as u64
        || (program.alignment > 1
            && (!program.alignment.is_power_of_two()
                || program.offset % program.alignment
                    != program.virtual_address % program.alignment))
    {
        return Err(SealedStaticApplicationErrorV1::SegmentLayout);
    }
    match program.kind {
        PT_NULL | PT_LOAD | PT_DYNAMIC | PT_NOTE | PT_PHDR | PT_TLS | PT_GNU_EH_FRAME
        | PT_GNU_STACK | PT_GNU_RELRO => Ok(()),
        PT_INTERP => Err(SealedStaticApplicationErrorV1::InterpreterPresent),
        _ => Err(SealedStaticApplicationErrorV1::UnsupportedProgramHeader),
    }
}

fn validate_load_segments(
    loads: &[ProgramHeader],
    program_offset: usize,
    table_end: usize,
    entrypoint: u64,
) -> Result<(), SealedStaticApplicationErrorV1> {
    if loads.is_empty() {
        return Err(SealedStaticApplicationErrorV1::SegmentLayout);
    }
    for load in loads {
        if load.flags & PF_R == 0 || load.flags & (PF_W | PF_X) == (PF_W | PF_X) {
            return Err(SealedStaticApplicationErrorV1::SegmentPermissions);
        }
        if load.memory_size == 0
            || load.alignment < X86_64_LOAD_PAGE_BYTES
            || load.offset % X86_64_LOAD_PAGE_BYTES != load.virtual_address % X86_64_LOAD_PAGE_BYTES
            || load.memory_end()? > X86_64_MAX_USER_ADDRESS
        {
            return Err(SealedStaticApplicationErrorV1::SegmentLayout);
        }
    }
    for (index, left) in loads.iter().enumerate() {
        for right in &loads[index + 1..] {
            if ranges_overlap(
                left.virtual_address,
                left.memory_end()?,
                right.virtual_address,
                right.memory_end()?,
            ) {
                return Err(SealedStaticApplicationErrorV1::SegmentLayout);
            }
            let left_virtual_pages = loader_virtual_pages(*left)?;
            let right_virtual_pages = loader_virtual_pages(*right)?;
            if ranges_overlap(
                left_virtual_pages.0,
                left_virtual_pages.1,
                right_virtual_pages.0,
                right_virtual_pages.1,
            ) {
                return Err(SealedStaticApplicationErrorV1::SegmentLayout);
            }
            let left_file_pages = loader_file_pages(*left)?;
            let right_file_pages = loader_file_pages(*right)?;
            if ranges_overlap(
                left_file_pages.0,
                left_file_pages.1,
                right_file_pages.0,
                right_file_pages.1,
            ) {
                // Linux may map one raw-disjoint boundary page twice with different permissions.
                // Writable PT_LOAD mappings are private/COW, so only an overlap in the declared
                // file bytes or in rounded virtual pages creates a W^X mapping.
                if ranges_overlap(
                    left.offset,
                    left.file_end()?,
                    right.offset,
                    right.file_end()?,
                ) {
                    return Err(SealedStaticApplicationErrorV1::SegmentLayout);
                }
            }
        }
    }
    let headers_covered = loads.iter().any(|load| {
        load.flags == PF_R
            && load
                .contains_file_range(0, table_end as u64)
                .unwrap_or(false)
            && load.offset == 0
            && program_offset >= ELF_HEADER_BYTES
    });
    let entrypoint_covered = loads
        .iter()
        .filter(|load| load.flags & PF_X != 0)
        .any(|load| {
            load.contains_memory_range(entrypoint, 1, true)
                .unwrap_or(false)
        });
    if !headers_covered || !entrypoint_covered {
        return Err(SealedStaticApplicationErrorV1::SegmentLayout);
    }
    Ok(())
}

fn loader_virtual_pages(load: ProgramHeader) -> Result<(u64, u64), SealedStaticApplicationErrorV1> {
    Ok((
        align_down(load.virtual_address, X86_64_LOAD_PAGE_BYTES),
        align_up(load.memory_end()?, X86_64_LOAD_PAGE_BYTES)?,
    ))
}

fn loader_file_pages(load: ProgramHeader) -> Result<(u64, u64), SealedStaticApplicationErrorV1> {
    if load.file_size == 0 {
        return Ok((0, 0));
    }
    Ok((
        align_down(load.offset, X86_64_LOAD_PAGE_BYTES),
        align_up(load.file_end()?, X86_64_LOAD_PAGE_BYTES)?,
    ))
}

const fn align_down(value: u64, alignment: u64) -> u64 {
    value & !(alignment - 1)
}

fn align_up(value: u64, alignment: u64) -> Result<u64, SealedStaticApplicationErrorV1> {
    value
        .checked_add(alignment - 1)
        .map(|value| align_down(value, alignment))
        .ok_or(SealedStaticApplicationErrorV1::SegmentLayout)
}

fn validate_auxiliary_segments(
    programs: &[ProgramHeader],
    loads: &[ProgramHeader],
    program_offset: usize,
    table_size: usize,
) -> Result<(), SealedStaticApplicationErrorV1> {
    let phdrs: Vec<_> = programs
        .iter()
        .filter(|program| program.kind == PT_PHDR)
        .collect();
    let stacks: Vec<_> = programs
        .iter()
        .filter(|program| program.kind == PT_GNU_STACK)
        .collect();
    if phdrs.len() != 1 || stacks.len() != 1 {
        return Err(SealedStaticApplicationErrorV1::SegmentLayout);
    }
    let phdr = **phdrs.first().unwrap();
    if phdr.flags != PF_R
        || phdr.offset != program_offset as u64
        || phdr.file_size != table_size as u64
        || phdr.memory_size != table_size as u64
        || phdr.alignment != 8
    {
        return Err(SealedStaticApplicationErrorV1::SegmentLayout);
    }
    let stack = **stacks.first().unwrap();
    if stack.flags != PF_R | PF_W || stack.file_size != 0 || stack.memory_size != 0 {
        return Err(SealedStaticApplicationErrorV1::SegmentPermissions);
    }

    let mut tls_count = 0;
    let mut relro_count = 0;
    for program in programs {
        match program.kind {
            PT_NULL | PT_LOAD | PT_GNU_STACK => continue,
            PT_PHDR | PT_NOTE | PT_GNU_EH_FRAME => {
                if program.flags != PF_R {
                    return Err(SealedStaticApplicationErrorV1::SegmentPermissions);
                }
            }
            PT_DYNAMIC | PT_TLS => {
                if program.kind == PT_TLS {
                    tls_count += 1;
                    if program.flags != PF_R || program.memory_size == 0 || program.alignment == 0 {
                        return Err(SealedStaticApplicationErrorV1::SegmentLayout);
                    }
                }
                let owner = owning_load(loads, *program)?;
                if owner.flags & PF_W == 0 || owner.flags & PF_X != 0 {
                    return Err(SealedStaticApplicationErrorV1::SegmentPermissions);
                }
            }
            PT_GNU_RELRO => {
                relro_count += 1;
                if program.flags != PF_R || program.memory_size == 0 {
                    return Err(SealedStaticApplicationErrorV1::SegmentLayout);
                }
                let owner = owning_load(loads, *program)?;
                if owner.flags & PF_W == 0 || owner.flags & PF_X != 0 {
                    return Err(SealedStaticApplicationErrorV1::SegmentPermissions);
                }
            }
            _ => unreachable!("program kind was filtered while parsing"),
        }
        if program.kind != PT_GNU_STACK {
            owning_load(loads, *program)?;
        }
    }
    if tls_count > 1 || relro_count > 1 {
        return Err(SealedStaticApplicationErrorV1::SegmentLayout);
    }
    Ok(())
}

fn owning_load(
    loads: &[ProgramHeader],
    segment: ProgramHeader,
) -> Result<ProgramHeader, SealedStaticApplicationErrorV1> {
    let mut owners = loads.iter().copied().filter(|load| {
        let file_contained = load
            .contains_file_range(segment.offset, segment.file_size)
            .unwrap_or(false);
        let memory_contained = load
            .contains_memory_range(segment.virtual_address, segment.memory_size, false)
            .unwrap_or(false);
        let same_mapping = segment.file_size == 0
            || segment.virtual_address.checked_sub(load.virtual_address)
                == segment.offset.checked_sub(load.offset);
        file_contained && memory_contained && same_mapping
    });
    let owner = owners
        .next()
        .ok_or(SealedStaticApplicationErrorV1::SegmentLayout)?;
    if owners.next().is_some() {
        return Err(SealedStaticApplicationErrorV1::SegmentLayout);
    }
    Ok(owner)
}

fn validate_dynamic_segment(
    bytes: &[u8],
    loads: &[ProgramHeader],
    dynamic: ProgramHeader,
    elf_type: u16,
) -> Result<(), SealedStaticApplicationErrorV1> {
    if dynamic.flags != PF_R | PF_W
        || dynamic.file_size == 0
        || dynamic.file_size != dynamic.memory_size
        || !dynamic.file_size.is_multiple_of(16)
    {
        return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
    }
    let start = usize::try_from(dynamic.offset)
        .map_err(|_| SealedStaticApplicationErrorV1::DynamicSegmentMalformed)?;
    let end = usize::try_from(dynamic.file_end()?)
        .map_err(|_| SealedStaticApplicationErrorV1::DynamicSegmentMalformed)?;
    let mut entries = Vec::new();
    let mut terminated = false;
    for encoded in bytes[start..end].chunks_exact(16) {
        let entry = DynamicEntry {
            tag: read_i64(encoded, 0)?,
            value: read_u64(encoded, 8)?,
        };
        if terminated {
            if entry.tag != DT_NULL || entry.value != 0 {
                return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
            }
            continue;
        }
        if entry.tag == DT_NULL {
            if entry.value != 0 {
                return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
            }
            terminated = true;
            continue;
        }
        if entries
            .iter()
            .any(|previous: &DynamicEntry| previous.tag == entry.tag)
        {
            return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
        }
        classify_dynamic_tag(entry)?;
        entries.push(entry);
    }
    if !terminated {
        return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
    }

    validate_dynamic_flags(&entries, elf_type)?;
    validate_dynamic_tables(bytes, loads, &entries)?;
    Ok(())
}

fn classify_dynamic_tag(entry: DynamicEntry) -> Result<(), SealedStaticApplicationErrorV1> {
    match entry.tag {
        DT_STRTAB | DT_SYMTAB | DT_RELA | DT_RELASZ | DT_RELAENT | DT_STRSZ | DT_SYMENT
        | DT_INIT | DT_FINI | DT_REL | DT_RELSZ | DT_RELENT | DT_DEBUG | DT_BIND_NOW
        | DT_INIT_ARRAY | DT_FINI_ARRAY | DT_INIT_ARRAYSZ | DT_FINI_ARRAYSZ | DT_FLAGS
        | DT_GNU_HASH | DT_RELACOUNT | DT_RELCOUNT | DT_FLAGS_1 => Ok(()),
        DT_NEEDED | DT_SONAME | DT_RPATH | DT_RUNPATH | DT_DEPAUDIT | DT_AUDIT | DT_AUXILIARY
        | DT_FILTER => Err(SealedStaticApplicationErrorV1::RuntimeDependencyPresent),
        DT_TEXTREL => Err(SealedStaticApplicationErrorV1::SegmentPermissions),
        _ => Err(SealedStaticApplicationErrorV1::UnsupportedDynamicTag),
    }
}

fn validate_dynamic_flags(
    entries: &[DynamicEntry],
    elf_type: u16,
) -> Result<(), SealedStaticApplicationErrorV1> {
    if let Some(value) = dynamic_value(entries, DT_DEBUG)
        && value != 0
    {
        return Err(SealedStaticApplicationErrorV1::UnsupportedDynamicTag);
    }
    if let Some(value) = dynamic_value(entries, DT_BIND_NOW)
        && value != 0
    {
        return Err(SealedStaticApplicationErrorV1::UnsupportedDynamicTag);
    }
    if let Some(value) = dynamic_value(entries, DT_FLAGS)
        && value != DF_BIND_NOW
    {
        return Err(SealedStaticApplicationErrorV1::UnsupportedDynamicTag);
    }
    if let Some(value) = dynamic_value(entries, DT_FLAGS_1) {
        let permitted = DF_1_NOW | DF_1_PIE;
        if value & !permitted != 0 || (elf_type == ET_DYN && value & DF_1_PIE == 0) {
            return Err(SealedStaticApplicationErrorV1::UnsupportedDynamicTag);
        }
    }
    Ok(())
}

fn validate_dynamic_tables(
    bytes: &[u8],
    loads: &[ProgramHeader],
    entries: &[DynamicEntry],
) -> Result<(), SealedStaticApplicationErrorV1> {
    validate_pair(entries, DT_STRTAB, DT_STRSZ)?;
    validate_pair(entries, DT_SYMTAB, DT_SYMENT)?;
    validate_triple(entries, DT_RELA, DT_RELASZ, DT_RELAENT)?;
    validate_triple(entries, DT_REL, DT_RELSZ, DT_RELENT)?;
    validate_pair(entries, DT_INIT_ARRAY, DT_INIT_ARRAYSZ)?;
    validate_pair(entries, DT_FINI_ARRAY, DT_FINI_ARRAYSZ)?;

    if let (Some(address), Some(size)) = (
        dynamic_value(entries, DT_STRTAB),
        dynamic_value(entries, DT_STRSZ),
    ) && (size == 0 || mapped_file_slice(bytes, loads, address, size, PF_R, PF_W)?.is_empty())
    {
        return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
    }
    if let (Some(address), Some(entry_size)) = (
        dynamic_value(entries, DT_SYMTAB),
        dynamic_value(entries, DT_SYMENT),
    ) {
        if entry_size != 24 {
            return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
        }
        mapped_file_slice(bytes, loads, address, entry_size, PF_R, PF_W)?;
    }
    if let Some(address) = dynamic_value(entries, DT_GNU_HASH) {
        mapped_file_slice(bytes, loads, address, 16, PF_R, PF_W)?;
    }
    for tag in [DT_INIT, DT_FINI] {
        if let Some(address) = dynamic_value(entries, tag)
            && !is_executable_address(loads, address)
        {
            return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
        }
    }
    validate_function_array(bytes, loads, entries, DT_INIT_ARRAY, DT_INIT_ARRAYSZ)?;
    validate_function_array(bytes, loads, entries, DT_FINI_ARRAY, DT_FINI_ARRAYSZ)?;
    validate_relocations(bytes, loads, entries, true)?;
    validate_relocations(bytes, loads, entries, false)?;
    Ok(())
}

fn validate_function_array(
    bytes: &[u8],
    loads: &[ProgramHeader],
    entries: &[DynamicEntry],
    address_tag: i64,
    size_tag: i64,
) -> Result<(), SealedStaticApplicationErrorV1> {
    let (Some(address), Some(size)) = (
        dynamic_value(entries, address_tag),
        dynamic_value(entries, size_tag),
    ) else {
        return Ok(());
    };
    if size % 8 != 0 {
        return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
    }
    let encoded = mapped_file_slice(bytes, loads, address, size, PF_R | PF_W, PF_X)?;
    for item in encoded.chunks_exact(8) {
        let function = read_u64(item, 0)?;
        if function != 0 && !is_executable_address(loads, function) {
            return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
        }
    }
    Ok(())
}

fn validate_relocations(
    bytes: &[u8],
    loads: &[ProgramHeader],
    entries: &[DynamicEntry],
    with_addend: bool,
) -> Result<(), SealedStaticApplicationErrorV1> {
    let (address_tag, size_tag, entry_tag, count_tag, expected_size) = if with_addend {
        (DT_RELA, DT_RELASZ, DT_RELAENT, DT_RELACOUNT, 24_u64)
    } else {
        (DT_REL, DT_RELSZ, DT_RELENT, DT_RELCOUNT, 16_u64)
    };
    let Some(address) = dynamic_value(entries, address_tag) else {
        if dynamic_value(entries, count_tag).is_some() {
            return Err(SealedStaticApplicationErrorV1::RelocationMalformed);
        }
        return Ok(());
    };
    let size = dynamic_value(entries, size_tag)
        .ok_or(SealedStaticApplicationErrorV1::RelocationMalformed)?;
    let entry_size = dynamic_value(entries, entry_tag)
        .ok_or(SealedStaticApplicationErrorV1::RelocationMalformed)?;
    if entry_size != expected_size || size % expected_size != 0 {
        return Err(SealedStaticApplicationErrorV1::RelocationMalformed);
    }
    let encoded = mapped_file_slice(bytes, loads, address, size, PF_R, PF_W)?;
    let relative_count = dynamic_value(entries, count_tag).unwrap_or(0);
    if relative_count > size / expected_size {
        return Err(SealedStaticApplicationErrorV1::RelocationMalformed);
    }
    for (index, relocation) in encoded.chunks_exact(expected_size as usize).enumerate() {
        let target = read_u64(relocation, 0)?;
        let info = read_u64(relocation, 8)?;
        let symbol = info >> 32;
        let kind = info as u32;
        if symbol != 0 || !matches!(kind, R_X86_64_RELATIVE | R_X86_64_IRELATIVE) {
            return Err(SealedStaticApplicationErrorV1::UnsupportedRelocation);
        }
        if index < relative_count as usize && kind != R_X86_64_RELATIVE {
            return Err(SealedStaticApplicationErrorV1::RelocationMalformed);
        }
        if !is_writable_target(loads, target, 8) {
            return Err(SealedStaticApplicationErrorV1::RelocationMalformed);
        }
        let addend = if with_addend {
            read_i64(relocation, 16)?
        } else {
            let target_bytes = mapped_file_slice(bytes, loads, target, 8, PF_R | PF_W, PF_X)?;
            read_i64(target_bytes, 0)?
        };
        let resolved = u64::try_from(addend)
            .map_err(|_| SealedStaticApplicationErrorV1::RelocationMalformed)?;
        if kind == R_X86_64_RELATIVE {
            if !is_allowed_mapped_address(loads, resolved) {
                return Err(SealedStaticApplicationErrorV1::RelocationMalformed);
            }
        } else {
            let resolver = resolved;
            if !is_executable_address(loads, resolver) {
                return Err(SealedStaticApplicationErrorV1::RelocationMalformed);
            }
        }
    }
    Ok(())
}

fn validate_pair(
    entries: &[DynamicEntry],
    first: i64,
    second: i64,
) -> Result<(), SealedStaticApplicationErrorV1> {
    if dynamic_value(entries, first).is_some() != dynamic_value(entries, second).is_some() {
        return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
    }
    Ok(())
}

fn validate_triple(
    entries: &[DynamicEntry],
    first: i64,
    second: i64,
    third: i64,
) -> Result<(), SealedStaticApplicationErrorV1> {
    let present = [first, second, third]
        .into_iter()
        .filter(|tag| dynamic_value(entries, *tag).is_some())
        .count();
    if !matches!(present, 0 | 3) {
        return Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed);
    }
    Ok(())
}

fn dynamic_value(entries: &[DynamicEntry], tag: i64) -> Option<u64> {
    entries
        .iter()
        .find(|entry| entry.tag == tag)
        .map(|entry| entry.value)
}

fn mapped_file_slice<'a>(
    bytes: &'a [u8],
    loads: &[ProgramHeader],
    address: u64,
    size: u64,
    required_flags: u32,
    forbidden_flags: u32,
) -> Result<&'a [u8], SealedStaticApplicationErrorV1> {
    let mut matches = loads.iter().filter(|load| {
        load.flags & required_flags == required_flags
            && load.flags & forbidden_flags == 0
            && load
                .contains_memory_range(address, size, true)
                .unwrap_or(false)
    });
    let load = matches
        .next()
        .ok_or(SealedStaticApplicationErrorV1::DynamicSegmentMalformed)?;
    if matches.next().is_some() {
        return Err(SealedStaticApplicationErrorV1::SegmentLayout);
    }
    let relative = address
        .checked_sub(load.virtual_address)
        .ok_or(SealedStaticApplicationErrorV1::DynamicSegmentMalformed)?;
    let start = load
        .offset
        .checked_add(relative)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(SealedStaticApplicationErrorV1::DynamicSegmentMalformed)?;
    let end = start
        .checked_add(
            usize::try_from(size)
                .map_err(|_| SealedStaticApplicationErrorV1::DynamicSegmentMalformed)?,
        )
        .filter(|end| *end <= bytes.len())
        .ok_or(SealedStaticApplicationErrorV1::DynamicSegmentMalformed)?;
    Ok(&bytes[start..end])
}

fn is_executable_address(loads: &[ProgramHeader], address: u64) -> bool {
    loads.iter().any(|load| {
        load.flags & PF_X != 0
            && load
                .contains_memory_range(address, 1, true)
                .unwrap_or(false)
    })
}

fn is_allowed_mapped_address(loads: &[ProgramHeader], address: u64) -> bool {
    address <= X86_64_MAX_USER_ADDRESS
        && loads.iter().any(|load| {
            load.flags & PF_R != 0
                && load.flags & (PF_W | PF_X) != (PF_W | PF_X)
                && load
                    .contains_memory_range(address, 1, false)
                    .unwrap_or(false)
        })
}

fn is_writable_target(loads: &[ProgramHeader], address: u64, size: u64) -> bool {
    loads.iter().any(|load| {
        load.flags & PF_W != 0
            && load.flags & PF_X == 0
            && load
                .contains_memory_range(address, size, false)
                .unwrap_or(false)
    })
}

fn ranges_overlap(left_start: u64, left_end: u64, right_start: u64, right_end: u64) -> bool {
    left_start < right_end && right_start < left_end
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, SealedStaticApplicationErrorV1> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(SealedStaticApplicationErrorV1::ProgramHeaderBounds)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, SealedStaticApplicationErrorV1> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(SealedStaticApplicationErrorV1::ProgramHeaderBounds)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, SealedStaticApplicationErrorV1> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(SealedStaticApplicationErrorV1::ProgramHeaderBounds)
}

fn read_i64(bytes: &[u8], offset: usize) -> Result<i64, SealedStaticApplicationErrorV1> {
    read_u64(bytes, offset).map(|value| i64::from_le_bytes(value.to_le_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: usize = ELF_HEADER_BYTES;
    const PROGRAM: usize = ELF_PROGRAM_HEADER_BYTES;

    fn write_program(bytes: &mut [u8], index: usize, program: ProgramHeader) {
        let start = HEADER + index * PROGRAM;
        bytes[start..start + 4].copy_from_slice(&program.kind.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&program.flags.to_le_bytes());
        bytes[start + 8..start + 16].copy_from_slice(&program.offset.to_le_bytes());
        bytes[start + 16..start + 24].copy_from_slice(&program.virtual_address.to_le_bytes());
        bytes[start + 32..start + 40].copy_from_slice(&program.file_size.to_le_bytes());
        bytes[start + 40..start + 48].copy_from_slice(&program.memory_size.to_le_bytes());
        bytes[start + 48..start + 56].copy_from_slice(&program.alignment.to_le_bytes());
    }

    fn program(
        kind: u32,
        flags: u32,
        offset: u64,
        virtual_address: u64,
        file_size: u64,
        memory_size: u64,
        alignment: u64,
    ) -> ProgramHeader {
        ProgramHeader {
            kind,
            flags,
            offset,
            virtual_address,
            file_size,
            memory_size,
            alignment,
        }
    }

    fn header(bytes: &mut [u8], elf_type: u16, programs: u16, entrypoint: u64) {
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        bytes[16..18].copy_from_slice(&elf_type.to_le_bytes());
        bytes[18..20].copy_from_slice(&ELF_MACHINE_X86_64.to_le_bytes());
        bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
        bytes[24..32].copy_from_slice(&entrypoint.to_le_bytes());
        bytes[32..40].copy_from_slice(&(HEADER as u64).to_le_bytes());
        bytes[52..54].copy_from_slice(&(HEADER as u16).to_le_bytes());
        bytes[54..56].copy_from_slice(&(PROGRAM as u16).to_le_bytes());
        bytes[56..58].copy_from_slice(&programs.to_le_bytes());
    }

    fn static_elf() -> Vec<u8> {
        const PROGRAMS: usize = 4;
        const CODE_OFFSET: usize = 0x1000;
        let mut bytes = vec![0_u8; CODE_OFFSET + 1];
        header(&mut bytes, ET_EXEC, PROGRAMS as u16, 0x401000);
        let table_size = (PROGRAM * PROGRAMS) as u64;
        write_program(
            &mut bytes,
            0,
            program(
                PT_PHDR,
                PF_R,
                HEADER as u64,
                0x400040,
                table_size,
                table_size,
                8,
            ),
        );
        write_program(
            &mut bytes,
            1,
            program(
                PT_LOAD,
                PF_R,
                0,
                0x400000,
                (HEADER as u64) + table_size,
                (HEADER as u64) + table_size,
                0x1000,
            ),
        );
        write_program(
            &mut bytes,
            2,
            program(
                PT_LOAD,
                PF_R | PF_X,
                CODE_OFFSET as u64,
                0x401000,
                1,
                1,
                0x1000,
            ),
        );
        write_program(
            &mut bytes,
            3,
            program(PT_GNU_STACK, PF_R | PF_W, 0, 0, 0, 0, 16),
        );
        bytes[CODE_OFFSET] = 0xc3;
        bytes
    }

    fn static_pie() -> Vec<u8> {
        const PROGRAMS: usize = 6;
        const RELA_OFFSET: usize = 0x1a0;
        const CODE_OFFSET: usize = 0x1000;
        const DATA_OFFSET: usize = 0x2000;
        const DYNAMIC_OFFSET: usize = 0x2080;
        const DYNAMIC_ENTRIES: usize = 7;
        let mut bytes = vec![0_u8; DATA_OFFSET + 0x200];
        header(&mut bytes, ET_DYN, PROGRAMS as u16, 0x1000);
        let table_size = (PROGRAM * PROGRAMS) as u64;
        write_program(
            &mut bytes,
            0,
            program(
                PT_PHDR,
                PF_R,
                HEADER as u64,
                HEADER as u64,
                table_size,
                table_size,
                8,
            ),
        );
        write_program(
            &mut bytes,
            1,
            program(PT_LOAD, PF_R, 0, 0, 0x200, 0x200, 0x1000),
        );
        write_program(
            &mut bytes,
            2,
            program(
                PT_LOAD,
                PF_R | PF_X,
                CODE_OFFSET as u64,
                0x1000,
                1,
                1,
                0x1000,
            ),
        );
        write_program(
            &mut bytes,
            3,
            program(
                PT_LOAD,
                PF_R | PF_W,
                DATA_OFFSET as u64,
                0x2000,
                0x200,
                0x200,
                0x1000,
            ),
        );
        write_program(
            &mut bytes,
            4,
            program(
                PT_DYNAMIC,
                PF_R | PF_W,
                DYNAMIC_OFFSET as u64,
                0x2080,
                (DYNAMIC_ENTRIES * 16) as u64,
                (DYNAMIC_ENTRIES * 16) as u64,
                8,
            ),
        );
        write_program(
            &mut bytes,
            5,
            program(PT_GNU_STACK, PF_R | PF_W, 0, 0, 0, 0, 16),
        );

        let dynamic = [
            (DT_FLAGS, DF_BIND_NOW),
            (DT_FLAGS_1, DF_1_NOW | DF_1_PIE),
            (DT_RELA, RELA_OFFSET as u64),
            (DT_RELASZ, 24),
            (DT_RELAENT, 24),
            (DT_RELACOUNT, 1),
            (DT_NULL, 0),
        ];
        for (index, (tag, value)) in dynamic.into_iter().enumerate() {
            let start = DYNAMIC_OFFSET + index * 16;
            bytes[start..start + 8].copy_from_slice(&tag.to_le_bytes());
            bytes[start + 8..start + 16].copy_from_slice(&value.to_le_bytes());
        }
        bytes[RELA_OFFSET..RELA_OFFSET + 8].copy_from_slice(&0x2000_u64.to_le_bytes());
        bytes[RELA_OFFSET + 8..RELA_OFFSET + 16]
            .copy_from_slice(&(R_X86_64_RELATIVE as u64).to_le_bytes());
        bytes[RELA_OFFSET + 16..RELA_OFFSET + 24].copy_from_slice(&0x1000_i64.to_le_bytes());
        bytes[CODE_OFFSET] = 0xc3;
        bytes
    }

    fn static_pie_with_tls() -> Vec<u8> {
        const PROGRAMS: usize = 7;
        let mut bytes = static_pie();
        bytes[56..58].copy_from_slice(&(PROGRAMS as u16).to_le_bytes());
        let table_size = (PROGRAM * PROGRAMS) as u64;
        let phdr = HEADER;
        bytes[phdr + 32..phdr + 40].copy_from_slice(&table_size.to_le_bytes());
        bytes[phdr + 40..phdr + 48].copy_from_slice(&table_size.to_le_bytes());
        write_program(
            &mut bytes,
            6,
            program(PT_TLS, PF_R, 0x2100, 0x2100, 0x20, 0x40, 16),
        );
        bytes
    }

    #[test]
    fn static_application_identity_has_an_exact_cross_component_golden() {
        assert_eq!(
            sealed_static_application_identity_v1(&static_elf()).unwrap(),
            [
                0x1c, 0x1f, 0x80, 0x10, 0x16, 0xa0, 0xe0, 0x7e, 0xbc, 0x20, 0xae, 0x1e, 0xc6, 0xc7,
                0x0f, 0xf4, 0x0f, 0x91, 0x1a, 0x4e, 0xab, 0xab, 0x88, 0xe6, 0xbd, 0x21, 0x0b, 0xc4,
                0x7e, 0x68, 0xfa, 0x93,
            ]
        );
    }

    #[test]
    fn accepts_bounded_static_executable_and_self_relocating_static_pie() {
        assert!(sealed_static_application_identity_v1(&static_elf()).is_ok());
        assert!(sealed_static_application_identity_v1(&static_pie()).is_ok());
    }

    #[test]
    fn rejects_header_table_and_segment_layout_attacks() {
        let mut bounds = static_elf();
        bounds[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&bounds),
            Err(SealedStaticApplicationErrorV1::ProgramHeaderBounds)
        );

        let mut overlap = static_elf();
        let code = HEADER + 2 * PROGRAM;
        overlap[code + 8..code + 16].copy_from_slice(&0_u64.to_le_bytes());
        overlap[code + 16..code + 24].copy_from_slice(&0x400000_u64.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&overlap),
            Err(SealedStaticApplicationErrorV1::SegmentLayout)
        );

        let mut entrypoint = static_elf();
        entrypoint[24..32].copy_from_slice(&0x500000_u64.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&entrypoint),
            Err(SealedStaticApplicationErrorV1::SegmentLayout)
        );
    }

    #[test]
    fn validates_page_rounded_load_mappings_and_file_page_alias_permissions() {
        let code = HEADER + 2 * PROGRAM;
        let adjacent_offset = (HEADER + 4 * PROGRAM) as u64;

        let mut benign_shared_file_page = static_elf();
        benign_shared_file_page[24..32].copy_from_slice(&0x401120_u64.to_le_bytes());
        benign_shared_file_page[code + 8..code + 16]
            .copy_from_slice(&adjacent_offset.to_le_bytes());
        benign_shared_file_page[code + 16..code + 24].copy_from_slice(&0x401120_u64.to_le_bytes());
        benign_shared_file_page[adjacent_offset as usize] = 0xc3;
        assert!(sealed_static_application_identity_v1(&benign_shared_file_page).is_ok());

        let mut adjacent_virtual_page = benign_shared_file_page.clone();
        adjacent_virtual_page[24..32].copy_from_slice(&0x400120_u64.to_le_bytes());
        adjacent_virtual_page[code + 16..code + 24].copy_from_slice(&0x400120_u64.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&adjacent_virtual_page),
            Err(SealedStaticApplicationErrorV1::SegmentLayout)
        );

        let raw_disjoint_private_file_alias = [
            program(PT_LOAD, PF_R, 0, 0, 0x200, 0x200, 0x1000),
            program(PT_LOAD, PF_R | PF_X, 0x200, 0x1200, 1, 1, 0x1000),
            program(PT_LOAD, PF_R | PF_W, 0x201, 0x2201, 0x200, 0x200, 0x1000),
        ];
        assert!(
            validate_load_segments(&raw_disjoint_private_file_alias, HEADER, 0x190, 0x1200,)
                .is_ok()
        );

        let mut overlapping_file_alias = raw_disjoint_private_file_alias;
        overlapping_file_alias[2].offset = 0x200;
        overlapping_file_alias[2].virtual_address = 0x2200;
        assert_eq!(
            validate_load_segments(&overlapping_file_alias, HEADER, 0x190, 0x1200),
            Err(SealedStaticApplicationErrorV1::SegmentLayout)
        );

        let mut subpage_alignment = static_elf();
        subpage_alignment[code + 48..code + 56].copy_from_slice(&0x100_u64.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&subpage_alignment),
            Err(SealedStaticApplicationErrorV1::SegmentLayout)
        );

        let mut incongruent = static_elf();
        incongruent[code + 8..code + 16].copy_from_slice(&0x1001_u64.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&incongruent),
            Err(SealedStaticApplicationErrorV1::SegmentLayout)
        );
    }

    #[test]
    fn admits_one_writable_load_backed_tls_segment() {
        assert!(sealed_static_application_identity_v1(&static_pie_with_tls()).is_ok());
    }

    #[test]
    fn rejects_malformed_executable_and_outside_tls_segments() {
        let tls = HEADER + 6 * PROGRAM;

        let mut malformed = static_pie_with_tls();
        malformed[tls + 48..tls + 56].copy_from_slice(&3_u64.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&malformed),
            Err(SealedStaticApplicationErrorV1::SegmentLayout)
        );

        let mut executable = static_pie_with_tls();
        write_program(
            &mut executable,
            6,
            program(PT_TLS, PF_R, 0x1000, 0x1000, 1, 1, 1),
        );
        assert_eq!(
            sealed_static_application_identity_v1(&executable),
            Err(SealedStaticApplicationErrorV1::SegmentPermissions)
        );

        let mut outside = static_pie_with_tls();
        write_program(
            &mut outside,
            6,
            program(PT_TLS, PF_R, 0x1c0, 0x3000, 0, 0x20, 16),
        );
        assert_eq!(
            sealed_static_application_identity_v1(&outside),
            Err(SealedStaticApplicationErrorV1::SegmentLayout)
        );
    }

    #[test]
    fn rejects_interpreter_unknown_and_executable_writable_segments() {
        let mut interpreter = static_elf();
        let stack = HEADER + 3 * PROGRAM;
        interpreter[stack..stack + 4].copy_from_slice(&PT_INTERP.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&interpreter),
            Err(SealedStaticApplicationErrorV1::InterpreterPresent)
        );

        let mut unknown = static_elf();
        unknown[stack..stack + 4].copy_from_slice(&0x6000_0000_u32.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&unknown),
            Err(SealedStaticApplicationErrorV1::UnsupportedProgramHeader)
        );

        let mut writable_code = static_elf();
        let code = HEADER + 2 * PROGRAM;
        writable_code[code + 4..code + 8].copy_from_slice(&(PF_R | PF_W | PF_X).to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&writable_code),
            Err(SealedStaticApplicationErrorV1::SegmentPermissions)
        );
    }

    #[test]
    fn rejects_unterminated_unknown_and_dependency_dynamic_metadata() {
        const DYNAMIC_OFFSET: usize = 0x2080;
        let mut unterminated = static_pie();
        let null = DYNAMIC_OFFSET + 6 * 16;
        unterminated[null..null + 8].copy_from_slice(&DT_FLAGS.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&unterminated),
            Err(SealedStaticApplicationErrorV1::DynamicSegmentMalformed)
        );

        let mut unknown = static_pie();
        unknown[DYNAMIC_OFFSET..DYNAMIC_OFFSET + 8].copy_from_slice(&0x1234_i64.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&unknown),
            Err(SealedStaticApplicationErrorV1::UnsupportedDynamicTag)
        );

        let mut dependency = static_pie();
        dependency[DYNAMIC_OFFSET..DYNAMIC_OFFSET + 8].copy_from_slice(&DT_NEEDED.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&dependency),
            Err(SealedStaticApplicationErrorV1::RuntimeDependencyPresent)
        );
    }

    #[test]
    fn rejects_relocation_symbol_type_target_and_resolver_attacks() {
        const RELA_OFFSET: usize = 0x1a0;
        let mut symbol = static_pie();
        symbol[RELA_OFFSET + 8..RELA_OFFSET + 16]
            .copy_from_slice(&((1_u64 << 32) | R_X86_64_RELATIVE as u64).to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&symbol),
            Err(SealedStaticApplicationErrorV1::UnsupportedRelocation)
        );

        let mut kind = static_pie();
        kind[RELA_OFFSET + 8..RELA_OFFSET + 16].copy_from_slice(&1_u64.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&kind),
            Err(SealedStaticApplicationErrorV1::UnsupportedRelocation)
        );

        let mut target = static_pie();
        target[RELA_OFFSET..RELA_OFFSET + 8].copy_from_slice(&0x1000_u64.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&target),
            Err(SealedStaticApplicationErrorV1::RelocationMalformed)
        );

        let mut resolver = static_pie();
        resolver[RELA_OFFSET + 8..RELA_OFFSET + 16]
            .copy_from_slice(&(R_X86_64_IRELATIVE as u64).to_le_bytes());
        resolver[RELA_OFFSET + 16..RELA_OFFSET + 24].copy_from_slice(&0x2000_i64.to_le_bytes());
        assert_eq!(
            sealed_static_application_identity_v1(&resolver),
            Err(SealedStaticApplicationErrorV1::RelocationMalformed)
        );
    }

    #[test]
    fn relative_relocations_require_bounded_mapped_nonnegative_addends() {
        const RELA_OFFSET: usize = 0x1a0;

        let mut writable_destination = static_pie();
        writable_destination[RELA_OFFSET + 16..RELA_OFFSET + 24]
            .copy_from_slice(&0x2000_i64.to_le_bytes());
        assert!(sealed_static_application_identity_v1(&writable_destination).is_ok());

        for addend in [-1_i64, 0x1800, 0x5000, i64::MAX] {
            let mut malformed = static_pie();
            malformed[RELA_OFFSET + 16..RELA_OFFSET + 24].copy_from_slice(&addend.to_le_bytes());
            assert_eq!(
                sealed_static_application_identity_v1(&malformed),
                Err(SealedStaticApplicationErrorV1::RelocationMalformed),
                "addend {addend:#x}"
            );
        }
    }
}
