//! Sealed completed batches may be rechecked after other layers use the queues.
//! This proves owner quiescence, NOT permission to reset or free old arenas.
use super::*;

fn retired_gate(
    values: Values,
    sealed: [(u64, u64); 2],
    previous: [(u64, u64); 2],
    writes: [u64; 2],
    completed: [u64; 2],
    previous_reads: [u64; 2],
    observed: [(u64, u64); 2],
) -> Result<()> {
    if values != [[0; PACKETS]; 2] {
        return Err("retired paired signals are not all completed".into());
    }
    for rank in 0..2 {
        if sealed[rank].0 < PACKETS as u64
            || sealed[rank].1 > sealed[rank].0
            || previous[rank].0 < sealed[rank].0
            || previous[rank].1 < sealed[rank].1
            || previous[rank].1 > previous[rank].0
            || writes[rank] < previous[rank].0
            || previous_reads[rank] < previous[rank].1
        {
            return Err("retired paired queue frontier regressed".into());
        }
        require_completed_frontier(completed[rank], writes[rank])?;
        validate_counters(writes[rank], previous_reads[rank], observed[rank])?;
    }
    Ok(())
}

/// Actual completed arena custody, never constructible from a Completion snapshot.
pub(in super::super) struct Retired {
    staged: Staged,
    sealed: [(u64, u64); 2],
    previous: [(u64, u64); 2],
}

impl Staged {
    /// The caller has already completed both typed owners through Native::finish.
    pub(in super::super) fn seal_retired(
        mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        deadline: Instant,
    ) -> Result<Retired> {
        group.require_active()?;
        profiles::validate_policy(group)?;
        self.require_retired()?;
        check_contexts(&mut group.contexts, group.shared_full_currentness)?;
        // Sealing still requires the exact original reservation. Only the sealed
        // recheck below can accept intervening, independently retired work.
        if !self.complete(group, deadline)? {
            return Err("retired paired seal requires all ten completed signals".into());
        }
        let (_, sealed) = self
            .last
            .ok_or("retired paired seal missing observations")?;
        let mut proof = Retired {
            staged: self,
            sealed,
            previous: sealed,
        };
        proof.recheck(group, deadline)?;
        Ok(proof)
    }
}

impl Retired {
    pub(in super::super) fn identities(&self) -> [(u64, u64); 2] {
        self.staged
            .arenas
            .each_ref()
            .map(|a| (a.token.group, a.token.id))
    }

    pub(in super::super) fn recheck(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        deadline: Instant,
    ) -> Result<()> {
        deadline_check(Instant::now(), deadline)?;
        group.require_active()?;
        profiles::validate_policy(group)?;
        self.staged.require_retired()?;
        self.staged.check(group)?;
        check_contexts(&mut group.contexts, group.shared_full_currentness)?;
        let writes = std::array::from_fn(|rank| group.contexts[rank].ring.write());
        let completed = std::array::from_fn(|rank| group.contexts[rank].completed_write);
        let previous_reads = std::array::from_fn(|rank| group.contexts[rank].last_observed_read);
        let mut observed = [(0, 0); 2];
        for rank in 0..2 {
            let context = &mut group.contexts[rank];
            observed[rank] =
                Backend::observe_aql_counters(&mut context.internal[CONTROL].mapping, PAGE_BYTES)
                    .map_err(explain)?;
            if Backend::observe_i64_acquire(&mut context.internal[CONTROL].mapping, PAGE_BYTES, 256)
                .map_err(explain)?
                != 0
            {
                return Err("retired paired queue exception".into());
            }
            deadline_check(Instant::now(), deadline)?;
        }
        let values = self.staged.read_signals(group)?;
        retired_gate(
            values,
            self.sealed,
            self.previous,
            writes,
            completed,
            previous_reads,
            observed,
        )?;
        for rank in 0..2 {
            // Keep only actual observations. Completed signals never create read credit.
            group.contexts[rank].last_observed_read = observed[rank].1;
        }
        check_contexts(&mut group.contexts, group.shared_full_currentness)?;
        self.staged.check(group)?;
        deadline_check(Instant::now(), deadline)?;
        self.previous = observed;
        Ok(())
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_combined_mlp_paired_retired_v1_tests.rs"]
mod tests;
