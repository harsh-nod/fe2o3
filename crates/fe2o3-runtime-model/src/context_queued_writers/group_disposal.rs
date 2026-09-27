//! Closed Unknown-group destruction, never successful content settlement.

use super::*;

/// Inert premise that every writer is quiescent and every unique allocation in
/// this union has been physically disposed. The runtime must authenticate those
/// facts separately. Per-writer rosters may overlap; the union must not duplicate
/// allocations. Writers are strictly ordered by local ID, allocations by key.
#[derive(Debug)]
pub struct ContextQueuedWriterGroupDisposalEvidenceV1<'a> {
    pub writers: &'a [ContextWriterDisposalEvidenceV1<'a>],
    pub allocations: &'a [ContextAllocationWriteV1],
}

/// Rejected means no semantic mutation. Terminal means the owner must never be
/// reused: a prefix may have committed. Unwind also leaves that terminal flag set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextQueuedWriterGroupDisposalErrorV1 {
    Rejected(Error),
    Terminal(Error),
}

impl ContextQueuedWriterJournalV1 {
    /// Visit direct co-owners for bounded component discovery by an adapter.
    /// References are descriptive, not quiescence or disposal authority. The
    /// complete local roster and neighbor identities are checked before the first
    /// callback. Duplicate neighbors are possible across destinations; the caller
    /// must deduplicate visited writers and validate the final closed group.
    /// Owner work is O(k), excluding callback work, with no owner allocation.
    pub fn visit_writer_neighbors(
        &self,
        writer: ContextWriterReferenceV1,
        mut visit: impl FnMut(ContextWriterReferenceV1),
    ) -> Result<(), Error> {
        let root = self.validate_root(writer)?;
        let mut next = root.head;
        for _ in 0..root.count {
            let member = self.member(next.ok_or(Error::InvalidState)?)?;
            for index in [member.previous_allocation, member.next_allocation]
                .into_iter()
                .flatten()
            {
                self.root(self.member(index)?.writer)?;
            }
            next = member.next_writer;
        }
        let mut next = root.head;
        for _ in 0..root.count {
            let member = self.member(next.ok_or(Error::InvalidState)?)?;
            for index in [member.previous_allocation, member.next_allocation]
                .into_iter()
                .flatten()
            {
                visit(self.member(index)?.writer);
            }
            next = member.next_writer;
        }
        Ok(())
    }

    pub fn disposal_is_terminal(&self) -> bool {
        self.disposal_terminal
    }

    pub(super) fn ensure_usable(&self) -> Result<(), Error> {
        if self.disposal_terminal {
            Err(Error::InvalidState)
        } else {
            Ok(())
        }
    }

    /// Preflights exact bipartite closure without assuming any native effect has
    /// occurred. Unknown roots with no destinations are legal. A closed union of
    /// disconnected components is also legal; unowned allocations are not.
    ///
    /// Cost is O((M + R) log(W + 1) + M log(A + 1) + W + A), where M/W/A are
    /// group members/writers/allocations and R is the configured read arena size.
    /// No allocation occurs. As elsewhere, partition preservation assumes a valid
    /// prestate; this is not a global audit of unrelated owner storage.
    pub fn validate_unknown_group_disposal(
        &self,
        writers: &[ContextWriterDisposalEvidenceV1<'_>],
        allocations: &[ContextAllocationWriteV1],
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        if writers.is_empty()
            || writers.len() > self.writer_capacity()
            || allocations.len() > self.allocation_capacity()
        {
            return Err(Error::RosterCapacity);
        }
        let mut previous_writer = None;
        let mut total = 0usize;
        for entry in writers {
            if previous_writer.is_some_and(|local| local >= entry.writer.key.local) {
                return Err(Error::NonCanonicalRoster);
            }
            let root = self.validate_root(entry.writer)?;
            match root.phase {
                Phase::Active => {
                    self.inner
                        .validate_unknown_disposal(entry.writer, entry.allocations)?;
                }
                Phase::Unknown => {}
                Phase::Queued => return Err(Error::InvalidState),
            }
            if root.count != entry.allocations.len() {
                return Err(Error::SettlementEvidenceMismatch);
            }
            total = total.checked_add(root.count).ok_or(Error::InvalidState)?;
            if total > self.members.len() {
                return Err(Error::InvalidState);
            }
            previous_writer = Some(entry.writer.key.local);
        }
        let returns = self
            .free
            .len()
            .checked_add(total)
            .ok_or(Error::InvalidState)?;
        if returns > self.members.len()
            || returns > self.free.capacity()
            || self
                .inner
                .remaining_writer_slots()
                .checked_add(writers.len())
                .is_none_or(|count| count > self.writer_capacity())
            || self
                .inner
                .remaining_allocation_slots()
                .checked_add(allocations.len())
                .is_none_or(|count| count > self.allocation_capacity())
        {
            return Err(Error::InvalidState);
        }
        let selected = |key: ContextWriterKeyV1| {
            writers
                .binary_search_by_key(&key.local, |entry| entry.writer.key.local)
                .is_ok_and(|index| writers[index].writer.key == key)
        };
        // A selected submission must release inputs outside the destination union
        // too. Destination reader counts alone cannot establish that condition.
        if self.inner.has_consumer_reads_matching_v1(selected) {
            return Err(Error::AllocationBusy);
        }
        let mut previous_allocation = None;
        for &write in allocations {
            if previous_allocation.is_some_and(|key| key >= write.allocation.key) {
                return Err(Error::NonCanonicalRoster);
            }
            let state = self.destination(write)?;
            if self.inner.reader_count(write.allocation)? != 0 {
                return Err(Error::AllocationBusy);
            }
            if state.pending_writer.is_none() {
                self.inner
                    .validate_allocation_retirement(&[write.allocation])?;
            }
            previous_allocation = Some(write.allocation.key);
        }
        for entry in writers {
            let root = self.root(entry.writer)?;
            let mut next = root.head;
            for write in entry.allocations {
                let member = self.member(next.ok_or(Error::InvalidState)?)?;
                if member.request.destination != *write {
                    return Err(Error::SettlementEvidenceMismatch);
                }
                let index = allocations
                    .binary_search_by_key(&write.allocation.key, |w| w.allocation.key)
                    .map_err(|_| Error::SettlementEvidenceMismatch)?;
                if allocations[index] != *write {
                    return Err(Error::SettlementEvidenceMismatch);
                }
                next = member.next_writer;
            }
        }
        let mut traversed = 0usize;
        for &write in allocations {
            let slot = write.allocation.slot;
            let mut next = self.heads[slot];
            if next.is_none() {
                return Err(Error::SettlementEvidenceMismatch);
            }
            let mut previous = None;
            let mut queued = 0usize;
            let mut active = None;
            while let Some(index) = next {
                if traversed >= total {
                    return Err(Error::InvalidState);
                }
                let member = self.member(index)?;
                self.validate_links(index, member)?;
                if member.request.destination != write || member.previous_allocation != previous {
                    return Err(Error::InvalidState);
                }
                let selected = writers
                    .binary_search_by_key(&member.writer.key.local, |entry| entry.writer.key.local)
                    .map_err(|_| Error::SettlementEvidenceMismatch)?;
                if writers[selected].writer != member.writer {
                    return Err(Error::InvalidReference);
                }
                match self.root(member.writer)?.phase {
                    Phase::Active if previous.is_none() => active = Some(member.writer),
                    Phase::Unknown => queued += 1,
                    _ => return Err(Error::InvalidState),
                }
                traversed += 1;
                previous = Some(index);
                next = member.next_allocation;
            }
            if self.tails[slot] != previous
                || self.queued_counts[slot] != queued
                || self
                    .inner
                    .lookup_allocation(write.allocation)?
                    .pending_writer
                    != active
            {
                return Err(Error::InvalidState);
            }
        }
        if traversed != total {
            return Err(Error::SettlementEvidenceMismatch);
        }
        Ok(())
    }

    /// Destroy only a fully disposed closed group. No callbacks or allocations
    /// occur here. Each inner subtransition is atomic, but their composition is
    /// not: any later error or unwind permanently disables ordinary owner use.
    /// Outer roots remain until all inner destruction has completed. The adapter
    /// must retain its own original complete custody through a terminal failure.
    pub fn dispose_unknown_group(
        &mut self,
        evidence: &ContextQueuedWriterGroupDisposalEvidenceV1<'_>,
    ) -> Result<(), ContextQueuedWriterGroupDisposalErrorV1> {
        use ContextQueuedWriterGroupDisposalErrorV1 as Failure;
        if self.disposal_terminal {
            return Err(Failure::Terminal(Error::InvalidState));
        }
        self.validate_unknown_group_disposal(evidence.writers, evidence.allocations)
            .map_err(Failure::Rejected)?;
        self.disposal_terminal = true;
        self.commit_unknown_group(evidence)
            .map_err(Failure::Terminal)?;
        self.disposal_terminal = false;
        Ok(())
    }

    fn commit_unknown_group(
        &mut self,
        evidence: &ContextQueuedWriterGroupDisposalEvidenceV1<'_>,
    ) -> Result<(), Error> {
        // Retire queued-only destinations before active-root disposal changes any
        // allocation lookup. Outer custody is deliberately retained until the end.
        for write in evidence.allocations {
            if self
                .inner
                .lookup_allocation(write.allocation)?
                .pending_writer
                .is_none()
            {
                #[cfg(test)]
                self.inject_disposal_fault()?;
                self.inner.retire_allocations(&[write.allocation])?;
            }
        }
        for entry in evidence.writers {
            let root = self.roots[entry.writer.slot].ok_or(Error::InvalidState)?;
            #[cfg(test)]
            self.inject_disposal_fault()?;
            match root.phase {
                Phase::Active => self.inner.dispose_unknown(entry.writer, entry)?,
                // This burns a Reserved identity as destruction bookkeeping, not
                // as proof that an accepted operation had no effects.
                Phase::Unknown => self.inner.abort_reserved(entry.writer)?,
                Phase::Queued => return Err(Error::InvalidState),
            }
        }
        #[cfg(test)]
        self.inject_disposal_fault()?;
        for entry in evidence.writers {
            let root = self.roots[entry.writer.slot].ok_or(Error::InvalidState)?;
            let mut next = root.head;
            for _ in 0..root.count {
                let index = next.ok_or(Error::InvalidState)?;
                let member = self.members[index].take().ok_or(Error::InvalidState)?;
                self.free.push(index);
                next = member.next_writer;
            }
            self.roots[entry.writer.slot] = None;
        }
        for write in evidence.allocations {
            let slot = write.allocation.slot;
            self.heads[slot] = None;
            self.tails[slot] = None;
            self.queued_counts[slot] = 0;
        }
        Ok(())
    }

    #[cfg(test)]
    fn inject_disposal_fault(&mut self) -> Result<(), Error> {
        if let Some((remaining, panic)) = &mut self.disposal_fault {
            if *remaining == 0 {
                assert!(!*panic, "injected disposal unwind");
                return Err(Error::InvalidState);
            }
            *remaining -= 1;
        }
        Ok(())
    }
}
