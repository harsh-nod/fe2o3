use super::*;
use core::mem::size_of;

use crate::context_version_journal::construction_probe::ReserveProbe;

fn requests(allocations: usize, writers: usize) -> [usize; 7] {
    [
        writers,
        writers,
        allocations,
        allocations,
        allocations,
        allocations,
        allocations,
    ]
}

fn reject(
    generation: u64,
    allocations: usize,
    writers: usize,
    outcomes: &[bool],
    expected: Error,
    expected_requests: &[usize],
) {
    let mut frozen = ReserveProbe::new(outcomes, 0);
    let mut actual = ReserveProbe::new(outcomes, 0);
    assert_eq!(
        Journal::baseline_new_with_allocator_v1(generation, allocations, writers, &mut frozen)
            .unwrap_err(),
        expected
    );
    assert_eq!(
        Journal::new_with_allocator_v1(generation, allocations, writers, &mut actual).unwrap_err(),
        expected
    );
    frozen.assert_requests(expected_requests);
    actual.assert_requests(expected_requests);
    frozen.assert_same_trace(&actual);
}

#[test]
fn construction_shared_journal_success_contents_sites_and_storage() {
    for (generation, allocations, writers) in
        [(1, 1, 1), (7, 3, 2), (u64::MAX - 1, 17, 9), (7, 4097, 3)]
    {
        for extra in [0, 5] {
            let outcomes = [true, true, true, true, true, true, true, false];
            let mut frozen_probe = ReserveProbe::new(&outcomes, extra);
            let mut actual_probe = ReserveProbe::new(&outcomes, extra);
            let frozen = Journal::baseline_new_with_allocator_v1(
                generation,
                allocations,
                writers,
                &mut frozen_probe,
            )
            .unwrap();
            let actual =
                Journal::new_with_allocator_v1(generation, allocations, writers, &mut actual_probe)
                    .unwrap();
            let native = Journal::new(generation, allocations, writers).unwrap();
            assert_eq!(alloc::format!("{frozen:?}"), alloc::format!("{actual:?}"));
            assert_eq!(alloc::format!("{frozen:?}"), alloc::format!("{native:?}"));
            let expected = requests(allocations, writers);
            frozen_probe.assert_requests(&expected);
            actual_probe.assert_requests(&expected);
            frozen_probe.assert_same_trace(&actual_probe);
            frozen_probe.assert_storage(&storage(&frozen));
            actual_probe.assert_storage(&storage(&actual));
            let widths = [
                size_of::<Option<WriterEntryV1>>(),
                size_of::<usize>(),
                size_of::<Option<AllocationEntryV1>>(),
                size_of::<usize>(),
                size_of::<Option<MemberEntryV1>>(),
                size_of::<usize>(),
                size_of::<Option<BeginMemberPlanV1>>(),
            ];
            assert_eq!(
                actual_probe
                    .events
                    .iter()
                    .map(|event| event.width)
                    .collect::<Vec<_>>(),
                widths
            );
            for owner in [&frozen, &actual, &native] {
                assert_eq!(owner.context_generation, generation);
                assert_eq!(owner.allocation_capacity, allocations);
                assert_eq!(owner.writer_capacity, writers);
                assert_eq!(owner.registration_watermark, 0);
                assert_eq!(owner.reserved_count, 0);
                assert_eq!(owner.writers, vec![None; writers]);
                assert_eq!(owner.allocations, vec![None; allocations]);
                assert_eq!(owner.members, vec![None; allocations]);
                assert_eq!(owner.scratch, vec![None; allocations]);
                assert_eq!(owner.free, (0..writers).rev().collect::<Vec<_>>());
                assert_eq!(
                    owner.allocation_free,
                    (0..allocations).rev().collect::<Vec<_>>()
                );
                assert_eq!(
                    owner.member_free,
                    (0..allocations).rev().collect::<Vec<_>>()
                );
                assert_eq!(owner.guard_accesses_for_test_v1(), 0);
                audit(owner);
            }
        }
    }
}

#[test]
fn construction_shared_journal_stops_at_each_reservation_failure() {
    for failed in 0..7 {
        let mut outcomes = [true; 9];
        outcomes[failed] = false;
        reject(
            7,
            3,
            2,
            &outcomes,
            Error::StorageAllocationFailed,
            &requests(3, 2)[..=failed],
        );
    }
}

#[test]
fn construction_shared_journal_missing_outcomes_stop_at_first_absence() {
    for present in 0..7 {
        reject(
            7,
            3,
            2,
            &[true; 7][..present],
            Error::StorageAllocationFailed,
            &requests(3, 2)[..=present],
        );
    }
}

#[test]
fn construction_shared_journal_validation_precedence_without_allocation() {
    let maximum = CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1;
    for generation in [0, 7, u64::MAX] {
        for allocations in [0, 3, maximum + 1, usize::MAX] {
            for writers in [0, 2, maximum + 1, usize::MAX] {
                let error = if generation == 0 || generation == u64::MAX {
                    Error::InvalidContextGeneration
                } else if allocations != 3 || writers != 2 {
                    Error::InvalidCapacity
                } else {
                    continue;
                };
                reject(generation, allocations, writers, &[], error, &[]);
            }
        }
    }
}

#[test]
fn construction_shared_journal_admits_maximum_without_large_allocation() {
    let maximum = CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1;
    for generation in [1, u64::MAX - 1] {
        for (allocations, writers) in [(maximum, maximum), (maximum, 1), (1, maximum)] {
            reject(
                generation,
                allocations,
                writers,
                &[false],
                Error::StorageAllocationFailed,
                &[writers],
            );
        }
    }
}

#[test]
fn construction_shared_journal_success_supports_lifecycle() {
    let mut frozen_probe = ReserveProbe::new(&[true; 7], 3);
    let mut actual_probe = ReserveProbe::new(&[true; 7], 3);
    let mut frozen = Journal::baseline_new_with_allocator_v1(7, 3, 2, &mut frozen_probe).unwrap();
    let mut actual = Journal::new_with_allocator_v1(7, 3, 2, &mut actual_probe).unwrap();
    for owner in [&mut frozen, &mut actual] {
        let reserved = owner.register_writer(key(10)).unwrap();
        assert_eq!(reserved.slot, 0);
        owner.abort_reserved(reserved).unwrap();
        let writer = owner.register_writer(key(11)).unwrap();
        assert_eq!(writer.slot, 0);
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
        let roster = [ContextAllocationWriteV1 {
            allocation,
            device,
            byte_extent: 64,
        }];
        owner.begin_write(writer, &roster).unwrap();
        owner.mark_unknown(writer).unwrap();
        owner
            .dispose_unknown(
                writer,
                &ContextWriterDisposalEvidenceV1 {
                    writer,
                    allocations: &roster,
                },
            )
            .unwrap();
        assert_eq!(
            owner.lookup_allocation(allocation),
            Err(Error::InvalidAllocationReference)
        );
        assert_eq!(owner.lookup_writer(writer), Err(Error::InvalidReference));
        let fresh = owner
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: 7,
                    local: 21,
                },
                device,
                64,
            )
            .unwrap();
        assert_eq!(fresh.slot, 0);
        owner.retire_allocations(&[fresh]).unwrap();
        audit(owner);
    }
    assert_eq!(alloc::format!("{frozen:?}"), alloc::format!("{actual:?}"));
    frozen_probe.assert_storage(&storage(&frozen));
    actual_probe.assert_storage(&storage(&actual));
}
