// Frozen at 36e5c16494da7bee7d743c3c98a52d4130396f96; method names, visibility and Target only.
use super::*;

impl ContextReadLeasedJournalV1 {
    pub(crate) fn baseline_inspection_remaining_read_slots_v1(&self) -> usize {
        self.free_reads.len()
    }

    pub(crate) fn baseline_inspection_deref_v1(&self) -> &ContextVersionJournalV1 {
        &self.journal
    }
}
