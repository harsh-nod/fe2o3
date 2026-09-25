// Frozen at 36e5c16494da7bee7d743c3c98a52d4130396f96; method names and visibility only.
use super::*;

impl ContextVersionJournalV1 {
    pub(crate) const fn baseline_inspection_context_generation_v1(&self) -> u64 {
        self.context_generation
    }

    pub(crate) const fn baseline_inspection_allocation_capacity_v1(&self) -> usize {
        self.allocation_capacity
    }

    pub(crate) const fn baseline_inspection_writer_capacity_v1(&self) -> usize {
        self.writer_capacity
    }

    pub(crate) const fn baseline_inspection_registration_watermark_v1(&self) -> u64 {
        self.registration_watermark
    }

    pub(crate) fn baseline_inspection_remaining_writer_slots_v1(&self) -> usize {
        self.free.len()
    }

    pub(crate) fn baseline_inspection_reserved_writer_count_v1(&self) -> usize {
        self.reserved_count
    }

    pub(crate) fn baseline_inspection_remaining_allocation_slots_v1(&self) -> usize {
        self.allocation_free.len()
    }
}
