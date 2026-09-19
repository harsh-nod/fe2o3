//! Fixed payload codec. The parent owns the operation tag and exact V16 owner.

use super::{KERNEL_IR_VERSION_V16, KernelIrDecodeError, KernelIrEncodeError, Reader, Writer};
use crate::{
    AssemblySourceIdentity, Gfx942OrderedRegionRegistersV1, Gfx942OrderedRegionV1, ValueId,
};

#[cfg(test)]
#[path = "ordered_region_v16_tests.rs"]
mod tests;

// Upper bound for four 32-byte nonzero checks and two five-binding shape checks
// (five range checks plus ten comparisons each). Charge before validation.
const PAYLOAD_VALIDATION_WORK: usize = 160;

pub(super) fn encode(
    writer: &mut Writer<'_>,
    region: &Gfx942OrderedRegionV1,
) -> Result<(), KernelIrEncodeError> {
    if writer.version != KERNEL_IR_VERSION_V16 {
        return Err(KernelIrEncodeError::UnsupportedInVersion {
            version: writer.version,
            feature: "gfx942 ordered region",
        });
    }
    writer.charge_work(PAYLOAD_VALIDATION_WORK)?;
    region
        .validate_shape()
        .map_err(|_| KernelIrEncodeError::NonCanonical {
            field: "gfx942 ordered region payload",
        })?;
    writer.u8(0)?;
    let source = region.source();
    writer.bytes(&source.frontend_unit)?;
    writer.bytes(&source.function)?;
    writer.bytes(&source.contract)?;
    writer.bytes(&source.statement)?;
    let registers = region.registers();
    writer.u8(registers.scratch())?;
    writer.u8(registers.output())?;
    for register in registers.inputs() {
        writer.u8(register)?;
    }
    for input in region.inputs() {
        writer.u32(input.0)?;
    }
    Ok(())
}

pub(super) fn decode(
    reader: &mut Reader<'_, '_>,
) -> Result<Gfx942OrderedRegionV1, KernelIrDecodeError> {
    if reader.version != KERNEL_IR_VERSION_V16 {
        return Err(KernelIrDecodeError::UnknownVersion(reader.version));
    }
    let profile = reader.u8()?;
    if profile != 0 {
        return Err(KernelIrDecodeError::UnknownTag {
            kind: "gfx942 ordered region profile",
            tag: profile,
        });
    }
    let source = AssemblySourceIdentity::new(
        reader.fixed()?,
        reader.fixed()?,
        reader.fixed()?,
        reader.fixed()?,
    );
    let scratch = reader.u8()?;
    let output = reader.u8()?;
    let inputs = [reader.u8()?, reader.u8()?, reader.u8()?];
    reader.charge_work(PAYLOAD_VALIDATION_WORK)?;
    let registers = Gfx942OrderedRegionRegistersV1::new(scratch, output, inputs)
        .map_err(|_| KernelIrDecodeError::NonCanonical)?;
    let values = [
        ValueId(reader.u32()?),
        ValueId(reader.u32()?),
        ValueId(reader.u32()?),
    ];
    Gfx942OrderedRegionV1::new(source, registers, values)
        .map_err(|_| KernelIrDecodeError::NonCanonical)
}
