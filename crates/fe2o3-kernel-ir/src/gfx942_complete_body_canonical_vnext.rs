//! Inert typed complete-body vocabulary, encoded only by exact KIR19.
//! Hashes/labels are not source authority; the containing FunctionBody is the
//! only executable CFG. Whole-function verification is separate from shape.

use crate::{Gfx942OrderedProgramRegistersV1, Gfx942ProgramInstructionV1, ValueId};

/// Exact retained compiler identity observations, not self-authenticating hashes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyOriginVNext {
    /// Function, item, monomorphization, generic-type, const-generic identities.
    pub root_axes: [[u8; 32]; 5],
    pub mir_body: [u8; 32],
    pub semantic_block: [u8; 32],
    pub source_signature: [u8; 32],
    pub rustc_fn_abi: [u8; 32],
    /// SHA256 of the actual current canonical frontend bytes; NOT an old
    /// frontend identity relabeled into a newly invented identity domain.
    pub frontend_bytes_sha256: [u8; 32],
    pub raw_block: u32,
}
impl Gfx942CompleteBodyOriginVNext {
    pub fn is_complete(self) -> bool {
        self.root_axes
            .into_iter()
            .chain([
                self.mir_body,
                self.semantic_block,
                self.source_signature,
                self.rustc_fn_abi,
                self.frontend_bytes_sha256,
            ])
            .all(|digest| digest != [0; 32])
    }
}

/// Source-order declaration at the root entry. No packed instructions, branch
/// targets, executable bytecode or caller-selected resource flags are retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyDeclarationVNext {
    pub origin: Gfx942CompleteBodyOriginVNext,
    pub registers: Gfx942OrderedProgramRegistersV1,
    /// Logical arguments: output slice, three u32 inputs, uniform u32 selector.
    pub parameters: [ValueId; 5],
    /// First block_count entries are actual authored labels; rest must be zero.
    /// Authored ordinal n corresponds to canonical BlockId(n), not label number.
    pub labels: [u8; 8],
    pub block_count: u8,
    pub instruction_count: u8,
}

/// One actual instruction, in its containing canonical block. Its result is
/// the containing Operation's one u32 ValueDef. Move has operands [Some, None];
/// Binary has [Some, Some]. A second generic Binary/Move operation is NOT emitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyStepVNext {
    pub authored_block: u8,
    pub authored_instruction: u8,
    pub instruction: Gfx942ProgramInstructionV1,
    pub operands: [Option<ValueId>; 2],
}

/// Structural requirement only; not authenticated source or execution authority.
pub const AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAMESPACE_V19: &str = "amd.gfx942";
pub const AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAME_V19: &str = "complete_body_u32_e32_v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyShapeErrorV19 {
    Origin,
    Count,
    Register,
    Parameter,
    Label,
    Ordinal,
    OperandArity,
}
impl Gfx942CompleteBodyDeclarationVNext {
    /// Fixed bounded payload shape only, not whole-function semantic admission.
    pub fn validate_shape(&self) -> Result<(), Gfx942CompleteBodyShapeErrorV19> {
        use Gfx942CompleteBodyShapeErrorV19 as E;
        if !self.origin.is_complete() {
            return Err(E::Origin);
        }
        let blocks = usize::from(self.block_count);
        if !(1..=8).contains(&blocks) || !(1..=16).contains(&self.instruction_count) {
            return Err(E::Count);
        }
        let registers = [
            self.registers.scratch(),
            self.registers.output(),
            self.registers.inputs()[0],
            self.registers.inputs()[1],
            self.registers.inputs()[2],
        ];
        if registers.iter().any(|register| !(8..64).contains(register)) {
            return Err(E::Register);
        }
        for (index, parameter) in self.parameters.iter().enumerate() {
            if self.parameters[..index].contains(parameter) {
                return Err(E::Parameter);
            }
        }
        for (index, label) in self.labels[..blocks].iter().enumerate() {
            if self.labels[..index].contains(label) {
                return Err(E::Label);
            }
        }
        if self.labels[blocks..].iter().any(|label| *label != 0) {
            return Err(E::Label);
        }
        Ok(())
    }
}
impl Gfx942CompleteBodyStepVNext {
    /// Preserves repeated SSA reads; ordinals are not compiler/source identity.
    pub fn validate_shape(&self) -> Result<(), Gfx942CompleteBodyShapeErrorV19> {
        use Gfx942CompleteBodyShapeErrorV19 as E;
        if self.authored_block >= 8 || self.authored_instruction >= 16 {
            return Err(E::Ordinal);
        }
        let valid = match self.instruction {
            Gfx942ProgramInstructionV1::Move { .. } => {
                self.operands[0].is_some() && self.operands[1].is_none()
            }
            Gfx942ProgramInstructionV1::Binary { .. } => self.operands.iter().all(Option::is_some),
        };
        if !valid {
            return Err(E::OperandArity);
        }
        Ok(())
    }
}
