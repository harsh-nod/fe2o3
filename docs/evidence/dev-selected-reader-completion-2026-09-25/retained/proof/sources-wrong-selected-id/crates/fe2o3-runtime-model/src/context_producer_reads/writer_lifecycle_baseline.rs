// Frozen at 9b55f0c15895c0eb4d825410a2732542334e8225; forwarding remains frozen.
use super::*;

impl ContextProducerReadJournalV1 {
    pub(crate) fn baseline_register_writer_v1(
        &mut self,
        key: ContextWriterKeyV1,
    ) -> Result<ContextWriterReferenceV1, ContextVersionJournalErrorV1> {
        self.stable.baseline_register_writer_v1(key)
    }

    pub(crate) fn baseline_abort_reserved_v1(
        &mut self,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.stable.baseline_abort_reserved_v1(writer)
    }
}
