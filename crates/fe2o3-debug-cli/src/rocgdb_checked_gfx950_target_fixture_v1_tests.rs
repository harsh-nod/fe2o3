//! Synthetic ELF fixture mechanically extracted from fe2o3-hsaco tests.
//! Not native execution or a checked-device substitute.
use fe2o3_amd_target::AmdTargetId;
use rmpv::{Value, encode::write_value};
const ELF_HEADER_BYTES: usize = 64;
const SECTION_HEADER_BYTES: usize = 64;
pub(super) const ENTRY_OFFSET: usize = 0xa00;

pub(super) fn artifact(target: &str) -> Vec<u8> {
    let parsed = AmdTargetId::parse(target).unwrap();
    let mut kernel = valid_kernel("vecadd", "vecadd.kd");
    set_field(&mut kernel, ".wavefront_size", Value::from(64));
    set_field(&mut kernel, ".vgpr_count", Value::from(11));
    remove_field(&mut kernel, ".workgroup_processor_mode");
    let document = map(vec![
        (
            "amdhsa.version",
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        (
            "amdhsa.target",
            Value::from(format!("amdgcn-amd-amdhsa--{target}")),
        ),
        ("amdhsa.kernels", Value::Array(vec![kernel])),
    ]);
    let mut fixture = binding_fixture_for_document_at(document, 0x9c0);
    assert_eq!(fixture.entry_offset, ENTRY_OFFSET);
    write_u32(&mut fixture.bytes, 48, parsed.amdhsa_elf_flags_v4_plus());
    write_u32(&mut fixture.bytes, fixture.descriptor_offset + 44, 1);
    write_u32(
        &mut fixture.bytes,
        fixture.descriptor_offset + 48,
        0x00af_0081,
    );
    write_u16(&mut fixture.bytes, fixture.descriptor_offset + 56, 0x001e);
    fixture.bytes
}
fn valid_kernel(name: &str, symbol: &str) -> Value {
    let mut arguments = vec![
        argument(Some("a_ptr"), 0, 8, "global_buffer", Some("global")),
        argument(Some("a_len"), 8, 8, "by_value", None),
    ];
    arguments.extend(v5_hidden_arguments(16));
    map(vec![
        (".name", Value::from(name)),
        (".symbol", Value::from(symbol)),
        (".args", Value::Array(arguments)),
        (".kernarg_segment_size", Value::from(272)),
        (".kernarg_segment_align", Value::from(8)),
        (".group_segment_fixed_size", Value::from(0)),
        (".private_segment_fixed_size", Value::from(16)),
        (".wavefront_size", Value::from(32)),
        (".sgpr_count", Value::from(14)),
        (".vgpr_count", Value::from(7)),
        (".agpr_count", Value::from(3)),
        (".sgpr_spill_count", Value::from(2)),
        (".vgpr_spill_count", Value::from(4)),
        (".workgroup_processor_mode", Value::from(1)),
        (".max_flat_workgroup_size", Value::from(1024)),
    ])
}

fn v5_hidden_arguments(base: u64) -> Vec<Value> {
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

fn as_map_mut(value: &mut Value) -> &mut Vec<(Value, Value)> {
    match value {
        Value::Map(map) => map,
        _ => panic!("expected map"),
    }
}

fn set_field(map: &mut Value, key: &str, replacement: Value) {
    let (_, value) = as_map_mut(map)
        .iter_mut()
        .find(|(candidate, _)| candidate.as_str() == Some(key))
        .unwrap();
    *value = replacement;
}

fn remove_field(map: &mut Value, key: &str) {
    as_map_mut(map).retain(|(candidate, _)| candidate.as_str() != Some(key));
}

fn encode(value: &Value) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_value(&mut bytes, value).unwrap();
    bytes
}

fn metadata_note(metadata: &[u8]) -> Vec<u8> {
    let owner = b"AMDGPU\0";
    let mut note = Vec::new();
    note.extend_from_slice(&u32::try_from(owner.len()).unwrap().to_le_bytes());
    note.extend_from_slice(&u32::try_from(metadata.len()).unwrap().to_le_bytes());
    note.extend_from_slice(&32u32.to_le_bytes());
    note.extend_from_slice(owner);
    align(&mut note, 4);
    note.extend_from_slice(metadata);
    align(&mut note, 4);
    note
}

struct BindingFixture {
    bytes: Vec<u8>,
    descriptor_offset: usize,
    entry_offset: usize,
}

fn binding_fixture_for_document_at(document: Value, descriptor_offset: usize) -> BindingFixture {
    const PROGRAM_HEADER_BYTES: usize = 56;
    const PROGRAM_COUNT: usize = 3;
    const SECTION_COUNT: usize = 7;

    let note = metadata_note(&encode(&document));
    let first_program_header = ELF_HEADER_BYTES;
    let second_program_header = first_program_header + PROGRAM_HEADER_BYTES;
    let mut bytes = vec![0; ELF_HEADER_BYTES + PROGRAM_COUNT * PROGRAM_HEADER_BYTES];
    align(&mut bytes, 64);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);
    align(&mut bytes, 64);
    assert!(bytes.len() <= descriptor_offset);
    bytes.resize(descriptor_offset, 0);
    bytes.resize(descriptor_offset + 64, 0);
    align(&mut bytes, 256);
    let entry_offset = bytes.len();
    bytes.resize(entry_offset + 64, 0xbf);
    let entry_address = entry_offset as u64 + 0x1000;
    let descriptor_address = descriptor_offset as u64;

    let strtab = b"\0vecadd\0vecadd.kd\0other\0";
    let entry_name_index = 1u32;
    let descriptor_name_index = 8u32;
    let other_name_index = 18u32;
    let strtab_offset = bytes.len();
    bytes.extend_from_slice(strtab);
    align(&mut bytes, 8);

    let symtab_offset = bytes.len();
    bytes.resize(symtab_offset + 4 * 24, 0);
    let entry_symbol = symtab_offset + 24;
    write_u32(&mut bytes, entry_symbol, entry_name_index);
    bytes[entry_symbol + 4] = 0x12;
    bytes[entry_symbol + 5] = 3;
    write_u16(&mut bytes, entry_symbol + 6, 3);
    write_u64(&mut bytes, entry_symbol + 8, entry_address);
    write_u64(&mut bytes, entry_symbol + 16, 64);

    let descriptor_symbol = symtab_offset + 48;
    write_u32(&mut bytes, descriptor_symbol, descriptor_name_index);
    bytes[descriptor_symbol + 4] = 0x11;
    write_u16(&mut bytes, descriptor_symbol + 6, 2);
    write_u64(&mut bytes, descriptor_symbol + 8, descriptor_address);
    write_u64(&mut bytes, descriptor_symbol + 16, 64);

    let spare_symbol = symtab_offset + 72;
    write_u32(&mut bytes, spare_symbol, other_name_index);
    bytes[spare_symbol + 4] = 0x10;
    write_u16(&mut bytes, spare_symbol + 6, 0xfff1);

    let shstrtab = b"\0.note\0.rodata\0.text\0.strtab\0.symtab\0.shstrtab\0";
    let shstrtab_offset = bytes.len();
    bytes.extend_from_slice(shstrtab);
    align(&mut bytes, 8);
    let section_offset = bytes.len();
    bytes.resize(section_offset + SECTION_COUNT * SECTION_HEADER_BYTES, 0);

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = 4;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 32, first_program_header as u64);
    write_u64(&mut bytes, 40, section_offset as u64);
    write_u32(&mut bytes, 48, 0x4a);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(&mut bytes, 56, PROGRAM_COUNT as u16);
    write_u16(&mut bytes, 58, SECTION_HEADER_BYTES as u16);
    write_u16(&mut bytes, 60, SECTION_COUNT as u16);
    write_u16(&mut bytes, 62, 6);

    write_u32(&mut bytes, first_program_header, 1);
    write_u32(&mut bytes, first_program_header + 4, 4);
    write_u64(&mut bytes, first_program_header + 8, 0);
    write_u64(&mut bytes, first_program_header + 16, 0);
    write_u64(
        &mut bytes,
        first_program_header + 32,
        (descriptor_offset + 64) as u64,
    );
    write_u64(
        &mut bytes,
        first_program_header + 40,
        (descriptor_offset + 64) as u64,
    );
    write_u64(&mut bytes, first_program_header + 48, 0x1000);

    write_u32(&mut bytes, second_program_header, 1);
    write_u32(&mut bytes, second_program_header + 4, 5);
    write_u64(&mut bytes, second_program_header + 8, entry_offset as u64);
    write_u64(&mut bytes, second_program_header + 16, entry_address);
    write_u64(&mut bytes, second_program_header + 32, 64);
    write_u64(&mut bytes, second_program_header + 40, 64);
    write_u64(&mut bytes, second_program_header + 48, 0x1000);

    let note_header = section_offset + SECTION_HEADER_BYTES;
    write_u32(&mut bytes, note_header, 1);
    write_u32(&mut bytes, note_header + 4, 7);
    write_u64(&mut bytes, note_header + 8, 2);
    write_u64(&mut bytes, note_header + 16, note_offset as u64);
    write_u64(&mut bytes, note_header + 24, note_offset as u64);
    write_u64(&mut bytes, note_header + 32, note.len() as u64);
    write_u64(&mut bytes, note_header + 48, 4);

    let rodata_header = section_offset + 2 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, rodata_header, 7);
    write_u32(&mut bytes, rodata_header + 4, 1);
    write_u64(&mut bytes, rodata_header + 8, 2);
    write_u64(&mut bytes, rodata_header + 16, descriptor_address);
    write_u64(&mut bytes, rodata_header + 24, descriptor_offset as u64);
    write_u64(&mut bytes, rodata_header + 32, 64);
    write_u64(&mut bytes, rodata_header + 48, 64);

    let text_header = section_offset + 3 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, text_header, 15);
    write_u32(&mut bytes, text_header + 4, 1);
    write_u64(&mut bytes, text_header + 8, 6);
    write_u64(&mut bytes, text_header + 16, entry_address);
    write_u64(&mut bytes, text_header + 24, entry_offset as u64);
    write_u64(&mut bytes, text_header + 32, 64);
    write_u64(&mut bytes, text_header + 48, 256);

    let strtab_header = section_offset + 4 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, strtab_header, 21);
    write_u32(&mut bytes, strtab_header + 4, 3);
    write_u64(&mut bytes, strtab_header + 24, strtab_offset as u64);
    write_u64(&mut bytes, strtab_header + 32, strtab.len() as u64);
    write_u64(&mut bytes, strtab_header + 48, 1);

    let symtab_header = section_offset + 5 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, symtab_header, 29);
    write_u32(&mut bytes, symtab_header + 4, 2);
    write_u64(&mut bytes, symtab_header + 24, symtab_offset as u64);
    write_u64(&mut bytes, symtab_header + 32, 4 * 24);
    write_u32(&mut bytes, symtab_header + 40, 4);
    write_u32(&mut bytes, symtab_header + 44, 1);
    write_u64(&mut bytes, symtab_header + 48, 8);
    write_u64(&mut bytes, symtab_header + 56, 24);

    let shstrtab_header = section_offset + 6 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, shstrtab_header, 37);
    write_u32(&mut bytes, shstrtab_header + 4, 3);
    write_u64(&mut bytes, shstrtab_header + 24, shstrtab_offset as u64);
    write_u64(&mut bytes, shstrtab_header + 32, shstrtab.len() as u64);
    write_u64(&mut bytes, shstrtab_header + 48, 1);

    write_u32(&mut bytes, descriptor_offset, 0);
    write_u32(&mut bytes, descriptor_offset + 4, 16);
    write_u32(&mut bytes, descriptor_offset + 8, 272);
    write_i64(
        &mut bytes,
        descriptor_offset + 16,
        i64::try_from(entry_address - descriptor_address).unwrap(),
    );
    write_u32(&mut bytes, descriptor_offset + 44, 0x40);
    write_u32(&mut bytes, descriptor_offset + 48, 0xe0af_0000);
    write_u32(&mut bytes, descriptor_offset + 52, 0x1391);
    write_u16(&mut bytes, descriptor_offset + 56, 0x041e);

    BindingFixture {
        bytes,
        descriptor_offset,
        entry_offset,
    }
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
