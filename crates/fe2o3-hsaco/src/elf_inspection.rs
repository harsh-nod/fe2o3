use object::{
    Endianness, elf,
    read::elf::{ElfFile64, FileHeader, NoteIterator, ProgramHeader, SectionHeader},
};

use crate::{
    CodeObjectVersion, InspectionError, MAX_ELF_NOTES, MAX_ELF_SECTIONS, MAX_ELF_SEGMENTS,
    MAX_HSACO_BYTES, MAX_MESSAGEPACK_STRING_BYTES, MAX_METADATA_BYTES,
};

const ELF64_HEADER_BYTES: usize = 64;
const ELF64_PROGRAM_HEADER_BYTES: usize = 56;
const ELF64_SECTION_HEADER_BYTES: usize = 64;
const ELFOSABI_AMDGPU_HSA: u8 = 64;
const NT_AMDGPU_METADATA: u32 = 32;

pub(crate) struct InspectedEnvelope<'a> {
    pub(crate) code_object_version: CodeObjectVersion,
    pub(crate) e_flags: u32,
    pub(crate) metadata: &'a [u8],
}

#[derive(Clone, Copy)]
struct MetadataNote<'a> {
    descriptor_offset: usize,
    descriptor: &'a [u8],
}

pub(crate) fn inspect_envelope(bytes: &[u8]) -> Result<InspectedEnvelope<'_>, InspectionError> {
    let code_object_version = preflight_header(bytes)?;
    let file = ElfFile64::<Endianness>::parse(bytes)
        .map_err(|_| InspectionError::InvalidElf("object parser rejected the file"))?;
    let endian = file.endian();
    if endian != Endianness::Little {
        return Err(InspectionError::UnsupportedEndianness);
    }
    let header = file.elf_header();
    if header.e_type(endian) != elf::ET_DYN {
        return Err(InspectionError::InvalidElf("HSACO ELF type must be ET_DYN"));
    }
    if header.e_machine(endian) != elf::EM_AMDGPU {
        return Err(InspectionError::UnsupportedMachine);
    }
    if header.e_version(endian) != u32::from(elf::EV_CURRENT) {
        return Err(InspectionError::InvalidElf("unsupported object version"));
    }

    let mut metadata = None;
    let mut note_count = 0usize;
    for section in file.elf_section_table().iter() {
        let Some(notes) = section
            .notes(endian, bytes)
            .map_err(|_| InspectionError::InvalidElf("invalid note section"))?
        else {
            continue;
        };
        scan_notes(notes, endian, bytes, &mut note_count, &mut metadata)?;
    }
    for segment in file.elf_program_headers() {
        let Some(notes) = segment
            .notes(endian, bytes)
            .map_err(|_| InspectionError::InvalidElf("invalid note segment"))?
        else {
            continue;
        };
        scan_notes(notes, endian, bytes, &mut note_count, &mut metadata)?;
    }

    let metadata = metadata.ok_or(InspectionError::MissingMetadataNote)?;
    Ok(InspectedEnvelope {
        code_object_version,
        e_flags: header.e_flags(endian),
        metadata: metadata.descriptor,
    })
}

fn scan_notes<'data>(
    mut notes: NoteIterator<'data, object::elf::FileHeader64<Endianness>>,
    endian: Endianness,
    bytes: &'data [u8],
    note_count: &mut usize,
    metadata: &mut Option<MetadataNote<'data>>,
) -> Result<(), InspectionError> {
    while let Some(note) = notes
        .next()
        .map_err(|_| InspectionError::InvalidElf("malformed ELF note"))?
    {
        *note_count = note_count
            .checked_add(1)
            .ok_or(InspectionError::TooManyNotes)?;
        if *note_count > MAX_ELF_NOTES {
            return Err(InspectionError::TooManyNotes);
        }
        if note.name_bytes().len() > MAX_MESSAGEPACK_STRING_BYTES {
            return Err(InspectionError::InvalidElf("ELF note owner is too long"));
        }
        if note.name() != b"AMDGPU" || note.n_type(endian) != NT_AMDGPU_METADATA {
            continue;
        }
        if note.desc().len() > MAX_METADATA_BYTES {
            return Err(InspectionError::MetadataNoteTooLarge);
        }
        let descriptor_offset = slice_offset(bytes, note.desc())?;
        let candidate = MetadataNote {
            descriptor_offset,
            descriptor: note.desc(),
        };
        if let Some(existing) = metadata {
            let same_physical_descriptor = existing.descriptor_offset == descriptor_offset
                && existing.descriptor.len() == candidate.descriptor.len();
            if !same_physical_descriptor {
                return Err(InspectionError::DuplicateMetadataNote);
            }
        } else {
            *metadata = Some(candidate);
        }
    }
    Ok(())
}

fn slice_offset(container: &[u8], slice: &[u8]) -> Result<usize, InspectionError> {
    let offset = (slice.as_ptr() as usize)
        .checked_sub(container.as_ptr() as usize)
        .ok_or(InspectionError::InvalidElf(
            "note descriptor is outside the file",
        ))?;
    let end = offset
        .checked_add(slice.len())
        .ok_or(InspectionError::InvalidElf(
            "note descriptor range overflow",
        ))?;
    if end > container.len() {
        return Err(InspectionError::InvalidElf(
            "note descriptor is outside the file",
        ));
    }
    Ok(offset)
}

fn preflight_header(bytes: &[u8]) -> Result<CodeObjectVersion, InspectionError> {
    if bytes.len() > MAX_HSACO_BYTES {
        return Err(InspectionError::InputTooLarge);
    }
    if bytes.len() < ELF64_HEADER_BYTES {
        return Err(InspectionError::InvalidElf("truncated ELF header"));
    }
    if bytes[..4] != elf::ELFMAG {
        return Err(InspectionError::InvalidElf("invalid ELF magic"));
    }
    if bytes[4] != elf::ELFCLASS64 {
        return Err(InspectionError::UnsupportedElfClass);
    }
    if bytes[5] != elf::ELFDATA2LSB {
        return Err(InspectionError::UnsupportedEndianness);
    }
    if bytes[6] != elf::EV_CURRENT {
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
    if read_u16(bytes, 16)? != elf::ET_DYN {
        return Err(InspectionError::InvalidElf("HSACO ELF type must be ET_DYN"));
    }
    if read_u16(bytes, 18)? != elf::EM_AMDGPU {
        return Err(InspectionError::UnsupportedMachine);
    }
    if read_u32(bytes, 20)? != u32::from(elf::EV_CURRENT) {
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
