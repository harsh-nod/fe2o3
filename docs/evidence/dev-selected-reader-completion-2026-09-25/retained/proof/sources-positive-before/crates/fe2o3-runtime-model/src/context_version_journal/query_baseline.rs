// Frozen from 9d2ec2b98819daf14b2a0bc72681f95a3cc571a9; only method names/visibility change.
use super::*;

impl ContextVersionJournalV1 {
    pub(crate) fn baseline_lookup_writer_v1(
        &self,
        reference: ContextWriterReferenceV1,
    ) -> Result<ContextWriterStateV1, ContextVersionJournalErrorV1> {
        let (key, state) = match self.read_slot(reference.slot).copied().flatten() {
            Some(WriterEntryV1::Reserved(key)) => (key, ContextWriterStateV1::Reserved),
            Some(WriterEntryV1::Pending { key, count, .. }) => (
                key,
                ContextWriterStateV1::Pending {
                    member_count: count,
                },
            ),
            Some(WriterEntryV1::Unknown { key, count, .. }) => (
                key,
                ContextWriterStateV1::Unknown {
                    member_count: count,
                },
            ),
            None => return Err(ContextVersionJournalErrorV1::InvalidReference),
        };
        if key != reference.key || key.context_generation != self.context_generation {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        Ok(state)
    }
}
