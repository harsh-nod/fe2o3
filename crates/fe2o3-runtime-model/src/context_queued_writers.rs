//! Exclusive queued destinations over the unchanged single-writer journal.
//! All completion and dependency values are inert model premises, not authority.

use crate::context_producer_reads::*;
use crate::context_read_leases::*;
use crate::context_version_journal::*;
use alloc::vec::Vec;

mod forward;
mod group_disposal;
mod reads;
pub use group_disposal::{
    ContextQueuedWriterFlatGroupDisposalEvidenceV1, ContextQueuedWriterGroupDisposalErrorV1,
    ContextQueuedWriterGroupDisposalEvidenceV1, ContextQueuedWriterGroupHeaderV1,
};
pub use reads::{ContextQueuedProducerReadReferenceV1, ContextQueuedProducerReadV1};
#[cfg(test)]
mod tests;

type Error = ContextVersionJournalErrorV1;

/// One whole-allocation successor binding. None reserves an idle destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextQueuedWriteV1 {
    pub destination: ContextAllocationWriteV1,
    pub predecessor: Option<ContextWriterReferenceV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextQueuedWriterStatusV1 {
    Waiting,
    Ready,
    Blocked,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Prerequisite {
    Pending,
    Success { epoch: u64, lineage: u64 },
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Active,
    Queued,
    Unknown,
}

#[derive(Clone, Copy, Debug)]
struct Root {
    writer: ContextWriterReferenceV1,
    head: Option<usize>,
    count: usize,
    phase: Phase,
    read_head: Option<usize>,
    read_count: usize,
}

#[derive(Clone, Copy, Debug)]
struct Member {
    writer: ContextWriterReferenceV1,
    request: ContextQueuedWriteV1,
    prerequisite: Prerequisite,
    next_writer: Option<usize>,
    previous_allocation: Option<usize>,
    next_allocation: Option<usize>,
}

/// Bounded exclusive writer queues. No mutable or immutable inner projection is
/// exposed: even read-only availability queries must respect queued destinations.
/// Construction reserves O(A + W + R + M) metadata; M bounds all active/queued
/// destination records. Writer admission and activation are O(k); settlement is
/// O(k + r) for its k destinations and r attached queued reads. There is no heap
/// allocation after construction. Cancellation unlinks only its own roster
/// and marks immediate descendants failed without reparenting them.
///
/// Queued writers remain Reserved in the inner journal until activation. Their
/// registered identity is burned on cancellation, but no attempt epoch is burned
/// before activation. Successful logical activation is separate from physical
/// execution; an adapter must authenticate all dependencies and outcome premises.
/// This owner has no formal-refinement or device-ordering qualification yet.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{ContextQueuedWriterJournalV1, ContextProducerReadJournalV1};
/// let mut owner = ContextQueuedWriterJournalV1::new(1, 4, 4, 4, 16).unwrap();
/// let bypass: &mut ContextProducerReadJournalV1 = &mut *owner;
/// ```
/// ```compile_fail
/// use fe2o3_runtime_model::{ContextQueuedWriterJournalV1, ContextProducerReadJournalV1};
/// let owner = ContextQueuedWriterJournalV1::new(1, 4, 4, 4, 16).unwrap();
/// let bypass: &ContextProducerReadJournalV1 = &*owner;
/// ```
#[derive(Debug)]
pub struct ContextQueuedWriterJournalV1 {
    inner: ContextProducerReadJournalV1,
    roots: Vec<Option<Root>>,
    members: Vec<Option<Member>>,
    free: Vec<usize>,
    heads: Vec<Option<usize>>,
    tails: Vec<Option<usize>>,
    queued_counts: Vec<usize>,
    scratch: Vec<ContextAllocationWriteV1>,
    queued_reads: Vec<Option<reads::Reservation>>,
    free_reads: Vec<usize>,
    read_counts: Vec<usize>,
    next_read_incarnation: u64,
    read_producer_scratch: Vec<ContextWriterReferenceV1>,
    disposal_terminal: bool,
    #[cfg(test)]
    disposal_fault: Option<(usize, bool)>,
}

fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>, Error> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Error::StorageAllocationFailed)?;
    result.resize(count, value);
    Ok(result)
}

impl ContextQueuedWriterJournalV1 {
    pub fn new(
        generation: u64,
        allocations: usize,
        writers: usize,
        reads: usize,
        members: usize,
    ) -> Result<Self, Error> {
        if members < allocations || members > CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1 {
            return Err(Error::InvalidCapacity);
        }
        let inner = ContextProducerReadJournalV1::new(generation, allocations, writers, reads)?;
        let mut free = Vec::new();
        free.try_reserve_exact(members)
            .map_err(|_| Error::StorageAllocationFailed)?;
        free.extend((0..members).rev());
        let mut free_reads = Vec::new();
        free_reads
            .try_reserve_exact(reads)
            .map_err(|_| Error::StorageAllocationFailed)?;
        free_reads.extend((0..reads).rev());
        let mut read_producer_scratch = Vec::new();
        read_producer_scratch
            .try_reserve_exact(reads)
            .map_err(|_| Error::StorageAllocationFailed)?;
        let placeholder = ContextAllocationWriteV1 {
            allocation: ContextAllocationReferenceV1 {
                slot: 0,
                key: ContextAllocationKeyV1 {
                    context_generation: 0,
                    local: 0,
                },
            },
            device: ContextJournalDeviceKeyV1 {
                context_generation: 0,
                local: 0,
            },
            byte_extent: 0,
        };
        Ok(Self {
            inner,
            roots: filled(writers, None)?,
            members: filled(members, None)?,
            free,
            heads: filled(allocations, None)?,
            tails: filled(allocations, None)?,
            queued_counts: filled(allocations, 0)?,
            scratch: filled(allocations, placeholder)?,
            queued_reads: filled(reads, None)?,
            free_reads,
            read_counts: filled(allocations, 0)?,
            next_read_incarnation: 1,
            read_producer_scratch,
            disposal_terminal: false,
            #[cfg(test)]
            disposal_fault: None,
        })
    }

    fn member(&self, index: usize) -> Result<Member, Error> {
        self.members
            .get(index)
            .copied()
            .flatten()
            .ok_or(Error::InvalidState)
    }

    fn root(&self, writer: ContextWriterReferenceV1) -> Result<Root, Error> {
        self.ensure_usable()?;
        self.inner.lookup_writer(writer)?;
        self.roots
            .get(writer.slot)
            .copied()
            .flatten()
            .filter(|root| root.writer == writer)
            .ok_or(Error::InvalidReference)
    }

    fn destination(
        &self,
        write: ContextAllocationWriteV1,
    ) -> Result<ContextAllocationStateV1, Error> {
        self.ensure_usable()?;
        let state = self.inner.lookup_allocation(write.allocation)?;
        if state.device != write.device {
            return Err(Error::AllocationDeviceMismatch);
        }
        if state.byte_extent != write.byte_extent {
            return Err(Error::AllocationExtentMismatch);
        }
        Ok(state)
    }

    fn require_unqueued(&self, allocation: ContextAllocationReferenceV1) -> Result<(), Error> {
        self.ensure_usable()?;
        let state = self.inner.lookup_allocation(allocation)?;
        let slot = allocation.slot;
        let count = *self.queued_counts.get(slot).ok_or(Error::InvalidState)?;
        match (self.heads.get(slot), self.tails.get(slot)) {
            (Some(None), Some(None)) if count == 0 && state.pending_writer.is_none() => {}
            (Some(Some(head)), Some(Some(tail))) => {
                let first = self.member(*head)?;
                let last = self.member(*tail)?;
                self.validate_links(*head, first)?;
                self.validate_links(*tail, last)?;
                if first.request.destination.allocation != allocation
                    || last.request.destination.allocation != allocation
                    || first.previous_allocation.is_some()
                    || last.next_allocation.is_some()
                {
                    return Err(Error::InvalidState);
                }
                let root = self.root(last.writer)?;
                if count == 0 {
                    if head != tail
                        || root.phase != Phase::Active
                        || state.pending_writer != Some(root.writer)
                    {
                        return Err(Error::InvalidState);
                    }
                } else if root.phase == Phase::Active {
                    return Err(Error::InvalidState);
                }
            }
            _ => return Err(Error::InvalidState),
        }
        if count != 0 {
            return Err(Error::AllocationBusy);
        }
        Ok(())
    }

    fn validate_links(&self, index: usize, member: Member) -> Result<(), Error> {
        let slot = member.request.destination.allocation.slot;
        let matches = |neighbor: Member| neighbor.request.destination == member.request.destination;
        match member.previous_allocation {
            Some(previous) => {
                let previous = self.member(previous)?;
                if !matches(previous) || previous.next_allocation != Some(index) {
                    return Err(Error::InvalidState);
                }
            }
            None if self.heads.get(slot) != Some(&Some(index)) => return Err(Error::InvalidState),
            None => {}
        }
        match member.next_allocation {
            Some(next) => {
                let next = self.member(next)?;
                if !matches(next) || next.previous_allocation != Some(index) {
                    return Err(Error::InvalidState);
                }
            }
            None if self.tails.get(slot) != Some(&Some(index)) => return Err(Error::InvalidState),
            None => {}
        }
        Ok(())
    }

    fn validate_root(&self, writer: ContextWriterReferenceV1) -> Result<Root, Error> {
        let root = self.root(writer)?;
        let state = self.inner.lookup_writer(writer)?;
        if root.count > self.scratch.len()
            || match root.phase {
                Phase::Active => {
                    !matches!(state, ContextWriterStateV1::Pending { member_count } | ContextWriterStateV1::Unknown { member_count } if member_count == root.count)
                }
                Phase::Queued | Phase::Unknown => state != ContextWriterStateV1::Reserved,
            }
        {
            return Err(Error::InvalidState);
        }
        let mut next = root.head;
        let mut previous_key = None;
        for _ in 0..root.count {
            let index = next.ok_or(Error::InvalidState)?;
            let member = self.member(index)?;
            if member.writer != writer {
                return Err(Error::InvalidState);
            }
            let write = member.request.destination;
            let allocation = self.destination(write)?;
            if previous_key.is_some_and(|key| key >= write.allocation.key) {
                return Err(Error::NonCanonicalRoster);
            }
            self.validate_links(index, member)?;
            if root.phase == Phase::Active {
                if member.previous_allocation.is_some() || allocation.pending_writer != Some(writer)
                {
                    return Err(Error::InvalidState);
                }
            } else if self.queued_counts[write.allocation.slot] == 0 {
                return Err(Error::InvalidState);
            }
            previous_key = Some(write.allocation.key);
            next = member.next_writer;
        }
        if next.is_some() {
            return Err(Error::InvalidState);
        }
        Ok(root)
    }

    fn preflight(
        &self,
        writer: ContextWriterReferenceV1,
        count: usize,
        queued: bool,
        request: impl Fn(usize) -> ContextQueuedWriteV1,
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner.lookup_reserved(writer)?;
        if self.roots[writer.slot].is_some() {
            return Err(Error::InvalidState);
        }
        self.preflight_roster(writer.key, count, queued, request)
    }

    fn preflight_roster(
        &self,
        writer: ContextWriterKeyV1,
        count: usize,
        queued: bool,
        request: impl Fn(usize) -> ContextQueuedWriteV1,
    ) -> Result<(), Error> {
        if count > self.scratch.len() {
            return Err(Error::RosterCapacity);
        }
        if queued && (count == 0 || writer.kind != ContextWriterKindV1::Submission) {
            return Err(Error::InvalidState);
        }
        if count > self.free.len() {
            return Err(Error::MemberCapacity);
        }
        let mut previous = None;
        for ordinal in 0..count {
            let request = request(ordinal);
            let write = request.destination;
            if previous.is_some_and(|key| key >= write.allocation.key) {
                return Err(Error::NonCanonicalRoster);
            }
            previous = Some(write.allocation.key);
            let state = self.destination(write)?;
            if self.reader_count(write.allocation)? != 0 {
                return Err(Error::AllocationBusy);
            }
            let slot = write.allocation.slot;
            match self.tails[slot] {
                Some(tail) => {
                    let parent = self.member(tail)?;
                    self.validate_links(tail, parent)?;
                    if !queued
                        || parent.request.destination != write
                        || request.predecessor != Some(parent.writer)
                        || parent.writer.key.local >= writer.local
                    {
                        return Err(Error::AllocationBusy);
                    }
                    let root = self.root(parent.writer)?;
                    if root.phase == Phase::Unknown
                        || matches!(
                            self.inner.lookup_writer(parent.writer)?,
                            ContextWriterStateV1::Unknown { .. }
                        )
                    {
                        return Err(Error::AllocationBusy);
                    }
                }
                None if self.heads[slot].is_some()
                    || request.predecessor.is_some()
                    || state.pending_writer.is_some() =>
                {
                    return Err(Error::AllocationBusy);
                }
                None => {}
            }
            let additional = if queued {
                self.queued_counts[slot]
                    .checked_add(1)
                    .ok_or(Error::EpochExhausted)?
            } else {
                1
            };
            state
                .attempt_epoch
                .checked_add(u64::try_from(additional).map_err(|_| Error::EpochExhausted)?)
                .ok_or(Error::EpochExhausted)?;
            let free = self.free[self.free.len() - 1 - ordinal];
            if self.members.get(free).is_none_or(Option::is_some) {
                return Err(Error::InvalidState);
            }
        }
        Ok(())
    }

    fn commit_begin(
        &mut self,
        writer: ContextWriterReferenceV1,
        count: usize,
        phase: Phase,
        request: impl Fn(usize) -> ContextQueuedWriteV1,
    ) {
        let head = (count != 0).then(|| self.free[self.free.len() - 1]);
        for ordinal in 0..count {
            let request = request(ordinal);
            let allocation = request.destination.allocation;
            let previous = self.tails[allocation.slot];
            let prerequisite = if request.predecessor.is_some() {
                Prerequisite::Pending
            } else {
                let state = self
                    .inner
                    .lookup_allocation(allocation)
                    .expect("preflighted destination");
                Prerequisite::Success {
                    epoch: state.attempt_epoch,
                    lineage: state.content_lineage,
                }
            };
            let index = self.free.pop().expect("preallocated member");
            let next_writer = (ordinal + 1 < count).then(|| self.free[self.free.len() - 1]);
            self.members[index] = Some(Member {
                writer,
                request,
                prerequisite,
                next_writer,
                previous_allocation: previous,
                next_allocation: None,
            });
            if let Some(previous) = previous {
                self.members[previous]
                    .as_mut()
                    .expect("preflighted tail")
                    .next_allocation = Some(index);
            } else {
                self.heads[allocation.slot] = Some(index);
            }
            self.tails[allocation.slot] = Some(index);
            if phase != Phase::Active {
                self.queued_counts[allocation.slot] += 1;
            }
        }
        self.roots[writer.slot] = Some(Root {
            writer,
            head,
            count,
            phase,
            read_head: None,
            read_count: 0,
        });
    }

    pub fn begin_write(
        &mut self,
        writer: ContextWriterReferenceV1,
        members: &[ContextAllocationWriteV1],
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner.preflight_begin_write_v1(writer, members)?;
        let request = |index: usize| ContextQueuedWriteV1 {
            destination: members[index],
            predecessor: None,
        };
        self.preflight(writer, members.len(), false, request)?;
        self.inner.begin_write(writer, members)?;
        self.commit_begin(writer, members.len(), Phase::Active, request);
        Ok(())
    }

    /// Atomically reserve the complete destination roster. Dependency/event
    /// authentication and success-gated physical execution are adapter obligations.
    pub fn begin_queued_write(
        &mut self,
        writer: ContextWriterReferenceV1,
        requests: &[ContextQueuedWriteV1],
    ) -> Result<(), Error> {
        self.preflight(writer, requests.len(), true, |index| requests[index])?;
        self.commit_begin(writer, requests.len(), Phase::Queued, |index| {
            requests[index]
        });
        Ok(())
    }

    /// Borrowed preflight before the adapter mints an ID or installs custody.
    /// The proposed key is not issued or reserved here. Begin rechecks everything
    /// against the registered identity, under the same exclusive owner. This
    /// checks public issuance eligibility, not private registration arena integrity;
    /// the inner register operation still validates that storage before issuance.
    pub fn validate_queued_write_admission(
        &self,
        proposed: ContextWriterKeyV1,
        requests: &[ContextQueuedWriteV1],
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        if proposed.context_generation != self.context_generation() {
            return Err(Error::ForeignContext);
        }
        if proposed.local == 0
            || proposed.local == u64::MAX
            || proposed.kind != ContextWriterKindV1::Submission
        {
            return Err(Error::InvalidWriterId);
        }
        if proposed.local <= self.registration_watermark() {
            return Err(Error::WriterReplay);
        }
        if self.remaining_writer_slots() == 0 {
            return Err(Error::WriterCapacity);
        }
        self.preflight_roster(proposed, requests.len(), true, |index| requests[index])
    }

    pub fn queued_writer_status(
        &self,
        writer: ContextWriterReferenceV1,
    ) -> Result<Option<ContextQueuedWriterStatusV1>, Error> {
        let root = self.validate_root(writer)?;
        if root.phase == Phase::Active {
            return Ok(None);
        }
        if root.phase == Phase::Unknown {
            return Ok(Some(ContextQueuedWriterStatusV1::Unknown));
        }
        let mut status = ContextQueuedWriterStatusV1::Ready;
        let mut next = root.head;
        for _ in 0..root.count {
            let member = self.member(next.ok_or(Error::InvalidState)?)?;
            match member.prerequisite {
                Prerequisite::Failed => status = ContextQueuedWriterStatusV1::Blocked,
                Prerequisite::Pending if status != ContextQueuedWriterStatusV1::Blocked => {
                    status = ContextQueuedWriterStatusV1::Waiting
                }
                _ => {}
            }
            next = member.next_writer;
        }
        Ok(Some(status))
    }

    pub fn validate_queued_writer(
        &self,
        writer: ContextWriterReferenceV1,
        requests: &[ContextQueuedWriteV1],
    ) -> Result<(), Error> {
        let root = self.validate_root(writer)?;
        if root.phase == Phase::Active || root.count != requests.len() {
            return Err(Error::InvalidState);
        }
        let mut next = root.head;
        for request in requests {
            let member = self.member(next.ok_or(Error::InvalidState)?)?;
            if member.request != *request {
                return Err(Error::InvalidReference);
            }
            next = member.next_writer;
        }
        Ok(())
    }

    /// Begin the inner writer only after actual predecessor Success. No future
    /// epoch/lineage is guessed at admission. Failure keeps exclusive reservations.
    pub fn activate_queued_writer(
        &mut self,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), Error> {
        if self.queued_writer_status(writer)? != Some(ContextQueuedWriterStatusV1::Ready) {
            return Err(Error::AllocationBusy);
        }
        let root = self.validate_root(writer)?;
        let mut next = root.head;
        for ordinal in 0..root.count {
            let member = self.member(next.ok_or(Error::InvalidState)?)?;
            let write = member.request.destination;
            let state = self.destination(write)?;
            if member.previous_allocation.is_some()
                || state.pending_writer.is_some()
                || self.inner.reader_count(write.allocation)? != 0
                || member.prerequisite
                    != (Prerequisite::Success {
                        epoch: state.attempt_epoch,
                        lineage: state.content_lineage,
                    })
            {
                return Err(Error::InvalidState);
            }
            self.scratch[ordinal] = write;
            next = member.next_writer;
        }
        self.inner
            .begin_write(writer, &self.scratch[..root.count])?;
        let mut next = root.head;
        for _ in 0..root.count {
            let member = self
                .member(next.expect("validated member"))
                .expect("validated member");
            self.queued_counts[member.request.destination.allocation.slot] -= 1;
            next = member.next_writer;
        }
        self.roots[writer.slot]
            .as_mut()
            .expect("retained writer")
            .phase = Phase::Active;
        Ok(())
    }

    fn unlink(&mut self, root: Root, success: bool) {
        let mut next = root.head;
        for _ in 0..root.count {
            let index = next.expect("validated writer roster");
            let member = self.members[index].take().expect("validated member");
            let allocation = member.request.destination.allocation;
            if let Some(previous) = member.previous_allocation {
                self.members[previous]
                    .as_mut()
                    .expect("validated previous")
                    .next_allocation = member.next_allocation;
            } else {
                self.heads[allocation.slot] = member.next_allocation;
            }
            if let Some(after) = member.next_allocation {
                let state = self
                    .inner
                    .lookup_allocation(allocation)
                    .expect("retained allocation");
                let after = self.members[after].as_mut().expect("validated next");
                after.previous_allocation = member.previous_allocation;
                if after.request.predecessor == Some(root.writer) {
                    after.prerequisite = if success {
                        Prerequisite::Success {
                            epoch: state.attempt_epoch,
                            lineage: state.content_lineage,
                        }
                    } else {
                        Prerequisite::Failed
                    };
                }
            } else {
                self.tails[allocation.slot] = member.previous_allocation;
            }
            if root.phase != Phase::Active {
                self.queued_counts[allocation.slot] -= 1;
            }
            self.free.push(index);
            next = member.next_writer;
        }
        self.roots[root.writer.slot] = None;
    }

    /// A successful activation may be a committed prefix if subsequent inner
    /// settlement fails. The root remains retained; adapters must quarantine it.
    pub fn settle_success(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterSuccessEvidenceV1,
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        if evidence.writer != writer {
            return Err(Error::SettlementEvidenceMismatch);
        }
        let root = self.validate_root(writer)?;
        self.validate_attached_reads(root)?;
        if root.phase != Phase::Active {
            self.activate_queued_writer(writer)?;
        }
        let root = self.validate_root(writer)?;
        self.inner.settle_success(writer, evidence)?;
        self.disposal_terminal = true;
        self.resolve_reads(root, ContextProducerReadStatusV1::Success);
        self.unlink(root, true);
        self.disposal_terminal = false;
        Ok(())
    }

    /// Only an authenticated NoEffect premise can release an accepted Queued
    /// writer. Unknown is fail-stop, even with a later NoEffect value. Predecessor
    /// failure and a core Reserved state are insufficient.
    pub fn settle_no_effect(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterNoEffectEvidenceV1,
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        if evidence.writer != writer {
            return Err(Error::SettlementEvidenceMismatch);
        }
        let root = self.validate_root(writer)?;
        if root.phase == Phase::Unknown {
            return Err(Error::InvalidState);
        }
        self.validate_attached_reads(root)?;
        if root.phase == Phase::Active {
            self.inner.settle_no_effect(writer, evidence)?;
        } else {
            self.inner.abort_reserved(writer)?;
        }
        self.disposal_terminal = true;
        self.resolve_reads(root, ContextProducerReadStatusV1::NoEffect);
        self.unlink(root, false);
        self.disposal_terminal = false;
        Ok(())
    }

    pub fn mark_unknown(&mut self, writer: ContextWriterReferenceV1) -> Result<(), Error> {
        let root = self.validate_root(writer)?;
        self.validate_attached_reads(root)?;
        if root.phase == Phase::Active {
            self.inner.mark_unknown(writer)?;
        } else {
            self.roots[writer.slot]
                .as_mut()
                .expect("retained root")
                .phase = Phase::Unknown;
        }
        self.disposal_terminal = true;
        self.resolve_reads(root, ContextProducerReadStatusV1::Unknown);
        self.disposal_terminal = false;
        Ok(())
    }
}
