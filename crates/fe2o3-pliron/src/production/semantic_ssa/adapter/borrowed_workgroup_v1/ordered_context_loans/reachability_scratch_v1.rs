//! Reusable DFS storage only; no retained connectivity or proof result.
use super::*;

pub(super) struct PathScratch {
    // A byte replaces each old bool, preserving the visited-buffer byte size.
    seen: Vec<u8>,
    pending: Vec<u32>,
    generation: u8,
}

impl PathScratch {
    pub(super) fn new(blocks: usize, budget: &mut Budget) -> Result<Self> {
        budget.charge(blocks.saturating_mul(2))?;
        Ok(Self {
            seen: vec![0; blocks],
            pending: Vec::with_capacity(blocks),
            generation: 0,
        })
    }

    pub(super) fn path(
        &mut self,
        edges: &[Vec<u32>],
        start: u32,
        end: u32,
        budget: &mut Budget,
    ) -> Result<bool> {
        budget.charge(1)?;
        if edges.len() != self.seen.len()
            || start as usize >= edges.len()
            || end as usize >= edges.len()
        {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        if let Some(next) = self.generation.checked_add(1) {
            self.generation = next;
        } else {
            // Reusing a generation without clearing would retain stale visits.
            // A failed reset charge cannot publish that generation or a result.
            budget.charge(self.seen.len())?;
            self.seen.fill(0);
            self.generation = 1;
        }
        self.pending.clear();
        self.seen[start as usize] = self.generation;
        self.pending.push(start);
        while let Some(current) = self.pending.pop() {
            budget.charge(1)?;
            for &next in &edges[current as usize] {
                budget.charge(1)?;
                // Check before the visited mark: start == end needs a real cycle.
                if next == end {
                    return Ok(true);
                }
                if self.seen[next as usize] != self.generation {
                    self.seen[next as usize] = self.generation;
                    self.pending.push(next);
                }
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
#[path = "reachability_scratch_v1/tests.rs"]
mod tests;
