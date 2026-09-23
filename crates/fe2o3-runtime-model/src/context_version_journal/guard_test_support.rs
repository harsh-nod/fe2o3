use super::*;

impl ContextVersionJournalV1 {
    // Register and abort change one slot and at most one free-stack suffix entry.
    pub(crate) fn restore_writer_for_test_v1(&mut self, before: &Self, slot: usize) {
        if let Some(value) = before.writers.get(slot) {
            self.writers[slot] = *value;
        }
        assert!(self.free.len().abs_diff(before.free.len()) <= 1);
        self.free.truncate(before.free.len());
        self.free.extend_from_slice(&before.free[self.free.len()..]);
        self.registration_watermark = before.registration_watermark;
        self.reserved_count = before.reserved_count;
        self.reset_access_count_for_test_v1();
    }

    pub(crate) fn fault_enrollment_for_test_v1(&mut self, free: &[usize], capacity: usize) {
        self.allocation_free.clear();
        self.allocation_free.extend_from_slice(free);
        self.allocation_capacity = capacity;
    }

    // Only selected slots and the consumed free suffix can change in enrollment.
    pub(crate) fn restore_enrollment_for_test_v1(&mut self, before: &Self, count: usize) {
        for &slot in before.allocation_free.iter().rev().take(count) {
            if let Some(value) = before.allocations.get(slot) {
                self.allocations[slot] = *value;
            }
        }
        self.allocation_free
            .extend_from_slice(&before.allocation_free[self.allocation_free.len()..]);
        self.reset_access_count_for_test_v1();
    }

    pub(crate) fn query_replace_writer_for_test_v1(
        &mut self,
        reference: ContextWriterReferenceV1,
        state: Option<ContextWriterStateV1>,
    ) {
        self.writers[reference.slot] = state.map(|state| match state {
            ContextWriterStateV1::Reserved => WriterEntryV1::Reserved(reference.key),
            ContextWriterStateV1::Pending { member_count } => WriterEntryV1::Pending {
                key: reference.key,
                head: Some(usize::MAX),
                count: member_count,
            },
            ContextWriterStateV1::Unknown { member_count } => WriterEntryV1::Unknown {
                key: reference.key,
                head: Some(usize::MAX),
                count: member_count,
            },
        });
    }

    pub(crate) fn guard_break_backlink_for_test_v1(
        &mut self,
        allocation: ContextAllocationReferenceV1,
    ) {
        self.allocations[allocation.slot]
            .as_mut()
            .unwrap()
            .pending_member = Some(usize::MAX);
    }

    pub(crate) fn guard_copy_for_test_v1(&self) -> Self {
        Self {
            context_generation: self.context_generation,
            allocation_capacity: self.allocation_capacity,
            writer_capacity: self.writer_capacity,
            registration_watermark: self.registration_watermark,
            reserved_count: self.reserved_count,
            writers: self.writers.clone(),
            free: self.free.clone(),
            allocations: self.allocations.clone(),
            allocation_free: self.allocation_free.clone(),
            members: self.members.clone(),
            member_free: self.member_free.clone(),
            scratch: self.scratch.clone(),
            indexed_accesses: Cell::new(0),
        }
    }

    pub(crate) fn guard_accesses_for_test_v1(&self) -> usize {
        self.indexed_accesses.get()
    }

    pub(crate) fn guard_storage_for_test_v1(&self) -> Vec<(usize, usize)> {
        alloc::vec![
            (self.writers.as_ptr() as usize, self.writers.capacity()),
            (self.free.as_ptr() as usize, self.free.capacity()),
            (
                self.allocations.as_ptr() as usize,
                self.allocations.capacity()
            ),
            (
                self.allocation_free.as_ptr() as usize,
                self.allocation_free.capacity()
            ),
            (self.members.as_ptr() as usize, self.members.capacity()),
            (
                self.member_free.as_ptr() as usize,
                self.member_free.capacity()
            ),
            (self.scratch.as_ptr() as usize, self.scratch.capacity()),
        ]
    }

    // Restore only possible Begin writes; full owner snapshots qualify this before timing.
    pub(crate) fn guard_restore_for_test_v1(
        &mut self,
        before: &Self,
        writer: ContextWriterReferenceV1,
        roster: &[ContextAllocationWriteV1],
    ) {
        if writer.slot < self.writers.len() {
            self.writers[writer.slot] = before.writers[writer.slot];
        }
        self.reserved_count = before.reserved_count;
        for destination in roster {
            if destination.allocation.slot < self.allocations.len() {
                self.allocations[destination.allocation.slot] =
                    before.allocations[destination.allocation.slot];
            }
        }
        for &slot in before.member_free.iter().rev().take(roster.len()) {
            self.members[slot] = before.members[slot];
        }
        let count = roster.len().min(self.scratch.len());
        self.scratch[..count].copy_from_slice(&before.scratch[..count]);
        self.member_free
            .extend_from_slice(&before.member_free[self.member_free.len()..]);
        self.reset_access_count_for_test_v1();
    }
}
