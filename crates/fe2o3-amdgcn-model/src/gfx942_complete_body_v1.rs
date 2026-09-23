//! Closed, inert intent for a provisional gfx942 complete assembly body.
//!
//! This is not Kernel IR, a source owner, a compiler continuation, a renderer or
//! a second simulator. No executable marker/schema number is allocated here.
//! Arithmetic reuses the existing closed ordered-program instruction types.
//! There are no text instructions, raw addresses, external labels, calls,
//! divergent predicates or caller-selectable effects hidden behind a string.
//!
//! Checking copies at most eight blocks and sixteen arithmetic steps into fixed
//! arrays. It uses no heap allocation, recursion or unbounded work. A fixed
//! conservative 512-unit debit is made on the existing work meter before any
//! traversal. Retaining this plain value in a future owner must separately
//! account for size_of::<Gfx942CompleteBodyPlanV1>(); this is not an owner receipt.
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1, CanonicalKernelIrWorkLimitV1,
    Gfx942OrderedProgramRegistersV1 as Registers, Gfx942ProgramDestinationV1 as Destination,
    Gfx942ProgramInstructionV1 as Instruction, Gfx942ProgramRoleV1 as Role,
};
use std::fmt;

#[path = "gfx942_complete_body_contract_v1.rs"]
mod contract;
pub use contract::*;

#[cfg(test)]
#[path = "gfx942_complete_body_v1_tests.rs"]
mod tests;

pub const GFX942_COMPLETE_BODY_MAX_BLOCKS_V1: usize = 8;
pub const GFX942_COMPLETE_BODY_MAX_STEPS_V1: usize = 16;
pub const GFX942_COMPLETE_BODY_VALIDATION_WORK_V1: usize = 512;

/// Numeric, body-local label only; it is not a source/CFG/canonical identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyLabelV1(pub u8);

/// Uniform control comes ONLY from the separate kernel-argument selector.
/// A VGPR result cannot be substituted as a branch predicate. Structural
/// control-flow checking is conservative even when repeated tests correlate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyTerminatorV1 {
    Jump(Gfx942CompleteBodyLabelV1),
    BranchSelectorZero {
        zero: Gfx942CompleteBodyLabelV1,
        nonzero: Gfx942CompleteBodyLabelV1,
    },
    GuardedStoreOutputAndEnd,
}

/// Borrowed untrusted intent; check() copies all admitted content.
#[derive(Clone, Copy, Debug)]
pub struct Gfx942CompleteBodyBlockV1<'a> {
    pub label: Gfx942CompleteBodyLabelV1,
    pub instructions: &'a [Instruction],
    pub terminator: Gfx942CompleteBodyTerminatorV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyErrorV1 {
    Work(CanonicalKernelIrWorkLimitV1),
    Boundary,
    ReservedVgpr {
        register: u8,
    },
    Resources,
    BlockCount {
        count: usize,
    },
    StepCount {
        count: usize,
    },
    DuplicateLabel {
        label: Gfx942CompleteBodyLabelV1,
    },
    MissingLabel {
        label: Gfx942CompleteBodyLabelV1,
    },
    NonForwardEdge {
        from: Gfx942CompleteBodyLabelV1,
        to: Gfx942CompleteBodyLabelV1,
    },
    AliasedBranchTargets {
        label: Gfx942CompleteBodyLabelV1,
    },
    UnreachableBlock {
        label: Gfx942CompleteBodyLabelV1,
    },
    LastBlockMustTerminate,
    ReadBeforeDefinition {
        label: Gfx942CompleteBodyLabelV1,
        instruction: usize,
        roles: u8,
    },
    OutputNotDefined {
        label: Gfx942CompleteBodyLabelV1,
    },
}

impl fmt::Display for Gfx942CompleteBodyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "bounded complete-body intent refused: {self:?}")
    }
}
impl std::error::Error for Gfx942CompleteBodyErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Block {
    label: Gfx942CompleteBodyLabelV1,
    first: u8,
    count: u8,
    terminator: Gfx942CompleteBodyTerminatorV1,
    defined_in: u8,
    defined_out: u8,
}

/// Immutable checked *intent*, with no serialization, execution or owner API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyPlanV1 {
    registers: Registers,
    blocks: [Option<Block>; GFX942_COMPLETE_BODY_MAX_BLOCKS_V1],
    instructions: [Instruction; GFX942_COMPLETE_BODY_MAX_STEPS_V1],
    block_count: u8,
    instruction_count: u8,
}

/// Borrowed immutable projection, including the conservative merge masks.
#[derive(Clone, Copy, Debug)]
pub struct Gfx942CompleteBodyBlockViewV1<'a> {
    pub label: Gfx942CompleteBodyLabelV1,
    pub instructions: &'a [Instruction],
    pub terminator: Gfx942CompleteBodyTerminatorV1,
    pub defined_in: u8,
    pub defined_out: u8,
}

impl Gfx942CompleteBodyPlanV1 {
    pub fn check(
        boundary: Gfx942CompleteBodyBoundaryV1,
        registers: Registers,
        resources: Gfx942CompleteBodyResourcesV1,
        blocks: &[Gfx942CompleteBodyBlockV1<'_>],
        work: &mut CanonicalKernelIrWorkBudgetV1,
    ) -> Result<Self, Gfx942CompleteBodyErrorV1> {
        use Gfx942CompleteBodyErrorV1 as Error;
        use Gfx942CompleteBodyTerminatorV1 as Terminator;
        work.charge_work(GFX942_COMPLETE_BODY_VALIDATION_WORK_V1)
            .map_err(Error::Work)?;
        if boundary != Gfx942CompleteBodyBoundaryV1::PROFILE {
            return Err(Error::Boundary);
        }
        for register in [
            registers.scratch(),
            registers.output(),
            registers.inputs()[0],
            registers.inputs()[1],
            registers.inputs()[2],
        ] {
            if register < Gfx942CompleteBodyResourcesV1::FIRST_AUTHOR_VGPR {
                return Err(Error::ReservedVgpr { register });
            }
        }
        if resources != Gfx942CompleteBodyResourcesV1::required(registers) {
            return Err(Error::Resources);
        }
        if blocks.is_empty() || blocks.len() > GFX942_COMPLETE_BODY_MAX_BLOCKS_V1 {
            return Err(Error::BlockCount {
                count: blocks.len(),
            });
        }
        let mut total = 0_usize;
        for (ordinal, block) in blocks.iter().enumerate() {
            if block.instructions.len() > GFX942_COMPLETE_BODY_MAX_STEPS_V1 {
                return Err(Error::StepCount {
                    count: block.instructions.len(),
                });
            }
            total += block.instructions.len(); // at most 8 * 16
            if total > GFX942_COMPLETE_BODY_MAX_STEPS_V1 {
                return Err(Error::StepCount { count: total });
            }
            if blocks[..ordinal]
                .iter()
                .any(|previous| previous.label == block.label)
            {
                return Err(Error::DuplicateLabel { label: block.label });
            }
        }
        if total == 0 {
            return Err(Error::StepCount { count: total });
        }
        if blocks[blocks.len() - 1].terminator != Terminator::GuardedStoreOutputAndEnd {
            return Err(Error::LastBlockMustTerminate);
        }
        // Destination lookup is bounded by 8 blocks. Require every target to be
        // strictly later in authored order: no loops, self edge or implicit fallthrough.
        let mut edges = [[None; 2]; GFX942_COMPLETE_BODY_MAX_BLOCKS_V1];
        for (ordinal, block) in blocks.iter().enumerate() {
            let targets = match block.terminator {
                Terminator::Jump(target) => [Some(target), None],
                Terminator::BranchSelectorZero { zero, nonzero } => {
                    if zero == nonzero {
                        return Err(Error::AliasedBranchTargets { label: zero });
                    }
                    [Some(zero), Some(nonzero)]
                }
                Terminator::GuardedStoreOutputAndEnd => [None, None],
            };
            for (edge, target) in targets.into_iter().enumerate() {
                if let Some(target) = target {
                    let destination = blocks
                        .iter()
                        .position(|value| value.label == target)
                        .ok_or(Error::MissingLabel { label: target })?;
                    if destination <= ordinal {
                        return Err(Error::NonForwardEdge {
                            from: block.label,
                            to: target,
                        });
                    }
                    edges[ordinal][edge] = Some(destination);
                }
            }
        }
        let mut result = Self {
            registers,
            blocks: [None; GFX942_COMPLETE_BODY_MAX_BLOCKS_V1],
            instructions: [Instruction::Move {
                destination: Destination::Scratch,
                source: Role::Input0,
            }; GFX942_COMPLETE_BODY_MAX_STEPS_V1],
            block_count: blocks.len() as u8,
            instruction_count: total as u8,
        };
        let mut incoming = [None; GFX942_COMPLETE_BODY_MAX_BLOCKS_V1];
        incoming[0] = Some(0b00111_u8);
        let mut first = 0_usize;
        for (ordinal, block) in blocks.iter().enumerate() {
            let defined_in =
                incoming[ordinal].ok_or(Error::UnreachableBlock { label: block.label })?;
            let mut defined = defined_in;
            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                let required = match instruction {
                    Instruction::Move { source, .. } => bit(*source),
                    Instruction::Binary { left, right, .. } => bit(*left) | bit(*right),
                };
                let absent = required & !defined;
                if absent != 0 {
                    return Err(Error::ReadBeforeDefinition {
                        label: block.label,
                        instruction: instruction_index,
                        roles: absent,
                    });
                }
                // Inputs are immutable by the reused instruction destination type.
                defined |= bit(instruction.destination().role());
            }
            if block.terminator == Terminator::GuardedStoreOutputAndEnd
                && defined & bit(Role::Output) == 0
            {
                return Err(Error::OutputNotDefined { label: block.label });
            }
            for destination in edges[ordinal].into_iter().flatten() {
                incoming[destination] =
                    Some(incoming[destination].map_or(defined, |previous| previous & defined));
            }
            let count = block.instructions.len();
            result.instructions[first..first + count].copy_from_slice(block.instructions);
            result.blocks[ordinal] = Some(Block {
                label: block.label,
                first: first as u8,
                count: count as u8,
                terminator: block.terminator,
                defined_in,
                defined_out: defined,
            });
            first += count;
        }
        Ok(result)
    }

    pub const fn registers(&self) -> Registers {
        self.registers
    }
    pub const fn block_count(&self) -> usize {
        self.block_count as usize
    }
    pub const fn instruction_count(&self) -> usize {
        self.instruction_count as usize
    }
    pub const fn boundary(&self) -> Gfx942CompleteBodyBoundaryV1 {
        Gfx942CompleteBodyBoundaryV1::PROFILE
    }
    pub const fn resources(&self) -> Gfx942CompleteBodyResourcesV1 {
        Gfx942CompleteBodyResourcesV1::required(self.registers)
    }
    pub fn block(&self, ordinal: usize) -> Option<Gfx942CompleteBodyBlockViewV1<'_>> {
        let block = self.blocks.get(ordinal).copied().flatten()?;
        let first = usize::from(block.first);
        Some(Gfx942CompleteBodyBlockViewV1 {
            label: block.label,
            instructions: &self.instructions[first..first + usize::from(block.count)],
            terminator: block.terminator,
            defined_in: block.defined_in,
            defined_out: block.defined_out,
        })
    }
}

const fn bit(role: Role) -> u8 {
    1 << role as u8
}
