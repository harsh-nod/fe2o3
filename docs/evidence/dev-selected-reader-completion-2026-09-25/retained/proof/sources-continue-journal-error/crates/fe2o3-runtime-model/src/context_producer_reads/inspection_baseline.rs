// Frozen at 36e5c16494da7bee7d743c3c98a52d4130396f96; method name, visibility and Target only.
use super::*;

impl ContextProducerReadJournalV1 {
    pub(crate) fn baseline_inspection_deref_v1(&self) -> &ContextReadLeasedJournalV1 {
        &self.stable
    }
}
