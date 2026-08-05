use std::str;

use crate::encode::{DEVICE_DESCRIPTOR_MAGIC, DEVICE_DESCRIPTOR_VERSION};
use crate::model::{
    AccessMode, AliasSemantics, BlockSizeV1, BuildEvidenceV1, CapabilityV1, CodeObjectVersion,
    CompilerIdentityV1, DescriptorKind, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
    DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
    KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1,
    MAX_ARGUMENTS_PER_KERNEL, MAX_CAPABILITIES, MAX_DESCRIPTOR_TABLE_BYTES, MAX_KERNELS,
    MAX_LAYOUT_RECORDS, MAX_NAME_BYTES, MAX_PHYSICAL_COMPONENTS_PER_KERNEL, MAX_TEXT_BYTES,
    MAX_TYPE_RECORDS, OwnershipSemantics, PhysicalAbiComponentKind, PhysicalAbiComponentV1,
    ProducerIdentityV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName,
};
use crate::{CanonicalCodeObjectDigest, DecodeError, DeviceLayoutIdentity, RustTypeIdentity};

pub fn decode_device_descriptor_table_v1(
    bytes: &[u8],
) -> Result<DeviceDescriptorTableV1, DecodeError> {
    if bytes.len() > MAX_DESCRIPTOR_TABLE_BYTES {
        return Err(DecodeError::TooLarge {
            max: MAX_DESCRIPTOR_TABLE_BYTES,
        });
    }

    let mut reader = Reader::new(bytes);
    if reader.fixed::<8>()? != DEVICE_DESCRIPTOR_MAGIC {
        return Err(DecodeError::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != DEVICE_DESCRIPTOR_VERSION {
        return Err(DecodeError::UnknownVersion(version));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(DecodeError::UnsupportedFlags(flags));
    }
    let declared_len = reader.u32()? as usize;
    if declared_len > bytes.len() {
        return Err(DecodeError::Truncated);
    }
    if declared_len < bytes.len() {
        return Err(DecodeError::TrailingBytes);
    }

    let canonical_code_object_digest = CanonicalCodeObjectDigest::from_bytes(reader.fixed::<32>()?);
    let code_object_version = parse_code_object_version(reader.u8()?)?;
    require_tag(reader.u8()?, 8, "pointer width")?;
    require_tag(reader.u8()?, 1, "endianness")?;
    reader.reserved_u8("table header")?;

    let compiler = CompilerIdentityV1::new(
        reader.text("compiler name")?,
        reader.text("compiler release")?,
        reader.fixed::<20>()?,
    );
    let producer = ProducerIdentityV1::new(
        reader.text("producer name")?,
        reader.text("producer version")?,
    );
    let device_target_text = reader.text("device target")?;
    let device_target = DeviceTargetV1::parse(device_target_text.as_str())?;

    let type_count = reader.count("type records", MAX_TYPE_RECORDS)?;
    let layout_count = reader.count("layout records", MAX_LAYOUT_RECORDS)?;
    let kernel_count = reader.count("kernels", MAX_KERNELS)?;
    reader.reserved_u16("table counts")?;

    let mut type_records = Vec::with_capacity(type_count);
    for _ in 0..type_count {
        type_records.push(parse_type_record(&mut reader)?);
    }
    let mut layout_records = Vec::with_capacity(layout_count);
    for _ in 0..layout_count {
        layout_records.push(parse_layout_record(&mut reader)?);
    }
    let mut kernels = Vec::with_capacity(kernel_count);
    for _ in 0..kernel_count {
        kernels.push(parse_kernel(&mut reader)?);
    }
    if !reader.is_empty() {
        return Err(DecodeError::TrailingBytes);
    }

    let table = DeviceDescriptorTableV1::from_wire(
        canonical_code_object_digest,
        code_object_version,
        compiler,
        producer,
        device_target,
        type_records,
        layout_records,
        kernels,
    )?;
    let canonical = crate::encode::encode_device_descriptor_table_v1(&table)?;
    if canonical != bytes {
        return Err(DecodeError::NonCanonical);
    }
    Ok(table)
}

fn parse_type_record(reader: &mut Reader<'_>) -> Result<SourceTypeRecordV1, DecodeError> {
    let identity = RustTypeIdentity::from_bytes(reader.fixed::<32>()?);
    let kind = parse_descriptor_kind(reader.u8()?)?;
    let element = parse_scalar(reader.u8()?)?;
    reader.reserved_u16("source type descriptor")?;
    let descriptor = match kind {
        DescriptorKind::Scalar => SourceTypeDescriptorV1::scalar(element),
        DescriptorKind::SharedSlice => SourceTypeDescriptorV1::shared_slice(element),
        DescriptorKind::DisjointSlice => SourceTypeDescriptorV1::disjoint_slice(element),
    };
    Ok(SourceTypeRecordV1::from_wire(identity, descriptor)?)
}

fn parse_layout_record(reader: &mut Reader<'_>) -> Result<DeviceLayoutRecordV1, DecodeError> {
    let identity = DeviceLayoutIdentity::from_bytes(reader.fixed::<32>()?);
    let kind = parse_descriptor_kind(reader.u8()?)?;
    let element = parse_scalar(reader.u8()?)?;
    let size = reader.u16()?;
    let alignment = reader.u16()?;
    let pointer_width = reader.u8()?;
    let length_width = reader.u8()?;
    reader.reserved_u16("device layout flags")?;
    reader.reserved_u16("device layout descriptor")?;
    let descriptor = DeviceLayoutDescriptorV1 {
        kind,
        element,
        size,
        alignment,
        pointer_width,
        length_width,
    };
    let expected = match kind {
        DescriptorKind::Scalar => DeviceLayoutDescriptorV1::scalar(element),
        DescriptorKind::SharedSlice => DeviceLayoutDescriptorV1::shared_slice(element),
        DescriptorKind::DisjointSlice => DeviceLayoutDescriptorV1::disjoint_slice(element),
    };
    if descriptor != expected {
        return Err(crate::ValidationError::InvalidArgument(
            "device layout is not the canonical V1 lowering",
        )
        .into());
    }
    Ok(DeviceLayoutRecordV1::from_wire(identity, descriptor)?)
}

fn parse_kernel(reader: &mut Reader<'_>) -> Result<KernelDescriptorV1, DecodeError> {
    let kernel_id = KernelId::from_bytes(reader.fixed::<32>()?);
    let logical_name = reader.name("kernel logical name")?;
    let entry_name = reader.name("kernel entry name")?;
    let descriptor_symbol = reader.name("kernel descriptor symbol")?;
    let source_evidence = parse_evidence(reader, 1, "source evidence")?;
    let executable_ir_evidence = parse_evidence(reader, 2, "executable-IR evidence")?;

    let capability_count = reader.count("kernel capabilities", MAX_CAPABILITIES)?;
    let mut capabilities = Vec::with_capacity(capability_count);
    for _ in 0..capability_count {
        capabilities.push(parse_capability(reader.u16()?)?);
    }

    let rank = reader.u8()?;
    let block_tag = reader.u8()?;
    reader.reserved_u16("launch constraints")?;
    let block_dimensions = [reader.u32()?, reader.u32()?, reader.u32()?];
    let block_size = match block_tag {
        0 => {
            if block_dimensions != [0, 0, 0] {
                return Err(DecodeError::NonzeroReserved {
                    field: "unconstrained block dimensions",
                });
            }
            BlockSizeV1::Any
        }
        1 => BlockSizeV1::Exact(DimensionsV1::new(
            block_dimensions[0],
            block_dimensions[1],
            block_dimensions[2],
        )?),
        2 => BlockSizeV1::AtMost(DimensionsV1::new(
            block_dimensions[0],
            block_dimensions[1],
            block_dimensions[2],
        )?),
        tag => {
            return Err(DecodeError::UnknownTag {
                kind: "block size",
                tag: u16::from(tag),
            });
        }
    };
    let max_grid = DimensionsV1::new(reader.u32()?, reader.u32()?, reader.u32()?)?;
    let launch = LaunchConstraintsV1::new(
        rank,
        block_size,
        max_grid,
        reader.u32()?,
        reader.u32()?,
        reader.u32()?,
    )?;

    let argument_count = reader.count("kernel arguments", MAX_ARGUMENTS_PER_KERNEL)?;
    let declared_component_count = reader.count(
        "physical ABI components",
        MAX_PHYSICAL_COMPONENTS_PER_KERNEL,
    )?;
    let abi_layout = KernelAbiLayoutV1::new(reader.u32()?, reader.u32()?, reader.u32()?)?;
    let mut arguments = Vec::with_capacity(argument_count);
    let mut actual_component_count = 0usize;
    for _ in 0..argument_count {
        let argument = parse_argument(reader)?;
        actual_component_count = actual_component_count
            .checked_add(argument.components.len())
            .ok_or(crate::ValidationError::Overflow {
                field: "physical component count",
            })?;
        if actual_component_count > MAX_PHYSICAL_COMPONENTS_PER_KERNEL {
            return Err(DecodeError::CountOutOfRange {
                field: "physical ABI components",
                count: actual_component_count as u64,
                max: MAX_PHYSICAL_COMPONENTS_PER_KERNEL,
            });
        }
        arguments.push(argument);
    }
    if actual_component_count != declared_component_count {
        return Err(crate::ValidationError::InvalidPhysicalAbi(
            "declared component count does not match arguments",
        )
        .into());
    }

    Ok(KernelDescriptorV1::new(
        kernel_id,
        logical_name,
        entry_name,
        descriptor_symbol,
        source_evidence,
        executable_ir_evidence,
        capabilities,
        abi_layout,
        launch,
        arguments,
    )?)
}

fn parse_evidence(
    reader: &mut Reader<'_>,
    expected_tag: u8,
    field: &'static str,
) -> Result<BuildEvidenceV1, DecodeError> {
    require_tag(reader.u8()?, expected_tag, field)?;
    require_tag(reader.u8()?, 1, "evidence identity scheme")?;
    require_tag(reader.u8()?, 1, "evidence digest algorithm")?;
    reader.reserved_u8("build evidence")?;
    Ok(BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(reader.fixed::<32>()?),
        EvidenceDigest::from_sha256_bytes(reader.fixed::<32>()?),
    ))
}

fn parse_argument(reader: &mut Reader<'_>) -> Result<LogicalArgumentV1, DecodeError> {
    let source_index = reader.u16()?;
    reader.reserved_u16("logical argument flags")?;
    let name = reader.name("argument name")?;
    let source_type = RustTypeIdentity::from_bytes(reader.fixed::<32>()?);
    let device_layout = DeviceLayoutIdentity::from_bytes(reader.fixed::<32>()?);
    let ownership = parse_ownership(reader.u8()?)?;
    let access = parse_access(reader.u8()?)?;
    let alias = parse_alias(reader.u8()?)?;
    reader.reserved_u8("logical argument")?;
    let component_count = reader.count(
        "argument physical components",
        MAX_PHYSICAL_COMPONENTS_PER_KERNEL,
    )?;
    reader.reserved_u16("logical argument components")?;
    let mut components = Vec::with_capacity(component_count);
    for _ in 0..component_count {
        components.push(parse_component(reader)?);
    }
    let value = LogicalArgumentV1 {
        source_index,
        name,
        source_type,
        device_layout,
        ownership,
        access,
        alias,
        components,
    };
    value.validate_local()?;
    Ok(value)
}

fn parse_component(reader: &mut Reader<'_>) -> Result<PhysicalAbiComponentV1, DecodeError> {
    let kind_tag = reader.u8()?;
    let scalar_tag = reader.u8()?;
    let kind = match kind_tag {
        1 => PhysicalAbiComponentKind::ScalarByValue(parse_scalar(scalar_tag)?),
        2 => {
            if scalar_tag != 0 {
                return Err(DecodeError::NonzeroReserved {
                    field: "global pointer scalar tag",
                });
            }
            PhysicalAbiComponentKind::GlobalPointer
        }
        3 => {
            if parse_scalar(scalar_tag)? != ScalarTypeV1::U64 {
                return Err(crate::ValidationError::InvalidPhysicalAbi(
                    "slice length must be tagged u64",
                )
                .into());
            }
            PhysicalAbiComponentKind::SliceLengthU64
        }
        tag => {
            return Err(DecodeError::UnknownTag {
                kind: "physical ABI component",
                tag: u16::from(tag),
            });
        }
    };
    let access = parse_access(reader.u8()?)?;
    let alias = parse_alias(reader.u8()?)?;
    let offset = reader.u32()?;
    let size = reader.u16()?;
    let alignment = reader.u16()?;
    reader.reserved_u16("physical ABI component flags")?;
    reader.reserved_u16("physical ABI component")?;
    Ok(PhysicalAbiComponentV1 {
        kind,
        offset,
        size,
        alignment,
        access,
        alias,
    })
}

fn parse_descriptor_kind(tag: u8) -> Result<DescriptorKind, DecodeError> {
    match tag {
        1 => Ok(DescriptorKind::Scalar),
        2 => Ok(DescriptorKind::SharedSlice),
        3 => Ok(DescriptorKind::DisjointSlice),
        _ => Err(DecodeError::UnknownTag {
            kind: "type descriptor",
            tag: u16::from(tag),
        }),
    }
}

fn parse_scalar(tag: u8) -> Result<ScalarTypeV1, DecodeError> {
    match tag {
        1 => Ok(ScalarTypeV1::I8),
        2 => Ok(ScalarTypeV1::U8),
        3 => Ok(ScalarTypeV1::I16),
        4 => Ok(ScalarTypeV1::U16),
        5 => Ok(ScalarTypeV1::I32),
        6 => Ok(ScalarTypeV1::U32),
        7 => Ok(ScalarTypeV1::I64),
        8 => Ok(ScalarTypeV1::U64),
        9 => Ok(ScalarTypeV1::F16),
        10 => Ok(ScalarTypeV1::F32),
        11 => Ok(ScalarTypeV1::F64),
        _ => Err(DecodeError::UnknownTag {
            kind: "scalar type",
            tag: u16::from(tag),
        }),
    }
}

fn parse_code_object_version(tag: u8) -> Result<CodeObjectVersion, DecodeError> {
    match tag {
        4 => Ok(CodeObjectVersion::V4),
        5 => Ok(CodeObjectVersion::V5),
        6 => Ok(CodeObjectVersion::V6),
        _ => Err(DecodeError::UnknownTag {
            kind: "code object version",
            tag: u16::from(tag),
        }),
    }
}

fn parse_ownership(tag: u8) -> Result<OwnershipSemantics, DecodeError> {
    match tag {
        1 => Ok(OwnershipSemantics::ByValue),
        2 => Ok(OwnershipSemantics::SharedBorrow),
        3 => Ok(OwnershipSemantics::UniqueBorrow),
        _ => Err(DecodeError::UnknownTag {
            kind: "ownership",
            tag: u16::from(tag),
        }),
    }
}

fn parse_access(tag: u8) -> Result<AccessMode, DecodeError> {
    match tag {
        1 => Ok(AccessMode::ByValue),
        2 => Ok(AccessMode::ReadOnly),
        3 => Ok(AccessMode::WriteOnly),
        4 => Ok(AccessMode::ReadWrite),
        _ => Err(DecodeError::UnknownTag {
            kind: "access mode",
            tag: u16::from(tag),
        }),
    }
}

fn parse_alias(tag: u8) -> Result<AliasSemantics, DecodeError> {
    match tag {
        1 => Ok(AliasSemantics::Value),
        2 => Ok(AliasSemantics::SharedReadOnly),
        3 => Ok(AliasSemantics::Exclusive),
        _ => Err(DecodeError::UnknownTag {
            kind: "alias semantics",
            tag: u16::from(tag),
        }),
    }
}

fn parse_capability(tag: u16) -> Result<CapabilityV1, DecodeError> {
    match tag {
        1 => Ok(CapabilityV1::Subgroup),
        2 => Ok(CapabilityV1::Ballot),
        3 => Ok(CapabilityV1::Shuffle),
        4 => Ok(CapabilityV1::WorkgroupMemory),
        5 => Ok(CapabilityV1::MatrixMultiply),
        6 => Ok(CapabilityV1::AsyncCopy),
        7 => Ok(CapabilityV1::Atomics),
        8 => Ok(CapabilityV1::AmdWave),
        9 => Ok(CapabilityV1::AmdMfma),
        10 => Ok(CapabilityV1::AmdWmma),
        11 => Ok(CapabilityV1::AmdDsPermute),
        _ => Err(DecodeError::UnknownTag {
            kind: "capability",
            tag,
        }),
    }
}

fn require_tag(actual: u8, expected: u8, kind: &'static str) -> Result<(), DecodeError> {
    if actual == expected {
        Ok(())
    } else {
        Err(DecodeError::UnknownTag {
            kind,
            tag: u16::from(actual),
        })
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(DecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(DecodeError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        self.take(N)?.try_into().map_err(|_| DecodeError::Truncated)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn count(&mut self, field: &'static str, max: usize) -> Result<usize, DecodeError> {
        let count = usize::from(self.u16()?);
        if count > max {
            Err(DecodeError::CountOutOfRange {
                field,
                count: count as u64,
                max,
            })
        } else {
            Ok(count)
        }
    }

    fn text(&mut self, field: &'static str) -> Result<Text, DecodeError> {
        let value = self.string_bytes(field, MAX_TEXT_BYTES)?;
        Text::new(value).map_err(Into::into)
    }

    fn name(&mut self, field: &'static str) -> Result<ValidName, DecodeError> {
        let value = self.string_bytes(field, MAX_NAME_BYTES)?;
        ValidName::new(value).map_err(Into::into)
    }

    fn string_bytes(&mut self, field: &'static str, max: usize) -> Result<&'a str, DecodeError> {
        let length = usize::from(self.u16()?);
        if length > max {
            return Err(DecodeError::CountOutOfRange {
                field,
                count: length as u64,
                max,
            });
        }
        let bytes = self.take(length)?;
        if !bytes.iter().all(u8::is_ascii) {
            return Err(DecodeError::InvalidText { field });
        }
        str::from_utf8(bytes).map_err(|_| DecodeError::InvalidText { field })
    }

    fn reserved_u8(&mut self, field: &'static str) -> Result<(), DecodeError> {
        if self.u8()? == 0 {
            Ok(())
        } else {
            Err(DecodeError::NonzeroReserved { field })
        }
    }

    fn reserved_u16(&mut self, field: &'static str) -> Result<(), DecodeError> {
        if self.u16()? == 0 {
            Ok(())
        } else {
            Err(DecodeError::NonzeroReserved { field })
        }
    }

    fn is_empty(&self) -> bool {
        self.position == self.bytes.len()
    }
}
