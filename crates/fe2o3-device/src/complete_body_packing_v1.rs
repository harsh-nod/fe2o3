//! Experimental inert const packing for a bounded gfx942 complete-body intent.
//!
//! These numeric words are NOT AMD instructions, a runtime entry point, a
//! source-authenticated kernel, or executable MIR/KIR. There is no marker call,
//! renderer, interpreter or admission owner. The compiler independently checks
//! grammar and must separately check CFG, initialization, target, ABI/resources
//! and current source custody. Empty blocks and cross-block role reads are
//! intentionally representable here.
//!
//! This no_std module has no compiler-crate dependency. Its exact numeric format
//! is parity-tested against fe2o3-kernel-ir by a separate normal consumer.

#[path = "complete_body_builder_v1.rs"]
mod builder;
pub use builder::Gfx942CompleteBodyBuilderV1;

pub const GFX942_COMPLETE_BODY_MAX_BLOCKS_V1: usize = 8;
pub const GFX942_COMPLETE_BODY_MAX_STEPS_V1: usize = 16;

/// A body-local numeric label, not a source or canonical identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyLabelV1(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyTerminatorV1 {
    Jump(Gfx942CompleteBodyLabelV1),
    BranchSelectorZero {
        zero: Gfx942CompleteBodyLabelV1,
        nonzero: Gfx942CompleteBodyLabelV1,
    },
    /// Declared intent only; no validated emitted ABI/prologue/store evidence.
    GuardedStoreOutputAndEnd,
}

/// Same closed arithmetic descriptor grammar as the ordered-program source API.
/// This wrapper checks encoding only, NOT definite assignment or CFG behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyInstructionV1(u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyDescriptorErrorV1 {
    ReservedBits { bits: u16 },
    Opcode { tag: u8 },
    Source { operand: u8, tag: u8 },
    MoveUnusedSource { tag: u8 },
}

impl Gfx942CompleteBodyInstructionV1 {
    /// Bits 0..2 opcode; bit 3 destination scratch/out; bits 4..6 source0;
    /// bits 7..9 source1. Move=0, add/sub/and/or/xor=1..5; source roles
    /// input0/input1/input2/scratch/out=0..4. Move source1 and bits 10..15
    /// must be zero. Existing ordered-program step macros produce these words.
    pub const fn from_descriptor(word: u16) -> Result<Self, Gfx942CompleteBodyDescriptorErrorV1> {
        use Gfx942CompleteBodyDescriptorErrorV1 as E;
        let reserved = word & 0xfc00;
        if reserved != 0 {
            return Err(E::ReservedBits { bits: reserved });
        }
        let opcode = (word & 7) as u8;
        if opcode > 5 {
            return Err(E::Opcode { tag: opcode });
        }
        let source0 = ((word >> 4) & 7) as u8;
        if source0 > 4 {
            return Err(E::Source {
                operand: 0,
                tag: source0,
            });
        }
        let source1 = ((word >> 7) & 7) as u8;
        if opcode == 0 {
            if source1 != 0 {
                return Err(E::MoveUnusedSource { tag: source1 });
            }
        } else if source1 > 4 {
            return Err(E::Source {
                operand: 1,
                tag: source1,
            });
        }
        Ok(Self(word))
    }

    pub const fn descriptor(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Gfx942CompleteBodyBlockV1<'a> {
    pub label: Gfx942CompleteBodyLabelV1,
    pub instructions: &'a [Gfx942CompleteBodyInstructionV1],
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
        error: Gfx942CompleteBodyDescriptorErrorV1,
    },
}

/// Fixed numeric data; private fields prevent unchecked construction.
///
/// Two u32 block descriptors occupy each u64, earlier block in low bits.
/// Block bits: label 0..7, count 8..12, terminator 13..14, target0 15..22,
/// target1 23..30, reserved zero bit 31. Jump=0 (target1=0), branch=1,
/// terminal=2 (both targets=0); tag 3 is forbidden. Four existing u16
/// arithmetic descriptors occupy each instruction word, earlier step low.
/// Every unused slot is zero. Numeric packing is host-endian independent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyPackedV1 {
    block_count: u8,
    instruction_count: u8,
    block_words: [u64; 4],
    instruction_words: [u64; 4],
}

impl Gfx942CompleteBodyPackedV1 {
    /// Closed, bounded grammar validation; no heap, source or execution rights.
    pub const fn from_words(
        block_count: u8,
        instruction_count: u8,
        block_words: [u64; 4],
        instruction_words: [u64; 4],
    ) -> Result<Self, Gfx942CompleteBodyPackingErrorV1> {
        use Gfx942CompleteBodyPackingErrorV1 as E;
        let blocks = block_count as usize;
        let steps = instruction_count as usize;
        if blocks == 0 || blocks > GFX942_COMPLETE_BODY_MAX_BLOCKS_V1 {
            return Err(E::BlockCount { count: blocks });
        }
        if steps == 0 || steps > GFX942_COMPLETE_BODY_MAX_STEPS_V1 {
            return Err(E::StepCount { count: steps });
        }
        let mut actual = 0_usize;
        let mut index = 0;
        while index < GFX942_COMPLETE_BODY_MAX_BLOCKS_V1 {
            let word = (block_words[index / 2] >> (32 * (index % 2))) as u32;
            if index >= blocks {
                if word != 0 {
                    return Err(E::NonZeroBlockPadding { block: index });
                }
            } else {
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
                let target0 = ((word >> 15) & 255) as u8;
                let target1 = ((word >> 23) & 255) as u8;
                match tag {
                    0 if target1 == 0 => {}
                    1 => {}
                    2 if target0 == 0 && target1 == 0 => {}
                    0 | 2 => return Err(E::NonZeroTargetPadding { block: index }),
                    _ => return Err(E::TerminatorTag { block: index, tag }),
                }
                actual += count; // at most 8 * 16, before indexing any steps
                if actual > GFX942_COMPLETE_BODY_MAX_STEPS_V1 {
                    return Err(E::StepCount { count: actual });
                }
            }
            index += 1;
        }
        if actual != steps {
            return Err(E::StepCountMismatch {
                declared: steps,
                actual,
            });
        }
        index = 0;
        while index < GFX942_COMPLETE_BODY_MAX_STEPS_V1 {
            let word = (instruction_words[index / 4] >> (16 * (index % 4))) as u16;
            if index >= steps {
                if word != 0 {
                    return Err(E::NonZeroInstructionPadding { instruction: index });
                }
            } else if let Err(error) = Gfx942CompleteBodyInstructionV1::from_descriptor(word) {
                return Err(E::Instruction {
                    instruction: index,
                    error,
                });
            }
            index += 1;
        }
        Ok(Self {
            block_count,
            instruction_count,
            block_words,
            instruction_words,
        })
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
}
