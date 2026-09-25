use super::*;
use crate::context_read_leases::StableReadFaultV1;

fn compare(
    owner: &mut ContextProducerReadJournalV1,
    writer: ContextWriterReferenceV1,
    expected: Result<(), Error>,
    accesses: usize,
) {
    let journal = owner.guard_copy_for_test_v1();
    let before = snapshot(owner);
    let storage = owner.guard_owner_storage_v1();
    let arena = (
        owner.reservations.clone(),
        owner.free.clone(),
        owner.counts.clone(),
        owner.next_incarnation,
    );
    owner.reset_access_count_for_test_v1();
    assert_eq!(owner.baseline_mark_unknown_v1(writer), expected);
    assert_eq!(owner.guard_accesses_for_test_v1(), accesses);
    let frozen = snapshot(owner);
    assert_eq!(owner.guard_owner_storage_v1(), storage);
    owner.guard_restore_owner_v1(&journal, writer, &[]);
    assert_eq!(snapshot(owner), before);
    assert_eq!(owner.mark_unknown(writer), expected);
    assert_eq!(owner.guard_accesses_for_test_v1(), accesses);
    assert_eq!(snapshot(owner), frozen);
    assert_eq!(owner.guard_owner_storage_v1(), storage);
    assert_eq!(
        (
            owner.reservations.clone(),
            owner.free.clone(),
            owner.counts.clone(),
            owner.next_incarnation,
        ),
        arena
    );
    if expected.is_err() {
        assert_eq!(snapshot(owner), before);
    }
}

#[test]
fn unknown_shared_preserves_live_custody_and_revalidates_on_replay() {
    let mut f = Fixture::new(4);
    let mut producer = [None];
    f.journal
        .acquire_producer_reads(key(20), &[f.request], &mut producer)
        .unwrap();
    let producer = producer[0].unwrap();
    let read = ContextAllocationReadV1 {
        allocation: f.other,
        attempt_epoch: 0,
        ..f.request.read
    };
    let mut stable = [None];
    f.journal
        .acquire_reads(key(20), &[read], &mut stable)
        .unwrap();
    let allocation = f.journal.lookup_allocation(f.source).unwrap();
    compare(&mut f.journal, f.producer, Ok(()), 4);
    assert_eq!(
        f.journal.producer_read_status(producer),
        Ok(Status::Unknown)
    );
    assert_eq!(f.journal.lookup_producer_read(producer), Ok(f.request));
    assert_eq!(f.journal.lookup_read(stable[0].unwrap()), Ok(read));
    assert_eq!(f.journal.lookup_allocation(f.source), Ok(allocation));
    assert_eq!(f.journal.reader_count(f.source), Ok(1));
    let unknown = snapshot(&f.journal);
    compare(&mut f.journal, f.producer, Ok(()), 3);
    assert_eq!(snapshot(&f.journal), unknown);
    assert_eq!(
        f.journal.validate_producer_read(&f.request),
        Err(Error::AllocationBusy)
    );
    f.journal
        .release_producer_reads(
            key(20),
            &[producer],
            &ContextReadQuiescenceEvidenceV1 { consumer: key(20) },
        )
        .unwrap();
    assert_eq!(f.journal.reader_count(f.source), Ok(0));
    assert_eq!(f.journal.lookup_read(stable[0].unwrap()), Ok(read));
}

#[test]
fn unknown_shared_validates_corrupt_unknown_and_prioritizes_writer_identity() {
    for repeated in [false, true] {
        let mut f = Fixture::new(4);
        if repeated {
            f.journal.mark_unknown(f.producer).unwrap();
        }
        f.journal.stable.guard_break_backlink_for_test_v1(f.source);
        for writer in [
            ContextWriterReferenceV1 {
                slot: usize::MAX,
                ..f.producer
            },
            ContextWriterReferenceV1 {
                key: key(11),
                ..f.producer
            },
            ContextWriterReferenceV1 {
                key: ContextWriterKeyV1 {
                    context_generation: 8,
                    ..f.producer.key
                },
                ..f.producer
            },
            ContextWriterReferenceV1 {
                key: ContextWriterKeyV1 {
                    kind: ContextWriterKindV1::Synchronous,
                    ..f.producer.key
                },
                ..f.producer
            },
        ] {
            compare(&mut f.journal, writer, Err(Error::InvalidReference), 1);
        }
        compare(&mut f.journal, f.producer, Err(Error::InvalidState), 3);
    }
}

#[test]
fn unknown_shared_ignores_unrelated_malformed_reader_storage_and_budget() {
    for repeated in [false, true] {
        let mut f = Fixture::new(4);
        if repeated {
            f.journal.mark_unknown(f.producer).unwrap();
        }
        f.journal.counts.clear();
        f.journal.free.extend([usize::MAX; 5]);
        f.journal.next_incarnation = 0;
        f.journal
            .stable
            .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(0));
        f.journal
            .stable
            .fault_reads_for_test_v1(StableReadFaultV1::NextIncarnation(u64::MAX));
        f.journal
            .stable
            .fault_reads_for_test_v1(StableReadFaultV1::FreeSlotFromEnd {
                distance: 1,
                slot: usize::MAX,
            });
        compare(
            &mut f.journal,
            f.producer,
            Ok(()),
            if repeated { 3 } else { 4 },
        );
    }
}

#[test]
fn unknown_shared_empty_chain_and_both_writer_kinds() {
    for kind in [
        ContextWriterKindV1::Submission,
        ContextWriterKindV1::Synchronous,
    ] {
        let mut owner = ContextProducerReadJournalV1::new(7, 4, 4, 4).unwrap();
        let writer = owner
            .register_writer(ContextWriterKeyV1 { kind, ..key(10) })
            .unwrap();
        compare(&mut owner, writer, Err(Error::InvalidReference), 1);
        owner.begin_write(writer, &[]).unwrap();
        compare(&mut owner, writer, Ok(()), 2);
        let before = snapshot(&owner);
        compare(&mut owner, writer, Ok(()), 1);
        assert_eq!(snapshot(&owner), before);
    }
}

#[test]
fn unknown_shared_traversal_depends_on_chain_length_not_arena_capacity() {
    for (k, capacity) in [
        (1, 8),
        (8, 8),
        (8, 65536),
        (64, 1024),
        (512, 1024),
        (4096, 8192),
    ] {
        let mut owner = ContextProducerReadJournalV1::new(7, capacity, 4, 4).unwrap();
        let device = ContextJournalDeviceKeyV1 {
            context_generation: 7,
            local: 1,
        };
        let roster: Vec<_> = (0..k)
            .map(|i| ContextAllocationWriteV1 {
                allocation: owner
                    .enroll_allocation(
                        ContextAllocationKeyV1 {
                            context_generation: 7,
                            local: i as u64 + 1,
                        },
                        device,
                        64,
                    )
                    .unwrap(),
                device,
                byte_extent: 64,
            })
            .collect();
        let writer = owner.register_writer(key(10)).unwrap();
        owner.begin_write(writer, &roster).unwrap();
        compare(&mut owner, writer, Ok(()), 2 * k + 2);
        compare(&mut owner, writer, Ok(()), 2 * k + 1);
        owner
            .stable
            .guard_break_backlink_for_test_v1(roster[k - 1].allocation);
        compare(&mut owner, writer, Err(Error::InvalidState), 2 * k + 1);
    }
}
