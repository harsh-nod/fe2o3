// Frozen at f1022d0d3d22f4773a6b40d8ed1e040051ce512f; reserve hooks and names only.
use super::*;
use crate::context_version_journal::construction::ConstructorAllocatorV1;

fn baseline_storage_v1<T>(
    capacity: usize,
    site: u8,
    allocator: &mut impl ConstructorAllocatorV1,
) -> Result<Vec<T>, ContextVersionJournalErrorV1> {
    let mut result = Vec::new();
    allocator
        .reserve(site, capacity, &mut result)
        .then_some(())
        .ok_or(ContextVersionJournalErrorV1::StorageAllocationFailed)?;
    Ok(result)
}

impl ContextProducerReadJournalV1 {
    pub(crate) fn baseline_new_with_allocator_v1(
        generation: u64,
        allocations: usize,
        writers: usize,
        reads: usize,
        allocator: &mut impl ConstructorAllocatorV1,
    ) -> Result<Self, ContextVersionJournalErrorV1> {
        let stable = ContextReadLeasedJournalV1::baseline_new_with_allocator_v1(
            generation,
            allocations,
            writers,
            reads,
            allocator,
        )?;
        let mut reservations = baseline_storage_v1(reads, 10, allocator)?;
        reservations.resize(reads, None);
        let mut free = baseline_storage_v1(reads, 11, allocator)?;
        free.extend((0..reads).rev());
        let mut counts = baseline_storage_v1(allocations, 12, allocator)?;
        counts.resize(allocations, 0);
        Ok(Self {
            stable,
            reservations,
            free,
            counts,
            next_incarnation: 1,
        })
    }
}
