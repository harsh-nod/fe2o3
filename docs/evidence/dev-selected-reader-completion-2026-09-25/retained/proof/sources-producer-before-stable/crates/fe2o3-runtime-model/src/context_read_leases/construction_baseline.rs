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

impl ContextReadLeasedJournalV1 {
    pub(crate) fn baseline_new_with_allocator_v1(
        generation: u64,
        allocations: usize,
        writers: usize,
        reads: usize,
        allocator: &mut impl ConstructorAllocatorV1,
    ) -> Result<Self, ContextVersionJournalErrorV1> {
        if !(1..=CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1).contains(&reads) {
            return Err(ContextVersionJournalErrorV1::InvalidCapacity);
        }
        let journal = ContextVersionJournalV1::baseline_new_with_allocator_v1(
            generation,
            allocations,
            writers,
            allocator,
        )?;
        let mut leases = baseline_storage_v1(reads, 7, allocator)?;
        leases.resize(reads, None);
        let mut free_reads = baseline_storage_v1(reads, 8, allocator)?;
        free_reads.extend((0..reads).rev());
        let mut readers = baseline_storage_v1(allocations, 9, allocator)?;
        readers.resize(allocations, 0);
        Ok(Self {
            journal,
            leases,
            free_reads,
            readers,
            next_incarnation: 1,
        })
    }
}
