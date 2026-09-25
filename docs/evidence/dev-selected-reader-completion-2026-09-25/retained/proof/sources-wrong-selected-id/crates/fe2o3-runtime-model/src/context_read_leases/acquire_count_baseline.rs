// Frozen from 4c237366268542e1a19542b25c00f7b58d9f24a8; only method names/visibility change.
use super::*;

impl ContextReadLeasedJournalV1 {
    pub(crate) fn baseline_retained_read_count_v1(&self) -> usize {
        self.leases.len() - self.free_reads.len()
    }
}
