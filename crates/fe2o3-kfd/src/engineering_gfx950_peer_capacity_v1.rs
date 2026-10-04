//! Count-only preflight for setup allocations owned by one engineering group.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnerCounts {
    buffers: usize,
    next_buffer: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Counts {
    owners: Vec<OwnerCounts>,
    group_buffers: usize,
    group_next_buffer: u64,
}

fn validate_counts(counts: &Counts, additional: &[usize]) -> Result<Vec<usize>> {
    let world = counts.owners.len();
    let group_limit = group_allocation_limit(world)?;
    if additional.len() != world {
        return Err("allocation preflight requires one count per retained owner".into());
    }
    let mut total = 0_usize;
    for (owner, &extra) in counts.owners.iter().zip(additional) {
        let used = owner
            .buffers
            .checked_add(extra)
            .ok_or("owner allocation count overflow")?;
        let ids = u64::try_from(extra).map_err(|_| "owner allocation ID count overflow")?;
        if used > MAX_ALLOCATIONS || owner.next_buffer.checked_add(ids).is_none() {
            return Err("owner allocation capacity or identity exhausted".into());
        }
        total = total
            .checked_add(extra)
            .ok_or("group allocation count overflow")?;
    }
    let used = counts
        .group_buffers
        .checked_add(total)
        .ok_or("group allocation count overflow")?;
    let ids = u64::try_from(total).map_err(|_| "group allocation ID count overflow")?;
    if used > group_limit || counts.group_next_buffer.checked_add(ids).is_none() {
        return Err("group allocation capacity or identity exhausted".into());
    }
    Ok(counts.owners.iter().map(|owner| owner.buffers).collect())
}

trait CapacityBackend {
    fn require_active(&self) -> Result<()>;
    fn currentness_and_idle(&mut self) -> Result<()>;
    fn counts(&self) -> Counts;
    fn finish(&mut self, result: Result<Vec<usize>>) -> Result<Vec<usize>>;
}

fn preflight(backend: &mut impl CapacityBackend, additional: &[usize]) -> Result<Vec<usize>> {
    backend.require_active()?;
    let result = (|| {
        backend.currentness_and_idle()?;
        let counts = backend.counts();
        let occupied = validate_counts(&counts, additional)?;
        backend.currentness_and_idle()?;
        // The exclusive owner borrow must not permit accounting to drift even
        // if a later native currentness implementation performs extra work.
        if backend.counts() != counts {
            return Err("allocation preflight accounting changed".into());
        }
        Ok(occupied)
    })();
    backend.finish(result)
}

impl CapacityBackend for Gfx950EngineeringPeerGroupV1 {
    fn require_active(&self) -> Result<()> {
        Gfx950EngineeringPeerGroupV1::require_active(self)
    }

    fn currentness_and_idle(&mut self) -> Result<()> {
        check_contexts(&mut self.contexts, self.shared_full_currentness)
    }

    fn counts(&self) -> Counts {
        Counts {
            owners: self
                .contexts
                .iter()
                .map(|context| OwnerCounts {
                    buffers: context.buffers.len(),
                    next_buffer: context.next_buffer,
                })
                .collect(),
            group_buffers: self.buffers.len(),
            group_next_buffer: self.next_buffer,
        }
    }

    fn finish(&mut self, result: Result<Vec<usize>>) -> Result<Vec<usize>> {
        Gfx950EngineeringPeerGroupV1::finish(self, result)
    }
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Check additional allocation counts against this group's actual retained
    /// owner/group counts and monotonically allocated IDs. Returns the current
    /// occupied counts in owner order after currentness and idle checks.
    ///
    /// This neither allocates nor reserves resources and grants no dispatch,
    /// pointer, state-reset, mapping, or descriptor authority. A private setup
    /// owner must perform its allocations immediately without interleaving other
    /// operations; each allocation still checks its own limits and may fail.
    /// Physical memory availability is not predicted by this count-only check.
    /// Every failure follows the existing whole-group quarantine behavior.
    pub fn preflight_additional_allocations_v1(
        &mut self,
        additional_per_owner: &[usize],
    ) -> Result<Vec<usize>> {
        preflight(self, additional_per_owner)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_capacity_v1_tests.rs"]
mod tests;
