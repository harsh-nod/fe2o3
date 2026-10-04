//! Fixed-size, inert complete-body packing for future typed source producers.
//!
//! No executable MIR/KIR schema, terminal identity or source admission is
//! allocated here. Words are numeric const-packing descriptors, not AMD ISA
//! encodings. Decoding checks grammar/counts/padding only: duplicate labels,
//! invalid edges and undefined role reads still require the complete-body plan
//! validator. There is no CFG interpreter, emitter or owner constructor here.
//!
//! Labels and instruction roles deliberately match the existing inert body
//! model. Moving these primitive types here avoids a lowerer -> target-model
//! dependency. This module has no dependency on fe2o3-device or amdgcn-model.

use crate::{
    Gfx942ProgramDescriptorErrorV1, Gfx942ProgramDestinationV1, Gfx942ProgramInstructionV1,
    Gfx942ProgramRoleV1,
};
use std::fmt;

#[path = "gfx942_complete_body_builder_v1.rs"]
mod builder;
pub use builder::*;

#[cfg(test)]
#[path = "gfx942_complete_body_packing_v1_tests.rs"]
mod tests;

pub const GFX942_COMPLETE_BODY_MAX_BLOCKS_V1: usize = 8;
pub const GFX942_COMPLETE_BODY_MAX_STEPS_V1: usize = 16;
/// Optional caller charge for one bounded decode plus borrowed block projection.
/// This is separate from complete-body semantic/CFG validation and owner storage.
pub const GFX942_COMPLETE_BODY_PACKING_WORK_V1: usize = 64;

/// Numeric, body-local label only; not a source/CFG/canonical identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyLabelV1(pub u8);

/// Uniform control comes only from the separate kernel-argument selector.
/// Structural checking is conservative even when repeated tests correlate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyTerminatorV1 {
    Jump(Gfx942CompleteBodyLabelV1),
    BranchSelectorZero {
        zero: Gfx942CompleteBodyLabelV1,
        nonzero: Gfx942CompleteBodyLabelV1,
    },
    GuardedStoreOutputAndEnd,
}

/// Borrowed untrusted intent; neither this view nor its packing grants admission.
#[derive(Clone, Copy, Debug)]
pub struct Gfx942CompleteBodyBlockV1<'a> {
    pub label: Gfx942CompleteBodyLabelV1,
    pub instructions: &'a [Gfx942ProgramInstructionV1],
    pub terminator: Gfx942CompleteBodyTerminatorV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyPackingErrorV1 {
    BlockCount {
        count: usize,
    },
    StepCount {
        count: usize,
    },
    BlockStepCount {
        block: usize,
        count: usize,
    },
    StepCountMismatch {
        declared: usize,
        actual: usize,
    },
    ReservedBlockBit {
        block: usize,
    },
    TerminatorTag {
        block: usize,
        tag: u8,
    },
    NonZeroTargetPadding {
        block: usize,
    },
    NonZeroBlockPadding {
        block: usize,
    },
    NonZeroInstructionPadding {
        instruction: usize,
    },
    Instruction {
        instruction: usize,
        error: Gfx942ProgramDescriptorErrorV1,
    },
}

impl fmt::Display for Gfx942CompleteBodyPackingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "bounded complete-body packing refused: {self:?}")
    }
}
impl std::error::Error for Gfx942CompleteBodyPackingErrorV1 {}

/// Closed numeric encoding, not a wire Module, serialized proof or native code.
///
/// Two u32 block descriptors occupy each u64, earlier block in low bits.
/// Block bits: label 0..7, step count 8..12, terminator 13..14, target0
/// 15..22, target1 23..30, reserved zero bit 31. Terminator tags are Jump=0,
/// BranchSelectorZero=1, GuardedStoreOutputAndEnd=2; tag 3 is forbidden.
/// Jump has zero target1, terminal has both targets zero.
///
/// Four existing u16 arithmetic descriptors occupy each instruction word,
/// earlier instruction in low bits. Unused blocks/steps are zero. No fields
/// encode target authentication, ABI resources, register assignment or source IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyPackedV1 {
    block_count: u8,
    instruction_count: u8,
    block_words: [u64; 4],
    instruction_words: [u64; 4],
}

impl Gfx942CompleteBodyPackedV1 {
    /// Bounded structural import; every unused field is checked before return.
    pub fn from_words(
        block_count: u8,
        instruction_count: u8,
        block_words: [u64; 4],
        instruction_words: [u64; 4],
    ) -> Result<Self, Gfx942CompleteBodyPackingErrorV1> {
        let packed = Self {
            block_count,
            instruction_count,
            block_words,
            instruction_words,
        };
        packed.decode()?;
        Ok(packed)
    }

    pub const fn block_count(self) -> u8 {
        self.block_count
    }
    pub const fn instruction_count(self) -> u8 {
        self.instruction_count
    }
    pub const fn block_words(self) -> [u64; 4] {
        self.block_words
    }
    pub const fn instruction_words(self) -> [u64; 4] {
        self.instruction_words
    }

    /// Fixed-array replay of the packing grammar. This does not check body CFG,
    /// initialization, target/ABI/resource claims, or source custody.
    pub fn decode(self) -> Result<Gfx942CompleteBodyDecodedV1, Gfx942CompleteBodyPackingErrorV1> {
        use Gfx942CompleteBodyPackingErrorV1 as E;
        let block_count = usize::from(self.block_count);
        let instruction_count = usize::from(self.instruction_count);
        if block_count == 0 || block_count > GFX942_COMPLETE_BODY_MAX_BLOCKS_V1 {
            return Err(E::BlockCount { count: block_count });
        }
        if instruction_count == 0 || instruction_count > GFX942_COMPLETE_BODY_MAX_STEPS_V1 {
            return Err(E::StepCount {
                count: instruction_count,
            });
        }
        let mut decoded = Gfx942CompleteBodyDecodedV1 {
            blocks: [Block::EMPTY; GFX942_COMPLETE_BODY_MAX_BLOCKS_V1],
            instructions: [EMPTY_INSTRUCTION; GFX942_COMPLETE_BODY_MAX_STEPS_V1],
            block_count: self.block_count,
            instruction_count: self.instruction_count,
        };
        let mut first = 0_usize;
        for index in 0..GFX942_COMPLETE_BODY_MAX_BLOCKS_V1 {
            let word = (self.block_words[index / 2] >> (32 * (index % 2))) as u32;
            if index >= block_count {
                if word != 0 {
                    return Err(E::NonZeroBlockPadding { block: index });
                }
                continue;
            }
            if word & (1 << 31) != 0 {
                return Err(E::ReservedBlockBit { block: index });
            }
            let count = ((word >> 8) & 31) as usize;
            if count > GFX942_COMPLETE_BODY_MAX_STEPS_V1 {
                return Err(E::BlockStepCount {
                    block: index,
                    count,
                });
            }
            let tag = ((word >> 13) & 3) as u8;
            let target0 = Gfx942CompleteBodyLabelV1(((word >> 15) & 255) as u8);
            let target1 = Gfx942CompleteBodyLabelV1(((word >> 23) & 255) as u8);
            let terminator = match tag {
                0 if target1.0 == 0 => Gfx942CompleteBodyTerminatorV1::Jump(target0),
                1 => Gfx942CompleteBodyTerminatorV1::BranchSelectorZero {
                    zero: target0,
                    nonzero: target1,
                },
                2 if target0.0 == 0 && target1.0 == 0 => {
                    Gfx942CompleteBodyTerminatorV1::GuardedStoreOutputAndEnd
                }
                0 | 2 => return Err(E::NonZeroTargetPadding { block: index }),
                _ => return Err(E::TerminatorTag { block: index, tag }),
            };
            // At most 8 * 16; reject the total before indexing the step array.
            let next = first + count;
            if next > GFX942_COMPLETE_BODY_MAX_STEPS_V1 {
                return Err(E::StepCount { count: next });
            }
            decoded.blocks[index] = Block {
                label: Gfx942CompleteBodyLabelV1((word & 255) as u8),
                first: first as u8,
                count: count as u8,
                terminator,
            };
            first = next;
        }
        if first != instruction_count {
            return Err(E::StepCountMismatch {
                declared: instruction_count,
                actual: first,
            });
        }
        for index in 0..GFX942_COMPLETE_BODY_MAX_STEPS_V1 {
            let word = (self.instruction_words[index / 4] >> (16 * (index % 4))) as u16;
            if index >= instruction_count {
                if word != 0 {
                    return Err(E::NonZeroInstructionPadding { instruction: index });
                }
            } else {
                decoded.instructions[index] = Gfx942ProgramInstructionV1::from_descriptor(word)
                    .map_err(|error| E::Instruction {
                        instruction: index,
                        error,
                    })?;
            }
        }
        Ok(decoded)
    }
}

const EMPTY_INSTRUCTION: Gfx942ProgramInstructionV1 = Gfx942ProgramInstructionV1::Move {
    destination: Gfx942ProgramDestinationV1::Scratch,
    source: Gfx942ProgramRoleV1::Input0,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Block {
    label: Gfx942CompleteBodyLabelV1,
    first: u8,
    count: u8,
    terminator: Gfx942CompleteBodyTerminatorV1,
}
impl Block {
    const EMPTY: Self = Self {
        label: Gfx942CompleteBodyLabelV1(0),
        first: 0,
        count: 0,
        terminator: Gfx942CompleteBodyTerminatorV1::GuardedStoreOutputAndEnd,
    };
}

/// Fixed-array, structurally decoded intent. No heap or execution authority.
/// Future retained owners must separately charge its full logical inline size;
/// this value is not itself a storage receipt or an RSS bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyDecodedV1 {
    blocks: [Block; GFX942_COMPLETE_BODY_MAX_BLOCKS_V1],
    instructions: [Gfx942ProgramInstructionV1; GFX942_COMPLETE_BODY_MAX_STEPS_V1],
    block_count: u8,
    instruction_count: u8,
}

impl Gfx942CompleteBodyDecodedV1 {
    pub const fn block_count(&self) -> usize {
        self.block_count as usize
    }
    pub const fn instruction_count(&self) -> usize {
        self.instruction_count as usize
    }

    pub fn blocks(&self) -> impl ExactSizeIterator<Item = Gfx942CompleteBodyBlockV1<'_>> {
        // All ranges were bounded before construction; there is no mutable escape.
        self.blocks[..self.block_count()].iter().map(|block| {
            let first = usize::from(block.first);
            Gfx942CompleteBodyBlockV1 {
                label: block.label,
                instructions: &self.instructions[first..first + usize::from(block.count)],
                terminator: block.terminator,
            }
        })
    }
}
