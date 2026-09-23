use super::*;

pub(crate) enum StableReadFaultV1 {
    NextIncarnation(u64),
    ReaderCount {
        allocation_slot: usize,
        value: usize,
    },
    TruncateReaders(usize),
    FreeSlotFromEnd {
        distance: usize,
        slot: usize,
    },
    TightFreeCapacity,
}

pub(crate) struct StableReadResetV1 {
    entries: Vec<(usize, Option<ReadLeaseV1>)>,
    counts: Vec<(usize, usize)>,
    free_length: usize,
    free_tail: Option<Vec<usize>>,
    incarnation: u64,
}

impl StableReadResetV1 {
    pub(crate) fn restore(&self, owner: &mut ContextReadLeasedJournalV1) {
        for &(slot, entry) in &self.entries {
            owner.leases[slot] = entry;
        }
        for &(slot, count) in &self.counts {
            owner.readers[slot] = count;
        }
        if let Some(tail) = &self.free_tail {
            assert!(owner.free_reads.len() <= self.free_length);
            let missing = self.free_length - owner.free_reads.len();
            owner
                .free_reads
                .extend_from_slice(&tail[tail.len() - missing..]);
            owner.next_incarnation = self.incarnation;
        } else {
            assert!(owner.free_reads.len() >= self.free_length);
            owner.free_reads.truncate(self.free_length);
            assert_eq!(owner.next_incarnation, self.incarnation);
        }
        owner.reset_access_count_for_test_v1();
    }
}

impl ContextReadLeasedJournalV1 {
    pub(crate) fn fault_enrollment_for_test_v1(&mut self, free: &[usize], capacity: usize) {
        self.journal.fault_enrollment_for_test_v1(free, capacity);
    }

    pub(crate) fn restore_enrollment_for_test_v1(
        &mut self,
        before: &ContextVersionJournalV1,
        count: usize,
    ) {
        self.journal.restore_enrollment_for_test_v1(before, count);
    }

    pub(crate) fn capture_acquire_reads_for_test_v1(
        &self,
        requests: &[ContextAllocationReadV1],
    ) -> StableReadResetV1 {
        let tail = self.free_reads[self.free_reads.len().saturating_sub(requests.len())..].to_vec();
        StableReadResetV1 {
            entries: tail
                .iter()
                .filter_map(|&slot| self.leases.get(slot).map(|&entry| (slot, entry)))
                .collect(),
            counts: requests
                .iter()
                .filter_map(|request| {
                    let slot = request.allocation.slot;
                    self.readers.get(slot).map(|&count| (slot, count))
                })
                .collect(),
            free_length: self.free_reads.len(),
            free_tail: Some(tail),
            incarnation: self.next_incarnation,
        }
    }

    pub(crate) fn capture_release_reads_for_test_v1(
        &self,
        references: &[ContextReadLeaseReferenceV1],
    ) -> StableReadResetV1 {
        let entries: Vec<_> = references
            .iter()
            .filter_map(|r| self.leases.get(r.slot).map(|&entry| (r.slot, entry)))
            .collect();
        let counts = entries
            .iter()
            .filter_map(|(_, entry)| {
                let slot = entry.as_ref()?.request.allocation.slot;
                self.readers.get(slot).map(|&count| (slot, count))
            })
            .collect();
        StableReadResetV1 {
            entries,
            counts,
            free_length: self.free_reads.len(),
            free_tail: None,
            incarnation: self.next_incarnation,
        }
    }

    pub(crate) fn fault_reads_for_test_v1(&mut self, fault: StableReadFaultV1) {
        match fault {
            StableReadFaultV1::NextIncarnation(value) => self.next_incarnation = value,
            StableReadFaultV1::ReaderCount {
                allocation_slot,
                value,
            } => self.readers[allocation_slot] = value,
            StableReadFaultV1::TruncateReaders(length) => self.readers.truncate(length),
            StableReadFaultV1::FreeSlotFromEnd { distance, slot } => {
                let index = self.free_reads.len() - distance;
                self.free_reads[index] = slot;
            }
            StableReadFaultV1::TightFreeCapacity => {
                self.free_reads = core::mem::take(&mut self.free_reads)
                    .into_boxed_slice()
                    .into_vec();
                assert_eq!(self.free_reads.len(), self.free_reads.capacity());
            }
        }
    }

    pub(crate) fn query_replace_writer_for_test_v1(
        &mut self,
        reference: ContextWriterReferenceV1,
        state: Option<ContextWriterStateV1>,
    ) {
        self.journal
            .query_replace_writer_for_test_v1(reference, state);
    }

    pub(crate) fn guard_break_backlink_for_test_v1(
        &mut self,
        allocation: ContextAllocationReferenceV1,
    ) {
        self.journal.guard_break_backlink_for_test_v1(allocation);
    }

    pub(crate) fn guard_owner_storage_v1(&self) -> Vec<(usize, usize)> {
        let mut storage = self.journal.guard_storage_for_test_v1();
        storage.extend([
            (self.leases.as_ptr() as usize, self.leases.capacity()),
            (
                self.free_reads.as_ptr() as usize,
                self.free_reads.capacity(),
            ),
            (self.readers.as_ptr() as usize, self.readers.capacity()),
        ]);
        storage
    }

    pub(crate) fn guard_restore_owner_v1(
        &mut self,
        before: &ContextVersionJournalV1,
        writer: ContextWriterReferenceV1,
        roster: &[ContextAllocationWriteV1],
    ) {
        self.journal
            .guard_restore_for_test_v1(before, writer, roster);
    }
}
