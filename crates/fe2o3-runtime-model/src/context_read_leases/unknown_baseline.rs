use super::*;

// Frozen e3ea84c1f adapter, redirected to the frozen journal adapter.
impl ContextReadLeasedJournalV1 {
    pub(crate) fn baseline_mark_unknown_v1(
        &mut self,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.journal.baseline_mark_unknown_v1(writer)
    }
}
