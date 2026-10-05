//! Closed, bounded same-block MOV prefix and terminal ADD over projected state.
//! Trace consistency is not extractor authentication, compiler entry equality,
//! whole-kernel refinement, or load/launch authority.

use super::{
    GFX942_ORDINARY_SGPR_COUNT_V1, Gfx942IntegerSemanticsErrorV1, Gfx942SAddU32V1, Gfx942SMovB32V1,
    Gfx942ScalarIntegerStateV1, Gfx942U32AddResultV1, Gfx942U32SourceV1,
};
use crate::PhysicalMachineTraceEvidenceV1;
use std::{error::Error, fmt};

pub const GFX942_MOV_PREFIX_ADD_MAX_INSTRUCTIONS_V1: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942U32OriginV1 {
    EntrySgpr(u8),
    Constant(u32),
}

include!("mov_prefix_add_body.rs");

macro_rules! prefix_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942MovPrefixAddU32V1 {
    function: String,
    block: u32,
    first_offset: u64,
    add_offset: u64,
    end_offset: u64,
    moves: Vec<Gfx942SMovB32V1>,
    add: Gfx942SAddU32V1,
}

impl Gfx942MovPrefixAddU32V1 {
    /// Select every instruction in the inclusive start/terminal-offset interval,
    /// in retained trace order. Never filter unsupported instructions out.
    pub fn decode(
        trace: &PhysicalMachineTraceEvidenceV1,
        function: &str,
        first_offset: u64,
        add_offset: u64,
    ) -> Result<Self, Gfx942MovPrefixAddErrorV1> {
        use Gfx942MovPrefixAddErrorV1 as E;
        if trace.target() != crate::PhysicalMachineTargetV1::Gfx942XnackMinusCov6 {
            return Err(E::Instruction(
                Gfx942IntegerSemanticsErrorV1::UnsupportedTarget,
            ));
        }
        if first_offset > add_offset {
            return Err(E::Interval);
        }
        let mut moves = Vec::new();
        let mut add = None;
        let mut block = None;
        let mut cursor = first_offset;
        let mut count = 0usize;
        for instruction in trace.instructions() {
            if instruction.function_symbol() != function {
                continue;
            }
            let offset = instruction.instruction_offset();
            let end = offset
                .checked_add(instruction.encoding().len() as u64)
                .ok_or(E::Interval)?;
            // A selected boundary must never split an instruction.
            if offset < first_offset && end > first_offset {
                return Err(E::Interval);
            }
            if offset < first_offset || offset > add_offset {
                continue;
            }
            count += 1;
            if count > GFX942_MOV_PREFIX_ADD_MAX_INSTRUCTIONS_V1 {
                return Err(E::TooManyInstructions);
            }
            if offset != cursor || end <= offset {
                return Err(E::Interval);
            }
            match block {
                None => block = Some(instruction.block_ordinal()),
                Some(expected) if expected == instruction.block_ordinal() => {}
                Some(_) => return Err(E::CrossBlock),
            }
            if offset == add_offset {
                add = Some(Gfx942SAddU32V1::decode(instruction).map_err(E::Instruction)?);
            } else {
                moves.push(Gfx942SMovB32V1::decode(instruction).map_err(E::Instruction)?);
            }
            cursor = end;
        }
        let add = add.ok_or(E::Interval)?;
        let block = block.ok_or(E::Interval)?;
        Ok(Self {
            function: function.to_owned(),
            block,
            first_offset,
            add_offset,
            end_offset: cursor,
            moves,
            add,
        })
    }

    pub fn function_symbol(&self) -> &str {
        &self.function
    }

    pub const fn block_ordinal(&self) -> u32 {
        self.block
    }

    pub const fn first_offset(&self) -> u64 {
        self.first_offset
    }

    pub const fn add_offset(&self) -> u64 {
        self.add_offset
    }

    pub const fn end_offset(&self) -> u64 {
        self.end_offset
    }

    pub fn moves(&self) -> &[Gfx942SMovB32V1] {
        &self.moves
    }

    pub const fn terminal_add(&self) -> &Gfx942SAddU32V1 {
        &self.add
    }

    pub fn terminal_origins(&self) -> [Gfx942U32OriginV1; 2] {
        prefix_origins(&self.moves, &self.add)
    }

    /// Execute the actual decoded MOVs in order and then the terminal ADD.
    pub fn execute(&self, state: &mut Gfx942ScalarIntegerStateV1) -> Gfx942U32AddResultV1 {
        let moves = self.moves.as_slice();
        let add = &self.add;
        gfx942_mov_prefix_execute_body_v1!(prefix_rust_expr, moves, add, state, index, [])
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }
}

fn prefix_origins(moves: &[Gfx942SMovB32V1], add: &Gfx942SAddU32V1) -> [Gfx942U32OriginV1; 2] {
    gfx942_mov_prefix_origins_body_v1!(prefix_rust_expr, moves, add, origins, index, [], [])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942MovPrefixAddErrorV1 {
    Interval,
    CrossBlock,
    TooManyInstructions,
    Instruction(Gfx942IntegerSemanticsErrorV1),
}

impl fmt::Display for Gfx942MovPrefixAddErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unsupported gfx942 MOV-prefix/ADD span: {self:?}"
        )
    }
}

impl Error for Gfx942MovPrefixAddErrorV1 {}

#[cfg(test)]
mod tests;
