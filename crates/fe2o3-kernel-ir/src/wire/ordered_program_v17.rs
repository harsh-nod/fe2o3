//! Exact 179-byte program payload; parent owns operation tag and V17 grammar.

use super::{KERNEL_IR_VERSION_V17, KernelIrDecodeError, KernelIrEncodeError, Reader, Writer};
use crate::{
    AssemblySourceIdentity, Gfx942OrderedProgramRegistersV1, Gfx942OrderedProgramV1,
    Gfx942U32ProgramV1, ValueId,
};

#[cfg(test)]
#[path = "ordered_program_v17_tests.rs"]
mod tests;

// Conservative fixed precharge for source/binding checks and repeated bounded
// validation of all sixteen descriptors, including inactive padding. Wire byte
// reads/writes and containing operation storage are charged separately.
const PAYLOAD_VALIDATION_WORK: usize = 2048;

pub(super) fn encode(
    writer: &mut Writer<'_>,
    program: &Gfx942OrderedProgramV1,
) -> Result<(), KernelIrEncodeError> {
    if writer.version != KERNEL_IR_VERSION_V17 {
        return Err(KernelIrEncodeError::UnsupportedInVersion {
            version: writer.version,
            feature: "gfx942 ordered program",
        });
    }
    writer.charge_work(PAYLOAD_VALIDATION_WORK)?;
    program
        .validate_shape()
        .map_err(|_| KernelIrEncodeError::NonCanonical {
            field: "gfx942 ordered program payload",
        })?;
    writer.u8(0)?;
    let source = program.source();
    writer.bytes(&source.frontend_unit)?;
    writer.bytes(&source.function)?;
    writer.bytes(&source.contract)?;
    writer.bytes(&source.statement)?;
    let registers = program.registers();
    writer.u8(registers.scratch())?;
    writer.u8(registers.output())?;
    for register in registers.inputs() {
        writer.u8(register)?;
    }
    for input in program.inputs() {
        writer.u32(input.0)?;
    }
    writer.u8(program.program().count())?;
    for descriptor in program.program().descriptors() {
        writer.bytes(&descriptor.to_le_bytes())?;
    }
    Ok(())
}

pub(super) fn decode(
    reader: &mut Reader<'_, '_>,
) -> Result<Gfx942OrderedProgramV1, KernelIrDecodeError> {
    if reader.version != KERNEL_IR_VERSION_V17 {
        return Err(KernelIrDecodeError::UnknownVersion(reader.version));
    }
    let revision = reader.u8()?;
    if revision != 0 {
        return Err(KernelIrDecodeError::UnknownTag {
            kind: "gfx942 ordered program revision",
            tag: revision,
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
    let values = [
        ValueId(reader.u32()?),
        ValueId(reader.u32()?),
        ValueId(reader.u32()?),
    ];
    let count = reader.u8()?;
    let mut descriptors = [0_u16; 16];
    for descriptor in &mut descriptors {
        *descriptor = u16::from_le_bytes(reader.fixed()?);
    }
    reader.charge_work(PAYLOAD_VALIDATION_WORK)?;
    let registers = Gfx942OrderedProgramRegistersV1::new(scratch, output, inputs)
        .map_err(|_| KernelIrDecodeError::NonCanonical)?;
    let instructions = Gfx942U32ProgramV1::from_descriptors(count, descriptors)
        .map_err(|_| KernelIrDecodeError::NonCanonical)?;
    Gfx942OrderedProgramV1::new(source, registers, values, instructions)
        .map_err(|_| KernelIrDecodeError::NonCanonical)
}
