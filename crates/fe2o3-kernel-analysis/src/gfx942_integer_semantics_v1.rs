//! Closed local gfx942 `S_ADD_U32` semantics over ordinary SGPRs and SCC.
//!
//! The encoding and scalar operand rules follow AMD's MI300 ISA Reference Guide,
//! 5-August-2025, sections 12.1 and 13.1.1 (tables 64-65). This is a projected
//! instruction model, not a complete wave state: it does not model PC, resource
//! allocation, memory, traps, or instruction scheduling. Decoding checks trace
//! consistency but does not authenticate the extractor or establish that source
//! values equal these machine inputs. It grants no load or launch authority.

use crate::{
    PhysicalMachineBranchKindV1, PhysicalMachineInstructionTraceV1, PhysicalMachineMemoryAccessV1,
    PhysicalMachineOperandValueV1,
};
use std::{error::Error, fmt};

/// Primary ISA reference; its exact bytes are pinned independently of LLVM.
pub const GFX942_INTEGER_ISA_REFERENCE_URL_V1: &str = "https://www.amd.com/content/dam/amd/en/documents/instinct-tech-docs/instruction-set-architectures/amd-instinct-mi300-cdna3-instruction-set-architecture.pdf";
pub const GFX942_INTEGER_ISA_REFERENCE_SHA256_V1: &str =
    "0cec4237cd93ce7dd76ee8502771429eb2308ab47dfa389f5bcc34f5903a6e2a";

pub const GFX942_ORDINARY_SGPR_COUNT_V1: usize = 102;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942U32AddResultV1 {
    pub value: u32,
    pub scc: bool,
}

include!("gfx942_integer_semantics_v1/add_u32_body.rs");
include!("gfx942_integer_semantics_v1/execute_body.rs");

/// Arithmetic used by the executable model and the separate arithmetic proof.
pub fn gfx942_add_u32_v1(a: u32, b: u32) -> Gfx942U32AddResultV1 {
    gfx942_add_u32_body_v1!(a, b)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942U32SourceV1 {
    Sgpr(u8),
    Constant(u32),
}

/// Caller-supplied projected register state, not a native machine-state receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942ScalarIntegerStateV1 {
    registers: [u32; GFX942_ORDINARY_SGPR_COUNT_V1],
    scc: bool,
}

impl Gfx942ScalarIntegerStateV1 {
    pub const fn new(registers: [u32; GFX942_ORDINARY_SGPR_COUNT_V1], scc: bool) -> Self {
        Self { registers, scc }
    }

    pub const fn registers(&self) -> &[u32; GFX942_ORDINARY_SGPR_COUNT_V1] {
        &self.registers
    }

    pub const fn scc(&self) -> bool {
        self.scc
    }
}

/// An exact supported instruction encoding with consistent LLVM trace facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942SAddU32V1 {
    destination: u8,
    sources: [Gfx942U32SourceV1; 2],
    bytes: [u8; 8],
    byte_len: usize,
}

impl Gfx942SAddU32V1 {
    /// Reject every instruction or trace shape outside this closed local model.
    ///
    /// Only SGPR0..SGPR101, integer inline constants -16..64, and the shared
    /// 32-bit literal word are supported. Special registers and floating inline
    /// constants are deliberately excluded even when hardware accepts them.
    pub fn decode(
        instruction: &PhysicalMachineInstructionTraceV1,
    ) -> Result<Self, Gfx942IntegerSemanticsErrorV1> {
        use Gfx942IntegerSemanticsErrorV1 as E;
        let encoding = instruction.encoding();
        let word_bytes: [u8; 4] = encoding
            .get(..4)
            .ok_or(E::InvalidEncodingLength)?
            .try_into()
            .map_err(|_| E::InvalidEncodingLength)?;
        let word = u32::from_le_bytes(word_bytes);
        // SOP2 occupies bits31:30=10; S_ADD_U32 has opcode0 in bits29:23.
        if word & 0xff80_0000 != 0x8000_0000 {
            return Err(E::UnsupportedEncoding);
        }
        let destination = ((word >> 16) & 0x7f) as u8;
        if usize::from(destination) >= GFX942_ORDINARY_SGPR_COUNT_V1 {
            return Err(E::UnsupportedDestination(destination));
        }
        let selectors = [word as u8, (word >> 8) as u8];
        let byte_len = if selectors.contains(&255) { 8 } else { 4 };
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
        let sources = [
            decode_source(selectors[0], literal)?,
            decode_source(selectors[1], literal)?,
        ];
        if instruction.opcode() != "S_ADD_U32_vi" {
            return Err(E::OpcodeMismatch);
        }
        let operands = instruction.operands();
        if instruction.explicit_definition_count() != 1 || operands.len() != 3 {
            return Err(E::OperandShapeMismatch);
        }
        if operands.iter().any(|operand| operand.tied_to().is_some()) {
            return Err(E::OperandShapeMismatch);
        }
        if !matches_register(operands[0].value(), destination)
            || !matches_source(operands[1].value(), sources[0])
            || !matches_source(operands[2].value(), sources[1])
        {
            return Err(E::OperandValueMismatch);
        }
        if instruction.implicit_definitions() != ["SCC"] || !instruction.implicit_uses().is_empty()
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
            sources,
            bytes,
            byte_len,
        })
    }

    pub const fn destination(&self) -> u8 {
        self.destination
    }

    pub const fn sources(&self) -> [Gfx942U32SourceV1; 2] {
        self.sources
    }

    pub fn encoding(&self) -> &[u8] {
        &self.bytes[..self.byte_len]
    }

    /// Execute one local step; source/destination aliasing reads the old values.
    /// All other ordinary SGPRs are preserved and incoming SCC is not an input.
    pub fn execute(&self, state: &mut Gfx942ScalarIntegerStateV1) -> Gfx942U32AddResultV1 {
        gfx942_s_add_u32_execute_body_v1!(self, state)
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }
}

fn decode_source(
    selector: u8,
    literal: Option<u32>,
) -> Result<Gfx942U32SourceV1, Gfx942IntegerSemanticsErrorV1> {
    Ok(match selector {
        0..=101 => Gfx942U32SourceV1::Sgpr(selector),
        128..=192 => Gfx942U32SourceV1::Constant(u32::from(selector - 128)),
        193..=208 => Gfx942U32SourceV1::Constant((192_i32 - i32::from(selector)) as u32),
        255 => Gfx942U32SourceV1::Constant(
            literal.ok_or(Gfx942IntegerSemanticsErrorV1::InvalidEncodingLength)?,
        ),
        _ => return Err(Gfx942IntegerSemanticsErrorV1::UnsupportedSource(selector)),
    })
}

fn matches_register(value: &PhysicalMachineOperandValueV1, index: u8) -> bool {
    matches!(value, PhysicalMachineOperandValueV1::Register(name) if name == &format!("SGPR{index}"))
}

fn matches_source(value: &PhysicalMachineOperandValueV1, source: Gfx942U32SourceV1) -> bool {
    match source {
        Gfx942U32SourceV1::Sgpr(index) => matches_register(value, index),
        Gfx942U32SourceV1::Constant(expected) => match value {
            // LLVM may represent an i32 literal as signed or zero-extended i64.
            PhysicalMachineOperandValueV1::SignedImmediate(actual)
                if (i64::from(i32::MIN)..=i64::from(u32::MAX)).contains(actual) =>
            {
                *actual as u32 == expected
            }
            _ => false,
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942IntegerSemanticsErrorV1 {
    InvalidEncodingLength,
    UnsupportedEncoding,
    UnsupportedDestination(u8),
    UnsupportedSource(u8),
    OpcodeMismatch,
    OperandShapeMismatch,
    OperandValueMismatch,
    ImplicitEffectsMismatch,
    InstructionEffectsMismatch,
}

impl fmt::Display for Gfx942IntegerSemanticsErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "gfx942 local integer semantics rejected instruction: {self:?}"
        )
    }
}

impl Error for Gfx942IntegerSemanticsErrorV1 {}

#[cfg(test)]
mod tests;
