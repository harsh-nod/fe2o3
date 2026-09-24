//! Exact V19 payloads: declaration 360 bytes, step 15 bytes, no heap allocation.
//! Parent owns tags 40/41; neither codec nor payload establishes source authority.
use super::{KERNEL_IR_VERSION_V19, KernelIrDecodeError, KernelIrEncodeError, Reader, Writer};
use crate::{
    Gfx942CompleteBodyDeclarationVNext as Declaration, Gfx942CompleteBodyOriginVNext as Origin,
    Gfx942CompleteBodyStepVNext as Step, Gfx942OrderedProgramRegistersV1,
    Gfx942ProgramInstructionV1, ValueId,
};

const DECLARATION_WORK: usize = 1024;
const STEP_WORK: usize = 64;

fn encode_version(writer: &Writer<'_>) -> Result<(), KernelIrEncodeError> {
    if writer.version != KERNEL_IR_VERSION_V19 {
        return Err(KernelIrEncodeError::UnsupportedInVersion {
            version: writer.version,
            feature: "gfx942 complete body",
        });
    }
    Ok(())
}
fn decode_version(reader: &Reader<'_, '_>) -> Result<(), KernelIrDecodeError> {
    if reader.version != KERNEL_IR_VERSION_V19 {
        return Err(KernelIrDecodeError::UnknownVersion(reader.version));
    }
    Ok(())
}
fn revision(reader: &mut Reader<'_, '_>) -> Result<(), KernelIrDecodeError> {
    let tag = reader.u8()?;
    if tag != 0 {
        return Err(KernelIrDecodeError::UnknownTag {
            kind: "gfx942 complete body revision",
            tag,
        });
    }
    Ok(())
}

pub(super) fn encode_declaration(
    writer: &mut Writer<'_>,
    declaration: &Declaration,
) -> Result<(), KernelIrEncodeError> {
    encode_version(writer)?;
    writer.charge_work(DECLARATION_WORK)?;
    declaration
        .validate_shape()
        .map_err(|_| KernelIrEncodeError::NonCanonical {
            field: "gfx942 complete body declaration",
        })?;
    writer.u8(0)?;
    for digest in declaration.origin.root_axes {
        writer.bytes(&digest)?;
    }
    for digest in [
        declaration.origin.mir_body,
        declaration.origin.semantic_block,
        declaration.origin.source_signature,
        declaration.origin.rustc_fn_abi,
        declaration.origin.frontend_bytes_sha256,
    ] {
        writer.bytes(&digest)?;
    }
    writer.u32(declaration.origin.raw_block)?;
    writer.u8(declaration.registers.scratch())?;
    writer.u8(declaration.registers.output())?;
    for register in declaration.registers.inputs() {
        writer.u8(register)?;
    }
    for parameter in declaration.parameters {
        writer.u32(parameter.0)?;
    }
    writer.bytes(&declaration.labels)?;
    writer.u8(declaration.block_count)?;
    writer.u8(declaration.instruction_count)?;
    Ok(())
}

pub(super) fn decode_declaration(
    reader: &mut Reader<'_, '_>,
) -> Result<Declaration, KernelIrDecodeError> {
    decode_version(reader)?;
    reader.charge_work(DECLARATION_WORK)?;
    revision(reader)?;
    let origin = Origin {
        root_axes: [
            reader.fixed()?,
            reader.fixed()?,
            reader.fixed()?,
            reader.fixed()?,
            reader.fixed()?,
        ],
        mir_body: reader.fixed()?,
        semantic_block: reader.fixed()?,
        source_signature: reader.fixed()?,
        rustc_fn_abi: reader.fixed()?,
        frontend_bytes_sha256: reader.fixed()?,
        raw_block: reader.u32()?,
    };
    let scratch = reader.u8()?;
    let output = reader.u8()?;
    let registers = Gfx942OrderedProgramRegistersV1::new(
        scratch,
        output,
        [reader.u8()?, reader.u8()?, reader.u8()?],
    )
    .map_err(|_| KernelIrDecodeError::NonCanonical)?;
    let declaration = Declaration {
        origin,
        registers,
        parameters: [
            ValueId(reader.u32()?),
            ValueId(reader.u32()?),
            ValueId(reader.u32()?),
            ValueId(reader.u32()?),
            ValueId(reader.u32()?),
        ],
        labels: reader.fixed()?,
        block_count: reader.u8()?,
        instruction_count: reader.u8()?,
    };
    declaration
        .validate_shape()
        .map_err(|_| KernelIrDecodeError::NonCanonical)?;
    Ok(declaration)
}

pub(super) fn encode_step(writer: &mut Writer<'_>, step: &Step) -> Result<(), KernelIrEncodeError> {
    encode_version(writer)?;
    writer.charge_work(STEP_WORK)?;
    step.validate_shape()
        .map_err(|_| KernelIrEncodeError::NonCanonical {
            field: "gfx942 complete body step",
        })?;
    writer.u8(0)?;
    writer.u8(step.authored_block)?;
    writer.u8(step.authored_instruction)?;
    writer.u16(step.instruction.descriptor())?;
    for operand in step.operands {
        writer.u8(u8::from(operand.is_some()))?;
        writer.u32(operand.map_or(0, |value| value.0))?;
    }
    Ok(())
}

pub(super) fn decode_step(reader: &mut Reader<'_, '_>) -> Result<Step, KernelIrDecodeError> {
    decode_version(reader)?;
    reader.charge_work(STEP_WORK)?;
    revision(reader)?;
    let authored_block = reader.u8()?;
    let authored_instruction = reader.u8()?;
    let instruction = Gfx942ProgramInstructionV1::from_descriptor(reader.u16()?)
        .map_err(|_| KernelIrDecodeError::NonCanonical)?;
    let mut operands = [None; 2];
    for operand in &mut operands {
        let tag = reader.u8()?;
        let value = reader.u32()?;
        *operand = match (tag, value) {
            (0, 0) => None,
            (1, value) => Some(ValueId(value)),
            (0, _) => return Err(KernelIrDecodeError::NonCanonical),
            (tag, _) => {
                return Err(KernelIrDecodeError::UnknownTag {
                    kind: "gfx942 complete body operand presence",
                    tag,
                });
            }
        };
    }
    let step = Step {
        authored_block,
        authored_instruction,
        instruction,
        operands,
    };
    step.validate_shape()
        .map_err(|_| KernelIrDecodeError::NonCanonical)?;
    Ok(step)
}

#[cfg(test)]
#[path = "complete_body_v19_tests.rs"]
mod tests;
