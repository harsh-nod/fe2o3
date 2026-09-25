// Frozen from e8134e770a30d12aa4abd190bd29c5083ea5b3b5; only method names/visibility change.
use super::*;

impl ContextVersionJournalV1 {
    pub(crate) fn baseline_lookup_allocation_v1(
        &self,
        reference: ContextAllocationReferenceV1,
    ) -> Result<ContextAllocationStateV1, ContextVersionJournalErrorV1> {
        let entry = self.exact_allocation(reference)?;
        let pending_writer = match entry.pending_member {
            None => None,
            Some(slot) => {
                self.count_indexed_access();
                let member = self
                    .members
                    .get(slot)
                    .and_then(Option::as_ref)
                    .filter(|member| member.allocation == reference)
                    .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
                Some(member.writer)
            }
        };
        Ok(ContextAllocationStateV1 {
            device: entry.device,
            byte_extent: entry.byte_extent,
            attempt_epoch: entry.attempt_epoch,
            content_lineage: entry.content_lineage,
            pending_writer,
        })
    }
}
