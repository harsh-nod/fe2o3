use crate::{
    AtomicRequirementsV2, DecodeError, DeviceDescriptorTableV2, KernelId,
    KernelTargetRequirementsV2, LdsRequirementsV2, MAX_DESCRIPTOR_TABLE_BYTES, MAX_KERNELS,
    RequiredWavefrontWidthV2, SynchronizationRequirementsV2, ValidationError,
    decode_device_descriptor_table_v1, encode_device_descriptor_table_v1,
};

use crate::DEVICE_DESCRIPTOR_MAGIC;

pub const DEVICE_DESCRIPTOR_VERSION_V2: u16 = 2;
const HEADER_BYTES_V2: usize = 24;
const REQUIREMENT_BYTES_V2: usize = 48;
/// Offset of the embedded V1 canonical code-object digest in a V2 table.
pub const CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V2: usize =
    HEADER_BYTES_V2 + crate::CANONICAL_CODE_OBJECT_DIGEST_OFFSET;

/// Encodes a V2 table as an unchanged canonical V1 table plus fixed-size
/// target-requirement records.
pub fn encode_device_descriptor_table_v2(
    table: &DeviceDescriptorTableV2,
) -> Result<Vec<u8>, ValidationError> {
    let base = encode_device_descriptor_table_v1(table.base())?;
    let base_len = u32::try_from(base.len()).map_err(|_| ValidationError::Overflow {
        field: "embedded V1 descriptor length",
    })?;
    let mut bytes = Vec::with_capacity(
        HEADER_BYTES_V2
            .checked_add(base.len())
            .and_then(|length| {
                length.checked_add(REQUIREMENT_BYTES_V2 * table.requirements().len())
            })
            .ok_or(ValidationError::Overflow {
                field: "V2 descriptor table length",
            })?,
    );
    bytes.extend_from_slice(&DEVICE_DESCRIPTOR_MAGIC);
    bytes.extend_from_slice(&DEVICE_DESCRIPTOR_VERSION_V2.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&base_len.to_le_bytes());
    bytes.extend_from_slice(&(table.requirements().len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&base);
    for requirement in table.requirements() {
        encode_requirement(&mut bytes, *requirement);
    }
    if bytes.len() > MAX_DESCRIPTOR_TABLE_BYTES {
        return Err(ValidationError::EncodedTableTooLarge {
            max: MAX_DESCRIPTOR_TABLE_BYTES,
        });
    }
    let total_len = u32::try_from(bytes.len()).map_err(|_| ValidationError::Overflow {
        field: "V2 descriptor table length",
    })?;
    bytes[12..16].copy_from_slice(&total_len.to_le_bytes());
    Ok(bytes)
}

pub(crate) fn validate_encoded_size_v2(
    table: &DeviceDescriptorTableV2,
) -> Result<(), ValidationError> {
    encode_device_descriptor_table_v2(table).map(|_| ())
}

/// Decodes V2 without weakening the canonical V1 decoder used for its base.
pub fn decode_device_descriptor_table_v2(
    bytes: &[u8],
) -> Result<DeviceDescriptorTableV2, DecodeError> {
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
    if version != DEVICE_DESCRIPTOR_VERSION_V2 {
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
    let base_len = reader.u32()? as usize;
    let requirement_count = reader.count("kernel target requirements", MAX_KERNELS)?;
    reader.reserved_u16("V2 table header")?;

    let minimum_requirements_bytes = requirement_count
        .checked_mul(REQUIREMENT_BYTES_V2)
        .ok_or(DecodeError::Truncated)?;
    if reader.remaining() < minimum_requirements_bytes
        || base_len != reader.remaining() - minimum_requirements_bytes
    {
        return Err(DecodeError::Truncated);
    }
    let base = decode_device_descriptor_table_v1(reader.take(base_len)?)?;
    let mut requirements = Vec::with_capacity(requirement_count);
    for _ in 0..requirement_count {
        requirements.push(parse_requirement(&mut reader)?);
    }
    if !reader.is_empty() {
        return Err(DecodeError::TrailingBytes);
    }
    let table = DeviceDescriptorTableV2::from_wire(base, requirements)?;
    if encode_device_descriptor_table_v2(&table)? != bytes {
        return Err(DecodeError::NonCanonical);
    }
    Ok(table)
}

fn encode_requirement(bytes: &mut Vec<u8>, requirement: KernelTargetRequirementsV2) {
    bytes.extend_from_slice(requirement.kernel_id().as_bytes());
    bytes.extend_from_slice(&requirement.lds().static_bytes().to_le_bytes());
    bytes.extend_from_slice(&requirement.lds().max_dynamic_bytes().to_le_bytes());
    bytes.push(match requirement.wavefront_width() {
        RequiredWavefrontWidthV2::Wave32 => 1,
        RequiredWavefrontWidthV2::Wave64 => 2,
    });
    bytes.push(u8::from(requirement.cooperative_launch()));
    bytes.extend_from_slice(&requirement.synchronization().bits().to_le_bytes());
    bytes.extend_from_slice(&requirement.atomics().bits().to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
}

fn parse_requirement(reader: &mut Reader<'_>) -> Result<KernelTargetRequirementsV2, DecodeError> {
    let kernel_id = KernelId::from_bytes(reader.fixed::<32>()?);
    let lds = LdsRequirementsV2::new(reader.u32()?, reader.u32()?)?;
    let wavefront_width = match reader.u8()? {
        1 => RequiredWavefrontWidthV2::Wave32,
        2 => RequiredWavefrontWidthV2::Wave64,
        tag => {
            return Err(DecodeError::UnknownTag {
                kind: "required wavefront width",
                tag: u16::from(tag),
            });
        }
    };
    let cooperative_launch = match reader.u8()? {
        0 => false,
        1 => true,
        tag => {
            return Err(DecodeError::UnknownTag {
                kind: "cooperative launch requirement",
                tag: u16::from(tag),
            });
        }
    };
    let synchronization = SynchronizationRequirementsV2::from_bits(reader.u16()?)?;
    let atomics = AtomicRequirementsV2::from_bits(reader.u16()?)?;
    reader.reserved_u16("kernel target requirement")?;
    Ok(KernelTargetRequirementsV2::new(
        kernel_id,
        lds,
        wavefront_width,
        cooperative_launch,
        synchronization,
        atomics,
    ))
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

    fn reserved_u16(&mut self, field: &'static str) -> Result<(), DecodeError> {
        if self.u16()? == 0 {
            Ok(())
        } else {
            Err(DecodeError::NonzeroReserved { field })
        }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }

    fn is_empty(&self) -> bool {
        self.position == self.bytes.len()
    }
}
