//! Closed projected `S_MOV_B32` semantics.
//! Pinned MI300 ISA: section 12.3 and section 13.1.3, tables 68-69.
//! Local projected MOV semantics only; no decoder or compiler refinement proof.

use super::{
    GFX942_ORDINARY_SGPR_COUNT_V1, Gfx942IntegerSemanticsErrorV1, Gfx942ScalarIntegerStateV1,
    Gfx942U32SourceV1, PhysicalMachineBranchKindV1, PhysicalMachineInstructionTraceV1,
    PhysicalMachineMemoryAccessV1, decode_source, matches_register, matches_source,
};

include!("mov_b32_body.rs");

/// A closed ordinary-SGPR/integer-source instruction with exact trace checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942SMovB32V1 {
    destination: u8,
    source: Gfx942U32SourceV1,
    bytes: [u8; 8],
    byte_len: usize,
}

impl Gfx942SMovB32V1 {
    /// This validates consistency, not the authenticity of the LLVM extractor.
    /// Only SGPR0..101, integer inline constants -16..64, and a literal are
    /// accepted. SCC/EXEC/special registers and floating selectors are excluded.
    pub fn decode(
        instruction: &PhysicalMachineInstructionTraceV1,
    ) -> Result<Self, Gfx942IntegerSemanticsErrorV1> {
        use Gfx942IntegerSemanticsErrorV1 as E;
        if instruction.target() != crate::PhysicalMachineTargetV1::Gfx942XnackMinusCov6 {
            return Err(E::UnsupportedTarget);
        }
        let encoding = instruction.encoding();
        let word_bytes: [u8; 4] = encoding
            .get(..4)
            .ok_or(E::InvalidEncodingLength)?
            .try_into()
            .map_err(|_| E::InvalidEncodingLength)?;
        let word = u32::from_le_bytes(word_bytes);
        // SOP1 bits31:23=10_1111101, with MOV opcode0 in bits15:8.
        if word & 0xff80_ff00 != 0xbe80_0000 {
            return Err(E::UnsupportedEncoding);
        }
        let destination = ((word >> 16) & 0x7f) as u8;
        if usize::from(destination) >= GFX942_ORDINARY_SGPR_COUNT_V1 {
            return Err(E::UnsupportedDestination(destination));
        }
        let selector = word as u8;
        let byte_len = if selector == 255 { 8 } else { 4 };
        if encoding.len() != byte_len {
            return Err(E::InvalidEncodingLength);
        }
        let literal = if byte_len == 8 {
            Some(u32::from_le_bytes(
                encoding[4..8]
                    .try_into()
                    .map_err(|_| E::InvalidEncodingLength)?,
            ))
        } else {
            None
        };
        let source = decode_source(selector, literal)?;
        if instruction.opcode() != "S_MOV_B32_vi" {
            return Err(E::OpcodeMismatch);
        }
        let operands = instruction.operands();
        if instruction.explicit_definition_count() != 1
            || operands.len() != 2
            || operands.iter().any(|operand| operand.tied_to().is_some())
        {
            return Err(E::OperandShapeMismatch);
        }
        if !matches_register(operands[0].value(), destination)
            || !matches_source(operands[1].value(), source)
        {
            return Err(E::OperandValueMismatch);
        }
        if !instruction.implicit_definitions().is_empty() || !instruction.implicit_uses().is_empty()
        {
            return Err(E::ImplicitEffectsMismatch);
        }
        if instruction.flags().bits() != 0
            || instruction.branch_kind() != PhysicalMachineBranchKindV1::None
            || instruction.branch_target().is_some()
            || instruction.memory_access() != PhysicalMachineMemoryAccessV1::None
        {
            return Err(E::InstructionEffectsMismatch);
        }
        let mut bytes = [0; 8];
        bytes[..byte_len].copy_from_slice(encoding);
        Ok(Self {
            destination,
            source,
            bytes,
            byte_len,
        })
    }

    pub const fn destination(&self) -> u8 {
        self.destination
    }

    pub const fn source(&self) -> Gfx942U32SourceV1 {
        self.source
    }

    pub fn encoding(&self) -> &[u8] {
        &self.bytes[..self.byte_len]
    }

    /// Read before writing, including self-aliases; preserve SCC and all other
    /// ordinary SGPRs. The returned value is the complete destination bitpattern.
    pub fn execute(&self, state: &mut Gfx942ScalarIntegerStateV1) -> u32 {
        gfx942_s_mov_b32_execute_body_v1!(self, state)
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests;
