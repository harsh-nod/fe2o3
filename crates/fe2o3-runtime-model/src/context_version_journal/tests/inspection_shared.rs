use super::*;

fn compare(owner: &Journal) {
    let before = alloc::format!("{owner:?}");
    let buffers = storage(owner);
    let accesses = owner.guard_accesses_for_test_v1();
    let expected = (
        owner.baseline_inspection_context_generation_v1(),
        owner.baseline_inspection_allocation_capacity_v1(),
        owner.baseline_inspection_writer_capacity_v1(),
        owner.baseline_inspection_registration_watermark_v1(),
        owner.baseline_inspection_remaining_writer_slots_v1(),
        owner.baseline_inspection_reserved_writer_count_v1(),
        owner.baseline_inspection_remaining_allocation_slots_v1(),
    );
    let actual = (
        owner.context_generation(),
        owner.allocation_capacity(),
        owner.writer_capacity(),
        owner.registration_watermark(),
        owner.remaining_writer_slots(),
        owner.reserved_writer_count(),
        owner.remaining_allocation_slots(),
    );
    assert_eq!(actual, expected);
    assert_eq!(alloc::format!("{owner:?}"), before);
    assert_eq!(storage(owner), buffers);
    assert_eq!(owner.guard_accesses_for_test_v1(), accesses);
}

#[test]
fn inspection_shared_journal_normal_lifecycle() {
    for (generation, allocations, writers) in [(1, 1, 1), (7, 3, 2), (u64::MAX - 1, 5, 4)] {
        let mut owner = Journal::new(generation, allocations, writers).unwrap();
        compare(&owner);
        let writer = owner
            .register_writer(ContextWriterKeyV1 {
                context_generation: generation,
                local: 9,
                kind: ContextWriterKindV1::Submission,
            })
            .unwrap();
        compare(&owner);
        owner.abort_reserved(writer).unwrap();
        compare(&owner);
        let allocation = owner
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: generation,
                    local: 10,
                },
                ContextJournalDeviceKeyV1 {
                    context_generation: generation,
                    local: 2,
                },
                64,
            )
            .unwrap();
        compare(&owner);
        owner.retire_allocations(&[allocation]).unwrap();
        compare(&owner);
    }
}

#[test]
fn inspection_shared_journal_raw_fields_and_lengths() {
    for (generation, allocations, writers, watermark, reserved, free, allocation_free) in [
        (
            0,
            0,
            usize::MAX,
            u64::MAX,
            usize::MAX,
            vec![],
            vec![usize::MAX, 0, 0],
        ),
        (
            u64::MAX,
            usize::MAX,
            0,
            0,
            0,
            vec![usize::MAX, usize::MAX, 0, 0],
            vec![],
        ),
        (1, 1, 1, u64::MAX - 1, 7, vec![9], vec![1, 1, 1, 1, 1]),
    ] {
        let mut owner = Journal::new(7, 3, 2).unwrap();
        owner.context_generation = generation;
        owner.allocation_capacity = allocations;
        owner.writer_capacity = writers;
        owner.registration_watermark = watermark;
        owner.reserved_count = reserved;
        owner.writers.clear();
        owner.allocations.clear();
        owner.members.clear();
        owner.scratch.clear();
        owner.free = free;
        owner.allocation_free = allocation_free;
        owner.member_free = vec![usize::MAX];
        owner.indexed_accesses.set(19);
        compare(&owner);
    }
}

#[test]
fn inspection_shared_journal_const_getters() {
    const OBSERVATIONS: (u64, usize, usize, u64) = {
        let owner = Journal {
            context_generation: u64::MAX,
            allocation_capacity: usize::MAX,
            writer_capacity: 0,
            registration_watermark: u64::MAX - 1,
            reserved_count: usize::MAX,
            writers: Vec::new(),
            free: Vec::new(),
            allocations: Vec::new(),
            allocation_free: Vec::new(),
            members: Vec::new(),
            member_free: Vec::new(),
            scratch: Vec::new(),
            indexed_accesses: Cell::new(0),
        };
        let result = (
            owner.context_generation(),
            owner.allocation_capacity(),
            owner.writer_capacity(),
            owner.registration_watermark(),
        );
        core::mem::forget(owner);
        result
    };
    assert_eq!(OBSERVATIONS, (u64::MAX, usize::MAX, 0, u64::MAX - 1));
}
