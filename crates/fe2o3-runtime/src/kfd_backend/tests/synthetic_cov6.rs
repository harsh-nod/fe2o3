use rmpv::{Value, encode::write_value};

const ELF_HEADER_BYTES: usize = 64;
const PROGRAM_HEADER_BYTES: usize = 56;
const PROGRAM_COUNT: usize = 8;
const SECTION_HEADER_BYTES: usize = 64;
const SECTION_COUNT: usize = 7;
const NOTE_OFFSET: usize = 0x200;
const DESCRIPTOR_OFFSET: usize = 0x1000;
const ENTRY_OFFSET: usize = 0x2000;
const DYNAMIC_OFFSET: usize = 0x3000;
const STRTAB_OFFSET: usize = 0x4000;
const SYMTAB_OFFSET: usize = 0x4040;
const SHSTRTAB_OFFSET: usize = 0x40c0;
const SECTION_OFFSET: usize = 0x4200;

/// Structurally valid loader fixture. Its entry bytes are not executable and
/// must only be used with the no-device mock backend.
pub(super) fn module() -> Vec<u8> {
    let metadata = encode(&metadata_document());
    let note = metadata_note(&metadata);
    assert!(NOTE_OFFSET + note.len() <= DESCRIPTOR_OFFSET);

    let mut bytes = vec![0_u8; SECTION_OFFSET + SECTION_COUNT * SECTION_HEADER_BYTES];
    bytes[NOTE_OFFSET..NOTE_OFFSET + note.len()].copy_from_slice(&note);
    bytes[ENTRY_OFFSET..ENTRY_OFFSET + 64].fill(0xbf);

    let strtab = b"\0vecadd\0vecadd.kd\0";
    bytes[STRTAB_OFFSET..STRTAB_OFFSET + strtab.len()].copy_from_slice(strtab);
    let shstrtab = b"\0.note\0.rodata\0.text\0.strtab\0.symtab\0.shstrtab\0";
    bytes[SHSTRTAB_OFFSET..SHSTRTAB_OFFSET + shstrtab.len()].copy_from_slice(shstrtab);

    write_elf_header(&mut bytes);
    write_program_headers(&mut bytes, note.len());
    write_dynamic_table(&mut bytes);
    write_symbols(&mut bytes);
    write_sections(&mut bytes, note.len(), strtab.len(), shstrtab.len());
    write_descriptor(&mut bytes);
    bytes
}

fn metadata_document() -> Value {
    Value::Map(vec![
        (
            Value::from("amdhsa.version"),
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        (
            Value::from("amdhsa.target"),
            Value::from("amdgcn-amd-amdhsa--gfx942:xnack-"),
        ),
        (
            Value::from("amdhsa.kernels"),
            Value::Array(vec![kernel_metadata()]),
        ),
    ])
}

fn kernel_metadata() -> Value {
    let mut arguments = vec![
        argument(Some("a_ptr"), 0, 8, "global_buffer", Some("global")),
        argument(Some("a_len"), 8, 8, "by_value", None),
    ];
    arguments.extend(hidden_arguments(16));
    map(vec![
        (".name", Value::from("vecadd")),
        (".symbol", Value::from("vecadd.kd")),
        (".args", Value::Array(arguments)),
        (".kernarg_segment_size", Value::from(272)),
        (".kernarg_segment_align", Value::from(8)),
        (".group_segment_fixed_size", Value::from(0)),
        (".private_segment_fixed_size", Value::from(16)),
        (".wavefront_size", Value::from(64)),
        (".sgpr_count", Value::from(14)),
        (".vgpr_count", Value::from(11)),
        (".agpr_count", Value::from(3)),
        (".sgpr_spill_count", Value::from(2)),
        (".vgpr_spill_count", Value::from(4)),
        (".max_flat_workgroup_size", Value::from(1024)),
    ])
}

fn hidden_arguments(base: u64) -> Vec<Value> {
    [
        (0, 4, "hidden_block_count_x"),
        (4, 4, "hidden_block_count_y"),
        (8, 4, "hidden_block_count_z"),
        (12, 2, "hidden_group_size_x"),
        (14, 2, "hidden_group_size_y"),
        (16, 2, "hidden_group_size_z"),
        (18, 2, "hidden_remainder_x"),
        (20, 2, "hidden_remainder_y"),
        (22, 2, "hidden_remainder_z"),
        (40, 8, "hidden_global_offset_x"),
        (48, 8, "hidden_global_offset_y"),
        (56, 8, "hidden_global_offset_z"),
        (64, 2, "hidden_grid_dims"),
    ]
    .into_iter()
    .map(|(offset, size, kind)| argument(None, base + offset, size, kind, None))
    .collect()
}

fn argument(
    name: Option<&str>,
    offset: u64,
    size: u64,
    value_kind: &str,
    address_space: Option<&str>,
) -> Value {
    let mut fields = vec![
        (Value::from(".offset"), Value::from(offset)),
        (Value::from(".size"), Value::from(size)),
        (Value::from(".value_kind"), Value::from(value_kind)),
    ];
    if let Some(name) = name {
        fields.push((Value::from(".name"), Value::from(name)));
    }
    if let Some(address_space) = address_space {
        fields.push((Value::from(".address_space"), Value::from(address_space)));
    }
    Value::Map(fields)
}

fn map(fields: Vec<(&str, Value)>) -> Value {
    Value::Map(
        fields
            .into_iter()
            .map(|(key, value)| (Value::from(key), value))
            .collect(),
    )
}

fn encode(value: &Value) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_value(&mut bytes, value).unwrap();
    bytes
}

fn metadata_note(metadata: &[u8]) -> Vec<u8> {
    let mut note = Vec::new();
    note.extend_from_slice(&7_u32.to_le_bytes());
    note.extend_from_slice(&u32::try_from(metadata.len()).unwrap().to_le_bytes());
    note.extend_from_slice(&32_u32.to_le_bytes());
    note.extend_from_slice(b"AMDGPU\0");
    align(&mut note, 4);
    note.extend_from_slice(metadata);
    align(&mut note, 4);
    note
}

fn write_elf_header(bytes: &mut [u8]) {
    bytes[..16].copy_from_slice(b"\x7fELF\x02\x01\x01\x40\x04\0\0\0\0\0\0\0");
    write_u16(bytes, 16, 3);
    write_u16(bytes, 18, 224);
    write_u32(bytes, 20, 1);
    write_u64(bytes, 32, ELF_HEADER_BYTES as u64);
    write_u64(bytes, 40, SECTION_OFFSET as u64);
    write_u32(bytes, 48, 0x64c);
    write_u16(bytes, 52, ELF_HEADER_BYTES as u16);
    write_u16(bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(bytes, 56, PROGRAM_COUNT as u16);
    write_u16(bytes, 58, SECTION_HEADER_BYTES as u16);
    write_u16(bytes, 60, SECTION_COUNT as u16);
    write_u16(bytes, 62, 6);
}

fn write_program_headers(bytes: &mut [u8], note_len: usize) {
    phdr(bytes, 0, 6, 4, 0x40, 0x40, 0x1c0, 0x1c0, 8);
    phdr(bytes, 1, 1, 4, 0, 0, 0x1100, 0x1100, 0x1000);
    phdr(bytes, 2, 1, 5, 0x2000, 0x4000, 0x100, 0x100, 0x1000);
    phdr(bytes, 3, 1, 6, 0x3000, 0x6000, 0x80, 0x1000, 0x1000);
    phdr(bytes, 4, 2, 6, 0x3000, 0x6000, 0x70, 0x70, 8);
    phdr(bytes, 5, 0x6474_e552, 4, 0x3000, 0x6000, 0x80, 0x1000, 1);
    phdr(bytes, 6, 0x6474_e551, 6, 0, 0, 0, 0, 0);
    phdr(
        bytes,
        7,
        4,
        4,
        NOTE_OFFSET as u64,
        NOTE_OFFSET as u64,
        note_len as u64,
        note_len as u64,
        4,
    );
}

#[allow(clippy::too_many_arguments)]
fn phdr(
    bytes: &mut [u8],
    index: usize,
    kind: u32,
    flags: u32,
    offset: u64,
    address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
) {
    let base = ELF_HEADER_BYTES + index * PROGRAM_HEADER_BYTES;
    write_u32(bytes, base, kind);
    write_u32(bytes, base + 4, flags);
    write_u64(bytes, base + 8, offset);
    write_u64(bytes, base + 16, address);
    write_u64(bytes, base + 24, address);
    write_u64(bytes, base + 32, file_size);
    write_u64(bytes, base + 40, memory_size);
    write_u64(bytes, base + 48, alignment);
}

fn write_dynamic_table(bytes: &mut [u8]) {
    for (index, (tag, value)) in [
        (6_u64, 0x800_u64),
        (11, 24),
        (5, 0x900),
        (10, 16),
        (0x6fff_fef5, 0xa00),
        (4, 0xb00),
        (0, 0),
    ]
    .into_iter()
    .enumerate()
    {
        write_u64(bytes, DYNAMIC_OFFSET + index * 16, tag);
        write_u64(bytes, DYNAMIC_OFFSET + index * 16 + 8, value);
    }
}

fn write_symbols(bytes: &mut [u8]) {
    let entry = SYMTAB_OFFSET + 24;
    write_u32(bytes, entry, 1);
    bytes[entry + 4] = 0x12;
    bytes[entry + 5] = 3;
    write_u16(bytes, entry + 6, 3);
    write_u64(bytes, entry + 8, 0x4000);
    write_u64(bytes, entry + 16, 64);

    let descriptor = SYMTAB_OFFSET + 48;
    write_u32(bytes, descriptor, 8);
    bytes[descriptor + 4] = 0x11;
    write_u16(bytes, descriptor + 6, 2);
    write_u64(bytes, descriptor + 8, DESCRIPTOR_OFFSET as u64);
    write_u64(bytes, descriptor + 16, 64);
}

fn write_sections(bytes: &mut [u8], note_len: usize, strtab_len: usize, shstrtab_len: usize) {
    section(
        bytes,
        1,
        1,
        7,
        2,
        NOTE_OFFSET as u64,
        NOTE_OFFSET as u64,
        note_len as u64,
        0,
        0,
        4,
        0,
    );
    section(
        bytes,
        2,
        7,
        1,
        2,
        DESCRIPTOR_OFFSET as u64,
        DESCRIPTOR_OFFSET as u64,
        64,
        0,
        0,
        64,
        0,
    );
    section(
        bytes,
        3,
        15,
        1,
        6,
        0x4000,
        ENTRY_OFFSET as u64,
        64,
        0,
        0,
        256,
        0,
    );
    section(
        bytes,
        4,
        21,
        3,
        0,
        0,
        STRTAB_OFFSET as u64,
        strtab_len as u64,
        0,
        0,
        1,
        0,
    );
    section(
        bytes,
        5,
        29,
        2,
        0,
        0,
        SYMTAB_OFFSET as u64,
        3 * 24,
        4,
        1,
        8,
        24,
    );
    section(
        bytes,
        6,
        37,
        3,
        0,
        0,
        SHSTRTAB_OFFSET as u64,
        shstrtab_len as u64,
        0,
        0,
        1,
        0,
    );
}

#[allow(clippy::too_many_arguments)]
fn section(
    bytes: &mut [u8],
    index: usize,
    name: u32,
    kind: u32,
    flags: u64,
    address: u64,
    offset: u64,
    size: u64,
    link: u32,
    info: u32,
    alignment: u64,
    entry_size: u64,
) {
    let base = SECTION_OFFSET + index * SECTION_HEADER_BYTES;
    write_u32(bytes, base, name);
    write_u32(bytes, base + 4, kind);
    write_u64(bytes, base + 8, flags);
    write_u64(bytes, base + 16, address);
    write_u64(bytes, base + 24, offset);
    write_u64(bytes, base + 32, size);
    write_u32(bytes, base + 40, link);
    write_u32(bytes, base + 44, info);
    write_u64(bytes, base + 48, alignment);
    write_u64(bytes, base + 56, entry_size);
}

fn write_descriptor(bytes: &mut [u8]) {
    write_u32(bytes, DESCRIPTOR_OFFSET, 0);
    write_u32(bytes, DESCRIPTOR_OFFSET + 4, 16);
    write_u32(bytes, DESCRIPTOR_OFFSET + 8, 272);
    write_i64(bytes, DESCRIPTOR_OFFSET + 16, 0x3000);
    write_u32(bytes, DESCRIPTOR_OFFSET + 44, 1);
    write_u32(bytes, DESCRIPTOR_OFFSET + 48, 0x00af_0081);
    write_u32(bytes, DESCRIPTOR_OFFSET + 52, 0x1391);
    write_u16(bytes, DESCRIPTOR_OFFSET + 56, 0x001e);
}

fn align(bytes: &mut Vec<u8>, alignment: usize) {
    while !bytes.len().is_multiple_of(alignment) {
        bytes.push(0);
    }
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_i64(bytes: &mut [u8], offset: usize, value: i64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
