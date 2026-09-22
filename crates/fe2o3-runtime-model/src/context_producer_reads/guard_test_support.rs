use super::*;

impl ContextProducerReadJournalV1 {
    pub(crate) fn guard_owner_storage_v1(&self) -> Vec<(usize, usize)> {
        let mut storage = self.stable.guard_owner_storage_v1();
        storage.extend([
            (
                self.reservations.as_ptr() as usize,
                self.reservations.capacity(),
            ),
            (self.free.as_ptr() as usize, self.free.capacity()),
            (self.counts.as_ptr() as usize, self.counts.capacity()),
        ]);
        storage
    }

    pub(crate) fn guard_restore_owner_v1(
        &mut self,
        before: &ContextVersionJournalV1,
        writer: ContextWriterReferenceV1,
        roster: &[ContextAllocationWriteV1],
    ) {
        self.stable.guard_restore_owner_v1(before, writer, roster);
    }
}
