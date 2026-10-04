use crate::{
    CodeObjectVersion, InspectionError, MAX_ELF_NOTES, MAX_ELF_SECTIONS, MAX_ELF_SEGMENTS,
    MAX_HSACO_BYTES, MAX_MESSAGEPACK_STRING_BYTES, MAX_METADATA_BYTES,
};

const ELF64_HEADER_BYTES: usize = 64;
const ELF64_PROGRAM_HEADER_BYTES: usize = 56;
const ELF64_SECTION_HEADER_BYTES: usize = 64;
const ELF_MAGIC: &[u8; 4] = b"\x7fELF";
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EV_CURRENT: u8 = 1;
const ELFOSABI_AMDGPU_HSA: u8 = 64;
const ET_DYN: u16 = 3;
const EM_AMDGPU: u16 = 224;
const PT_NOTE: u32 = 4;
const SHT_NOTE: u32 = 7;
const NT_AMDGPU_METADATA: u32 = 32;

pub(crate) struct InspectedEnvelope<'a> {
    pub(crate) code_object_version: CodeObjectVersion,
    pub(crate) e_flags: u32,
    pub(crate) metadata_offset: usize,
    pub(crate) metadata: &'a [u8],
}

#[derive(Clone, Copy)]
struct MetadataNote<'a> {
    descriptor_offset: usize,
    descriptor: &'a [u8],
}

pub(crate) fn inspect_envelope(bytes: &[u8]) -> Result<InspectedEnvelope<'_>, InspectionError> {
    let code_object_version = preflight_header(bytes)?;
    let mut metadata = None;
    let mut note_count = 0usize;
    let section_offset = read_u64(bytes, 40)?;
    let section_count = usize::from(read_u16(bytes, 60)?);
    for index in 0..section_count {
        let base = table_entry_base(section_offset, ELF64_SECTION_HEADER_BYTES, index)?;
        if read_u32(bytes, base + 4)? != SHT_NOTE {
            continue;
        }
        scan_notes(
            bytes,
            read_u64(bytes, base + 24)?,
            read_u64(bytes, base + 32)?,
            &mut note_count,
            &mut metadata,
        )?;
    }

    let program_offset = read_u64(bytes, 32)?;
    let program_count = usize::from(read_u16(bytes, 56)?);
    for index in 0..program_count {
        let base = table_entry_base(program_offset, ELF64_PROGRAM_HEADER_BYTES, index)?;
        if read_u32(bytes, base)? != PT_NOTE {
            continue;
        }
        scan_notes(
            bytes,
            read_u64(bytes, base + 8)?,
            read_u64(bytes, base + 32)?,
            &mut note_count,
            &mut metadata,
        )?;
    }

    let metadata = metadata.ok_or(InspectionError::MissingMetadataNote)?;
    Ok(InspectedEnvelope {
        code_object_version,
        e_flags: read_u32(bytes, 48)?,
        metadata_offset: metadata.descriptor_offset,
        metadata: metadata.descriptor,
    })
}

fn scan_notes<'data>(
    bytes: &'data [u8],
    file_offset: u64,
    byte_len: u64,
    note_count: &mut usize,
    metadata: &mut Option<MetadataNote<'data>>,
) -> Result<(), InspectionError> {
    let region_end = file_offset
        .checked_add(byte_len)
        .ok_or(InspectionError::InvalidElf("ELF note range overflow"))?;
    let mut cursor = usize::try_from(file_offset)
        .map_err(|_| InspectionError::InvalidElf("ELF note offset overflows usize"))?;
    let region_end = usize::try_from(region_end)
        .map_err(|_| InspectionError::InvalidElf("ELF note end overflows usize"))?;
    if region_end > bytes.len() {
        return Err(InspectionError::InvalidElf(
            "ELF note range is out of bounds",
        ));
    }

    while cursor < region_end {
        let header_end = cursor
            .checked_add(12)
            .filter(|end| *end <= region_end)
            .ok_or(InspectionError::InvalidElf("malformed ELF note"))?;
        let name_len = usize::try_from(read_u32(bytes, cursor)?)
            .map_err(|_| InspectionError::InvalidElf("ELF note owner length overflows usize"))?;
        let descriptor_len = usize::try_from(read_u32(bytes, cursor + 4)?).map_err(|_| {
            InspectionError::InvalidElf("ELF note descriptor length overflows usize")
        })?;
        let note_type = read_u32(bytes, cursor + 8)?;
        let name_end = header_end
            .checked_add(name_len)
            .filter(|end| *end <= region_end)
            .ok_or(InspectionError::InvalidElf("malformed ELF note owner"))?;
        let descriptor_offset = header_end
            .checked_add(aligned_len4(name_len)?)
            .filter(|offset| *offset <= region_end)
            .ok_or(InspectionError::InvalidElf(
                "malformed ELF note owner padding",
            ))?;
        let descriptor_end = descriptor_offset
            .checked_add(descriptor_len)
            .filter(|end| *end <= region_end)
            .ok_or(InspectionError::InvalidElf("malformed ELF note descriptor"))?;
        let next = descriptor_offset
            .checked_add(aligned_len4(descriptor_len)?)
            .ok_or(InspectionError::InvalidElf("ELF note record overflow"))?;
        if next > region_end {
            return Err(InspectionError::InvalidElf("malformed ELF note padding"));
        }

        *note_count = note_count
            .checked_add(1)
            .ok_or(InspectionError::TooManyNotes)?;
        if *note_count > MAX_ELF_NOTES {
            return Err(InspectionError::TooManyNotes);
        }
        if name_len > MAX_MESSAGEPACK_STRING_BYTES {
            return Err(InspectionError::InvalidElf("ELF note owner is too long"));
        }
        let owner = &bytes[header_end..name_end];
        let owner = owner.strip_suffix(&[0]).unwrap_or(owner);
        if owner != b"AMDGPU" || note_type != NT_AMDGPU_METADATA {
            cursor = next;
            continue;
        }
        if descriptor_len > MAX_METADATA_BYTES {
            return Err(InspectionError::MetadataNoteTooLarge);
        }
        let candidate = MetadataNote {
            descriptor_offset,
            descriptor: &bytes[descriptor_offset..descriptor_end],
        };
        if let Some(existing) = metadata {
            let same_physical_descriptor = existing.descriptor_offset == descriptor_offset
                && existing.descriptor.len() == candidate.descriptor.len();
            let same_descriptor_bytes = existing.descriptor == candidate.descriptor;
            if !same_physical_descriptor && !same_descriptor_bytes {
                return Err(InspectionError::DuplicateMetadataNote);
            }
        } else {
            *metadata = Some(candidate);
        }
        cursor = next;
    }
    Ok(())
}

fn table_entry_base(
    table_offset: u64,
    entry_size: usize,
    index: usize,
) -> Result<usize, InspectionError> {
    let table_offset = usize::try_from(table_offset)
        .map_err(|_| InspectionError::InvalidElf("ELF table offset overflows usize"))?;
    let relative = entry_size
        .checked_mul(index)
        .ok_or(InspectionError::InvalidElf("ELF table index overflow"))?;
    table_offset
        .checked_add(relative)
        .ok_or(InspectionError::InvalidElf("ELF table entry overflow"))
}

fn aligned_len4(value: usize) -> Result<usize, InspectionError> {
    value
        .checked_add(3)
        .map(|value| value & !3)
        .ok_or(InspectionError::InvalidElf("ELF note alignment overflow"))
}

fn preflight_header(bytes: &[u8]) -> Result<CodeObjectVersion, InspectionError> {
    if bytes.len() > MAX_HSACO_BYTES {
        return Err(InspectionError::InputTooLarge);
    }
    if bytes.len() < ELF64_HEADER_BYTES {
        return Err(InspectionError::InvalidElf("truncated ELF header"));
    }
    if bytes[..4] != *ELF_MAGIC {
        return Err(InspectionError::InvalidElf("invalid ELF magic"));
    }
    if bytes[4] != ELFCLASS64 {
        return Err(InspectionError::UnsupportedElfClass);
    }
    if bytes[5] != ELFDATA2LSB {
        return Err(InspectionError::UnsupportedEndianness);
    }
    if bytes[6] != EV_CURRENT {
        return Err(InspectionError::InvalidElf("unsupported ident version"));
    }
    if bytes[7] != ELFOSABI_AMDGPU_HSA {
        return Err(InspectionError::UnsupportedOsAbi);
    }
    let code_object_version = match bytes[8] {
        2 => CodeObjectVersion::V4,
        3 => CodeObjectVersion::V5,
        4 => CodeObjectVersion::V6,
        _ => return Err(InspectionError::UnsupportedCodeObjectVersion),
    };
    if read_u16(bytes, 16)? != ET_DYN {
        return Err(InspectionError::InvalidElf("HSACO ELF type must be ET_DYN"));
    }
    if read_u16(bytes, 18)? != EM_AMDGPU {
        return Err(InspectionError::UnsupportedMachine);
    }
    if read_u32(bytes, 20)? != u32::from(EV_CURRENT) {
        return Err(InspectionError::InvalidElf("unsupported object version"));
    }
    if usize::from(read_u16(bytes, 52)?) != ELF64_HEADER_BYTES {
        return Err(InspectionError::InvalidElf("invalid ELF header size"));
    }

    let program_offset = read_u64(bytes, 32)?;
    let program_entry_size = usize::from(read_u16(bytes, 54)?);
    let program_count = usize::from(read_u16(bytes, 56)?);
    if program_count > MAX_ELF_SEGMENTS {
        return Err(InspectionError::TooManySegments);
    }
    if program_count != 0 {
        if program_entry_size != ELF64_PROGRAM_HEADER_BYTES {
            return Err(InspectionError::InvalidElf("invalid program header size"));
        }
        validate_table_range(
            bytes.len(),
            program_offset,
            program_entry_size,
            program_count,
        )?;
    }

    let section_offset = read_u64(bytes, 40)?;
    let section_entry_size = usize::from(read_u16(bytes, 58)?);
    let section_count = usize::from(read_u16(bytes, 60)?);
    if section_count > MAX_ELF_SECTIONS {
        return Err(InspectionError::TooManySections);
    }
    let string_table_index = usize::from(read_u16(bytes, 62)?);
    if section_count == 0 {
        if section_offset != 0 || string_table_index != 0 {
            return Err(InspectionError::InvalidElf(
                "extended section counts are unsupported",
            ));
        }
    } else {
        if section_entry_size != ELF64_SECTION_HEADER_BYTES {
            return Err(InspectionError::InvalidElf("invalid section header size"));
        }
        validate_table_range(
            bytes.len(),
            section_offset,
            section_entry_size,
            section_count,
        )?;
        if string_table_index >= section_count {
            return Err(InspectionError::InvalidElf(
                "invalid section-name string table index",
            ));
        }
    }
    Ok(code_object_version)
}

fn validate_table_range(
    input_length: usize,
    offset: u64,
    entry_size: usize,
    count: usize,
) -> Result<(), InspectionError> {
    let offset = usize::try_from(offset)
        .map_err(|_| InspectionError::InvalidElf("ELF table offset overflows usize"))?;
    let size = entry_size
        .checked_mul(count)
        .ok_or(InspectionError::InvalidElf("ELF table size overflow"))?;
    let end = offset
        .checked_add(size)
        .ok_or(InspectionError::InvalidElf("ELF table end overflow"))?;
    if end > input_length {
        return Err(InspectionError::InvalidElf("ELF table is out of bounds"));
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, InspectionError> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, InspectionError> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, InspectionError> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], InspectionError> {
    let end = offset
        .checked_add(N)
        .ok_or(InspectionError::InvalidElf("ELF field offset overflow"))?;
    bytes
        .get(offset..end)
        .ok_or(InspectionError::InvalidElf("truncated ELF field"))?
        .try_into()
        .map_err(|_| InspectionError::InvalidElf("invalid ELF field width"))
}
