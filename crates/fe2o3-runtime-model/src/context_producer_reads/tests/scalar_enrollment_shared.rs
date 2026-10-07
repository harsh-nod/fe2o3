use super::*;
use crate::context_read_leases::StableReadFaultV1;

fn entry(local: u64) -> ContextAllocationEnrollmentV1 {
    ContextAllocationEnrollmentV1 {
        key: ContextAllocationKeyV1 {
            context_generation: 7,
            local,
        },
        device: ContextJournalDeviceKeyV1 {
            context_generation: 7,
            local: 11,
        },
        byte_extent: u64::MAX,
    }
}

fn compare(
    owner: &mut ContextProducerReadJournalV1,
    input: ContextAllocationEnrollmentV1,
    expected: Result<ContextAllocationReferenceV1, Error>,
    accesses: usize,
) {
    let saved = owner.guard_copy_for_test_v1();
    let before = snapshot(owner);
    let storage = owner.guard_owner_storage_v1();
    assert_eq!(
        owner.baseline_enroll_allocation_v1(input.key, input.device, input.byte_extent),
        expected
    );
    assert_eq!(owner.guard_accesses_for_test_v1(), accesses);
    let frozen = snapshot(owner);
    assert_eq!(owner.guard_owner_storage_v1(), storage);
    // Each public forwarding layer is compared on the same original allocation.
    for stable_only in [true, false] {
        owner.restore_enrollment_for_test_v1(&saved, 1);
        assert_eq!(snapshot(owner), before);
        assert_eq!(owner.guard_owner_storage_v1(), storage);
        let result = if stable_only {
            owner
                .stable
                .enroll_allocation(input.key, input.device, input.byte_extent)
        } else {
            owner.enroll_allocation(input.key, input.device, input.byte_extent)
        };
        assert_eq!(result, expected);
        assert_eq!(owner.guard_accesses_for_test_v1(), accesses);
        assert_eq!(snapshot(owner), frozen);
        assert_eq!(owner.guard_owner_storage_v1(), storage);
        if expected.is_err() {
            assert_eq!(snapshot(owner), before);
        }
    }
}

#[test]
fn scalar_enrollment_shared_preserves_live_stable_and_all_producer_statuses() {
    for status in [
        Status::Pending,
        Status::Success,
        Status::NoEffect,
        Status::Unknown,
    ] {
        let mut f = Fixture::new(4);
        let producer = f.acquire(20);
        let read = f.stable_read(f.other);
        let mut stable = [None];
        f.journal
            .acquire_reads(key(21), &[read], &mut stable)
            .unwrap();
        match status {
            Status::Pending => {}
            Status::Success => f
                .journal
                .settle_success(
                    f.producer,
                    &ContextWriterSuccessEvidenceV1 { writer: f.producer },
                )
                .unwrap(),
            Status::NoEffect => f
                .journal
                .settle_no_effect(
                    f.producer,
                    &ContextWriterNoEffectEvidenceV1 { writer: f.producer },
                )
                .unwrap(),
            Status::Unknown => f.journal.mark_unknown(f.producer).unwrap(),
        }
        let source = f.journal.lookup_allocation(f.source).unwrap();
        let input = entry(30);
        compare(
            &mut f.journal,
            input,
            Ok(ContextAllocationReferenceV1 {
                slot: 2,
                key: input.key,
            }),
            8,
        );
        compare(&mut f.journal, input, Err(Error::AllocationReplay), 3);
        assert_eq!(f.journal.producer_read_status(producer), Ok(status));
        assert_eq!(f.journal.lookup_producer_read(producer), Ok(f.request));
        assert_eq!(f.journal.lookup_read(stable[0].unwrap()), Ok(read));
        assert_eq!(f.journal.lookup_allocation(f.source), Ok(source));
        assert_eq!(f.journal.reader_count(f.source), Ok(1));
        assert_invariant(&f.journal);
    }
}

#[test]
fn scalar_enrollment_shared_frames_malformed_reader_metadata_on_success_and_error() {
    let mut owner = ContextProducerReadJournalV1::new(7, 4, 1, 1).unwrap();
    owner
        .stable
        .fault_enrollment_for_test_v1(&[usize::MAX, 3, 3, 3, 3], 0);
    owner.counts.clear();
    owner.free.extend([usize::MAX; 3]);
    owner.next_incarnation = 0;
    owner
        .stable
        .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(0));
    owner
        .stable
        .fault_reads_for_test_v1(StableReadFaultV1::NextIncarnation(u64::MAX));
    let input = entry(10);
    compare(
        &mut owner,
        input,
        Ok(ContextAllocationReferenceV1 {
            slot: 3,
            key: input.key,
        }),
        8,
    );
    compare(&mut owner, input, Err(Error::AllocationReplay), 4);
    compare(&mut owner, entry(20), Err(Error::InvalidState), 6);
    let mut invalid = entry(0);
    invalid.device.context_generation = 8;
    compare(&mut owner, invalid, Err(Error::InvalidAllocationId), 0);
    assert!(owner.counts.is_empty());
    assert_eq!(owner.next_incarnation, 0);
}

#[test]
fn scalar_enrollment_shared_reuses_retired_slot_without_reviving_reference() {
    let mut f = Fixture::new(2);
    let producer = f.acquire(20);
    f.journal.retire_allocations(&[f.other]).unwrap();
    let input = entry(30);
    let replacement = ContextAllocationReferenceV1 {
        slot: f.other.slot,
        key: input.key,
    };
    compare(&mut f.journal, input, Ok(replacement), 8);
    assert_eq!(
        f.journal.lookup_allocation(f.other),
        Err(Error::InvalidAllocationReference)
    );
    assert_eq!(f.journal.reader_count(replacement), Ok(0));
    assert_eq!(
        f.journal.producer_read_status(producer),
        Ok(Status::Pending)
    );
    assert_invariant(&f.journal);
}
