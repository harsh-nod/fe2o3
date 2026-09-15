//! Release consumed single-edge inputs without changing the move lattice.

use super::state::{Budget, Error, State, Storage};

type Result<T> = std::result::Result<T, Error>;

pub(super) struct InputRetention {
    incoming_edges: Box<[u8]>,
    _storage: Storage,
}

impl InputRetention {
    pub(super) fn new(
        blocks: usize,
        entry: usize,
        budget: &Budget,
        visit_edges: impl FnOnce(&mut dyn FnMut(usize, usize) -> Result<()>) -> Result<()>,
    ) -> Result<Self> {
        if entry >= blocks {
            return Err(Error::Overflow);
        }
        // Counts saturate at two: parallel edges, including call return/unwind,
        // are distinct inputs even when they share their source block.
        let words = blocks
            .div_ceil(size_of::<usize>())
            .checked_add(4)
            .ok_or(Error::Overflow)?;
        let storage = budget.reserve(words)?;
        budget.work(blocks)?;
        let mut incoming_edges = vec![0u8; blocks].into_boxed_slice();
        visit_edges(&mut |source, target| {
            budget.work(1)?;
            if source >= blocks || target >= blocks {
                return Err(Error::Overflow);
            }
            incoming_edges[target] = incoming_edges[target].saturating_add(1).min(2);
            Ok(())
        })?;
        // External entry is an additional input, including when a backedge is
        // its only CFG predecessor. Retaining it also breaks every root cycle.
        incoming_edges[entry] = 2;
        Ok(Self {
            incoming_edges,
            _storage: storage,
        })
    }

    pub(super) fn begin(&self, block: usize, incoming: &mut [Option<State>]) -> Option<State> {
        // Fixed successful transfers are monotone. A sole predecessor sends
        // its complete current output, so its old input need not be retained.
        // Entry and joins retain history; every reachable cycle crosses one.
        if self.incoming_edges[block] == 1 {
            incoming[block].take()
        } else {
            incoming[block].clone()
        }
    }
}

#[cfg(test)]
#[path = "incoming_retention_v1/tests.rs"]
mod tests;
