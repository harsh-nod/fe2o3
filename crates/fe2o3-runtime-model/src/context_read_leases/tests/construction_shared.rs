use super::*;
use alloc::vec;
use core::mem::size_of;

use crate::context_version_journal::construction_probe::ReserveProbe;

fn requests(allocations: usize, writers: usize, reads: usize) -> [usize; 10] {
    [
        writers,
        writers,
        allocations,
        allocations,
        allocations,
        allocations,
        allocations,
        reads,
        reads,
        allocations,
    ]
}

fn reject(
    params: (u64, usize, usize, usize),
    outcomes: &[bool],
    expected: Error,
    expected_requests: &[usize],
) {
    let (generation, allocations, writers, reads) = params;
    let mut frozen = ReserveProbe::new(outcomes, 0);
    let mut actual = ReserveProbe::new(outcomes, 0);
    assert_eq!(
        Journal::baseline_new_with_allocator_v1(
            generation,
            allocations,
            writers,
            reads,
            &mut frozen
        )
        .unwrap_err(),
        expected
    );
    assert_eq!(
        Journal::new_with_allocator_v1(generation, allocations, writers, reads, &mut actual)
            .unwrap_err(),
        expected
    );
    frozen.assert_requests(expected_requests);
    actual.assert_requests(expected_requests);
    frozen.assert_same_trace(&actual);
}

#[test]
fn construction_shared_stable_success_contents_sites_and_storage() {
    for (generation, allocations, writers, reads) in [
        (1, 1, 1, 1),
        (7, 3, 2, 4),
        (u64::MAX - 1, 17, 9, 11),
        (7, 4097, 3, 5),
    ] {
        for extra in [0, 5] {
            let outcomes = [
                true, true, true, true, true, true, true, true, true, true, false,
            ];
            let mut frozen_probe = ReserveProbe::new(&outcomes, extra);
            let mut actual_probe = ReserveProbe::new(&outcomes, extra);
            let frozen = Journal::baseline_new_with_allocator_v1(
                generation,
                allocations,
                writers,
                reads,
                &mut frozen_probe,
            )
            .unwrap();
            let actual = Journal::new_with_allocator_v1(
                generation,
                allocations,
                writers,
                reads,
                &mut actual_probe,
            )
            .unwrap();
            let native = Journal::new(generation, allocations, writers, reads).unwrap();
            assert_eq!(snapshot(&frozen), snapshot(&actual));
            assert_eq!(snapshot(&frozen), snapshot(&native));
            let expected = requests(allocations, writers, reads);
            frozen_probe.assert_requests(&expected);
            actual_probe.assert_requests(&expected);
            frozen_probe.assert_same_trace(&actual_probe);
            frozen_probe.assert_storage(&frozen.guard_owner_storage_v1());
            actual_probe.assert_storage(&actual.guard_owner_storage_v1());
            assert_eq!(
                actual_probe.events[7].width,
                size_of::<Option<ReadLeaseV1>>()
            );
            assert_eq!(actual_probe.events[8].width, size_of::<usize>());
            assert_eq!(actual_probe.events[9].width, size_of::<usize>());
            for owner in [&frozen, &actual, &native] {
                assert_eq!(owner.context_generation(), generation);
                assert_eq!(owner.allocation_capacity(), allocations);
                assert_eq!(owner.writer_capacity(), writers);
                assert_eq!(owner.registration_watermark(), 0);
                assert_eq!(owner.reserved_writer_count(), 0);
                assert_eq!(owner.leases, vec![None; reads]);
                assert_eq!(owner.free_reads, (0..reads).rev().collect::<Vec<_>>());
                assert_eq!(owner.readers, vec![0; allocations]);
                assert_eq!(owner.next_incarnation, 1);
                assert_eq!(owner.remaining_read_slots(), reads);
                assert_reader_invariant(owner);
            }
        }
    }
}

#[test]
fn construction_shared_stable_stops_at_each_reservation_failure() {
    for failed in 0..10 {
        let mut outcomes = [true; 12];
        outcomes[failed] = false;
        reject(
            (7, 3, 2, 4),
            &outcomes,
            Error::StorageAllocationFailed,
            &requests(3, 2, 4)[..=failed],
        );
    }
}

#[test]
fn construction_shared_stable_missing_outcomes_stop_at_first_absence() {
    for present in 0..10 {
        reject(
            (7, 3, 2, 4),
            &[true; 10][..present],
            Error::StorageAllocationFailed,
            &requests(3, 2, 4)[..=present],
        );
    }
}

#[test]
fn construction_shared_stable_validation_precedence_without_allocation() {
    let maximum = CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1;
    for generation in [0, 7, u64::MAX] {
        for allocations in [0, 3, maximum + 1, usize::MAX] {
            for writers in [0, 2, maximum + 1, usize::MAX] {
                for reads in [0, 4, maximum + 1, usize::MAX] {
                    let error = if reads != 4 {
                        Error::InvalidCapacity
                    } else if generation == 0 || generation == u64::MAX {
                        Error::InvalidContextGeneration
                    } else if allocations != 3 || writers != 2 {
                        Error::InvalidCapacity
                    } else {
                        continue;
                    };
                    reject((generation, allocations, writers, reads), &[], error, &[]);
                }
            }
        }
    }
}

#[test]
fn construction_shared_stable_admits_maximum_without_large_allocation() {
    let maximum = CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1;
    for generation in [1, u64::MAX - 1] {
        for (allocations, writers, reads) in [
            (maximum, maximum, maximum),
            (maximum, 1, 1),
            (1, maximum, 1),
            (1, 1, maximum),
        ] {
            reject(
                (generation, allocations, writers, reads),
                &[false],
                Error::StorageAllocationFailed,
                &[writers],
            );
        }
    }
}

#[test]
fn construction_shared_stable_success_supports_lifecycle() {
    let mut frozen_probe = ReserveProbe::new(&[true; 10], 3);
    let mut actual_probe = ReserveProbe::new(&[true; 10], 3);
    let mut frozen =
        Journal::baseline_new_with_allocator_v1(7, 3, 2, 4, &mut frozen_probe).unwrap();
    let mut actual = Journal::new_with_allocator_v1(7, 3, 2, 4, &mut actual_probe).unwrap();
    for owner in [&mut frozen, &mut actual] {
        let device = ContextJournalDeviceKeyV1 {
            context_generation: 7,
            local: 2,
        };
        let allocation = owner
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: 7,
                    local: 20,
                },
                device,
                64,
            )
            .unwrap();
        assert_eq!(allocation.slot, 0);
        let request = ContextAllocationReadV1 {
            allocation,
            device,
            byte_extent: 64,
            byte_offset: 0,
            byte_len: 16,
            attempt_epoch: 0,
            content_lineage: 0,
        };
        let mut leases = [None];
        owner
            .acquire_reads(consumer(20), &[request], &mut leases)
            .unwrap();
        let lease = leases[0].unwrap();
        assert_eq!((lease.slot, lease.incarnation), (0, 1));
        assert_eq!(owner.lookup_read(lease), Ok(request));
        assert_eq!(owner.reader_count(allocation), Ok(1));
        let writer = owner.register_writer(consumer(10)).unwrap();
        assert_eq!(writer.slot, 0);
        let roster = [member(request)];
        assert_eq!(
            owner.begin_write(writer, &roster),
            Err(Error::AllocationBusy)
        );
        owner
            .release_reads(
                consumer(20),
                &[lease],
                &ContextReadQuiescenceEvidenceV1 {
                    consumer: consumer(20),
                },
            )
            .unwrap();
        owner.begin_write(writer, &roster).unwrap();
        owner
            .settle_success(writer, &ContextWriterSuccessEvidenceV1 { writer })
            .unwrap();
        let request = ContextAllocationReadV1 {
            attempt_epoch: 1,
            content_lineage: 1,
            ..request
        };
        leases[0] = None;
        owner
            .acquire_reads(consumer(21), &[request], &mut leases)
            .unwrap();
        let lease = leases[0].unwrap();
        assert_eq!((lease.slot, lease.incarnation), (0, 2));
        owner
            .release_reads(
                consumer(21),
                &[lease],
                &ContextReadQuiescenceEvidenceV1 {
                    consumer: consumer(21),
                },
            )
            .unwrap();
        owner.retire_allocations(&[allocation]).unwrap();
        assert_eq!(
            owner.lookup_allocation(allocation),
            Err(Error::InvalidAllocationReference)
        );
        assert_reader_invariant(owner);
    }
    assert_eq!(snapshot(&frozen), snapshot(&actual));
    frozen_probe.assert_storage(&frozen.guard_owner_storage_v1());
    actual_probe.assert_storage(&actual.guard_owner_storage_v1());
}
