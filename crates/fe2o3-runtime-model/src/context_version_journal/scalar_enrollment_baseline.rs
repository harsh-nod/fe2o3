// Frozen at 376a60343c906313fee87d1c78744bda662cb248; only names and visibility are redirected.
use super::*;

impl ContextVersionJournalV1 {
    pub(crate) fn baseline_enroll_allocation_v1(
        &mut self,
        key: ContextAllocationKeyV1,
        device: ContextJournalDeviceKeyV1,
        byte_extent: u64,
    ) -> Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1> {
        if key.context_generation != self.context_generation {
            return Err(ContextVersionJournalErrorV1::ForeignContext);
        }
        if !issuable_context_id(key.local) {
            return Err(ContextVersionJournalErrorV1::InvalidAllocationId);
        }
        if device.context_generation != self.context_generation {
            return Err(ContextVersionJournalErrorV1::ForeignContext);
        }
        if !issuable_context_id(device.local) {
            return Err(ContextVersionJournalErrorV1::InvalidDeviceId);
        }
        if byte_extent == 0 {
            return Err(ContextVersionJournalErrorV1::InvalidExtent);
        }
        for index in 0..self.allocations.len() {
            if self
                .read_allocation(index)
                .is_some_and(|entry| entry.key == key)
            {
                return Err(ContextVersionJournalErrorV1::AllocationReplay);
            }
        }
        self.count_indexed_access();
        let slot = self
            .allocation_free
            .last()
            .copied()
            .ok_or(ContextVersionJournalErrorV1::AllocationCapacity)?;
        self.count_indexed_access();
        if self.allocations.get(slot) != Some(&None) {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        self.count_indexed_access();
        let _ = self.allocation_free.pop();
        self.count_indexed_access();
        self.allocations[slot] = Some(AllocationEntryV1 {
            key,
            device,
            byte_extent,
            attempt_epoch: 0,
            content_lineage: 0,
            pending_member: None,
        });
        Ok(ContextAllocationReferenceV1 { slot, key })
    }
}
