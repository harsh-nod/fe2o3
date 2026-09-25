use super::*;
use alloc::vec;
use core::ops::Deref;

fn compare(owner: &Journal) {
    let before = alloc::format!("{owner:?}");
    let buffers = owner.guard_owner_storage_v1();
    let frozen = owner.baseline_inspection_deref_v1();
    let accesses = frozen.guard_accesses_for_test_v1();
    let explicit = <Journal as Deref>::deref(owner);
    let automatic: &ContextVersionJournalV1 = owner;
    assert!(core::ptr::eq(frozen, &owner.journal));
    assert!(core::ptr::eq(explicit, frozen));
    assert!(core::ptr::eq(automatic, frozen));
    assert_eq!(
        (
            owner.context_generation(),
            owner.allocation_capacity(),
            owner.writer_capacity(),
            owner.registration_watermark(),
            owner.remaining_writer_slots(),
            owner.reserved_writer_count(),
            owner.remaining_allocation_slots(),
            owner.remaining_read_slots(),
        ),
        (
            frozen.baseline_inspection_context_generation_v1(),
            frozen.baseline_inspection_allocation_capacity_v1(),
            frozen.baseline_inspection_writer_capacity_v1(),
            frozen.baseline_inspection_registration_watermark_v1(),
            frozen.baseline_inspection_remaining_writer_slots_v1(),
            frozen.baseline_inspection_reserved_writer_count_v1(),
            frozen.baseline_inspection_remaining_allocation_slots_v1(),
            owner.baseline_inspection_remaining_read_slots_v1(),
        )
    );
    assert_eq!(alloc::format!("{owner:?}"), before);
    assert_eq!(owner.guard_owner_storage_v1(), buffers);
    assert_eq!(frozen.guard_accesses_for_test_v1(), accesses);
}

#[test]
fn inspection_shared_stable_live_projection_and_getters() {
    let (mut owner, requests) = fixture(4);
    compare(&owner);
    let mut output = [None; 2];
    owner
        .acquire_reads(consumer(20), &requests[..2], &mut output)
        .unwrap();
    let writer = owner.register_writer(consumer(30)).unwrap();
    compare(&owner);
    owner.abort_reserved(writer).unwrap();
    owner
        .release_reads(
            consumer(20),
            &[output[0].unwrap(), output[1].unwrap()],
            &ContextReadQuiescenceEvidenceV1 {
                consumer: consumer(20),
            },
        )
        .unwrap();
    compare(&owner);
}

#[test]
fn inspection_shared_stable_raw_projection_and_free_length() {
    for free in [vec![], vec![usize::MAX], vec![0, 0, usize::MAX, 0, 0, 0]] {
        let mut owner = Journal::new(7, 3, 2, 4).unwrap();
        owner.leases.clear();
        owner.free_reads = free;
        owner.readers.clear();
        owner.next_incarnation = 0;
        owner
            .journal
            .fault_enrollment_for_test_v1(&[usize::MAX, usize::MAX], usize::MAX);
        compare(&owner);
    }
}
