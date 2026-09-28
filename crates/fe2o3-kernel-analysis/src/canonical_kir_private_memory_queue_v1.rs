//! The existing fixed-capacity paid queue, shared by physical and source CFGs.
use super::*;

/// Opaque worklist only, never a physical or source proof.
#[doc(hidden)]
pub struct CanonicalKirPrivateDataflowQueueV1 {
    pub(super) rows: Vec<usize>,
    pub(super) queued: Vec<bool>,
    pub(super) head: usize,
    pub(super) length: usize,
}

impl CanonicalKirPrivateDataflowQueueV1 {
    /// Legacy/source-CFG adapter. The caller's enclosing scratch transaction
    /// retains credit until this queue drops, then releases its entire delta.
    #[doc(hidden)]
    pub fn new_retaining_scratch_v1(blocks: usize, budget: &mut Budget<'_>) -> R<Self> {
        Self::new(blocks, budget)
    }

    pub(super) fn new(blocks: usize, budget: &mut Budget<'_>) -> R<Self> {
        charge(budget, 2)?;
        budget
            .reserve_storage(
                std::mem::size_of::<usize>()
                    .checked_mul(2)
                    .ok_or_else(arithmetic)?,
            )
            .map_err(Error::Resource)?;
        let mut rows = scratch::<usize>(blocks, budget)?;
        let mut queued = scratch::<bool>(blocks, budget)?;
        charge(budget, blocks.checked_mul(2).ok_or_else(arithmetic)?)?;
        rows.resize(blocks, 0);
        queued.resize(blocks, false);
        Ok(Self {
            rows,
            queued,
            head: 0,
            length: 0,
        })
    }

    pub fn reset(&mut self, budget: &mut Budget<'_>) -> R<()> {
        charge(
            budget,
            self.queued.len().checked_add(2).ok_or_else(arithmetic)?,
        )?;
        self.queued.fill(false);
        self.head = 0;
        self.length = 0;
        Ok(())
    }

    pub fn push(&mut self, block: usize, budget: &mut Budget<'_>) -> R<()> {
        charge(budget, 5)?;
        if block >= self.rows.len() {
            return Err(arithmetic());
        }
        if self.queued[block] {
            return Ok(());
        }
        if self.length == self.rows.len() {
            return Err(arithmetic());
        }
        let tail = self.head.checked_add(self.length).ok_or_else(arithmetic)? % self.rows.len();
        self.rows[tail] = block;
        self.queued[block] = true;
        self.length = self.length.checked_add(1).ok_or_else(arithmetic)?;
        Ok(())
    }

    pub fn pop(&mut self, budget: &mut Budget<'_>) -> R<Option<usize>> {
        charge(budget, 4)?;
        if self.length == 0 {
            return Ok(None);
        }
        let block = self.rows[self.head];
        self.head = self.head.checked_add(1).ok_or_else(arithmetic)? % self.rows.len();
        self.length -= 1;
        self.queued[block] = false;
        Ok(Some(block))
    }
}
