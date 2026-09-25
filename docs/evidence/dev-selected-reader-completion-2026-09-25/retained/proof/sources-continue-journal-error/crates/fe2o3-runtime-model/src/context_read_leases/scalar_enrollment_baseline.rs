// Frozen at 376a60343c906313fee87d1c78744bda662cb248; only names and visibility are redirected.
use super::*;

impl ContextReadLeasedJournalV1 {
    pub(crate) fn baseline_enroll_allocation_v1(
        &mut self,
        key: ContextAllocationKeyV1,
        device: ContextJournalDeviceKeyV1,
        extent: u64,
    ) -> Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1> {
        self.journal
            .baseline_enroll_allocation_v1(key, device, extent)
    }
}
