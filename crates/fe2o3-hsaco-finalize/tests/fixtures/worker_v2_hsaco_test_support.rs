// Shared synthetic gfx942 HSACO builder reused by typed publication tests.

use fe2o3_hsaco_finalize::DEVICE_DESCRIPTOR_SECTION_NAME;
use rmpv::{Value, encode::write_value};

const ELF_HEADER_BYTES: usize = 64;
const PROGRAM_HEADER_BYTES: usize = 56;
const SECTION_HEADER_BYTES: usize = 64;
const NOTE_SECTION_INDEX: usize = 1;
const RODATA_SECTION_INDEX: usize = 2;
const TEXT_SECTION_INDEX: usize = 3;
const STRTAB_SECTION_INDEX: usize = 4;
const SYMTAB_SECTION_INDEX: usize = 5;
const CANONICAL_DESCRIPTOR_SECTION_INDEX: usize = 6;
const SHSTRTAB_SECTION_INDEX: usize = 7;

#[derive(Clone, Copy)]
struct FixtureOptions<'a> {
    target: &'a str,
    code_object_version: u8,
    entry: &'a str,
    descriptor: &'a str,
    required_workgroup_size: [u32; 3],
    max_flat_workgroup_size: u32,
    wavefront_size: u32,
    descriptor_wavefront_size: u32,
    include_export: bool,
    include_canonical_descriptor_section_name: bool,
}

impl FixtureOptions<'static> {
    fn valid() -> Self {
        Self {
            target: "gfx942",
            code_object_version: 4,
            entry: "vecadd",
            descriptor: "vecadd.kd",
            required_workgroup_size: [256, 1, 1],
            max_flat_workgroup_size: 256,
            wavefront_size: 64,
            descriptor_wavefront_size: 64,
            include_export: true,
            include_canonical_descriptor_section_name: false,
        }
    }
}

struct Fixture {
    bytes: Vec<u8>,
    text_offset: usize,
}

fn fixture(options: FixtureOptions<'_>) -> Fixture {
    fixture_with_descriptor_table(options, None)
}

fn fixture_with_descriptor_table(
    options: FixtureOptions<'_>,
    descriptor_table: Option<&[u8]>,
) -> Fixture {
    const PROGRAM_COUNT: usize = 2;
    let metadata = metadata(options);
    let note = metadata_note(&metadata);
    let mut bytes = vec![0; ELF_HEADER_BYTES + PROGRAM_COUNT * PROGRAM_HEADER_BYTES];

    align(&mut bytes, 64);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);

    align(&mut bytes, 64);
    let rodata_offset = bytes.len();
    let descriptor_offset = bytes.len();
    bytes.resize(bytes.len() + 64, 0);
    let rodata_end = bytes.len();

    align(&mut bytes, 256);
    let text_offset = bytes.len();
    bytes.resize(bytes.len() + 64, 0xbf);
    let export_offset = if options.include_export {
        align(&mut bytes, 256);
        let offset = bytes.len();
        bytes.resize(bytes.len() + 64, 0xbe);
        Some(offset)
    } else {
        None
    };
    let text_end = bytes.len();

    let mut strtab = vec![0];
    let entry_name = push_name(&mut strtab, options.entry);
    let descriptor_name = push_name(&mut strtab, options.descriptor);
    let export_name = options
        .include_export
        .then(|| push_name(&mut strtab, "ffi_export"));
    let strtab_offset = bytes.len();
    bytes.extend_from_slice(&strtab);

    align(&mut bytes, 8);
    let symtab_offset = bytes.len();
    let symbol_count = 3 + usize::from(options.include_export);
    bytes.resize(symtab_offset + symbol_count * 24, 0);
    let entry_symbol = symtab_offset + 24;
    write_u32(&mut bytes, entry_symbol, entry_name);
    bytes[entry_symbol + 4] = 0x12;
    bytes[entry_symbol + 5] = 3;
    write_u16(&mut bytes, entry_symbol + 6, TEXT_SECTION_INDEX as u16);
    let entry_address = (text_offset + 0x1000) as u64;
    write_u64(&mut bytes, entry_symbol + 8, entry_address);
    write_u64(&mut bytes, entry_symbol + 16, 64);

    let descriptor_symbol = symtab_offset + 48;
    write_u32(&mut bytes, descriptor_symbol, descriptor_name);
    bytes[descriptor_symbol + 4] = 0x11;
    write_u16(
        &mut bytes,
        descriptor_symbol + 6,
        RODATA_SECTION_INDEX as u16,
    );
    write_u64(&mut bytes, descriptor_symbol + 8, descriptor_offset as u64);
    write_u64(&mut bytes, descriptor_symbol + 16, 64);

    if let (Some(name), Some(offset)) = (export_name, export_offset) {
        let export_symbol = symtab_offset + 72;
        write_u32(&mut bytes, export_symbol, name);
        bytes[export_symbol + 4] = 0x12;
        write_u16(&mut bytes, export_symbol + 6, TEXT_SECTION_INDEX as u16);
        write_u64(&mut bytes, export_symbol + 8, (offset + 0x1000) as u64);
        write_u64(&mut bytes, export_symbol + 16, 64);
    }

    align(&mut bytes, 8);
    let canonical_descriptor_offset = bytes.len();
    if let Some(table) = descriptor_table {
        bytes.extend_from_slice(table);
    }

    write_u32(&mut bytes, descriptor_offset + 8, 272);
    write_i64(
        &mut bytes,
        descriptor_offset + 16,
        i64::try_from(entry_address - descriptor_offset as u64).unwrap(),
    );
    write_u32(&mut bytes, descriptor_offset + 44, 1);
    write_u32(&mut bytes, descriptor_offset + 48, 0x00af_0081);
    write_u32(&mut bytes, descriptor_offset + 52, 0x1390);
    write_u16(
        &mut bytes,
        descriptor_offset + 56,
        if options.descriptor_wavefront_size == 32 {
            0x041e
        } else {
            0x001e
        },
    );

    let mut shstr = vec![0];
    let note_name = push_name(&mut shstr, ".note");
    let rodata_name = push_name(&mut shstr, ".rodata");
    let text_name = push_name(&mut shstr, ".text");
    let strtab_name = push_name(&mut shstr, ".strtab");
    let symtab_name = push_name(&mut shstr, ".symtab");
    let canonical_descriptor_name = push_name(
        &mut shstr,
        if options.include_canonical_descriptor_section_name || descriptor_table.is_some() {
            DEVICE_DESCRIPTOR_SECTION_NAME
        } else {
            ".fixture"
        },
    );
    let shstrtab_name = push_name(&mut shstr, ".shstrtab");
    let shstrtab_offset = bytes.len();
    bytes.extend_from_slice(&shstr);
    align(&mut bytes, 8);
    let section_table_offset = bytes.len();
    bytes.resize(section_table_offset + 8 * SECTION_HEADER_BYTES, 0);

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = options.code_object_version;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 32, ELF_HEADER_BYTES as u64);
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u32(&mut bytes, 48, target_flags(options.target));
    write_u16(&mut bytes, 52, ELF_HEADER_BYTES as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(&mut bytes, 56, PROGRAM_COUNT as u16);
    write_u16(&mut bytes, 58, SECTION_HEADER_BYTES as u16);
    write_u16(&mut bytes, 60, 8);
    write_u16(&mut bytes, 62, SHSTRTAB_SECTION_INDEX as u16);

    let rodata_program = ELF_HEADER_BYTES;
    write_u32(&mut bytes, rodata_program, 1);
    write_u32(&mut bytes, rodata_program + 4, 4);
    write_u64(&mut bytes, rodata_program + 32, rodata_end as u64);
    write_u64(&mut bytes, rodata_program + 40, rodata_end as u64);
    write_u64(&mut bytes, rodata_program + 48, 0x1000);

    let text_program = ELF_HEADER_BYTES + PROGRAM_HEADER_BYTES;
    write_u32(&mut bytes, text_program, 1);
    write_u32(&mut bytes, text_program + 4, 5);
    write_u64(&mut bytes, text_program + 8, text_offset as u64);
    write_u64(&mut bytes, text_program + 16, (text_offset + 0x1000) as u64);
    write_u64(
        &mut bytes,
        text_program + 32,
        (text_end - text_offset) as u64,
    );
    write_u64(
        &mut bytes,
        text_program + 40,
        (text_end - text_offset) as u64,
    );
    write_u64(&mut bytes, text_program + 48, 0x1000);

    let note_header = section_table_offset + NOTE_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, note_header, note_name);
    write_u32(&mut bytes, note_header + 4, 7);
    write_u64(&mut bytes, note_header + 8, 2);
    write_u64(&mut bytes, note_header + 24, note_offset as u64);
    write_u64(&mut bytes, note_header + 32, note.len() as u64);
    write_u64(&mut bytes, note_header + 48, 4);

    let rodata_header = section_table_offset + RODATA_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, rodata_header, rodata_name);
    write_u32(&mut bytes, rodata_header + 4, 1);
    write_u64(&mut bytes, rodata_header + 8, 2);
    write_u64(&mut bytes, rodata_header + 16, rodata_offset as u64);
    write_u64(&mut bytes, rodata_header + 24, rodata_offset as u64);
    write_u64(
        &mut bytes,
        rodata_header + 32,
        (rodata_end - rodata_offset) as u64,
    );
    write_u64(&mut bytes, rodata_header + 48, 64);

    let text_header = section_table_offset + TEXT_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, text_header, text_name);
    write_u32(&mut bytes, text_header + 4, 1);
    write_u64(&mut bytes, text_header + 8, 6);
    write_u64(&mut bytes, text_header + 16, (text_offset + 0x1000) as u64);
    write_u64(&mut bytes, text_header + 24, text_offset as u64);
    write_u64(
        &mut bytes,
        text_header + 32,
        (text_end - text_offset) as u64,
    );
    write_u64(&mut bytes, text_header + 48, 256);

    let strtab_header = section_table_offset + STRTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, strtab_header, strtab_name);
    write_u32(&mut bytes, strtab_header + 4, 3);
    write_u64(&mut bytes, strtab_header + 24, strtab_offset as u64);
    write_u64(&mut bytes, strtab_header + 32, strtab.len() as u64);
    write_u64(&mut bytes, strtab_header + 48, 1);

    let symtab_header = section_table_offset + SYMTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, symtab_header, symtab_name);
    write_u32(&mut bytes, symtab_header + 4, 2);
    write_u64(&mut bytes, symtab_header + 24, symtab_offset as u64);
    write_u64(&mut bytes, symtab_header + 32, (symbol_count * 24) as u64);
    write_u32(&mut bytes, symtab_header + 40, STRTAB_SECTION_INDEX as u32);
    write_u32(&mut bytes, symtab_header + 44, 1);
    write_u64(&mut bytes, symtab_header + 48, 8);
    write_u64(&mut bytes, symtab_header + 56, 24);

    let canonical_descriptor_header =
        section_table_offset + CANONICAL_DESCRIPTOR_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(
        &mut bytes,
        canonical_descriptor_header,
        canonical_descriptor_name,
    );
    write_u32(&mut bytes, canonical_descriptor_header + 4, 1);
    write_u64(
        &mut bytes,
        canonical_descriptor_header + 24,
        canonical_descriptor_offset as u64,
    );
    write_u64(
        &mut bytes,
        canonical_descriptor_header + 32,
        descriptor_table.map_or(0, |table| table.len()) as u64,
    );
    write_u64(&mut bytes, canonical_descriptor_header + 48, 8);

    let shstrtab_header = section_table_offset + SHSTRTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, shstrtab_header, shstrtab_name);
    write_u32(&mut bytes, shstrtab_header + 4, 3);
    write_u64(&mut bytes, shstrtab_header + 24, shstrtab_offset as u64);
    write_u64(&mut bytes, shstrtab_header + 32, shstr.len() as u64);
    write_u64(&mut bytes, shstrtab_header + 48, 1);

    Fixture { bytes, text_offset }
}

fn metadata(options: FixtureOptions<'_>) -> Vec<u8> {
    let mut arguments = vec![
        argument(Some("values_ptr"), 0, 8, "global_buffer", Some("global")),
        argument(Some("values_len"), 8, 8, "by_value", None),
    ];
    arguments.extend(v5_hidden_arguments(16));
    let kernel = Value::Map(vec![
        (Value::from(".name"), Value::from(options.entry)),
        (Value::from(".symbol"), Value::from(options.descriptor)),
        (Value::from(".args"), Value::Array(arguments)),
        (Value::from(".kernarg_segment_size"), Value::from(272)),
        (Value::from(".kernarg_segment_align"), Value::from(8)),
        (Value::from(".group_segment_fixed_size"), Value::from(0)),
        (Value::from(".private_segment_fixed_size"), Value::from(0)),
        (
            Value::from(".wavefront_size"),
            Value::from(options.wavefront_size),
        ),
        (Value::from(".sgpr_count"), Value::from(14)),
        (Value::from(".vgpr_count"), Value::from(11)),
        (Value::from(".agpr_count"), Value::from(3)),
        (Value::from(".sgpr_spill_count"), Value::from(2)),
        (Value::from(".vgpr_spill_count"), Value::from(4)),
        (
            Value::from(".max_flat_workgroup_size"),
            Value::from(options.max_flat_workgroup_size),
        ),
        (
            Value::from(".reqd_workgroup_size"),
            Value::Array(
                options
                    .required_workgroup_size
                    .into_iter()
                    .map(Value::from)
                    .collect(),
            ),
        ),
    ]);
    let root = Value::Map(vec![
        (
            Value::from("amdhsa.version"),
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        (
            Value::from("amdhsa.target"),
            Value::from(format!("amdgcn-amd-amdhsa--{}", options.target)),
        ),
        (Value::from("amdhsa.kernels"), Value::Array(vec![kernel])),
    ]);
    let mut encoded = Vec::new();
    write_value(&mut encoded, &root).unwrap();
    encoded
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

fn metadata_note(metadata: &[u8]) -> Vec<u8> {
    let owner = b"AMDGPU\0";
    let mut note = Vec::new();
    note.extend_from_slice(&(owner.len() as u32).to_le_bytes());
    note.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    note.extend_from_slice(&32_u32.to_le_bytes());
    note.extend_from_slice(owner);
    align(&mut note, 4);
    note.extend_from_slice(metadata);
    align(&mut note, 4);
    note
}

fn target_flags(target: &str) -> u32 {
    match target {
        "gfx942" => 0x54c,
        "gfx942:xnack-" => 0x64c,
        "gfx950" => 0x54f,
        _ => panic!("unsupported test target"),
    }
}

fn push_name(strings: &mut Vec<u8>, name: &str) -> u32 {
    let offset = strings.len() as u32;
    strings.extend_from_slice(name.as_bytes());
    strings.push(0);
    offset
}

fn align(bytes: &mut Vec<u8>, alignment: usize) {
    let remainder = bytes.len() % alignment;
    if remainder != 0 {
        bytes.resize(bytes.len() + alignment - remainder, 0);
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
