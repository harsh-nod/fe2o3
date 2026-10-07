// Frozen from e8134e770a30d12aa4abd190bd29c5083ea5b3b5; only method names/visibility change.
use super::*;

impl ContextProducerReadJournalV1 {
    pub(crate) fn baseline_reader_count_v1(
        &self,
        allocation: ContextAllocationReferenceV1,
    ) -> Result<usize, ContextVersionJournalErrorV1> {
        Ok(self.stable.baseline_reader_count_v1(allocation)? + self.counts[allocation.slot])
    }

    pub(crate) fn baseline_require_unread_v1(
        &self,
        references: impl IntoIterator<Item = ContextAllocationReferenceV1>,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        for reference in references {
            if self.baseline_reader_count_v1(reference)? != 0 {
                return Err(ContextVersionJournalErrorV1::AllocationBusy);
            }
        }
        Ok(())
    }

    pub(crate) fn baseline_begin_write_v1(
        &mut self,
        writer: ContextWriterReferenceV1,
        members: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.baseline_require_unread_v1(members.iter().map(|member| member.allocation))?;
        self.stable.baseline_begin_write_v1(writer, members)
    }
}
