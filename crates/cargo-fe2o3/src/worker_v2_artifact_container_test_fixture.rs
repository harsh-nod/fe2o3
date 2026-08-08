use fe2o3_hsaco_finalize::{
    DEVICE_DESCRIPTOR_SECTION_ALIGNMENT, DEVICE_DESCRIPTOR_SECTION_NAME, finalize_unfinalized,
};
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
const SHSTRTAB_SECTION_INDEX: usize = 7;

#[derive(Clone, Copy, Debug)]
pub(super) enum ProfileMutation {
    None,
    MissingCapability,
    WriteOnlyAccess,
    SharedOwnership,
    SharedAlias,
}

pub(super) struct AlphaZetaFixture {
    pub(super) bytes: Vec<u8>,
    pub(super) is_finalized: bool,
}

pub(super) fn alpha_zeta_fixture(mutation: ProfileMutation) -> AlphaZetaFixture {
    let access = if matches!(mutation, ProfileMutation::WriteOnlyAccess) {
        AccessMode::WriteOnly
    } else {
        AccessMode::ReadWrite
    };
    let capabilities = if matches!(mutation, ProfileMutation::MissingCapability) {
        vec![]
    } else {
        vec![CapabilityV1::AmdWave]
    };
    let mut table = encode_device_descriptor_table_v1(&descriptor_table(access, capabilities))
        .expect("encode alpha/zeta descriptor table");
    match mutation {
        ProfileMutation::SharedOwnership => mutate_output_semantic(&mut table, 0, 2),
        ProfileMutation::SharedAlias => mutate_output_semantic(&mut table, 2, 2),
        _ => {}
    }
    let bytes = build_hsaco(&table, access, false);
    if matches!(
        mutation,
        ProfileMutation::SharedOwnership | ProfileMutation::SharedAlias
    ) {
        AlphaZetaFixture {
            bytes,
            is_finalized: false,
        }
    } else {
        AlphaZetaFixture {
            bytes: finalize_unfinalized(&bytes)
                .expect("finalize alpha/zeta fixture")
                .into_bytes(),
            is_finalized: true,
        }
    }
}

#[allow(dead_code)] // Used only by the standalone production-handoff fixture target.
pub(super) fn canonical_alpha_zeta_unfinalized_fixture() -> Vec<u8> {
    let table = encode_device_descriptor_table_v1(&descriptor_table(
        AccessMode::ReadWrite,
        vec![CapabilityV1::AmdWave],
    ))
    .expect("encode canonical alpha/zeta descriptor table");
    build_hsaco(&table, AccessMode::ReadWrite, true)
}

fn descriptor_table(
    disjoint_access: AccessMode,
    capabilities: Vec<CapabilityV1>,
) -> DeviceDescriptorTableV1 {
    let scalar_source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
    let shared_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let disjoint_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let scalar_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
    let shared_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let disjoint_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));

    let alpha = KernelDescriptorV1::new(
        KernelId::from_bytes([0x61; 32]),
        name("alpha"),
        name("alpha"),
        name("alpha.kd"),
        evidence(0x11, 0x12),
        evidence(0x13, 0x14),
        capabilities.clone(),
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
                disjoint_access,
                24,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let zeta = KernelDescriptorV1::new(
        KernelId::from_bytes([0x7a; 32]),
        name("zeta"),
        name("zeta"),
        name("zeta.kd"),
        evidence(0x21, 0x22),
        evidence(0x23, 0x24),
        capabilities,
        KernelAbiLayoutV1::new(56, 312, 8).unwrap(),
        launch(),
        vec![
            LogicalArgumentV1::shared_slice(0, name("a"), &shared_source, &shared_layout, 0)
                .unwrap(),
            LogicalArgumentV1::shared_slice(1, name("b"), &shared_source, &shared_layout, 16)
                .unwrap(),
            LogicalArgumentV1::scalar(2, name("bias"), &scalar_source, &scalar_layout, 32).unwrap(),
            LogicalArgumentV1::disjoint_slice(
                3,
                name("output"),
                &disjoint_source,
                &disjoint_layout,
                disjoint_access,
                40,
            )
            .unwrap(),
        ],
    )
    .unwrap();

    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(text("rustc-codegen-fe2o3"), text("test"), [0x31; 20]),
        ProducerIdentityV1::new(text("rustc-codegen-fe2o3"), text("test")),
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
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

fn name(value: &str) -> ValidName {
    ValidName::new(value).unwrap()
}

fn text(value: &str) -> Text {
    Text::new(value).unwrap()
}

fn mutate_output_semantic(table: &mut [u8], semantic_offset: usize, replacement: u8) {
    let marker = b"\x06\0output";
    let start = table
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("alpha output argument marker");
    let semantics = start + marker.len() + 64;
    table[semantics + semantic_offset] = replacement;
}

fn build_hsaco(table: &[u8], disjoint_access: AccessMode, include_ffi_export: bool) -> Vec<u8> {
    const KERNELS: [(&str, &str, u32); 2] = [("alpha", "alpha.kd", 296), ("zeta", "zeta.kd", 312)];
    let note = metadata_note(&metadata(disjoint_access));
    let mut bytes = vec![0; ELF_HEADER_BYTES + 2 * PROGRAM_HEADER_BYTES];

    align(&mut bytes, 64);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);

    align(&mut bytes, 64);
    let rodata_offset = bytes.len();
    let mut descriptor_offsets = Vec::new();
    for _ in KERNELS {
        align(&mut bytes, 64);
        descriptor_offsets.push(bytes.len());
        bytes.resize(bytes.len() + 64, 0);
    }
    let rodata_end = bytes.len();

    let mut entry_offsets = Vec::new();
    for _ in KERNELS {
        align(&mut bytes, 256);
        entry_offsets.push(bytes.len());
        bytes.resize(bytes.len() + 64, 0xbf);
    }
    let text_offset = entry_offsets[0];
    let ffi_export_offset = include_ffi_export.then(|| {
        align(&mut bytes, 256);
        let offset = bytes.len();
        bytes.resize(bytes.len() + 64, 0xbe);
        offset
    });
    let text_end = bytes.len();

    let mut strtab = vec![0];
    let symbol_names = KERNELS
        .iter()
        .map(|(entry, descriptor, _)| {
            (
                push_name(&mut strtab, entry),
                push_name(&mut strtab, descriptor),
            )
        })
        .collect::<Vec<_>>();
    let ffi_export_name = include_ffi_export.then(|| push_name(&mut strtab, "ffi_export"));
    let strtab_offset = bytes.len();
    bytes.extend_from_slice(&strtab);
    align(&mut bytes, 8);
    let symtab_offset = bytes.len();
    let symbol_count = 1 + KERNELS.len() * 2 + usize::from(include_ffi_export);
    bytes.resize(symtab_offset + symbol_count * 24, 0);

    for (index, ((entry_name, descriptor_name), descriptor_offset)) in
        symbol_names.iter().zip(&descriptor_offsets).enumerate()
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

        write_u32(&mut bytes, *descriptor_offset + 8, KERNELS[index].2);
        write_i64(
            &mut bytes,
            *descriptor_offset + 16,
            i64::try_from(entry_address - *descriptor_offset as u64).unwrap(),
        );
        write_u32(&mut bytes, *descriptor_offset + 44, 1);
        write_u32(&mut bytes, *descriptor_offset + 48, 0x00af_0081);
        write_u32(&mut bytes, *descriptor_offset + 52, 0x1390);
        write_u16(&mut bytes, *descriptor_offset + 56, 0x001e);
    }
    if let (Some(name), Some(offset)) = (ffi_export_name, ffi_export_offset) {
        let export_symbol = symtab_offset + (1 + KERNELS.len() * 2) * 24;
        write_u32(&mut bytes, export_symbol, name);
        bytes[export_symbol + 4] = 0x12;
        bytes[export_symbol + 5] = 3;
        write_u16(&mut bytes, export_symbol + 6, TEXT_SECTION_INDEX as u16);
        write_u64(&mut bytes, export_symbol + 8, (offset + 0x1000) as u64);
        write_u64(&mut bytes, export_symbol + 16, 64);
    }

    align(&mut bytes, DEVICE_DESCRIPTOR_SECTION_ALIGNMENT as usize);
    let table_offset = bytes.len();
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
    bytes.resize(section_table_offset + 8 * SECTION_HEADER_BYTES, 0);

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = 4;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 32, ELF_HEADER_BYTES as u64);
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u32(&mut bytes, 48, 0x64c);
    write_u16(&mut bytes, 52, ELF_HEADER_BYTES as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(&mut bytes, 56, 2);
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

    section_header(
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
    section_header(
        &mut bytes,
        section_table_offset,
        RODATA_SECTION_INDEX,
        rodata_name,
        1,
        2,
        rodata_offset,
        rodata_offset,
        rodata_end - rodata_offset,
        64,
    );
    section_header(
        &mut bytes,
        section_table_offset,
        TEXT_SECTION_INDEX,
        text_name,
        1,
        6,
        text_offset + 0x1000,
        text_offset,
        text_end - text_offset,
        256,
    );
    section_header(
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
    section_header(
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
    section_header(
        &mut bytes,
        section_table_offset,
        DESCRIPTOR_SECTION_INDEX,
        descriptor_name,
        1,
        0,
        0,
        table_offset,
        table.len(),
        DEVICE_DESCRIPTOR_SECTION_ALIGNMENT as usize,
    );
    section_header(
        &mut bytes,
        section_table_offset,
        SHSTRTAB_SECTION_INDEX,
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

fn metadata(disjoint_access: AccessMode) -> Vec<u8> {
    let access = match disjoint_access {
        AccessMode::WriteOnly => "write_only",
        AccessMode::ReadWrite => "read_write",
        _ => unreachable!("fixture uses a writable disjoint access mode"),
    };
    let kernels = vec![
        metadata_kernel(
            "alpha",
            "alpha.kd",
            296,
            vec![
                scalar_argument(0, 4),
                pointer_argument(8, "read_only"),
                scalar_argument(16, 8),
                pointer_argument(24, access),
                scalar_argument(32, 8),
            ],
            40,
        ),
        metadata_kernel(
            "zeta",
            "zeta.kd",
            312,
            vec![
                pointer_argument(0, "read_only"),
                scalar_argument(8, 8),
                pointer_argument(16, "read_only"),
                scalar_argument(24, 8),
                scalar_argument(32, 4),
                pointer_argument(40, access),
                scalar_argument(48, 8),
            ],
            56,
        ),
    ];
    let root = Value::Map(vec![
        (
            Value::from("amdhsa.version"),
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        (
            Value::from("amdhsa.target"),
            Value::from("amdgcn-amd-amdhsa--gfx942:xnack-"),
        ),
        (Value::from("amdhsa.kernels"), Value::Array(kernels)),
    ]);
    let mut encoded = Vec::new();
    write_value(&mut encoded, &root).unwrap();
    encoded
}

fn metadata_kernel(
    entry: &str,
    symbol: &str,
    kernarg_size: u32,
    mut arguments: Vec<Value>,
    implicit_base: u64,
) -> Value {
    arguments.extend(hidden_arguments(implicit_base));
    Value::Map(vec![
        (Value::from(".name"), Value::from(entry)),
        (Value::from(".symbol"), Value::from(symbol)),
        (Value::from(".args"), Value::Array(arguments)),
        (
            Value::from(".kernarg_segment_size"),
            Value::from(kernarg_size),
        ),
        (Value::from(".kernarg_segment_align"), Value::from(8)),
        (Value::from(".group_segment_fixed_size"), Value::from(0)),
        (Value::from(".private_segment_fixed_size"), Value::from(0)),
        (Value::from(".wavefront_size"), Value::from(64)),
        (Value::from(".sgpr_count"), Value::from(14)),
        (Value::from(".vgpr_count"), Value::from(11)),
        (Value::from(".agpr_count"), Value::from(3)),
        (Value::from(".max_flat_workgroup_size"), Value::from(256)),
        (
            Value::from(".reqd_workgroup_size"),
            Value::Array(vec![Value::from(256), Value::from(1), Value::from(1)]),
        ),
    ])
}

fn pointer_argument(offset: u64, access: &str) -> Value {
    Value::Map(vec![
        (Value::from(".offset"), Value::from(offset)),
        (Value::from(".size"), Value::from(8)),
        (Value::from(".align"), Value::from(8)),
        (Value::from(".value_kind"), Value::from("global_buffer")),
        (Value::from(".address_space"), Value::from("global")),
        (Value::from(".access"), Value::from(access)),
        (Value::from(".actual_access"), Value::from(access)),
        (Value::from(".pointee_align"), Value::from(4)),
    ])
}

fn scalar_argument(offset: u64, size: u64) -> Value {
    Value::Map(vec![
        (Value::from(".offset"), Value::from(offset)),
        (Value::from(".size"), Value::from(size)),
        (Value::from(".align"), Value::from(size)),
        (Value::from(".value_kind"), Value::from("by_value")),
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
    .map(|(offset, size, kind)| {
        Value::Map(vec![
            (Value::from(".offset"), Value::from(base + offset)),
            (Value::from(".size"), Value::from(size)),
            (Value::from(".value_kind"), Value::from(kind)),
        ])
    })
    .collect()
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

#[allow(clippy::too_many_arguments)]
fn section_header(
    bytes: &mut [u8],
    table: usize,
    index: usize,
    name: u32,
    kind: u32,
    flags: u64,
    address: usize,
    offset: usize,
    size: usize,
    alignment: usize,
) {
    let header = table + index * SECTION_HEADER_BYTES;
    write_u32(bytes, header, name);
    write_u32(bytes, header + 4, kind);
    write_u64(bytes, header + 8, flags);
    write_u64(bytes, header + 16, address as u64);
    write_u64(bytes, header + 24, offset as u64);
    write_u64(bytes, header + 32, size as u64);
    write_u64(bytes, header + 48, alignment as u64);
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
