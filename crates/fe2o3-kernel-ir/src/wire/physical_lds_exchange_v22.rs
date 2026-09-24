//! Exact allocated KIR22 payloads. No old-version interpretation or source authority.
#[cfg(test)]
#[path = "physical_lds_exchange_v22_tests.rs"]
mod tests;
use super::{KERNEL_IR_VERSION_V22, KernelIrDecodeError, KernelIrEncodeError, Reader, Writer};
use crate::{
    Gfx942PhysicalEntryBlockContractVNext as Block,
    Gfx942PhysicalEntryBranchEncodingVNext as Branch, Gfx942PhysicalEntryOriginVNext as Origin,
    Gfx942PhysicalEntrySourceSiteVNext as Site,
    Gfx942PhysicalLdsExchangeDeclarationV1 as Declaration,
    Gfx942PhysicalLdsExchangeFrameV1 as LdsFrame,
    Gfx942PhysicalLdsExchangeInstructionV1 as Instruction, Gfx942PhysicalLdsExchangeStepV1 as Step,
    ValueId,
};
fn encode_version(w: &Writer<'_>) -> Result<(), KernelIrEncodeError> {
    if w.version != KERNEL_IR_VERSION_V22 {
        return Err(KernelIrEncodeError::UnsupportedInVersion {
            version: w.version,
            feature: "gfx942 physical LDS exchange",
        });
    }
    Ok(())
}
fn revision(r: &mut Reader<'_, '_>) -> Result<(), KernelIrDecodeError> {
    if r.version != KERNEL_IR_VERSION_V22 {
        return Err(KernelIrDecodeError::UnknownVersion(r.version));
    }
    let tag = r.u8()?;
    if tag != 0 {
        return Err(KernelIrDecodeError::UnknownTag {
            kind: "gfx942 physical LDS exchange revision",
            tag,
        });
    }
    Ok(())
}
fn encode_site(w: &mut Writer<'_>, site: Site) -> Result<(), KernelIrEncodeError> {
    w.u8(site.occurrence)?;
    w.u32(site.raw_block)?;
    w.u32(site.semantic_block_index)?;
    w.bytes(&site.semantic_block_identity)?;
    w.u32(site.semantic_callable_index)
}
fn decode_site(r: &mut Reader<'_, '_>) -> Result<Site, KernelIrDecodeError> {
    Ok(Site {
        occurrence: r.u8()?,
        raw_block: r.u32()?,
        semantic_block_index: r.u32()?,
        semantic_block_identity: r.fixed()?,
        semantic_callable_index: r.u32()?,
    })
}
pub(super) fn encode_declaration(
    w: &mut Writer<'_>,
    d: &Declaration,
) -> Result<(), KernelIrEncodeError> {
    encode_version(w)?;
    w.charge_work(1024)?;
    d.validate_shape()
        .map_err(|_| KernelIrEncodeError::NonCanonical {
            field: "LDS exchange declaration",
        })?;
    w.u8(0)?;
    for digest in d.origin.root_axes {
        w.bytes(&digest)?;
    }
    for digest in [
        d.origin.mir_body,
        d.origin.source_signature,
        d.origin.rustc_fn_abi,
        d.origin.frontend_bytes_sha256,
    ] {
        w.bytes(&digest)?;
    }
    encode_site(w, d.begin_site)?;
    for value in d.parameters {
        w.u32(value.0)?;
    }
    w.u8(d.native_instruction_count)?;
    for value in d.workgroup {
        w.u32(value)?;
    }
    for value in d.maximum_workgroups {
        w.u32(value)?;
    }
    w.u32(d.lds_frame.byte_offset)?;
    w.u32(d.lds_frame.byte_length)?;
    w.u32(d.lds_frame.alignment)?;
    w.u8(d.lds_frame.publication_epoch)?;
    let block = d.block;
    w.u8(block.label)?;
    encode_site(w, block.label_site)?;
    w.u8(block.encoding as u8)?;
    encode_site(w, block.terminator_site)?;
    w.u8(u8::from(block.native_ordinal.is_some()))?;
    w.u8(block.native_ordinal.unwrap_or(0))
}
pub(super) fn decode_declaration(
    r: &mut Reader<'_, '_>,
) -> Result<Declaration, KernelIrDecodeError> {
    r.charge_work(1024)?;
    revision(r)?;
    let origin = Origin {
        root_axes: [r.fixed()?, r.fixed()?, r.fixed()?, r.fixed()?, r.fixed()?],
        mir_body: r.fixed()?,
        source_signature: r.fixed()?,
        rustc_fn_abi: r.fixed()?,
        frontend_bytes_sha256: r.fixed()?,
    };
    let begin_site = decode_site(r)?;
    let parameters = [ValueId(r.u32()?), ValueId(r.u32()?)];
    let native_instruction_count = r.u8()?;
    let workgroup = [r.u32()?, r.u32()?, r.u32()?];
    let maximum_workgroups = [r.u32()?, r.u32()?, r.u32()?];
    let lds_frame = LdsFrame {
        byte_offset: r.u32()?,
        byte_length: r.u32()?,
        alignment: r.u32()?,
        publication_epoch: r.u8()?,
    };
    let label = r.u8()?;
    let label_site = decode_site(r)?;
    let encoding = match r.u8()? {
        4 => Branch::Endpgm0,
        tag => {
            return Err(KernelIrDecodeError::UnknownTag {
                kind: "LDS exchange terminal encoding",
                tag,
            });
        }
    };
    let terminator_site = decode_site(r)?;
    let native_ordinal = match (r.u8()?, r.u8()?) {
        (1, value) => Some(value),
        (0, 0) => None,
        (0, _) => return Err(KernelIrDecodeError::NonCanonical),
        (tag, _) => {
            return Err(KernelIrDecodeError::UnknownTag {
                kind: "LDS exchange terminal ordinal",
                tag,
            });
        }
    };
    let value = Declaration {
        origin,
        begin_site,
        parameters,
        native_instruction_count,
        workgroup,
        maximum_workgroups,
        lds_frame,
        block: Block {
            label,
            label_site,
            encoding,
            terminator_site,
            native_ordinal,
        },
    };
    value
        .validate_shape()
        .map_err(|_| KernelIrDecodeError::NonCanonical)?;
    Ok(value)
}
pub(super) fn encode_step(w: &mut Writer<'_>, step: &Step) -> Result<(), KernelIrEncodeError> {
    encode_version(w)?;
    w.charge_work(256)?;
    step.validate_shape()
        .map_err(|_| KernelIrEncodeError::NonCanonical {
            field: "LDS exchange step",
        })?;
    w.u8(0)?;
    encode_site(w, step.site)?;
    w.u8(step.native_ordinal)?;
    w.bytes(&step.instruction.descriptor())?;
    for value in step.operands {
        w.u8(u8::from(value.is_some()))?;
        w.u32(value.map_or(0, |id| id.0))?;
    }
    Ok(())
}
pub(super) fn decode_step(r: &mut Reader<'_, '_>) -> Result<Step, KernelIrDecodeError> {
    r.charge_work(256)?;
    revision(r)?;
    let site = decode_site(r)?;
    let native_ordinal = r.u8()?;
    let instruction =
        Instruction::from_descriptor(r.fixed()?).map_err(|_| KernelIrDecodeError::NonCanonical)?;
    let mut operands = [None; 5];
    for operand in &mut operands {
        *operand = match (r.u8()?, r.u32()?) {
            (0, 0) => None,
            (1, value) => Some(ValueId(value)),
            (0, _) => return Err(KernelIrDecodeError::NonCanonical),
            (tag, _) => {
                return Err(KernelIrDecodeError::UnknownTag {
                    kind: "LDS exchange operand presence",
                    tag,
                });
            }
        };
    }
    let value = Step {
        site,
        native_ordinal,
        instruction,
        operands,
    };
    value
        .validate_shape()
        .map_err(|_| KernelIrDecodeError::NonCanonical)?;
    Ok(value)
}
