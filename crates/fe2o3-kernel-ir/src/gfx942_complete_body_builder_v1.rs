//! Bounded append-only packing builder; grammar only, never CFG admission.
use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Gfx942CompleteBodyBuilderV1 {
    block_count: u8,
    instruction_count: u8,
    block_words: [u64; 4],
    instruction_words: [u64; 4],
}

impl Gfx942CompleteBodyBuilderV1 {
    pub const fn new() -> Self {
        Self {
            block_count: 0,
            instruction_count: 0,
            block_words: [0; 4],
            instruction_words: [0; 4],
        }
    }

    /// Appends only after all count checks succeed. On error this builder is
    /// byte-for-byte unchanged; labels/edges/definitions are deliberately not
    /// validated here. Duplicate/invalid CFG intent still packs as inert data.
    pub fn push_block(
        &mut self,
        block: Gfx942CompleteBodyBlockV1<'_>,
    ) -> Result<(), Gfx942CompleteBodyPackingErrorV1> {
        use Gfx942CompleteBodyPackingErrorV1 as E;
        let index = usize::from(self.block_count);
        if index == GFX942_COMPLETE_BODY_MAX_BLOCKS_V1 {
            return Err(E::BlockCount { count: index + 1 });
        }
        let count = block.instructions.len();
        if count > GFX942_COMPLETE_BODY_MAX_STEPS_V1 {
            return Err(E::BlockStepCount {
                block: index,
                count,
            });
        }
        let first = usize::from(self.instruction_count);
        let total = first + count; // each operand <= 16
        if total > GFX942_COMPLETE_BODY_MAX_STEPS_V1 {
            return Err(E::StepCount { count: total });
        }
        let (tag, target0, target1) = match block.terminator {
            Gfx942CompleteBodyTerminatorV1::Jump(target) => (0_u32, u32::from(target.0), 0),
            Gfx942CompleteBodyTerminatorV1::BranchSelectorZero { zero, nonzero } => {
                (1, u32::from(zero.0), u32::from(nonzero.0))
            }
            Gfx942CompleteBodyTerminatorV1::GuardedStoreOutputAndEnd => (2, 0, 0),
        };
        let descriptor = u32::from(block.label.0)
            | ((count as u32) << 8)
            | (tag << 13)
            | (target0 << 15)
            | (target1 << 23);
        self.block_words[index / 2] |= u64::from(descriptor) << (32 * (index % 2));
        for (offset, instruction) in block.instructions.iter().enumerate() {
            let ordinal = first + offset;
            self.instruction_words[ordinal / 4] |=
                u64::from(instruction.descriptor()) << (16 * (ordinal % 4));
        }
        self.block_count += 1;
        self.instruction_count = total as u8;
        Ok(())
    }

    pub const fn block_count(&self) -> usize {
        self.block_count as usize
    }
    pub const fn instruction_count(&self) -> usize {
        self.instruction_count as usize
    }

    /// Requires 1..=8 blocks and 1..=16 total steps; empty individual blocks
    /// remain representable. This does not establish a legal complete body.
    pub fn finish(self) -> Result<Gfx942CompleteBodyPackedV1, Gfx942CompleteBodyPackingErrorV1> {
        Gfx942CompleteBodyPackedV1::from_words(
            self.block_count,
            self.instruction_count,
            self.block_words,
            self.instruction_words,
        )
    }
}
