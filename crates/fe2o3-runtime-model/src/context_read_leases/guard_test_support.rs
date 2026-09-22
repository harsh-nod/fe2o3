use super::*;

impl ContextReadLeasedJournalV1 {
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
