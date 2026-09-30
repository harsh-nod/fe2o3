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

/// One exact writer segment in a packed flat group roster. Segments cover the
/// member slice in order, without gaps or overlap. Empty segments are legal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextQueuedWriterGroupHeaderV1 {
    pub writer: ContextWriterReferenceV1,
    pub member_start: usize,
    pub member_count: usize,
}

/// Flat equivalent of the nested inert disposal premise. Members concatenate
/// complete writer rosters and may repeat destinations across writers; only
/// allocations is the unique union. Adapters can retain the backing vectors
/// before native effects and borrow this view without allocating at commit.
#[derive(Debug)]
pub struct ContextQueuedWriterFlatGroupDisposalEvidenceV1<'a> {
    pub writers: &'a [ContextQueuedWriterGroupHeaderV1],
    pub members: &'a [ContextAllocationWriteV1],
    pub allocations: &'a [ContextAllocationWriteV1],
}

#[derive(Clone, Copy)]
enum GroupView<'a> {
    Nested(&'a [ContextWriterDisposalEvidenceV1<'a>]),
    Flat(
        &'a [ContextQueuedWriterGroupHeaderV1],
        &'a [ContextAllocationWriteV1],
    ),
}

impl<'a> GroupView<'a> {
    fn len(self) -> usize {
        match self {
            Self::Nested(writers) => writers.len(),
            Self::Flat(writers, _) => writers.len(),
        }
    }

    fn writer(self, index: usize) -> ContextWriterReferenceV1 {
        match self {
            Self::Nested(writers) => writers[index].writer,
            Self::Flat(writers, _) => writers[index].writer,
        }
    }

    fn entry(self, index: usize) -> Result<ContextWriterDisposalEvidenceV1<'a>, Error> {
        let allocations = match self {
            Self::Nested(writers) => writers[index].allocations,
            Self::Flat(writers, members) => {
                let header = writers[index];
                let end = header
                    .member_start
                    .checked_add(header.member_count)
                    .ok_or(Error::SettlementEvidenceMismatch)?;
                members
                    .get(header.member_start..end)
                    .ok_or(Error::SettlementEvidenceMismatch)?
            }
        };
        Ok(ContextWriterDisposalEvidenceV1 {
            writer: self.writer(index),
            allocations,
        })
    }

    fn validate_layout(self, capacity: usize) -> Result<(), Error> {
        if let Self::Flat(writers, members) = self {
            if members.len() > capacity {
                return Err(Error::RosterCapacity);
            }
            let mut end = 0;
            for header in writers {
                if header.member_start != end {
                    return Err(Error::SettlementEvidenceMismatch);
                }
                end = end
                    .checked_add(header.member_count)
                    .ok_or(Error::SettlementEvidenceMismatch)?;
                if end > members.len() {
                    return Err(Error::SettlementEvidenceMismatch);
                }
            }
            if end != members.len() {
                return Err(Error::SettlementEvidenceMismatch);
            }
        }
        Ok(())
    }

    fn find(self, local: u64) -> Option<usize> {
        match self {
            Self::Nested(writers) => writers
                .binary_search_by_key(&local, |entry| entry.writer.key.local)
                .ok(),
            Self::Flat(writers, _) => writers
                .binary_search_by_key(&local, |entry| entry.writer.key.local)
                .ok(),
        }
    }
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
        queued_ensure_usable_body_v1!(self)
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
        self.validate_group(GroupView::Nested(writers), allocations)
    }

    pub fn validate_flat_unknown_group_disposal(
        &self,
        evidence: &ContextQueuedWriterFlatGroupDisposalEvidenceV1<'_>,
    ) -> Result<(), Error> {
        self.validate_group(
            GroupView::Flat(evidence.writers, evidence.members),
            evidence.allocations,
        )
    }

    fn validate_group(
        &self,
        writers: GroupView<'_>,
        allocations: &[ContextAllocationWriteV1],
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        if writers.len() == 0
            || writers.len() > self.writer_capacity()
            || allocations.len() > self.allocation_capacity()
        {
            return Err(Error::RosterCapacity);
        }
        writers.validate_layout(self.member_capacity())?;
        let mut previous_writer = None;
        let mut total = 0usize;
        for index in 0..writers.len() {
            let entry = writers.entry(index)?;
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
                .find(key.local)
                .is_some_and(|index| writers.writer(index).key == key)
        };
        // A selected submission must release inputs outside the destination union
        // too. Destination reader counts alone cannot establish that condition.
        if self.inner.has_consumer_reads_matching_v1(selected)
            || self.has_queued_consumer_reads_matching(selected)
        {
            return Err(Error::AllocationBusy);
        }
        let mut previous_allocation = None;
        for &write in allocations {
            if previous_allocation.is_some_and(|key| key >= write.allocation.key) {
                return Err(Error::NonCanonicalRoster);
            }
            let state = self.destination(write)?;
            if self.reader_count(write.allocation)? != 0 {
                return Err(Error::AllocationBusy);
            }
            if state.pending_writer.is_none() {
                self.inner
                    .validate_allocation_retirement(&[write.allocation])?;
            }
            previous_allocation = Some(write.allocation.key);
        }
        for index in 0..writers.len() {
            let entry = writers.entry(index)?;
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
                    .find(member.writer.key.local)
                    .ok_or(Error::SettlementEvidenceMismatch)?;
                if writers.writer(selected) != member.writer {
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
        self.dispose_group(GroupView::Nested(evidence.writers), evidence.allocations)
    }

    /// Allocation-free flat entry point with the same preflight and terminal
    /// committed-prefix semantics as `dispose_unknown_group`.
    pub fn dispose_flat_unknown_group(
        &mut self,
        evidence: &ContextQueuedWriterFlatGroupDisposalEvidenceV1<'_>,
    ) -> Result<(), ContextQueuedWriterGroupDisposalErrorV1> {
        self.dispose_group(
            GroupView::Flat(evidence.writers, evidence.members),
            evidence.allocations,
        )
    }

    fn dispose_group(
        &mut self,
        writers: GroupView<'_>,
        allocations: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextQueuedWriterGroupDisposalErrorV1> {
        use ContextQueuedWriterGroupDisposalErrorV1 as Failure;
        if self.disposal_terminal {
            return Err(Failure::Terminal(Error::InvalidState));
        }
        self.validate_group(writers, allocations)
            .map_err(Failure::Rejected)?;
        self.disposal_terminal = true;
        self.commit_unknown_group(writers, allocations)
            .map_err(Failure::Terminal)?;
        self.disposal_terminal = false;
        Ok(())
    }

    fn commit_unknown_group(
        &mut self,
        writers: GroupView<'_>,
        allocations: &[ContextAllocationWriteV1],
    ) -> Result<(), Error> {
        // Retire queued-only destinations before active-root disposal changes any
        // allocation lookup. Outer custody is deliberately retained until the end.
        for write in allocations {
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
        for index in 0..writers.len() {
            let entry = writers.entry(index)?;
            let root = self.roots[entry.writer.slot].ok_or(Error::InvalidState)?;
            #[cfg(test)]
            self.inject_disposal_fault()?;
            match root.phase {
                Phase::Active => self.inner.dispose_unknown(entry.writer, &entry)?,
                // This burns a Reserved identity as destruction bookkeeping, not
                // as proof that an accepted operation had no effects.
                Phase::Unknown => self.inner.abort_reserved(entry.writer)?,
                Phase::Queued => return Err(Error::InvalidState),
            }
        }
        #[cfg(test)]
        self.inject_disposal_fault()?;
        for index in 0..writers.len() {
            let entry = writers.entry(index)?;
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
        for write in allocations {
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
