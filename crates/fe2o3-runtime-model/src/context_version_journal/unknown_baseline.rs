use super::*;

// Frozen e3ea84c1f journal adapter; the retained leaf is unchanged.
impl ContextVersionJournalV1 {
    pub(crate) fn baseline_mark_unknown_v1(
        &mut self,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        retained::shared_retained_unknown_v1(self, writer)
    }
}
