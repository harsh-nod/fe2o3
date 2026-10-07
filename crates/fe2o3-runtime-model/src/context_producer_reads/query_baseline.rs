// Frozen from 9d2ec2b98819daf14b2a0bc72681f95a3cc571a9; only method names/visibility change.
use super::*;

impl ContextProducerReadJournalV1 {
    pub(crate) fn baseline_status_v1(
        &self,
        request: &ContextProducerReadV1,
    ) -> Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1> {
        use ContextProducerReadStatusV1 as S;
        use ContextVersionJournalErrorV1 as E;
        let read = request.read;
        let state = self.stable.lookup_allocation(read.allocation)?;
        if state.device != read.device {
            return Err(E::AllocationDeviceMismatch);
        }
        if state.byte_extent != read.byte_extent {
            return Err(E::AllocationExtentMismatch);
        }
        if read.byte_len == 0
            || read
                .byte_offset
                .checked_add(read.byte_len)
                .is_none_or(|end| end > read.byte_extent)
        {
            return Err(E::InvalidExtent);
        }
        if request.producer.key.context_generation != self.context_generation()
            || request.producer.key.kind != ContextWriterKindV1::Submission
            || read.content_lineage >= read.attempt_epoch
            || state.attempt_epoch != read.attempt_epoch
        {
            return Err(E::InvalidState);
        }
        match state.pending_writer {
            Some(writer) => {
                if writer != request.producer || state.content_lineage != read.content_lineage {
                    return Err(E::InvalidState);
                }
                match self.stable.baseline_lookup_writer_v1(writer)? {
                    ContextWriterStateV1::Pending { .. } => Ok(S::Pending),
                    ContextWriterStateV1::Unknown { .. } => Ok(S::Unknown),
                    ContextWriterStateV1::Reserved => Err(E::InvalidState),
                }
            }
            // The old producer slot may already have been reused elsewhere.
            None if state.content_lineage == read.attempt_epoch => Ok(S::Success),
            None if state.content_lineage == read.content_lineage => Ok(S::NoEffect),
            None => Err(E::InvalidState),
        }
    }

    pub(crate) fn baseline_validate_producer_read_v1(
        &self,
        request: &ContextProducerReadV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        if self.baseline_status_v1(request)? != ContextProducerReadStatusV1::Pending {
            return Err(ContextVersionJournalErrorV1::AllocationBusy);
        }
        Ok(())
    }

    pub(crate) fn baseline_lookup_producer_read_v1(
        &self,
        reference: ContextProducerReadReferenceV1,
    ) -> Result<ContextProducerReadV1, ContextVersionJournalErrorV1> {
        let entry = self
            .reservations
            .get(reference.slot)
            .copied()
            .flatten()
            .filter(|entry| entry.reference == reference)
            .ok_or(ContextVersionJournalErrorV1::InvalidReference)?;
        self.baseline_status_v1(&entry.request)?;
        Ok(entry.request)
    }

    pub(crate) fn baseline_producer_read_status_v1(
        &self,
        reference: ContextProducerReadReferenceV1,
    ) -> Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1> {
        self.baseline_status_v1(&self.baseline_lookup_producer_read_v1(reference)?)
    }
}
