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
            local: 1,
        },
        byte_extent: 64,
    }
}

fn compare(
    owner: &mut ContextProducerReadJournalV1,
    entries: &[ContextAllocationEnrollmentV1],
    original: &[Option<ContextAllocationReferenceV1>],
    expected: Result<(), Error>,
    accesses: usize,
) -> Vec<Option<ContextAllocationReferenceV1>> {
    let journal = owner.guard_copy_for_test_v1();
    let before = snapshot(owner);
    let storage = owner.guard_owner_storage_v1();
    let mut output = original.to_vec();
    let output_storage = (output.as_ptr(), output.capacity());
    assert_eq!(
        owner.baseline_enroll_allocations_v1(entries, &mut output),
        expected
    );
    assert_eq!(owner.guard_accesses_for_test_v1(), accesses);
    let frozen = snapshot(owner);
    let frozen_output = output.clone();
    assert_eq!(owner.guard_owner_storage_v1(), storage);
    owner.restore_enrollment_for_test_v1(&journal, entries.len());
    assert_eq!(snapshot(owner), before);
    output.copy_from_slice(original);
    assert_eq!(owner.enroll_allocations(entries, &mut output), expected);
    assert_eq!(owner.guard_accesses_for_test_v1(), accesses);
    assert_eq!(snapshot(owner), frozen);
    assert_eq!(output, frozen_output);
    assert_eq!(owner.guard_owner_storage_v1(), storage);
    assert_eq!((output.as_ptr(), output.capacity()), output_storage);
    if expected.is_err() || entries.is_empty() {
        assert_eq!(snapshot(owner), before);
        assert_eq!(output, original);
    }
    output
}

#[test]
fn enrollment_shared_preserves_all_producer_statuses_and_stable_custody() {
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
        let output = compare(
            &mut f.journal,
            &[entry(30), entry(31)],
            &[None, None],
            Ok(()),
            4,
        );
        assert_eq!(f.journal.producer_read_status(producer), Ok(status));
        assert_eq!(f.journal.lookup_producer_read(producer), Ok(f.request));
        assert_eq!(f.journal.lookup_read(stable[0].unwrap()), Ok(read));
        assert_eq!(f.journal.lookup_allocation(f.source), Ok(source));
        assert_eq!(f.journal.reader_count(f.source), Ok(1));
        assert_eq!(output[0].unwrap().key, entry(30).key);
        assert_eq!(output[1].unwrap().key, entry(31).key);
        assert_invariant(&f.journal);
    }
}

#[test]
fn enrollment_shared_restores_key_associations_after_sorting_slots() {
    let mut owner = ContextProducerReadJournalV1::new(7, 4, 1, 1).unwrap();
    owner.stable.fault_enrollment_for_test_v1(&[3, 1, 0, 2], 4);
    let entries = [entry(10), entry(20), entry(30)];
    let output = compare(&mut owner, &entries, &[None; 3], Ok(()), 4);
    for (index, slot) in [2, 0, 1].into_iter().enumerate() {
        let reference = output[index].unwrap();
        assert_eq!(reference.slot, slot);
        assert_eq!(reference.key, entries[index].key);
        assert_eq!(owner.lookup_allocation(reference).unwrap().attempt_epoch, 0);
    }
    assert_invariant(&owner);
}

#[test]
fn enrollment_shared_late_rejections_restore_output_and_entire_owner() {
    for free in [&[3, 2, 1, 1][..], &[1, 2, 0, 1], &[3, 2, 0, usize::MAX]] {
        let mut owner = ContextProducerReadJournalV1::new(7, 4, 1, 1).unwrap();
        owner.stable.fault_enrollment_for_test_v1(free, 4);
        compare(
            &mut owner,
            &[entry(10), entry(20)],
            &[None; 2],
            Err(Error::InvalidState),
            4,
        );
    }
    let mut f = Fixture::new(1);
    f.journal
        .stable
        .fault_enrollment_for_test_v1(&[3, f.source.slot], 4);
    compare(
        &mut f.journal,
        &[entry(10)],
        &[None],
        Err(Error::InvalidState),
        4,
    );
}

#[test]
fn enrollment_shared_preserves_raw_domain_without_reader_or_capacity_repairs() {
    let mut owner = ContextProducerReadJournalV1::new(7, 4, 1, 1).unwrap();
    owner
        .stable
        .fault_enrollment_for_test_v1(&[usize::MAX, 3], 2);
    owner.counts.clear();
    owner.free.extend([usize::MAX; 3]);
    owner.next_incarnation = 0;
    owner
        .stable
        .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(0));
    owner
        .stable
        .fault_reads_for_test_v1(StableReadFaultV1::NextIncarnation(u64::MAX));
    let output = compare(&mut owner, &[entry(10)], &[None], Ok(()), 4);
    // A selected slot may exceed logical capacity if it is in the actual arena.
    assert_eq!(output[0].unwrap().slot, 3);
    assert_eq!(owner.remaining_allocation_slots(), 1);
    assert!(owner.counts.is_empty());
    assert_eq!(owner.next_incarnation, 0);
    owner
        .stable
        .fault_enrollment_for_test_v1(&[usize::MAX; 5], 0);
    compare(&mut owner, &[], &[], Ok(()), 0);
}

#[test]
fn enrollment_shared_header_and_replay_precedence_is_unchanged() {
    let mut f = Fixture::new(1);
    f.journal
        .stable
        .fault_enrollment_for_test_v1(&[usize::MAX; 5], 4);
    let mut bad = entry(0);
    bad.device.context_generation = 8;
    bad.device.local = 0;
    bad.byte_extent = 0;
    compare(&mut f.journal, &[bad], &[], Err(Error::RosterCapacity), 0);
    compare(
        &mut f.journal,
        &[bad],
        &[Some(f.source)],
        Err(Error::InvalidState),
        0,
    );
    compare(
        &mut f.journal,
        &[bad],
        &[None],
        Err(Error::ForeignContext),
        0,
    );
    bad.device.context_generation = 7;
    compare(
        &mut f.journal,
        &[bad],
        &[None],
        Err(Error::InvalidAllocationId),
        0,
    );
    bad.key.local = 10;
    compare(
        &mut f.journal,
        &[bad],
        &[None],
        Err(Error::InvalidDeviceId),
        0,
    );
    bad.device.local = 1;
    compare(
        &mut f.journal,
        &[bad],
        &[None],
        Err(Error::InvalidExtent),
        0,
    );
    compare(
        &mut f.journal,
        &[entry(20), entry(10)],
        &[None; 2],
        Err(Error::NonCanonicalRoster),
        0,
    );
    compare(
        &mut f.journal,
        &[entry(1)],
        &[None],
        Err(Error::InvalidState),
        0,
    );
    f.journal.stable.fault_enrollment_for_test_v1(&[], 4);
    compare(
        &mut f.journal,
        &[entry(1)],
        &[None],
        Err(Error::AllocationReplay),
        1,
    );
    compare(
        &mut f.journal,
        &[entry(10)],
        &[None],
        Err(Error::AllocationCapacity),
        4,
    );
}

#[test]
fn enrollment_shared_retirement_reuse_keeps_old_references_stale() {
    let mut f = Fixture::new(2);
    let producer = f.acquire(20);
    f.journal.retire_allocations(&[f.other]).unwrap();
    let output = compare(&mut f.journal, &[entry(30)], &[None], Ok(()), 4);
    let replacement = output[0].unwrap();
    assert_eq!(replacement.slot, f.other.slot);
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

#[test]
fn enrollment_shared_counts_only_replay_scan_not_total_algorithmic_work() {
    for (k, capacity) in [
        (1, 8),
        (8, 8),
        (8, 65536),
        (64, 1024),
        (512, 1024),
        (4096, 8192),
    ] {
        let mut owner = ContextProducerReadJournalV1::new(7, capacity, 1, 1).unwrap();
        let entries: Vec<_> = (0..k).map(|i| entry(i as u64 + 1)).collect();
        compare(
            &mut owner,
            &entries,
            &alloc::vec![None; k],
            Ok(()),
            capacity,
        );
        compare(
            &mut owner,
            &[entries[k - 1]],
            &[None],
            Err(Error::AllocationReplay),
            k,
        );
    }
}
