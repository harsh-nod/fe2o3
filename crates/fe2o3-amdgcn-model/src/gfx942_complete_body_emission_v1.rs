//! Inert bounded LLVM mechanism for an already checked complete-body intent.
//!
//! No source/canonical owner, importer, proof, continuation or artifact authority
//! is constructed here. A future normal emitter must independently establish the
//! complete current canonical-CFG-to-plan relation. Native ABI/branch/descriptor
//! validation is separate; these strings alone do not qualify that relation.
use crate::Gfx942CompleteBodyPlanV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1, CanonicalKernelIrWorkLimitV1, Gfx942CompleteBodyLabelV1,
    Gfx942OrderedProgramRegistersV1,
};
use std::fmt;

#[path = "gfx942_complete_body_emit_v1.rs"]
mod emit;
#[cfg(test)]
#[path = "gfx942_complete_body_emission_v1_tests.rs"]
mod tests;

pub const GFX942_COMPLETE_BODY_ASSEMBLY_BYTES_V1: usize = 4 * 1024;
pub const GFX942_COMPLETE_BODY_LLVM_BYTES_V1: usize = 16 * 1024;
/// Conservative prepaid logical work, not measured CPU time or allocation/RSS.
pub const GFX942_COMPLETE_BODY_RENDER_WORK_V1: usize = 65_536;

/// Validated unquoted LLVM identifier; never caller-provided instruction text.
/// Borrowing this string confers no source or symbol authenticity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodySymbolV1<'a>(&'a str);
impl<'a> Gfx942CompleteBodySymbolV1<'a> {
    pub fn new(value: &'a str) -> Result<Self, Gfx942CompleteBodyEmissionErrorV1> {
        if value.is_empty()
            || value.len() > 128
            || !value.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_alphabetic() || byte == b'_' || (index > 0 && byte.is_ascii_digit())
            })
        {
            return Err(Gfx942CompleteBodyEmissionErrorV1::Symbol);
        }
        Ok(Self(value))
    }
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyTextKindV1 {
    Assembly,
    Llvm,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyEmissionErrorV1 {
    Symbol,
    Work(CanonicalKernelIrWorkLimitV1),
    Allocation,
    TextLimit(Gfx942CompleteBodyTextKindV1),
    PlanInvariant,
}
impl fmt::Display for Gfx942CompleteBodyEmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "bounded complete-body LLVM mechanism refused: {self:?}"
        )
    }
}
impl std::error::Error for Gfx942CompleteBodyEmissionErrorV1 {}

/// Successors name authored ordinals, never raw label numbers or native PCs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyLoweredTerminatorV1 {
    Jump {
        target_ordinal: u8,
    },
    BranchSelectorZero {
        zero_ordinal: u8,
        nonzero_ordinal: u8,
    },
    CompilerTail,
}

/// Zero-based lines in assembly_template(); not native instruction addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyEmittedBlockV1 {
    pub ordinal: u8,
    pub label: Gfx942CompleteBodyLabelV1,
    pub label_line: u16,
    pub first_instruction_ordinal: u8,
    pub instruction_count: u8,
    pub terminator_line: u16,
    pub terminator_line_count: u8,
    pub terminator: Gfx942CompleteBodyLoweredTerminatorV1,
}
/// Every typed arithmetic step has one exact line, including dead/self writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyEmittedInstructionV1 {
    pub ordinal: u8,
    pub block_ordinal: u8,
    pub instruction_in_block: u8,
    pub descriptor: u16,
    pub assembly_line: u16,
}
/// Additional compiler-owned junction, never an authored block/label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyEmittedTailV1 {
    pub label_line: u16,
    pub first_instruction_line: u16,
    pub instruction_count: u8,
}

/// Immutable diagnostic text and bounded correspondence, not a source owner.
/// The assembly text is an LLVM inline-assembly template: generated local names
/// contain LLVM's unique-instance substitution, not caller-selected labels.
#[derive(Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyEmissionV1 {
    llvm: String,
    assembly: String,
    registers: Gfx942OrderedProgramRegistersV1,
    blocks: [Option<Gfx942CompleteBodyEmittedBlockV1>; 8],
    instructions: [Option<Gfx942CompleteBodyEmittedInstructionV1>; 16],
    tail: Gfx942CompleteBodyEmittedTailV1,
}
impl Gfx942CompleteBodyEmissionV1 {
    pub(crate) fn into_llvm_ir_v19(self) -> String {
        self.llvm
    }
    pub(crate) fn retained_text_capacity_v19(&self) -> usize {
        self.llvm.capacity() + self.assembly.capacity()
    }
    pub fn llvm_ir(&self) -> &str {
        &self.llvm
    }
    pub fn assembly_template(&self) -> &str {
        &self.assembly
    }
    pub const fn registers(&self) -> Gfx942OrderedProgramRegistersV1 {
        self.registers
    }
    pub fn blocks(&self) -> impl Iterator<Item = &Gfx942CompleteBodyEmittedBlockV1> {
        self.blocks.iter().flatten()
    }
    pub fn instructions(&self) -> impl Iterator<Item = &Gfx942CompleteBodyEmittedInstructionV1> {
        self.instructions.iter().flatten()
    }
    pub const fn compiler_tail(&self) -> Gfx942CompleteBodyEmittedTailV1 {
        self.tail
    }
}

/// Emits only the fixed gfx942:xnack-/Wave64 six-machine-parameter shell.
/// The last i32 parameter is the uniform selector, forwarded once to {s22}.
/// The intended explicit parameter offsets are 0,8,16,20,24,28; independent
/// AMDHSA metadata/native checks must establish the actual emitted ABI.
/// Source-derived IDs, target overrides, caller operands and clobber lists are
/// deliberately absent. No protected or production artifact API is called.
///
/// Logical output caps and prepaid work do not bound rustc/LLVM internal memory,
/// allocator rounding, or the caller's retention of multiple returned values.
pub fn render_gfx942_complete_body_llvm_v1(
    plan: &Gfx942CompleteBodyPlanV1,
    symbol: Gfx942CompleteBodySymbolV1<'_>,
    work: &mut CanonicalKernelIrWorkBudgetV1,
) -> Result<Gfx942CompleteBodyEmissionV1, Gfx942CompleteBodyEmissionErrorV1> {
    work.charge_work(GFX942_COMPLETE_BODY_RENDER_WORK_V1)
        .map_err(Gfx942CompleteBodyEmissionErrorV1::Work)?;
    emit::render(plan, symbol)
}

/// Only for the canonical bridge after its full same-ledger RENDER_WORK debit.
pub(crate) fn render_prepaid_v19(
    plan: &Gfx942CompleteBodyPlanV1,
    symbol: Gfx942CompleteBodySymbolV1<'_>,
) -> Result<Gfx942CompleteBodyEmissionV1, Gfx942CompleteBodyEmissionErrorV1> {
    emit::render(plan, symbol)
}
