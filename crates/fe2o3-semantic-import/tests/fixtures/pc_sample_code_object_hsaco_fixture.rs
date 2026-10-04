use rmpv::{Value, encode::write_value};

const ELF_HEADER_BYTES: usize = 64;
const PROGRAM_HEADER_BYTES: usize = 56;
const SECTION_HEADER_BYTES: usize = 64;
const DESCRIPTOR_OFFSET: usize = 0x1000;
const ENTRY_OFFSET: usize = 0x2000;
const SECOND_ENTRY_OFFSET: u64 = 0x100;
const TEXT_MEMORY_SIZE: usize = SECOND_ENTRY_OFFSET as usize + 64;
const LOAD_VIRTUAL_BASE: u64 = 0x4000;
const TEXT_VIRTUAL_BASE: u64 = 0x9000;

#[derive(Clone, Copy, Debug)]
pub(crate) struct FixtureKernelV1 {
    pub(crate) descriptor_address: u64,
    pub(crate) entry_address: u64,
    pub(crate) entry_size: u64,
    pub(crate) kernarg_size: u32,
    pub(crate) kernarg_alignment: u32,
    pub(crate) group_segment_size: u32,
    pub(crate) private_segment_size: u32,
}

#[derive(Debug)]
pub(crate) struct ExactHsacoFixtureV1 {
    pub(crate) bytes: Vec<u8>,
    pub(crate) virtual_base: u64,
    pub(crate) memory_size: u64,
    pub(crate) kernels: [FixtureKernelV1; 2],
}

pub(crate) fn exact_sparse_two_kernel_hsaco_v1() -> ExactHsacoFixtureV1 {
    exact_sparse_two_kernel_hsaco_with_wavefront_v1(64)
}

pub(crate) fn exact_sparse_two_kernel_hsaco_with_wavefront_v1(
    wavefront_size: u32,
) -> ExactHsacoFixtureV1 {
    const PROGRAM_COUNT: usize = 2;
    const SECTION_COUNT: usize = 7;
    const KERNEL_COUNT: usize = 2;
    assert!(matches!(wavefront_size, 32 | 64));

    let kernels = [
        FixtureKernelV1 {
            descriptor_address: LOAD_VIRTUAL_BASE + DESCRIPTOR_OFFSET as u64,
            entry_address: TEXT_VIRTUAL_BASE,
            entry_size: 64,
            kernarg_size: 272,
            kernarg_alignment: 8,
            group_segment_size: 0,
            private_segment_size: 16,
        },
        FixtureKernelV1 {
            descriptor_address: LOAD_VIRTUAL_BASE + DESCRIPTOR_OFFSET as u64 + 64,
            entry_address: TEXT_VIRTUAL_BASE + SECOND_ENTRY_OFFSET,
            entry_size: 64,
            kernarg_size: 272,
            kernarg_alignment: 8,
            group_segment_size: 256,
            private_segment_size: 32,
        },
    ];
    let metadata = metadata_note(&encode(&metadata_document(&kernels, wavefront_size)));
    let first_program_header = ELF_HEADER_BYTES;
    let second_program_header = first_program_header + PROGRAM_HEADER_BYTES;
    let mut bytes = vec![0; ELF_HEADER_BYTES + PROGRAM_COUNT * PROGRAM_HEADER_BYTES];
    align(&mut bytes, 64);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&metadata);
    align(&mut bytes, 64);
    assert!(bytes.len() <= DESCRIPTOR_OFFSET);
    bytes.resize(DESCRIPTOR_OFFSET + KERNEL_COUNT * 64, 0);
    align_to(&mut bytes, ENTRY_OFFSET);
    bytes.resize(ENTRY_OFFSET + TEXT_MEMORY_SIZE, 0xbf);

    let mut strtab = vec![0];
    let mut name_indices = Vec::new();
    for (entry, descriptor) in [("first", "first.kd"), ("second", "second.kd")] {
        let entry_index = u32::try_from(strtab.len()).unwrap();
        strtab.extend_from_slice(entry.as_bytes());
        strtab.push(0);
        let descriptor_index = u32::try_from(strtab.len()).unwrap();
        strtab.extend_from_slice(descriptor.as_bytes());
        strtab.push(0);
        name_indices.push((entry_index, descriptor_index));
    }
    let strtab_offset = bytes.len();
    bytes.extend_from_slice(&strtab);
    align(&mut bytes, 8);
    let symbol_count = 1 + KERNEL_COUNT * 2;
    let symtab_offset = bytes.len();
    bytes.resize(symtab_offset + symbol_count * 24, 0);
    for (index, kernel) in kernels.iter().enumerate() {
        let entry_symbol = symtab_offset + (1 + index * 2) * 24;
        write_u32(&mut bytes, entry_symbol, name_indices[index].0);
        bytes[entry_symbol + 4] = 0x12;
        write_u16(&mut bytes, entry_symbol + 6, 3);
        write_u64(&mut bytes, entry_symbol + 8, kernel.entry_address);
        write_u64(&mut bytes, entry_symbol + 16, kernel.entry_size);

        let descriptor_symbol = entry_symbol + 24;
        write_u32(&mut bytes, descriptor_symbol, name_indices[index].1);
        bytes[descriptor_symbol + 4] = 0x11;
        write_u16(&mut bytes, descriptor_symbol + 6, 2);
        write_u64(&mut bytes, descriptor_symbol + 8, kernel.descriptor_address);
        write_u64(&mut bytes, descriptor_symbol + 16, 64);
    }

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
    write_u16(&mut bytes, 52, ELF_HEADER_BYTES as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(&mut bytes, 56, PROGRAM_COUNT as u16);
    write_u16(&mut bytes, 58, SECTION_HEADER_BYTES as u16);
    write_u16(&mut bytes, 60, SECTION_COUNT as u16);
    write_u16(&mut bytes, 62, 6);

    write_load(
        &mut bytes,
        first_program_header,
        4,
        0,
        LOAD_VIRTUAL_BASE,
        (DESCRIPTOR_OFFSET + KERNEL_COUNT * 64) as u64,
    );
    write_load(
        &mut bytes,
        second_program_header,
        5,
        ENTRY_OFFSET as u64,
        TEXT_VIRTUAL_BASE,
        TEXT_MEMORY_SIZE as u64,
    );

    let note_header = section_offset + SECTION_HEADER_BYTES;
    write_section(
        &mut bytes,
        note_header,
        1,
        7,
        2,
        LOAD_VIRTUAL_BASE + note_offset as u64,
        note_offset as u64,
        metadata.len() as u64,
        0,
        0,
        4,
        0,
    );
    let rodata_header = section_offset + 2 * SECTION_HEADER_BYTES;
    write_section(
        &mut bytes,
        rodata_header,
        7,
        1,
        2,
        kernels[0].descriptor_address,
        DESCRIPTOR_OFFSET as u64,
        (KERNEL_COUNT * 64) as u64,
        0,
        0,
        64,
        0,
    );
    let text_header = section_offset + 3 * SECTION_HEADER_BYTES;
    write_section(
        &mut bytes,
        text_header,
        15,
        1,
        6,
        TEXT_VIRTUAL_BASE,
        ENTRY_OFFSET as u64,
        TEXT_MEMORY_SIZE as u64,
        0,
        0,
        256,
        0,
    );
    let strtab_header = section_offset + 4 * SECTION_HEADER_BYTES;
    write_section(
        &mut bytes,
        strtab_header,
        21,
        3,
        0,
        0,
        strtab_offset as u64,
        strtab.len() as u64,
        0,
        0,
        1,
        0,
    );
    let symtab_header = section_offset + 5 * SECTION_HEADER_BYTES;
    write_section(
        &mut bytes,
        symtab_header,
        29,
        2,
        0,
        0,
        symtab_offset as u64,
        (symbol_count * 24) as u64,
        4,
        1,
        8,
        24,
    );
    let shstrtab_header = section_offset + 6 * SECTION_HEADER_BYTES;
    write_section(
        &mut bytes,
        shstrtab_header,
        37,
        3,
        0,
        0,
        shstrtab_offset as u64,
        shstrtab.len() as u64,
        0,
        0,
        1,
        0,
    );

    for (index, kernel) in kernels.iter().enumerate() {
        let descriptor_offset = DESCRIPTOR_OFFSET + index * 64;
        write_u32(&mut bytes, descriptor_offset, kernel.group_segment_size);
        write_u32(
            &mut bytes,
            descriptor_offset + 4,
            kernel.private_segment_size,
        );
        write_u32(&mut bytes, descriptor_offset + 8, kernel.kernarg_size);
        write_i64(
            &mut bytes,
            descriptor_offset + 16,
            i64::try_from(kernel.entry_address - kernel.descriptor_address).unwrap(),
        );
        write_u32(&mut bytes, descriptor_offset + 44, 0x40);
        let vgpr_blocks = if wavefront_size == 32 { 0 } else { 1 };
        write_u32(
            &mut bytes,
            descriptor_offset + 48,
            0xe0af_0000 | vgpr_blocks,
        );
        write_u32(&mut bytes, descriptor_offset + 52, 0x1391);
        let wave32_property: u16 = if wavefront_size == 32 { 1 << 10 } else { 0 };
        write_u16(&mut bytes, descriptor_offset + 56, 0x001e | wave32_property);
    }

    let memory_size = kernels[1].entry_address + kernels[1].entry_size - LOAD_VIRTUAL_BASE;
    ExactHsacoFixtureV1 {
        bytes,
        virtual_base: LOAD_VIRTUAL_BASE,
        memory_size,
        kernels,
    }
}

pub(crate) fn official_rocprof_source_v1(fixture: &ExactHsacoFixtureV1) -> Vec<u8> {
    const SOURCE: &[u8] = include_bytes!("rocprofv3-1.1-stochastic-pc-sampling.json");
    let mut source: serde_json::Value = serde_json::from_slice(SOURCE).unwrap();
    let process = &mut source["rocprofiler-sdk-tool"][0];
    let deltas = [-0x2000_i64, 0x10_0000_i64];
    process["code_objects"] = serde_json::Value::Array(
        [2_u64, 3]
            .into_iter()
            .zip(deltas)
            .map(|(code_object_id, load_delta)| {
                serde_json::json!({
                    "code_object_id": code_object_id,
                    "agent_id": {"handle": 18217},
                    "uri": format!("file:///capture/{code_object_id}.hsaco"),
                    "load_base": add_signed(fixture.virtual_base, load_delta),
                    "load_size": fixture.memory_size,
                    "load_delta": load_delta,
                    "storage_type": 1,
                    "memory_base": 0,
                    "memory_size": 0
                })
            })
            .collect(),
    );
    let mut symbols = Vec::new();
    for (code_object_index, (code_object_id, load_delta)) in
        [2_u64, 3].into_iter().zip(deltas).enumerate()
    {
        for (kernel_index, kernel) in fixture.kernels.iter().enumerate() {
            let name = if kernel_index == 0 { "first" } else { "second" };
            symbols.push(serde_json::json!({
                "size": 80,
                "kernel_id": 100 + code_object_index * 2 + kernel_index,
                "code_object_id": code_object_id,
                "kernel_name": name,
                "kernel_object": add_signed(kernel.descriptor_address, load_delta),
                "kernarg_segment_size": kernel.kernarg_size,
                "kernarg_segment_alignment": kernel.kernarg_alignment,
                "group_segment_size": kernel.group_segment_size,
                "private_segment_size": kernel.private_segment_size,
                "formatted_kernel_name": name,
                "demangled_kernel_name": name,
                "truncated_kernel_name": name
            }));
        }
    }
    process["kernel_symbols"] = serde_json::Value::Array(symbols);
    let first_offset = fixture.kernels[0].entry_address - fixture.virtual_base;
    let second_offset = fixture.kernels[1].entry_address - fixture.virtual_base;
    for (index, offset) in [
        first_offset,
        first_offset + 4,
        second_offset,
        second_offset + 4,
    ]
    .into_iter()
    .enumerate()
    {
        process["buffer_records"]["pc_sample_stochastic"][index]["record"]["pc"]["code_object_offset"] =
            serde_json::json!(offset);
    }
    serde_json::to_vec(&source).unwrap()
}

fn add_signed(value: u64, delta: i64) -> u64 {
    if delta >= 0 {
        value.checked_add(delta as u64).unwrap()
    } else {
        value.checked_sub(delta.unsigned_abs()).unwrap()
    }
}

fn metadata_document(kernels: &[FixtureKernelV1; 2], wavefront_size: u32) -> Value {
    let metadata_kernels = kernels
        .iter()
        .enumerate()
        .map(|(index, kernel)| {
            let (name, symbol) = if index == 0 {
                ("first", "first.kd")
            } else {
                ("second", "second.kd")
            };
            valid_kernel(name, symbol, kernel, wavefront_size)
        })
        .collect();
    map(vec![
        (
            "amdhsa.version",
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        ("amdhsa.target", Value::from("amdgcn-amd-amdhsa--gfx1151")),
        ("amdhsa.kernels", Value::Array(metadata_kernels)),
    ])
}

fn valid_kernel(name: &str, symbol: &str, kernel: &FixtureKernelV1, wavefront_size: u32) -> Value {
    let mut arguments = vec![
        argument(Some("a_ptr"), 0, 8, "global_buffer", Some("global")),
        argument(Some("a_len"), 8, 8, "by_value", None),
    ];
    arguments.extend(hidden_arguments(16));
    map(vec![
        (".name", Value::from(name)),
        (".symbol", Value::from(symbol)),
        (".args", Value::Array(arguments)),
        (".kernarg_segment_size", Value::from(kernel.kernarg_size)),
        (
            ".kernarg_segment_align",
            Value::from(kernel.kernarg_alignment),
        ),
        (
            ".group_segment_fixed_size",
            Value::from(kernel.group_segment_size),
        ),
        (
            ".private_segment_fixed_size",
            Value::from(kernel.private_segment_size),
        ),
        (".wavefront_size", Value::from(wavefront_size)),
        (".sgpr_count", Value::from(14)),
        (".vgpr_count", Value::from(7)),
        (".agpr_count", Value::from(3)),
        (".sgpr_spill_count", Value::from(2)),
        (".vgpr_spill_count", Value::from(4)),
        (".workgroup_processor_mode", Value::from(1)),
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

fn write_load(
    bytes: &mut [u8],
    offset: usize,
    flags: u32,
    file_offset: u64,
    virtual_address: u64,
    size: u64,
) {
    write_u32(bytes, offset, 1);
    write_u32(bytes, offset + 4, flags);
    write_u64(bytes, offset + 8, file_offset);
    write_u64(bytes, offset + 16, virtual_address);
    write_u64(bytes, offset + 24, virtual_address);
    write_u64(bytes, offset + 32, size);
    write_u64(bytes, offset + 40, size);
    write_u64(bytes, offset + 48, 0x1000);
}

#[allow(clippy::too_many_arguments)]
fn write_section(
    bytes: &mut [u8],
    offset: usize,
    name: u32,
    section_type: u32,
    flags: u64,
    address: u64,
    file_offset: u64,
    size: u64,
    link: u32,
    info: u32,
    alignment: u64,
    entry_size: u64,
) {
    write_u32(bytes, offset, name);
    write_u32(bytes, offset + 4, section_type);
    write_u64(bytes, offset + 8, flags);
    write_u64(bytes, offset + 16, address);
    write_u64(bytes, offset + 24, file_offset);
    write_u64(bytes, offset + 32, size);
    write_u32(bytes, offset + 40, link);
    write_u32(bytes, offset + 44, info);
    write_u64(bytes, offset + 48, alignment);
    write_u64(bytes, offset + 56, entry_size);
}

fn align(bytes: &mut Vec<u8>, alignment: usize) {
    while !bytes.len().is_multiple_of(alignment) {
        bytes.push(0);
    }
}

fn align_to(bytes: &mut Vec<u8>, offset: usize) {
    assert!(bytes.len() <= offset);
    bytes.resize(offset, 0);
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
