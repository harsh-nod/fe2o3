//! Closed semantic contract for the existing gfx942 integer assembly carrier.
//!
//! This validates an operation's shape, not its source authority, physical register
//! allocation, exact encoding, or hardware availability. Consumers still enforce their
//! own canonical-module, target, capability, and source-admission boundaries.

use std::fmt;

use crate::{
    AssemblyConstraint, AssemblyOperandKind, AssemblyOption, InlineAssemblyTarget, Operation,
    OperationKind, ScalarType, ValueId,
};

/// Instructions already admitted by the bounded gfx942 LLVM lowering profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942InlineAssemblyInstructionV1 {
    VMovB32,
    SMovB32,
    VAddU32,
    VSubU32,
    VAndB32,
    VOrB32,
    VXorB32,
}

impl Gfx942InlineAssemblyInstructionV1 {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::VMovB32 => "v_mov_b32",
            Self::SMovB32 => "s_mov_b32",
            Self::VAddU32 => "v_add_u32",
            Self::VSubU32 => "v_sub_u32",
            Self::VAndB32 => "v_and_b32",
            Self::VOrB32 => "v_or_b32",
            Self::VXorB32 => "v_xor_b32",
        }
    }

    pub const fn constraint(self) -> AssemblyConstraint {
        match self {
            Self::SMovB32 => AssemblyConstraint::Sgpr32,
            _ => AssemblyConstraint::Vgpr32,
        }
    }

    pub const fn input_count(self) -> usize {
        match self {
            Self::VMovB32 | Self::SMovB32 => 1,
            _ => 2,
        }
    }

    /// Whether `amdgpu_asm!` has this spelling for `u32` operands.
    ///
    /// The frontend's V30 subset admits these six spellings only for `u32`
    /// with NoMemory and authenticated per-call source references. This predicate
    /// alone grants neither source admission nor final GPU artifact authority.
    pub const fn has_source_macro(self) -> bool {
        !matches!(self, Self::SMovB32)
    }

    /// Evaluates the instruction's 32-bit data result with exact input arity.
    ///
    /// Add/sub discard carry/borrow; signed KIR carriers retain the same result bits.
    /// This scalar evaluator does not model SGPR uniformity, EXEC, register placement,
    /// scheduling, or instruction encoding. A simulator must separately admit those
    /// aspects before evaluating an operation.
    pub fn evaluate_bits(self, inputs: &[u32]) -> Result<u32, Gfx942InlineAssemblyErrorV1> {
        if inputs.len() != self.input_count() {
            return Err(Gfx942InlineAssemblyErrorV1::InputArity);
        }
        Ok(match self {
            Self::VMovB32 | Self::SMovB32 => inputs[0],
            Self::VAddU32 => inputs[0].wrapping_add(inputs[1]),
            Self::VSubU32 => inputs[0].wrapping_sub(inputs[1]),
            Self::VAndB32 => inputs[0] & inputs[1],
            Self::VOrB32 => inputs[0] | inputs[1],
            Self::VXorB32 => inputs[0] ^ inputs[1],
        })
    }
}

/// Resolves only canonical, closed instruction names; never accepts assembly text.
pub fn gfx942_inline_assembly_instruction_v1(
    mnemonic: &str,
) -> Option<Gfx942InlineAssemblyInstructionV1> {
    Some(match mnemonic {
        "v_mov_b32" => Gfx942InlineAssemblyInstructionV1::VMovB32,
        "s_mov_b32" => Gfx942InlineAssemblyInstructionV1::SMovB32,
        "v_add_u32" => Gfx942InlineAssemblyInstructionV1::VAddU32,
        "v_sub_u32" => Gfx942InlineAssemblyInstructionV1::VSubU32,
        "v_and_b32" => Gfx942InlineAssemblyInstructionV1::VAndB32,
        "v_or_b32" => Gfx942InlineAssemblyInstructionV1::VOrB32,
        "v_xor_b32" => Gfx942InlineAssemblyInstructionV1::VXorB32,
        _ => return None,
    })
}

/// Bounded failures of the closed instruction contract, without cloned source text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942InlineAssemblyErrorV1 {
    NotInlineAssembly,
    IncompleteSourceIdentity,
    UnsupportedTarget,
    UnsupportedInstruction,
    EffectMismatch,
    ResultArity,
    ResultType,
    OperandCount,
    OutputOperand,
    InputOperand,
    InputType,
    InputArity,
}

impl fmt::Display for Gfx942InlineAssemblyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotInlineAssembly => "expected an inline assembly operation",
            Self::IncompleteSourceIdentity => "inline assembly source identities are incomplete",
            Self::UnsupportedTarget => "integer inline assembly requires the gfx942 target",
            Self::UnsupportedInstruction => {
                "instruction is outside the closed gfx942 integer subset"
            }
            Self::EffectMismatch => "integer inline assembly must be NoMemory and effect-free",
            Self::ResultArity => "integer inline assembly requires exactly one result",
            Self::ResultType => "integer inline assembly result must be i32 or u32",
            Self::OperandCount => {
                "integer inline assembly operand count does not match the instruction"
            }
            Self::OutputOperand => {
                "first assembly operand must be result zero with the exact register constraint"
            }
            Self::InputOperand => {
                "assembly inputs must have input-only roles and exact register constraints"
            }
            Self::InputType => "assembly inputs must have the exact result type",
            Self::InputArity => "instruction evaluation requires the exact input count",
        })
    }
}

impl std::error::Error for Gfx942InlineAssemblyErrorV1 {}

/// A validated view of an existing operation, with no new executable graph or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedGfx942InlineAssemblyV1 {
    instruction: Gfx942InlineAssemblyInstructionV1,
    result: ValueId,
    scalar_type: ScalarType,
    inputs: [ValueId; 2],
}

impl ValidatedGfx942InlineAssemblyV1 {
    pub const fn instruction(self) -> Gfx942InlineAssemblyInstructionV1 {
        self.instruction
    }

    pub const fn result(self) -> ValueId {
        self.result
    }

    pub const fn scalar_type(self) -> ScalarType {
        self.scalar_type
    }

    pub fn inputs(&self) -> &[ValueId] {
        &self.inputs[..self.instruction.input_count()]
    }
}

/// Checks the complete bounded instruction shape without allocating.
///
/// `value_type` resolves input definitions in the operation's containing function.
/// A missing definition or a non-scalar type must return `None`. Nonzero source
/// identities are necessary shape checks; only the normal frontend can authenticate
/// their relationship to actual Rust source.
pub fn validate_gfx942_inline_assembly_v1(
    operation: &Operation,
    value_type: impl Fn(ValueId) -> Option<ScalarType>,
) -> Result<ValidatedGfx942InlineAssemblyV1, Gfx942InlineAssemblyErrorV1> {
    let OperationKind::InlineAssembly(assembly) = &operation.kind else {
        return Err(Gfx942InlineAssemblyErrorV1::NotInlineAssembly);
    };
    if !assembly.source.is_complete() {
        return Err(Gfx942InlineAssemblyErrorV1::IncompleteSourceIdentity);
    }
    if assembly.target != InlineAssemblyTarget::AmdGpuGfx942 {
        return Err(Gfx942InlineAssemblyErrorV1::UnsupportedTarget);
    }
    let instruction = gfx942_inline_assembly_instruction_v1(&assembly.mnemonic)
        .ok_or(Gfx942InlineAssemblyErrorV1::UnsupportedInstruction)?;
    if !assembly.declared_effects.is_empty()
        || !assembly.options.contains(&AssemblyOption::NoMemory)
        || assembly.options.contains(&AssemblyOption::ReadOnly)
    {
        return Err(Gfx942InlineAssemblyErrorV1::EffectMismatch);
    }
    let [result] = operation.results.as_slice() else {
        return Err(Gfx942InlineAssemblyErrorV1::ResultArity);
    };
    let scalar_type = result
        .ty
        .as_scalar()
        .filter(|scalar| matches!(scalar, ScalarType::I32 | ScalarType::U32))
        .ok_or(Gfx942InlineAssemblyErrorV1::ResultType)?;
    if assembly.operands.len() != instruction.input_count() + 1 {
        return Err(Gfx942InlineAssemblyErrorV1::OperandCount);
    }
    let output = &assembly.operands[0];
    if output.kind != (AssemblyOperandKind::Output { result_index: 0 })
        || output.constraint != instruction.constraint()
    {
        return Err(Gfx942InlineAssemblyErrorV1::OutputOperand);
    }
    let mut inputs = [ValueId(0); 2];
    for (index, operand) in assembly.operands[1..].iter().enumerate() {
        let AssemblyOperandKind::Input(value) = operand.kind else {
            return Err(Gfx942InlineAssemblyErrorV1::InputOperand);
        };
        if operand.constraint != instruction.constraint() {
            return Err(Gfx942InlineAssemblyErrorV1::InputOperand);
        }
        if value_type(value) != Some(scalar_type) {
            return Err(Gfx942InlineAssemblyErrorV1::InputType);
        }
        inputs[index] = value;
    }
    Ok(ValidatedGfx942InlineAssemblyV1 {
        instruction,
        result: result.id,
        scalar_type,
        inputs,
    })
}
