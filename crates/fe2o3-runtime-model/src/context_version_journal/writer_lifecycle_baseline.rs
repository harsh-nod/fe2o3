// Frozen at 9b55f0c15895c0eb4d825410a2732542334e8225; only names are redirected.
use super::*;

impl ContextVersionJournalV1 {
    pub(crate) fn baseline_register_writer_v1(
        &mut self,
        key: ContextWriterKeyV1,
    ) -> Result<ContextWriterReferenceV1, ContextVersionJournalErrorV1> {
        if key.context_generation != self.context_generation {
            return Err(ContextVersionJournalErrorV1::ForeignContext);
        }
        if !issuable_context_id(key.local) {
            return Err(ContextVersionJournalErrorV1::InvalidWriterId);
        }
        if key.local <= self.registration_watermark {
            return Err(ContextVersionJournalErrorV1::WriterReplay);
        }
        let slot = self
            .next_free()
            .ok_or(ContextVersionJournalErrorV1::WriterCapacity)?;
        if self.read_slot(slot) != Some(&None) {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        let reserved_count = self
            .reserved_count
            .checked_add(1)
            .filter(|count| *count <= self.writer_capacity)
            .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
        // Exclusive preflight fixes the free slot; commit has no fallible work.
        self.pop_free();
        self.store_slot(slot, Some(WriterEntryV1::Reserved(key)));
        self.reserved_count = reserved_count;
        self.registration_watermark = key.local;
        Ok(ContextWriterReferenceV1 { slot, key })
    }

    pub(crate) fn baseline_lookup_reserved_v1(
        &self,
        reference: ContextWriterReferenceV1,
    ) -> Result<ContextWriterKeyV1, ContextVersionJournalErrorV1> {
        match self.read_slot(reference.slot).copied().flatten() {
            Some(WriterEntryV1::Reserved(key))
                if key == reference.key && key.context_generation == self.context_generation =>
            {
                Ok(key)
            }
            _ => Err(ContextVersionJournalErrorV1::InvalidReference),
        }
    }

    pub(crate) fn baseline_abort_reserved_v1(
        &mut self,
        reference: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.baseline_lookup_reserved_v1(reference)?;
        if self.free.len() >= self.writer_capacity || self.free.len() >= self.free.capacity() {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        let reserved_count = self
            .reserved_count
            .checked_sub(1)
            .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
        self.store_slot(reference.slot, None);
        self.push_free(reference.slot);
        self.reserved_count = reserved_count;
        Ok(())
    }
}
