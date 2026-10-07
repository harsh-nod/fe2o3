use super::*;
use crate::context_read_leases::StableReadFaultV1;

fn storage(owner: &ContextProducerReadJournalV1) -> Vec<(usize, usize)> {
    let mut result = owner.stable.guard_owner_storage_v1();
    result.extend([
        (
            owner.reservations.as_ptr() as usize,
            owner.reservations.capacity(),
        ),
        (owner.free.as_ptr() as usize, owner.free.capacity()),
        (owner.counts.as_ptr() as usize, owner.counts.capacity()),
    ]);
    result
}

fn rosters(f: &Fixture) -> ([ContextAllocationReadV1; 2], [ContextProducerReadV1; 2]) {
    let stable = f.stable_read(f.other);
    (
        [
            stable,
            ContextAllocationReadV1 {
                byte_offset: 16,
                ..stable
            },
        ],
        [
            f.request,
            ContextProducerReadV1 {
                read: ContextAllocationReadV1 {
                    byte_offset: 32,
                    ..f.request.read
                },
                ..f.request
            },
        ],
    )
}

#[test]
fn mixed_acquire_exact_rosters_share_one_budget_without_storage_growth() {
    let mut f = Fixture::new(4);
    let (stable, producers) = rosters(&f);
    let before_storage = storage(&f.journal);
    let writer = f.journal.lookup_writer(f.producer).unwrap();
    let mut reads = [None; 2];
    let mut pending = [None; 2];
    f.journal
        .acquire_mixed_reads(key(20), &stable, &mut reads, &producers, &mut pending)
        .unwrap();
    assert_eq!(storage(&f.journal), before_storage);
    assert_eq!(f.journal.lookup_writer(f.producer).unwrap(), writer);
    assert_eq!(f.journal.retained_read_count(), 4);
    assert_eq!(f.journal.remaining_read_slots(), 0);
    assert_eq!(f.journal.reader_count(f.other), Ok(2));
    assert_eq!(f.journal.reader_count(f.source), Ok(2));
    for index in 0..2 {
        let read = reads[index].unwrap();
        let reservation = pending[index].unwrap();
        assert_eq!(read.consumer, key(20));
        assert_eq!(reservation.consumer, key(20));
        assert_eq!(f.journal.lookup_read(read), Ok(stable[index]));
        assert_eq!(
            f.journal.lookup_producer_read(reservation),
            Ok(producers[index])
        );
        assert_eq!(
            f.journal.producer_read_status(reservation),
            Ok(Status::Pending)
        );
    }
    let evidence = ContextReadQuiescenceEvidenceV1 { consumer: key(20) };
    f.journal
        .release_reads(key(20), &reads.map(Option::unwrap), &evidence)
        .unwrap();
    f.journal
        .release_producer_reads(key(20), &pending.map(Option::unwrap), &evidence)
        .unwrap();
    assert_eq!(f.journal.retained_read_count(), 0);
    assert_eq!(storage(&f.journal), before_storage);
}

#[test]
fn mixed_acquire_late_rejection_on_either_side_preserves_every_owner_and_output() {
    for producer_side in [false, true] {
        let mut f = Fixture::new(4);
        let (mut stable, mut producers) = rosters(&f);
        if producer_side {
            producers[1].read.byte_len = 0;
        } else {
            stable[1].byte_len = 0;
        }
        let before = snapshot(&f.journal);
        let before_storage = storage(&f.journal);
        let mut reads = [None; 2];
        let mut pending = [None; 2];
        assert_eq!(
            f.journal
                .acquire_mixed_reads(key(20), &stable, &mut reads, &producers, &mut pending),
            Err(Error::InvalidExtent)
        );
        assert_eq!(snapshot(&f.journal), before);
        assert_eq!(storage(&f.journal), before_storage);
        assert_eq!(reads, [None; 2]);
        assert_eq!(pending, [None; 2]);
    }
}

#[test]
fn mixed_acquire_checks_combined_headroom_before_either_commit() {
    let mut f = Fixture::new(3);
    let (stable, producers) = rosters(&f);
    assert!(f.journal.validate_read_capacity(2).is_ok());
    assert!(f.journal.validate_producer_read_capacity(2).is_ok());
    let before = snapshot(&f.journal);
    let mut reads = [None; 2];
    let mut pending = [None; 2];
    assert_eq!(
        f.journal
            .acquire_mixed_reads(key(20), &stable, &mut reads, &producers, &mut pending),
        Err(Error::MemberCapacity)
    );
    assert_eq!(snapshot(&f.journal), before);
    assert_eq!(reads, [None; 2]);
    assert_eq!(pending, [None; 2]);
}

#[test]
fn mixed_acquire_independent_incarnation_exhaustion_is_atomic() {
    for stable_side in [false, true] {
        for incarnation in [0, u64::MAX - 1, u64::MAX] {
            let mut f = Fixture::new(4);
            let (stable, producers) = rosters(&f);
            if stable_side {
                f.journal
                    .stable
                    .fault_reads_for_test_v1(StableReadFaultV1::NextIncarnation(incarnation));
            } else {
                f.journal.next_incarnation = incarnation;
            }
            let before = snapshot(&f.journal);
            let mut reads = [None; 2];
            let mut pending = [None; 2];
            assert_eq!(
                f.journal.acquire_mixed_reads(
                    key(20),
                    &stable,
                    &mut reads,
                    &producers,
                    &mut pending
                ),
                Err(Error::EpochExhausted)
            );
            assert_eq!(snapshot(&f.journal), before);
            assert_eq!(reads, [None; 2]);
            assert_eq!(pending, [None; 2]);
        }
    }
}

#[test]
fn mixed_acquire_empty_sides_skip_only_the_unused_arena() {
    for stable_count in 0..=1 {
        for producer_count in 0..=1 {
            let mut f = Fixture::new(2);
            let (stable, producers) = rosters(&f);
            if stable_count == 0 {
                f.journal
                    .stable
                    .fault_reads_for_test_v1(StableReadFaultV1::NextIncarnation(0));
            }
            if producer_count == 0 {
                f.journal.next_incarnation = 0;
            }
            let before = snapshot(&f.journal);
            let mut reads = [None];
            let mut pending = [None];
            f.journal
                .acquire_mixed_reads(
                    key(20),
                    &stable[..stable_count],
                    &mut reads[..stable_count],
                    &producers[..producer_count],
                    &mut pending[..producer_count],
                )
                .unwrap();
            assert_eq!(
                f.journal.retained_read_count(),
                stable_count + producer_count
            );
            if stable_count + producer_count == 0 {
                assert_eq!(snapshot(&f.journal), before);
            }
        }
    }
}

#[test]
fn mixed_acquire_validates_consumer_empty_output_shapes_and_occupied_outputs() {
    for fault in 0..8 {
        let mut f = Fixture::new(4);
        let (stable, producers) = rosters(&f);
        let mut consumer = key(20);
        let mut reads = [None; 2];
        let mut pending = [None; 2];
        let mut stable_count = 2;
        let mut producer_count = 2;
        let expected = match fault {
            0 => {
                consumer.context_generation += 1;
                Error::ForeignContext
            }
            1 => {
                consumer.kind = ContextWriterKindV1::Synchronous;
                Error::InvalidWriterId
            }
            2 => {
                consumer.local = 0;
                Error::InvalidWriterId
            }
            3 => {
                consumer.local = u64::MAX;
                Error::InvalidWriterId
            }
            4 => {
                stable_count = 0;
                Error::RosterCapacity
            }
            5 => {
                producer_count = 0;
                Error::RosterCapacity
            }
            6 => {
                reads[1] = Some(ContextReadLeaseReferenceV1 {
                    slot: 0,
                    incarnation: 1,
                    consumer,
                });
                Error::InvalidState
            }
            7 => {
                pending[1] = Some(ContextProducerReadReferenceV1 {
                    slot: 0,
                    incarnation: 1,
                    consumer,
                });
                Error::InvalidState
            }
            _ => unreachable!(),
        };
        let before = snapshot(&f.journal);
        let original = (reads, pending);
        assert_eq!(
            f.journal.acquire_mixed_reads(
                consumer,
                &stable[..stable_count],
                &mut reads,
                &producers[..producer_count],
                &mut pending
            ),
            Err(expected)
        );
        assert_eq!(snapshot(&f.journal), before);
        assert_eq!((reads, pending), original);
    }
}

#[test]
fn mixed_acquire_canonical_rosters_and_cross_kind_aliases_reject_atomically() {
    for fault in 0..5 {
        let mut f = Fixture::new(4);
        let (mut stable, mut producers) = rosters(&f);
        let expected = match fault {
            0 => {
                stable.swap(0, 1);
                Error::NonCanonicalRoster
            }
            1 => {
                producers.swap(0, 1);
                Error::NonCanonicalRoster
            }
            2 => {
                stable[1] = stable[0];
                Error::NonCanonicalRoster
            }
            3 => {
                producers[1] = producers[0];
                Error::NonCanonicalRoster
            }
            4 => {
                stable[0] = f.request.read;
                Error::AllocationBusy
            }
            _ => unreachable!(),
        };
        let before = snapshot(&f.journal);
        let mut reads = [None; 2];
        let mut pending = [None; 2];
        assert_eq!(
            f.journal
                .acquire_mixed_reads(key(20), &stable, &mut reads, &producers, &mut pending),
            Err(expected)
        );
        assert_eq!(snapshot(&f.journal), before);
        assert_eq!((reads, pending), ([None; 2], [None; 2]));
    }
}
