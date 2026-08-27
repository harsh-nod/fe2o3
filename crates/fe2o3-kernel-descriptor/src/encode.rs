use crate::ValidationError;
use crate::model::{
    AccessMode, AliasSemantics, BlockSizeV1, CapabilityV1, CodeObjectVersion, DescriptorKind,
    DeviceDescriptorTableV1, DeviceLayoutDescriptorV1, KernelDescriptorV1, LogicalArgumentV1,
    MAX_DESCRIPTOR_TABLE_BYTES, OwnershipSemantics, PhysicalAbiComponentKind, ScalarTypeV1,
    SourceTypeDescriptorV1,
};

pub const DEVICE_DESCRIPTOR_MAGIC: [u8; 8] = *b"FE2O3KD\0";
pub const DEVICE_DESCRIPTOR_VERSION: u16 = 1;
pub const CANONICAL_CODE_OBJECT_DIGEST_OFFSET: usize = 16;
const HEADER_BYTES: usize = 48;

pub fn encode_device_descriptor_table_v1(
    table: &DeviceDescriptorTableV1,
) -> Result<Vec<u8>, ValidationError> {
    let mut writer = Writer::new();
    writer.bytes(&DEVICE_DESCRIPTOR_MAGIC);
    writer.u16(DEVICE_DESCRIPTOR_VERSION);
    writer.u16(0);
    writer.u32(0);
    debug_assert_eq!(writer.bytes.len(), CANONICAL_CODE_OBJECT_DIGEST_OFFSET);
    writer.bytes(table.canonical_code_object_digest.as_bytes());
    debug_assert_eq!(writer.bytes.len(), HEADER_BYTES);

    writer.u8(code_object_version_tag(table.code_object_version));
    writer.u8(8);
    writer.u8(1);
    writer.u8(0);
    writer.text(table.compiler.name.as_str());
    writer.text(table.compiler.release.as_str());
    writer.bytes(&table.compiler.commit);
    writer.text(table.producer.name.as_str());
    writer.text(table.producer.version.as_str());
    writer.text(&table.device_target.to_string());
    writer.u16(table.type_records.len() as u16);
    writer.u16(table.layout_records.len() as u16);
    writer.u16(table.kernels.len() as u16);
    writer.u16(0);

    for record in &table.type_records {
        writer.bytes(record.identity.as_bytes());
        writer.bytes(&encode_source_type_descriptor(&record.descriptor));
    }
    for record in &table.layout_records {
        writer.bytes(record.identity.as_bytes());
        writer.bytes(&encode_device_layout_descriptor(&record.descriptor));
    }
    for kernel in &table.kernels {
        encode_kernel(&mut writer, kernel);
    }

    if writer.bytes.len() > MAX_DESCRIPTOR_TABLE_BYTES {
        return Err(ValidationError::EncodedTableTooLarge {
            max: MAX_DESCRIPTOR_TABLE_BYTES,
        });
    }
    let total_len = u32::try_from(writer.bytes.len()).map_err(|_| ValidationError::Overflow {
        field: "descriptor table length",
    })?;
    writer.bytes[12..16].copy_from_slice(&total_len.to_le_bytes());
    Ok(writer.bytes)
}

pub(crate) fn validate_encoded_size(
    table: &DeviceDescriptorTableV1,
) -> Result<(), ValidationError> {
    encode_device_descriptor_table_v1(table).map(|_| ())
}

pub(crate) fn encode_source_type_descriptor(descriptor: &SourceTypeDescriptorV1) -> Vec<u8> {
    vec![
        descriptor_kind_tag(descriptor.kind),
        scalar_tag(descriptor.element),
        0,
        0,
    ]
}

pub(crate) fn encode_device_layout_descriptor(descriptor: &DeviceLayoutDescriptorV1) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.u8(descriptor_kind_tag(descriptor.kind));
    writer.u8(scalar_tag(descriptor.element));
    writer.u16(descriptor.size);
    writer.u16(descriptor.alignment);
    writer.u8(descriptor.pointer_width);
    writer.u8(descriptor.length_width);
    writer.u16(0);
    writer.u16(0);
    writer.bytes
}

pub(crate) fn encode_kernel_descriptor(kernel: &KernelDescriptorV1) -> Vec<u8> {
    let mut writer = Writer::new();
    encode_kernel(&mut writer, kernel);
    writer.bytes
}

fn encode_kernel(writer: &mut Writer, kernel: &KernelDescriptorV1) {
    writer.bytes(kernel.kernel_id.as_bytes());
    writer.text(kernel.logical_name.as_str());
    writer.text(kernel.entry_name.as_str());
    writer.text(kernel.descriptor_symbol.as_str());
    encode_evidence(writer, 1, kernel.source_evidence);
    encode_evidence(writer, 2, kernel.executable_ir_evidence);

    writer.u16(kernel.capabilities.len() as u16);
    for capability in &kernel.capabilities {
        writer.u16(capability_tag(*capability));
    }

    writer.u8(kernel.launch.rank);
    let (block_tag, dimensions) = match kernel.launch.block_size {
        BlockSizeV1::Any => (0, [0, 0, 0]),
        BlockSizeV1::Exact(value) => (1, [value.x(), value.y(), value.z()]),
        BlockSizeV1::AtMost(value) => (2, [value.x(), value.y(), value.z()]),
    };
    writer.u8(block_tag);
    writer.u16(0);
    for dimension in dimensions {
        writer.u32(dimension);
    }
    writer.u32(kernel.launch.max_grid.x());
    writer.u32(kernel.launch.max_grid.y());
    writer.u32(kernel.launch.max_grid.z());
    writer.u32(kernel.launch.max_flat_workgroup_size);
    writer.u32(kernel.launch.static_shared_memory_bytes);
    writer.u32(kernel.launch.max_dynamic_shared_memory_bytes);

    writer.u16(kernel.arguments.len() as u16);
    let component_count: usize = kernel
        .arguments
        .iter()
        .map(|argument| argument.components.len())
        .sum();
    writer.u16(component_count as u16);
    writer.u32(kernel.abi_layout.explicit_argument_size);
    writer.u32(kernel.abi_layout.kernarg_segment_size);
    writer.u32(kernel.abi_layout.kernarg_segment_alignment);
    for argument in &kernel.arguments {
        encode_argument(writer, argument);
    }
}

fn encode_evidence(writer: &mut Writer, evidence_tag: u8, evidence: crate::BuildEvidenceV1) {
    writer.u8(evidence_tag);
    writer.u8(1);
    writer.u8(1);
    writer.u8(0);
    writer.bytes(evidence.identity.as_bytes());
    writer.bytes(evidence.digest.as_bytes());
}

fn encode_argument(writer: &mut Writer, argument: &LogicalArgumentV1) {
    writer.u16(argument.source_index);
    writer.u16(0);
    writer.text(argument.name.as_str());
    writer.bytes(argument.source_type.as_bytes());
    writer.bytes(argument.device_layout.as_bytes());
    writer.u8(ownership_tag(argument.ownership));
    writer.u8(access_tag(argument.access));
    writer.u8(alias_tag(argument.alias));
    writer.u8(0);
    writer.u16(argument.components.len() as u16);
    writer.u16(0);
    for component in &argument.components {
        let (kind, scalar) = match component.kind {
            PhysicalAbiComponentKind::ScalarByValue(value) => (1, scalar_tag(value)),
            PhysicalAbiComponentKind::GlobalPointer => (2, 0),
            PhysicalAbiComponentKind::SliceLengthU64 => (3, scalar_tag(ScalarTypeV1::U64)),
        };
        writer.u8(kind);
        writer.u8(scalar);
        writer.u8(access_tag(component.access));
        writer.u8(alias_tag(component.alias));
        writer.u32(component.offset);
        writer.u16(component.size);
        writer.u16(component.alignment);
        writer.u16(0);
        writer.u16(0);
    }
}

pub(crate) const fn descriptor_kind_tag(value: DescriptorKind) -> u8 {
    match value {
        DescriptorKind::Scalar => 1,
        DescriptorKind::SharedSlice => 2,
        DescriptorKind::DisjointSlice => 3,
        DescriptorKind::GlobalMutPointer => 4,
    }
}

pub(crate) const fn scalar_tag(value: ScalarTypeV1) -> u8 {
    match value {
        ScalarTypeV1::I8 => 1,
        ScalarTypeV1::U8 => 2,
        ScalarTypeV1::I16 => 3,
        ScalarTypeV1::U16 => 4,
        ScalarTypeV1::I32 => 5,
        ScalarTypeV1::U32 => 6,
        ScalarTypeV1::I64 => 7,
        ScalarTypeV1::U64 => 8,
        ScalarTypeV1::F16 => 9,
        ScalarTypeV1::F32 => 10,
        ScalarTypeV1::F64 => 11,
    }
}

pub(crate) const fn code_object_version_tag(value: CodeObjectVersion) -> u8 {
    value.number()
}

pub(crate) const fn ownership_tag(value: OwnershipSemantics) -> u8 {
    match value {
        OwnershipSemantics::ByValue => 1,
        OwnershipSemantics::SharedBorrow => 2,
        OwnershipSemantics::UniqueBorrow => 3,
    }
}

pub(crate) const fn access_tag(value: AccessMode) -> u8 {
    match value {
        AccessMode::ByValue => 1,
        AccessMode::ReadOnly => 2,
        AccessMode::WriteOnly => 3,
        AccessMode::ReadWrite => 4,
    }
}

pub(crate) const fn alias_tag(value: AliasSemantics) -> u8 {
    match value {
        AliasSemantics::Value => 1,
        AliasSemantics::SharedReadOnly => 2,
        AliasSemantics::Exclusive => 3,
    }
}

pub(crate) const fn capability_tag(value: CapabilityV1) -> u16 {
    match value {
        CapabilityV1::Subgroup => 1,
        CapabilityV1::Ballot => 2,
        CapabilityV1::Shuffle => 3,
        CapabilityV1::WorkgroupMemory => 4,
        CapabilityV1::MatrixMultiply => 5,
        CapabilityV1::AsyncCopy => 6,
        CapabilityV1::Atomics => 7,
        CapabilityV1::AmdWave => 8,
        CapabilityV1::AmdMfma => 9,
        CapabilityV1::AmdWmma => 10,
        CapabilityV1::AmdDsPermute => 11,
    }
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn text(&mut self, value: &str) {
        self.u16(value.len() as u16);
        self.bytes(value.as_bytes());
    }
}
