//! Structural admission for the static Cargo-to-binding-wrapper trampoline.

const MARKER: &[u8] = b"FE2O3_CARGO_BINDING_TRAMPOLINE_SECCOMP_EXEC_V1";
const ELF_HEADER_BYTES: usize = 64;
const PROGRAM_HEADER_BYTES: usize = 56;
const DYNAMIC_ENTRY_BYTES: usize = 16;
const MAX_PROGRAM_HEADERS: usize = 64;

const ET_DYN: u16 = 3;
const EM_X86_64: u16 = 62;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PT_SHLIB: u32 = 5;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

const DT_NULL: u64 = 0;
const DT_NEEDED: u64 = 1;
const DT_RPATH: u64 = 15;
const DT_FLAGS: u64 = 30;
const DT_RUNPATH: u64 = 29;
const DT_DEPAUDIT: u64 = 0x6fff_fefb;
const DT_AUDIT: u64 = 0x6fff_fefc;
const DT_FLAGS_1: u64 = 0x6fff_fffb;
const DT_AUXILIARY: u64 = 0x7fff_fffd;
const DT_FILTER: u64 = 0x7fff_ffff;
const DF_BIND_NOW: u64 = 0x8;
const DF_1_NOW: u64 = 0x1;
const DF_1_PIE: u64 = 0x0800_0000;

pub(crate) fn validate_v1(bytes: &[u8]) -> Result<(), String> {
    let header = bytes
        .get(..ELF_HEADER_BYTES)
        .ok_or_else(|| "Cargo binding trampoline is not an ELF64 image".to_owned())?;
    if &header[..7] != b"\x7fELF\x02\x01\x01"
        || read_u16(header, 16)? != ET_DYN
        || read_u16(header, 18)? != EM_X86_64
        || read_u32(header, 20)? != 1
        || read_u32(header, 48)? != 0
        || read_u16(header, 52)? as usize != ELF_HEADER_BYTES
    {
        return Err("Cargo binding trampoline is not a canonical x86-64 static PIE".to_owned());
    }
    let table_offset = usize::try_from(read_u64(header, 32)?)
        .map_err(|_| "Cargo binding trampoline program table is out of bounds".to_owned())?;
    let entry_size = read_u16(header, 54)? as usize;
    let entry_count = read_u16(header, 56)? as usize;
    let table_bytes = entry_size
        .checked_mul(entry_count)
        .and_then(|size| table_offset.checked_add(size))
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| "Cargo binding trampoline program table is out of bounds".to_owned())?;
    if entry_size != PROGRAM_HEADER_BYTES
        || entry_count == 0
        || entry_count > MAX_PROGRAM_HEADERS
        || table_offset < ELF_HEADER_BYTES
        || table_bytes > bytes.len()
    {
        return Err("Cargo binding trampoline program table is noncanonical".to_owned());
    }

    let mut loads = 0;
    let mut executable_loads = 0;
    let mut writable_loads = 0;
    let mut stacks = 0;
    let mut dynamic = None;
    for index in 0..entry_count {
        let start = table_offset + index * entry_size;
        let program = &bytes[start..start + entry_size];
        let kind = read_u32(program, 0)?;
        let flags = read_u32(program, 4)?;
        let offset = usize::try_from(read_u64(program, 8)?)
            .map_err(|_| "Cargo binding trampoline segment is out of bounds".to_owned())?;
        let file_size = usize::try_from(read_u64(program, 32)?)
            .map_err(|_| "Cargo binding trampoline segment is out of bounds".to_owned())?;
        let memory_size = usize::try_from(read_u64(program, 40)?)
            .map_err(|_| "Cargo binding trampoline segment is out of bounds".to_owned())?;
        let alignment = read_u64(program, 48)?;
        let end = offset
            .checked_add(file_size)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| "Cargo binding trampoline segment is out of bounds".to_owned())?;
        if file_size > memory_size
            || flags & !(PF_R | PF_W | PF_X) != 0
            || flags & (PF_W | PF_X) == (PF_W | PF_X)
            || (alignment > 1 && !alignment.is_power_of_two())
        {
            return Err("Cargo binding trampoline segment policy is invalid".to_owned());
        }
        match kind {
            PT_LOAD => {
                loads += 1;
                executable_loads += usize::from(flags == (PF_R | PF_X));
                writable_loads += usize::from(flags == (PF_R | PF_W));
                if flags != PF_R && flags != (PF_R | PF_X) && flags != (PF_R | PF_W) {
                    return Err("Cargo binding trampoline load permissions are invalid".to_owned());
                }
            }
            PT_DYNAMIC if dynamic.is_some() => {
                return Err("Cargo binding trampoline has duplicate dynamic metadata".to_owned());
            }
            PT_DYNAMIC => dynamic = Some((offset, end)),
            PT_INTERP | PT_SHLIB => {
                return Err("Cargo binding trampoline has a runtime interpreter".to_owned());
            }
            PT_GNU_STACK => {
                stacks += 1;
                if flags != (PF_R | PF_W) || file_size != 0 || memory_size != 0 {
                    return Err("Cargo binding trampoline stack policy is invalid".to_owned());
                }
            }
            _ => {}
        }
    }
    if loads != 4 || executable_loads != 1 || writable_loads != 1 || stacks != 1 {
        return Err("Cargo binding trampoline load layout is outside policy".to_owned());
    }
    validate_dynamic(
        bytes,
        dynamic.ok_or_else(|| {
            "Cargo binding trampoline has no static-PIE dynamic metadata".to_owned()
        })?,
    )?;
    if !bytes.windows(MARKER.len()).any(|window| window == MARKER) {
        return Err("Cargo binding trampoline profile marker is absent".to_owned());
    }
    Ok(())
}

fn validate_dynamic(bytes: &[u8], range: (usize, usize)) -> Result<(), String> {
    let dynamic = &bytes[range.0..range.1];
    if dynamic.is_empty() || !dynamic.len().is_multiple_of(DYNAMIC_ENTRY_BYTES) {
        return Err("Cargo binding trampoline dynamic metadata is malformed".to_owned());
    }
    let mut terminated = false;
    let mut flags = None;
    let mut flags_1 = None;
    for entry in dynamic.chunks_exact(DYNAMIC_ENTRY_BYTES) {
        let tag = read_u64(entry, 0)?;
        let value = read_u64(entry, 8)?;
        if tag == DT_NULL {
            terminated = true;
            break;
        }
        if matches!(
            tag,
            DT_NEEDED | DT_RPATH | DT_RUNPATH | DT_DEPAUDIT | DT_AUDIT | DT_AUXILIARY | DT_FILTER
        ) {
            return Err(
                "Cargo binding trampoline has a runtime dependency or search path".to_owned(),
            );
        }
        if tag == DT_FLAGS {
            flags = Some(value);
        } else if tag == DT_FLAGS_1 {
            flags_1 = Some(value);
        }
    }
    if !terminated
        || flags.is_none_or(|value| value & DF_BIND_NOW == 0)
        || flags_1.is_none_or(|value| value & (DF_1_NOW | DF_1_PIE) != DF_1_NOW | DF_1_PIE)
    {
        return Err(
            "Cargo binding trampoline does not require immediate static-PIE binding".to_owned(),
        );
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or_else(|| "Cargo binding trampoline ELF field is truncated".to_owned())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "Cargo binding trampoline ELF field is truncated".to_owned())
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or_else(|| "Cargo binding trampoline ELF field is truncated".to_owned())
}
