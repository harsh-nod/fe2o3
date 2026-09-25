// Frozen from 72bcb1da989ea79c2c48085f5a6c9ba3ce1633f1; only method names/visibility change.
use super::*;

impl ContextReadLeasedJournalV1 {
    pub(crate) fn baseline_validate_read_capacity_v1(
        &self,
        count: usize,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        if count > self.free_reads.len() {
            return Err(ContextVersionJournalErrorV1::MemberCapacity);
        }
        if self.next_incarnation == 0 || self.next_incarnation.checked_add(count as u64).is_none() {
            return Err(ContextVersionJournalErrorV1::EpochExhausted);
        }
        Ok(())
    }

    pub(crate) fn baseline_validate_read_v1(
        &self,
        request: &ContextAllocationReadV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        let state = self.journal.lookup_allocation(request.allocation)?;
        if state.device != request.device {
            return Err(E::AllocationDeviceMismatch);
        }
        if state.byte_extent != request.byte_extent {
            return Err(E::AllocationExtentMismatch);
        }
        if request.byte_len == 0
            || request
                .byte_offset
                .checked_add(request.byte_len)
                .is_none_or(|end| end > request.byte_extent)
        {
            return Err(E::InvalidExtent);
        }
        if state.pending_writer.is_some() {
            return Err(E::AllocationBusy);
        }
        if state.attempt_epoch != request.attempt_epoch
            || state.content_lineage != request.content_lineage
        {
            return Err(E::InvalidState);
        }
        Ok(())
    }

    pub(crate) fn baseline_acquire_reads_v1(
        &mut self,
        consumer: ContextWriterKeyV1,
        requests: &[ContextAllocationReadV1],
        output: &mut [Option<ContextReadLeaseReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        if consumer.context_generation != self.context_generation() {
            return Err(E::ForeignContext);
        }
        if consumer.local == 0 || consumer.local == u64::MAX {
            return Err(E::InvalidWriterId);
        }
        if requests.is_empty() || requests.len() != output.len() {
            return Err(E::RosterCapacity);
        }
        if output.iter().any(Option::is_some) {
            return Err(E::InvalidState);
        }
        self.baseline_validate_read_capacity_v1(requests.len())?;
        let next = self
            .next_incarnation
            .checked_add(requests.len() as u64)
            .ok_or(E::EpochExhausted)?;
        if self.next_incarnation == 0 {
            return Err(E::EpochExhausted);
        }
        let mut previous = None;
        let mut group = 0;
        for (index, request) in requests.iter().enumerate() {
            self.baseline_validate_read_v1(request)?;
            let key = read_key(request);
            if previous.is_some_and(|prior| prior >= key) {
                return Err(E::NonCanonicalRoster);
            }
            group = if previous.is_some_and(|prior: (u64, u64, u64)| prior.0 == key.0) {
                group + 1
            } else {
                1
            };
            if self.readers[request.allocation.slot]
                .checked_add(group)
                .is_none_or(|count| count > self.leases.len())
            {
                return Err(E::InvalidState);
            }
            previous = Some(key);
            let slot = self.free_reads[self.free_reads.len() - index - 1];
            if self.leases.get(slot) != Some(&None) {
                return Err(E::InvalidState);
            }
        }
        for (index, request) in requests.iter().enumerate() {
            let slot = self.free_reads.pop().expect("preflighted free slot");
            let reference = ContextReadLeaseReferenceV1 {
                slot,
                incarnation: self.next_incarnation + index as u64,
                consumer,
            };
            self.leases[slot] = Some(ReadLeaseV1 {
                reference,
                request: *request,
            });
            self.readers[request.allocation.slot] += 1;
            output[index] = Some(reference);
        }
        self.next_incarnation = next;
        Ok(())
    }
}
