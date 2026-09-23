//! Append-only const data builder. No CFG or definite-assignment checking.
use super::{
    GFX942_COMPLETE_BODY_MAX_BLOCKS_V1, GFX942_COMPLETE_BODY_MAX_STEPS_V1,
    Gfx942CompleteBodyBlockV1, Gfx942CompleteBodyPackedV1, Gfx942CompleteBodyPackingErrorV1,
    Gfx942CompleteBodyTerminatorV1,
};

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

    /// All fallible checks precede writes; any error leaves this builder intact.
    /// Instructions are already closed descriptor wrappers, not arbitrary text.
    /// Empty blocks are allowed; finish requires at least one step in total.
    pub const fn push_block(
        &mut self,
        block: Gfx942CompleteBodyBlockV1<'_>,
    ) -> Result<(), Gfx942CompleteBodyPackingErrorV1> {
        use Gfx942CompleteBodyPackingErrorV1 as E;
        let index = self.block_count as usize;
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
        let first = self.instruction_count as usize;
        let total = first + count; // each operand <= 16
        if total > GFX942_COMPLETE_BODY_MAX_STEPS_V1 {
            return Err(E::StepCount { count: total });
        }
        let (tag, target0, target1) = match block.terminator {
            Gfx942CompleteBodyTerminatorV1::Jump(target) => (0_u32, target.0 as u32, 0),
            Gfx942CompleteBodyTerminatorV1::BranchSelectorZero { zero, nonzero } => {
                (1, zero.0 as u32, nonzero.0 as u32)
            }
            Gfx942CompleteBodyTerminatorV1::GuardedStoreOutputAndEnd => (2, 0, 0),
        };
        let descriptor = block.label.0 as u32
            | ((count as u32) << 8)
            | (tag << 13)
            | (target0 << 15)
            | (target1 << 23);
        self.block_words[index / 2] |= (descriptor as u64) << (32 * (index % 2));
        let mut offset = 0;
        while offset < count {
            let ordinal = first + offset;
            self.instruction_words[ordinal / 4] |=
                (block.instructions[offset].descriptor() as u64) << (16 * (ordinal % 4));
            offset += 1;
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
    pub const fn finish(
        self,
    ) -> Result<Gfx942CompleteBodyPackedV1, Gfx942CompleteBodyPackingErrorV1> {
        Gfx942CompleteBodyPackedV1::from_words(
            self.block_count,
            self.instruction_count,
            self.block_words,
            self.instruction_words,
        )
    }
}
