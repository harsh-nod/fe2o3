use core::fmt;

use sha2::{Digest, Sha256};

const IDENTITY_DOMAIN: &[u8] = b"FE2O3/SEALED-STATIC-APPLICATION-IDENTITY/V1\0";
const ELF_HEADER_BYTES: usize = 64;
const ELF_PROGRAM_HEADER_BYTES: usize = 56;
const ELF_MACHINE_X86_64: u16 = 62;
const PT_INTERP: u32 = 3;
const PT_DYNAMIC: u32 = 2;
const DT_NULL: i64 = 0;
const DT_NEEDED: i64 = 1;
const DT_DEPAUDIT: i64 = 0x6fff_fefb;
const DT_AUDIT: i64 = 0x6fff_fefc;
const DT_AUXILIARY: i64 = 0x7fff_fffd;
const DT_FILTER: i64 = 0x7fff_ffff;
const MAX_PROGRAM_HEADERS: usize = 1_024;

/// Rejection while deriving the identity of a loader-independent application image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SealedStaticApplicationErrorV1 {
    InvalidElf,
    UnsupportedElf,
    ProgramHeaderBounds,
    InterpreterPresent,
    RuntimeDependencyPresent,
}

impl fmt::Display for SealedStaticApplicationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidElf => formatter.write_str("application is not a canonical ELF64 image"),
            Self::UnsupportedElf => formatter.write_str(
                "application is not a little-endian x86-64 executable or static PIE",
            ),
            Self::ProgramHeaderBounds => {
                formatter.write_str("application ELF program headers are out of bounds")
            }
            Self::InterpreterPresent => formatter.write_str(
                "application has an ELF interpreter and is outside the sealed-static profile",
            ),
            Self::RuntimeDependencyPresent => formatter.write_str(
                "application has a dynamic dependency/audit/filter entry and is outside the sealed-static profile",
            ),
        }
    }
}

impl std::error::Error for SealedStaticApplicationErrorV1 {}

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
        || read_u16(header, 18)? != ELF_MACHINE_X86_64
        || read_u32(header, 20)? != 1
        || !matches!(read_u16(header, 16)?, 2 | 3)
        || read_u16(header, 52)? as usize != ELF_HEADER_BYTES
    {
        return Err(SealedStaticApplicationErrorV1::UnsupportedElf);
    }
    let program_offset = usize::try_from(read_u64(header, 32)?)
        .map_err(|_| SealedStaticApplicationErrorV1::ProgramHeaderBounds)?;
    let entry_size = read_u16(header, 54)? as usize;
    let entry_count = read_u16(header, 56)? as usize;
    if entry_size != ELF_PROGRAM_HEADER_BYTES
        || entry_count == 0
        || entry_count > MAX_PROGRAM_HEADERS
    {
        return Err(SealedStaticApplicationErrorV1::ProgramHeaderBounds);
    }
    let table_bytes = entry_size
        .checked_mul(entry_count)
        .and_then(|size| program_offset.checked_add(size))
        .filter(|end| *end <= bytes.len())
        .ok_or(SealedStaticApplicationErrorV1::ProgramHeaderBounds)?;
    debug_assert!(table_bytes <= bytes.len());

    for index in 0..entry_count {
        let start = program_offset + index * entry_size;
        let program = &bytes[start..start + entry_size];
        match read_u32(program, 0)? {
            PT_INTERP => return Err(SealedStaticApplicationErrorV1::InterpreterPresent),
            PT_DYNAMIC => validate_dynamic_segment(bytes, program)?,
            _ => {}
        }
    }
    Ok(())
}

fn validate_dynamic_segment(
    bytes: &[u8],
    program: &[u8],
) -> Result<(), SealedStaticApplicationErrorV1> {
    let offset = usize::try_from(read_u64(program, 8)?)
        .map_err(|_| SealedStaticApplicationErrorV1::ProgramHeaderBounds)?;
    let size = usize::try_from(read_u64(program, 32)?)
        .map_err(|_| SealedStaticApplicationErrorV1::ProgramHeaderBounds)?;
    let end = offset
        .checked_add(size)
        .filter(|end| *end <= bytes.len())
        .ok_or(SealedStaticApplicationErrorV1::ProgramHeaderBounds)?;
    if size % 16 != 0 {
        return Err(SealedStaticApplicationErrorV1::ProgramHeaderBounds);
    }
    for entry in bytes[offset..end].chunks_exact(16) {
        let tag = read_i64(entry, 0)?;
        if tag == DT_NULL {
            break;
        }
        if matches!(
            tag,
            DT_NEEDED | DT_DEPAUDIT | DT_AUDIT | DT_AUXILIARY | DT_FILTER
        ) {
            return Err(SealedStaticApplicationErrorV1::RuntimeDependencyPresent);
        }
    }
    Ok(())
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

    fn elf(program_type: u32, dynamic_tag: i64) -> Vec<u8> {
        let mut bytes = vec![0_u8; ELF_HEADER_BYTES + ELF_PROGRAM_HEADER_BYTES + 16];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        bytes[16..18].copy_from_slice(&3_u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&ELF_MACHINE_X86_64.to_le_bytes());
        bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
        bytes[32..40].copy_from_slice(&(ELF_HEADER_BYTES as u64).to_le_bytes());
        bytes[52..54].copy_from_slice(&(ELF_HEADER_BYTES as u16).to_le_bytes());
        bytes[54..56].copy_from_slice(&(ELF_PROGRAM_HEADER_BYTES as u16).to_le_bytes());
        bytes[56..58].copy_from_slice(&1_u16.to_le_bytes());
        let program = ELF_HEADER_BYTES;
        bytes[program..program + 4].copy_from_slice(&program_type.to_le_bytes());
        bytes[program + 8..program + 16]
            .copy_from_slice(&((ELF_HEADER_BYTES + ELF_PROGRAM_HEADER_BYTES) as u64).to_le_bytes());
        bytes[program + 32..program + 40].copy_from_slice(&16_u64.to_le_bytes());
        bytes[ELF_HEADER_BYTES + ELF_PROGRAM_HEADER_BYTES..][..8]
            .copy_from_slice(&dynamic_tag.to_le_bytes());
        bytes
    }

    #[test]
    fn accepts_self_relocating_static_pie() {
        assert!(sealed_static_application_identity_v1(&elf(PT_DYNAMIC, DT_NULL)).is_ok());
    }

    #[test]
    fn rejects_interpreter_and_dynamic_dependency() {
        assert_eq!(
            sealed_static_application_identity_v1(&elf(PT_INTERP, DT_NULL)),
            Err(SealedStaticApplicationErrorV1::InterpreterPresent)
        );
        assert_eq!(
            sealed_static_application_identity_v1(&elf(PT_DYNAMIC, DT_NEEDED)),
            Err(SealedStaticApplicationErrorV1::RuntimeDependencyPresent)
        );
    }
}
