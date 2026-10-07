use super::*;
use alloc::vec;
use core::ops::Deref;

fn compare(owner: &ContextProducerReadJournalV1) {
    let before = alloc::format!("{owner:?}");
    let buffers = owner.guard_owner_storage_v1();
    let frozen_stable = owner.baseline_inspection_deref_v1();
    let frozen_journal = frozen_stable.baseline_inspection_deref_v1();
    let accesses = frozen_journal.guard_accesses_for_test_v1();
    let explicit_stable = <ContextProducerReadJournalV1 as Deref>::deref(owner);
    let explicit_journal = <ContextReadLeasedJournalV1 as Deref>::deref(explicit_stable);
    let automatic_stable: &ContextReadLeasedJournalV1 = owner;
    let automatic_journal: &ContextVersionJournalV1 = owner;
    assert!(core::ptr::eq(frozen_stable, &owner.stable));
    assert!(core::ptr::eq(explicit_stable, frozen_stable));
    assert!(core::ptr::eq(automatic_stable, frozen_stable));
    assert!(core::ptr::eq(explicit_journal, frozen_journal));
    assert!(core::ptr::eq(automatic_journal, frozen_journal));
    assert_eq!(
        (
            owner.context_generation(),
            owner.allocation_capacity(),
            owner.writer_capacity(),
            owner.registration_watermark(),
            owner.remaining_writer_slots(),
            owner.reserved_writer_count(),
            owner.remaining_allocation_slots(),
            automatic_stable.remaining_read_slots(),
        ),
        (
            frozen_journal.baseline_inspection_context_generation_v1(),
            frozen_journal.baseline_inspection_allocation_capacity_v1(),
            frozen_journal.baseline_inspection_writer_capacity_v1(),
            frozen_journal.baseline_inspection_registration_watermark_v1(),
            frozen_journal.baseline_inspection_remaining_writer_slots_v1(),
            frozen_journal.baseline_inspection_reserved_writer_count_v1(),
            frozen_journal.baseline_inspection_remaining_allocation_slots_v1(),
            frozen_stable.baseline_inspection_remaining_read_slots_v1(),
        )
    );
    assert_eq!(alloc::format!("{owner:?}"), before);
    assert_eq!(owner.guard_owner_storage_v1(), buffers);
    assert_eq!(frozen_journal.guard_accesses_for_test_v1(), accesses);
}

#[test]
fn inspection_shared_producer_live_projections_and_shadowed_budget() {
    for status in 0..4 {
        let mut fixture = Fixture::new(4);
        fixture.acquire(20);
        let stable_read = fixture.stable_read(fixture.other);
        fixture
            .journal
            .acquire_reads(key(21), &[stable_read], &mut [None])
            .unwrap();
        match status {
            0 => {}
            1 => fixture.journal.mark_unknown(fixture.producer).unwrap(),
            2 => fixture
                .journal
                .settle_success(
                    fixture.producer,
                    &ContextWriterSuccessEvidenceV1 {
                        writer: fixture.producer,
                    },
                )
                .unwrap(),
            _ => fixture
                .journal
                .settle_no_effect(
                    fixture.producer,
                    &ContextWriterNoEffectEvidenceV1 {
                        writer: fixture.producer,
                    },
                )
                .unwrap(),
        }
        compare(&fixture.journal);
        let stable: &ContextReadLeasedJournalV1 = &fixture.journal;
        assert_eq!(stable.remaining_read_slots(), 3);
        assert_eq!(fixture.journal.remaining_read_slots(), 2);
        assert_eq!(
            fixture.journal.remaining_read_slots(),
            fixture.journal.baseline_remaining_read_slots_v1()
        );
    }
}

#[test]
fn inspection_shared_producer_raw_nested_projections() {
    for free in [vec![], vec![usize::MAX], vec![0, 0, usize::MAX, 0, 0, 0]] {
        let mut owner = ContextProducerReadJournalV1::new(7, 3, 2, 4).unwrap();
        owner.reservations.clear();
        owner.free = free;
        owner.counts.clear();
        owner.next_incarnation = u64::MAX;
        owner
            .stable
            .fault_enrollment_for_test_v1(&[usize::MAX, usize::MAX], usize::MAX);
        owner
            .stable
            .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(0));
        owner
            .stable
            .fault_reads_for_test_v1(StableReadFaultV1::NextIncarnation(0));
        owner
            .stable
            .fault_reads_for_test_v1(StableReadFaultV1::FreeSlotFromEnd {
                distance: 1,
                slot: usize::MAX,
            });
        compare(&owner);
    }
}
