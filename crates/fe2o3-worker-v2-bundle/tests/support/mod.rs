use fe2o3_hsaco_finalize::{DEVICE_DESCRIPTOR_SECTION_ALIGNMENT, DEVICE_DESCRIPTOR_SECTION_NAME};
use fe2o3_kernel_descriptor::{
    AccessMode, BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CapabilityV1,
    CodeObjectVersion, CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
    DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
    KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1,
    ProducerIdentityV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName,
    encode_device_descriptor_table_v1,
};
use rmpv::{Value, encode::write_value};

const ELF_HEADER_BYTES: usize = 64;
const PROGRAM_HEADER_BYTES: usize = 56;
const SECTION_HEADER_BYTES: usize = 64;
const NOTE_SECTION_INDEX: usize = 1;
const RODATA_SECTION_INDEX: usize = 2;
const TEXT_SECTION_INDEX: usize = 3;
const STRTAB_SECTION_INDEX: usize = 4;
const SYMTAB_SECTION_INDEX: usize = 5;
const DESCRIPTOR_SECTION_INDEX: usize = 6;

#[derive(Clone, Debug)]
pub struct HsacoFixture {
    pub bytes: Vec<u8>,
    pub descriptor_source: Vec<u8>,
}

pub fn alpha_zeta_hsaco(
    version: CodeObjectVersion,
    target: &str,
    evidence_seed: u8,
    code_fill: u8,
) -> HsacoFixture {
    let table = descriptor_table(version, target, evidence_seed);
    let descriptor_source = encode_device_descriptor_table_v1(&table).unwrap();
    let metadata = metadata(target);
    let mut bytes = build_elf(&descriptor_source, &metadata, code_fill);
    bytes[8] = match version {
        CodeObjectVersion::V4 => 2,
        CodeObjectVersion::V5 => 3,
        CodeObjectVersion::V6 => 4,
    };
    write_u32(
        &mut bytes,
        48,
        match target {
            "gfx942" => 0x54c,
            "gfx942:xnack-" => 0x64c,
            "gfx942:xnack+" => 0x74c,
            _ => panic!("unsupported fixture target"),
        },
    );
    HsacoFixture {
        bytes,
        descriptor_source,
    }
}

fn descriptor_table(
    version: CodeObjectVersion,
    target: &str,
    evidence_seed: u8,
) -> DeviceDescriptorTableV1 {
    let scalar_source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
    let scalar_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
    let shared_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let shared_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let disjoint_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let disjoint_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));

    let alpha = KernelDescriptorV1::new(
        KernelId::from_bytes([0xa1; 32]),
        name("alpha"),
        name("alpha"),
        name("alpha.kd"),
        evidence(evidence_seed, evidence_seed.wrapping_add(1)),
        evidence(evidence_seed.wrapping_add(2), evidence_seed.wrapping_add(3)),
        vec![CapabilityV1::AmdWave],
        KernelAbiLayoutV1::new(40, 296, 8).unwrap(),
        launch(),
        vec![
            LogicalArgumentV1::scalar(0, name("scale"), &scalar_source, &scalar_layout, 0).unwrap(),
            LogicalArgumentV1::shared_slice(1, name("input"), &shared_source, &shared_layout, 8)
                .unwrap(),
            LogicalArgumentV1::disjoint_slice(
                2,
                name("output"),
                &disjoint_source,
                &disjoint_layout,
                AccessMode::ReadWrite,
                24,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let zeta = KernelDescriptorV1::new(
        KernelId::from_bytes([0xb2; 32]),
        name("zeta"),
        name("zeta"),
        name("zeta.kd"),
        evidence(evidence_seed.wrapping_add(4), evidence_seed.wrapping_add(5)),
        evidence(evidence_seed.wrapping_add(6), evidence_seed.wrapping_add(7)),
        vec![CapabilityV1::AmdWave],
        KernelAbiLayoutV1::new(56, 312, 8).unwrap(),
        launch(),
        vec![
            LogicalArgumentV1::shared_slice(0, name("left"), &shared_source, &shared_layout, 0)
                .unwrap(),
            LogicalArgumentV1::shared_slice(1, name("right"), &shared_source, &shared_layout, 16)
                .unwrap(),
            LogicalArgumentV1::scalar(2, name("bias"), &scalar_source, &scalar_layout, 32).unwrap(),
            LogicalArgumentV1::disjoint_slice(
                3,
                name("output"),
                &disjoint_source,
                &disjoint_layout,
                AccessMode::ReadWrite,
                40,
            )
            .unwrap(),
        ],
    )
    .unwrap();

    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        version,
        CompilerIdentityV1::new(text("rustc-codegen-fe2o3"), text("test"), [0x21; 20]),
        ProducerIdentityV1::new(
            text("rustc-codegen-fe2o3-worker-v2"),
            text("typed-general-gfx942-cov6-v1"),
        ),
        DeviceTargetV1::parse(target).unwrap(),
        vec![scalar_source, shared_source, disjoint_source],
        vec![scalar_layout, shared_layout, disjoint_layout],
        vec![alpha, zeta],
    )
    .unwrap()
}

fn launch() -> LaunchConstraintsV1 {
    LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
        DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
        256,
        0,
        0,
    )
    .unwrap()
}

fn evidence(identity: u8, digest: u8) -> BuildEvidenceV1 {
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([identity; 32]),
        EvidenceDigest::from_sha256_bytes([digest; 32]),
    )
}

fn metadata(target: &str) -> Vec<u8> {
    let root = Value::Map(vec![
        (
            Value::from("amdhsa.version"),
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        (
            Value::from("amdhsa.target"),
            Value::from(format!("amdgcn-amd-amdhsa--{target}")),
        ),
        (
            Value::from("amdhsa.kernels"),
            Value::Array(vec![metadata_alpha(), metadata_zeta()]),
        ),
    ]);
    let mut encoded = Vec::new();
    write_value(&mut encoded, &root).unwrap();
    encoded
}

fn metadata_alpha() -> Value {
    metadata_kernel(
        "alpha",
        "alpha.kd",
        40,
        vec![
            argument("scale", 0, 4, "by_value", None),
            argument("input_ptr", 8, 8, "global_buffer", Some("global")),
            argument("input_len", 16, 8, "by_value", None),
            argument("output_ptr", 24, 8, "global_buffer", Some("global")),
            argument("output_len", 32, 8, "by_value", None),
        ],
    )
}

fn metadata_zeta() -> Value {
    metadata_kernel(
        "zeta",
        "zeta.kd",
        56,
        vec![
            argument("left_ptr", 0, 8, "global_buffer", Some("global")),
            argument("left_len", 8, 8, "by_value", None),
            argument("right_ptr", 16, 8, "global_buffer", Some("global")),
            argument("right_len", 24, 8, "by_value", None),
            argument("bias", 32, 4, "by_value", None),
            argument("output_ptr", 40, 8, "global_buffer", Some("global")),
            argument("output_len", 48, 8, "by_value", None),
        ],
    )
}

fn metadata_kernel(entry: &str, symbol: &str, size: u32, arguments: Vec<Value>) -> Value {
    Value::Map(vec![
        (Value::from(".name"), Value::from(entry)),
        (Value::from(".symbol"), Value::from(symbol)),
        (Value::from(".args"), Value::Array(arguments)),
        (Value::from(".kernarg_segment_size"), Value::from(size)),
        (Value::from(".kernarg_segment_align"), Value::from(8)),
        (Value::from(".group_segment_fixed_size"), Value::from(0)),
        (Value::from(".private_segment_fixed_size"), Value::from(0)),
        (Value::from(".wavefront_size"), Value::from(64)),
        (Value::from(".sgpr_count"), Value::from(14)),
        (Value::from(".vgpr_count"), Value::from(11)),
        (Value::from(".agpr_count"), Value::from(3)),
        (Value::from(".sgpr_spill_count"), Value::from(2)),
        (Value::from(".vgpr_spill_count"), Value::from(4)),
        (Value::from(".max_flat_workgroup_size"), Value::from(256)),
        (
            Value::from(".reqd_workgroup_size"),
            Value::Array(vec![Value::from(256), Value::from(1), Value::from(1)]),
        ),
    ])
}

fn argument(
    name: &str,
    offset: u64,
    size: u64,
    value_kind: &str,
    address_space: Option<&str>,
) -> Value {
    let mut fields = vec![
        (Value::from(".name"), Value::from(name)),
        (Value::from(".offset"), Value::from(offset)),
        (Value::from(".size"), Value::from(size)),
        (Value::from(".value_kind"), Value::from(value_kind)),
    ];
    if let Some(address_space) = address_space {
        fields.push((Value::from(".address_space"), Value::from(address_space)));
    }
    Value::Map(fields)
}

fn build_elf(table: &[u8], metadata: &[u8], code_fill: u8) -> Vec<u8> {
    const PROGRAM_COUNT: usize = 3;
    let kernels = [("alpha", "alpha.kd", 40_u32), ("zeta", "zeta.kd", 56_u32)];
    let note = metadata_note(metadata);
    let mut bytes = vec![0; ELF_HEADER_BYTES + PROGRAM_COUNT * PROGRAM_HEADER_BYTES];
    align(&mut bytes, 64);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);

    align(&mut bytes, 64);
    let rodata_offset = bytes.len();
    let mut kernel_descriptor_offsets = Vec::new();
    for _ in kernels {
        align(&mut bytes, 64);
        kernel_descriptor_offsets.push(bytes.len());
        bytes.resize(bytes.len() + 64, 0);
    }
    let rodata_end = bytes.len();

    let mut entry_offsets = Vec::new();
    for _ in kernels {
        align(&mut bytes, 256);
        entry_offsets.push(bytes.len());
        bytes.resize(bytes.len() + 64, code_fill);
    }
    let text_offset = entry_offsets[0];
    let text_end = bytes.len();

    let mut strtab = vec![0];
    let names = kernels
        .iter()
        .map(|(entry, descriptor, _)| {
            (
                push_name(&mut strtab, entry),
                push_name(&mut strtab, descriptor),
            )
        })
        .collect::<Vec<_>>();
    let strtab_offset = bytes.len();
    bytes.extend_from_slice(&strtab);
    align(&mut bytes, 8);
    let symtab_offset = bytes.len();
    let symbol_count = 1 + kernels.len() * 2;
    bytes.resize(symtab_offset + symbol_count * 24, 0);
    for (index, ((entry_name, descriptor_name), descriptor_offset)) in
        names.iter().zip(&kernel_descriptor_offsets).enumerate()
    {
        let entry_symbol = symtab_offset + (1 + index * 2) * 24;
        write_u32(&mut bytes, entry_symbol, *entry_name);
        bytes[entry_symbol + 4] = 0x12;
        bytes[entry_symbol + 5] = 3;
        write_u16(&mut bytes, entry_symbol + 6, TEXT_SECTION_INDEX as u16);
        let entry_address = (entry_offsets[index] + 0x1000) as u64;
        write_u64(&mut bytes, entry_symbol + 8, entry_address);
        write_u64(&mut bytes, entry_symbol + 16, 64);

        let descriptor_symbol = symtab_offset + (2 + index * 2) * 24;
        write_u32(&mut bytes, descriptor_symbol, *descriptor_name);
        bytes[descriptor_symbol + 4] = 0x11;
        write_u16(
            &mut bytes,
            descriptor_symbol + 6,
            RODATA_SECTION_INDEX as u16,
        );
        write_u64(&mut bytes, descriptor_symbol + 8, *descriptor_offset as u64);
        write_u64(&mut bytes, descriptor_symbol + 16, 64);

        write_u32(&mut bytes, *descriptor_offset + 8, kernels[index].2);
        write_i64(
            &mut bytes,
            *descriptor_offset + 16,
            i64::try_from(entry_address - *descriptor_offset as u64).unwrap(),
        );
        write_u32(&mut bytes, *descriptor_offset + 44, 1);
        write_u32(&mut bytes, *descriptor_offset + 48, 0x00af_0081);
        write_u16(&mut bytes, *descriptor_offset + 56, 0x001e);
    }

    align(&mut bytes, DEVICE_DESCRIPTOR_SECTION_ALIGNMENT as usize);
    let descriptor_offset = bytes.len();
    bytes.extend_from_slice(table);

    let mut shstr = vec![0];
    let note_name = push_name(&mut shstr, ".note");
    let rodata_name = push_name(&mut shstr, ".rodata");
    let text_name = push_name(&mut shstr, ".text");
    let strtab_name = push_name(&mut shstr, ".strtab");
    let symtab_name = push_name(&mut shstr, ".symtab");
    let descriptor_name = push_name(&mut shstr, DEVICE_DESCRIPTOR_SECTION_NAME);
    let shstr_name = push_name(&mut shstr, ".shstrtab");
    let shstr_offset = bytes.len();
    bytes.extend_from_slice(&shstr);
    align(&mut bytes, 8);
    let section_table_offset = bytes.len();
    let section_count = 8;
    bytes.resize(
        section_table_offset + section_count * SECTION_HEADER_BYTES,
        0,
    );

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 32, ELF_HEADER_BYTES as u64);
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u16(&mut bytes, 52, ELF_HEADER_BYTES as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(&mut bytes, 56, PROGRAM_COUNT as u16);
    write_u16(&mut bytes, 58, SECTION_HEADER_BYTES as u16);
    write_u16(&mut bytes, 60, section_count as u16);
    write_u16(&mut bytes, 62, 7);

    let load_rodata = ELF_HEADER_BYTES;
    write_u32(&mut bytes, load_rodata, 1);
    write_u32(&mut bytes, load_rodata + 4, 4);
    write_u64(&mut bytes, load_rodata + 32, rodata_end as u64);
    write_u64(&mut bytes, load_rodata + 40, rodata_end as u64);
    write_u64(&mut bytes, load_rodata + 48, 0x1000);

    let load_text = load_rodata + PROGRAM_HEADER_BYTES;
    write_u32(&mut bytes, load_text, 1);
    write_u32(&mut bytes, load_text + 4, 5);
    write_u64(&mut bytes, load_text + 8, text_offset as u64);
    write_u64(&mut bytes, load_text + 16, (text_offset + 0x1000) as u64);
    write_u64(&mut bytes, load_text + 32, (text_end - text_offset) as u64);
    write_u64(&mut bytes, load_text + 40, (text_end - text_offset) as u64);
    write_u64(&mut bytes, load_text + 48, 0x1000);

    section(
        &mut bytes,
        section_table_offset,
        NOTE_SECTION_INDEX,
        note_name,
        7,
        2,
        0,
        note_offset,
        note.len(),
        4,
    );
    section(
        &mut bytes,
        section_table_offset,
        RODATA_SECTION_INDEX,
        rodata_name,
        1,
        2,
        rodata_offset as u64,
        rodata_offset,
        rodata_end - rodata_offset,
        64,
    );
    section(
        &mut bytes,
        section_table_offset,
        TEXT_SECTION_INDEX,
        text_name,
        1,
        6,
        (text_offset + 0x1000) as u64,
        text_offset,
        text_end - text_offset,
        256,
    );
    section(
        &mut bytes,
        section_table_offset,
        STRTAB_SECTION_INDEX,
        strtab_name,
        3,
        0,
        0,
        strtab_offset,
        strtab.len(),
        1,
    );
    section(
        &mut bytes,
        section_table_offset,
        SYMTAB_SECTION_INDEX,
        symtab_name,
        2,
        0,
        0,
        symtab_offset,
        symbol_count * 24,
        8,
    );
    let symtab_header = section_table_offset + SYMTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, symtab_header + 40, STRTAB_SECTION_INDEX as u32);
    write_u32(&mut bytes, symtab_header + 44, 1);
    write_u64(&mut bytes, symtab_header + 56, 24);
    section(
        &mut bytes,
        section_table_offset,
        DESCRIPTOR_SECTION_INDEX,
        descriptor_name,
        1,
        0,
        0,
        descriptor_offset,
        table.len(),
        DEVICE_DESCRIPTOR_SECTION_ALIGNMENT,
    );
    section(
        &mut bytes,
        section_table_offset,
        7,
        shstr_name,
        3,
        0,
        0,
        shstr_offset,
        shstr.len(),
        1,
    );
    bytes
}

#[allow(clippy::too_many_arguments)]
fn section(
    bytes: &mut [u8],
    table: usize,
    index: usize,
    name: u32,
    kind: u32,
    flags: u64,
    address: u64,
    offset: usize,
    size: usize,
    alignment: u64,
) {
    let header = table + index * SECTION_HEADER_BYTES;
    write_u32(bytes, header, name);
    write_u32(bytes, header + 4, kind);
    write_u64(bytes, header + 8, flags);
    write_u64(bytes, header + 16, address);
    write_u64(bytes, header + 24, offset as u64);
    write_u64(bytes, header + 32, size as u64);
    write_u64(bytes, header + 48, alignment);
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

fn push_name(strings: &mut Vec<u8>, name: &str) -> u32 {
    let offset = strings.len() as u32;
    strings.extend_from_slice(name.as_bytes());
    strings.push(0);
    offset
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

fn name(value: &str) -> ValidName {
    ValidName::new(value).unwrap()
}

fn text(value: &str) -> Text {
    Text::new(value).unwrap()
}
