// Frozen at f1022d0d3d22f4773a6b40d8ed1e040051ce512f; reserve hooks and names only.
use super::*;
use construction::ConstructorAllocatorV1;

fn baseline_issuable_context_id_v1(value: u64) -> bool {
    value != 0 && value != u64::MAX
}

fn baseline_vacant_slots_v1<T>(
    capacity: usize,
    site: u8,
    allocator: &mut impl ConstructorAllocatorV1,
) -> Result<Vec<Option<T>>, ContextVersionJournalErrorV1> {
    let mut slots = Vec::new();
    allocator
        .reserve(site, capacity, &mut slots)
        .then_some(())
        .ok_or(ContextVersionJournalErrorV1::StorageAllocationFailed)?;
    slots.resize_with(capacity, || None);
    Ok(slots)
}

fn baseline_free_slots_v1(
    capacity: usize,
    site: u8,
    allocator: &mut impl ConstructorAllocatorV1,
) -> Result<Vec<usize>, ContextVersionJournalErrorV1> {
    let mut free = Vec::new();
    allocator
        .reserve(site, capacity, &mut free)
        .then_some(())
        .ok_or(ContextVersionJournalErrorV1::StorageAllocationFailed)?;
    free.extend((0..capacity).rev());
    Ok(free)
}

impl ContextVersionJournalV1 {
    pub(crate) fn baseline_new_with_allocator_v1(
        context_generation: u64,
        allocation_capacity: usize,
        writer_capacity: usize,
        allocator: &mut impl ConstructorAllocatorV1,
    ) -> Result<Self, ContextVersionJournalErrorV1> {
        if !baseline_issuable_context_id_v1(context_generation) {
            return Err(ContextVersionJournalErrorV1::InvalidContextGeneration);
        }
        if !(1..=CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1).contains(&allocation_capacity)
            || !(1..=CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1).contains(&writer_capacity)
        {
            return Err(ContextVersionJournalErrorV1::InvalidCapacity);
        }
        Ok(Self {
            context_generation,
            allocation_capacity,
            writer_capacity,
            registration_watermark: 0,
            reserved_count: 0,
            writers: baseline_vacant_slots_v1(writer_capacity, 0, allocator)?,
            free: baseline_free_slots_v1(writer_capacity, 1, allocator)?,
            allocations: baseline_vacant_slots_v1(allocation_capacity, 2, allocator)?,
            allocation_free: baseline_free_slots_v1(allocation_capacity, 3, allocator)?,
            members: baseline_vacant_slots_v1(allocation_capacity, 4, allocator)?,
            member_free: baseline_free_slots_v1(allocation_capacity, 5, allocator)?,
            scratch: baseline_vacant_slots_v1(allocation_capacity, 6, allocator)?,
            #[cfg(test)]
            indexed_accesses: Cell::new(0),
        })
    }
}
