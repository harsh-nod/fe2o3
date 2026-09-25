use super::*;

// Frozen e3ea84c1f adapter, redirected through the frozen inner adapters.
impl ContextProducerReadJournalV1 {
    pub(crate) fn baseline_mark_unknown_v1(
        &mut self,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.stable.baseline_mark_unknown_v1(writer)
    }
}
