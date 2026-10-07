use super::*;

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
        byte_extent: 64,
    }
}

fn compare(
    journal: &mut Journal,
    entry: ContextAllocationEnrollmentV1,
    expected: Result<ContextAllocationReferenceV1, Error>,
    accesses: usize,
) {
    let copy = journal.guard_copy_for_test_v1();
    let before = snapshot(journal);
    journal.reset_access_count_for_test_v1();
    assert_eq!(
        journal.baseline_enroll_allocation_v1(entry.key, entry.device, entry.byte_extent),
        expected
    );
    assert_eq!(journal.guard_accesses_for_test_v1(), accesses);
    let frozen = snapshot(journal);
    assert_eq!(storage(journal), before.storage);
    journal.restore_enrollment_for_test_v1(&copy, 1);
    assert_eq!(snapshot(journal), before);
    assert_eq!(
        journal.enroll_allocation(entry.key, entry.device, entry.byte_extent),
        expected
    );
    assert_eq!(journal.guard_accesses_for_test_v1(), accesses);
    assert_eq!(snapshot(journal), frozen);
    assert_eq!(storage(journal), before.storage);
    if expected.is_err() {
        assert_eq!(snapshot(journal), before);
    }
}

#[test]
fn scalar_enrollment_shared_preserves_ordered_argument_errors() {
    for context in [7, 8] {
        for local in [0, 10, u64::MAX] {
            for device_context in [7, 8] {
                for device_local in [0, 11, u64::MAX] {
                    for extent in [0, 64, u64::MAX] {
                        let mut journal = Journal::new(7, 4, 1).unwrap();
                        journal.fault_enrollment_for_test_v1(&[usize::MAX; 5], 0);
                        let mut input = entry(local);
                        input.key.context_generation = context;
                        input.device.context_generation = device_context;
                        input.device.local = device_local;
                        input.byte_extent = extent;
                        let (error, accesses) = if context != 7 {
                            (Error::ForeignContext, 0)
                        } else if local == 0 || local == u64::MAX {
                            (Error::InvalidAllocationId, 0)
                        } else if device_context != 7 {
                            (Error::ForeignContext, 0)
                        } else if device_local == 0 || device_local == u64::MAX {
                            (Error::InvalidDeviceId, 0)
                        } else if extent == 0 {
                            (Error::InvalidExtent, 0)
                        } else {
                            (Error::InvalidState, 6)
                        };
                        compare(&mut journal, input, Err(error), accesses);
                    }
                }
            }
        }
    }
}

#[test]
fn scalar_enrollment_shared_replay_precedes_free_storage_checks() {
    for slot in 0..4 {
        for free in [&[][..], &[usize::MAX], &[usize::MAX; 5]] {
            let mut journal = Journal::new(7, 4, 1).unwrap();
            let input = entry(10);
            journal.allocations[slot] = Some(AllocationEntryV1 {
                key: input.key,
                device: ContextJournalDeviceKeyV1 {
                    context_generation: 8,
                    local: 0,
                },
                byte_extent: 0,
                attempt_epoch: u64::MAX,
                content_lineage: u64::MAX,
                pending_member: Some(usize::MAX),
            });
            journal.fault_enrollment_for_test_v1(free, 0);
            compare(&mut journal, input, Err(Error::AllocationReplay), slot + 1);
        }
    }
}

#[test]
fn scalar_enrollment_shared_compares_complete_stored_keys() {
    let mut journal = Journal::new(7, 4, 1).unwrap();
    let input = entry(10);
    journal.allocations[0] = Some(AllocationEntryV1 {
        key: ContextAllocationKeyV1 {
            context_generation: 8,
            local: 10,
        },
        device: input.device,
        byte_extent: 64,
        attempt_epoch: 0,
        content_lineage: 0,
        pending_member: None,
    });
    journal.fault_enrollment_for_test_v1(&[3], 0);
    compare(
        &mut journal,
        input,
        Ok(ContextAllocationReferenceV1 {
            slot: 3,
            key: input.key,
        }),
        8,
    );
}

#[test]
fn scalar_enrollment_shared_ignores_unrelated_raw_storage() {
    for capacity in [0, 1, 4] {
        for free in [&[][..], &[usize::MAX], &[0], &[usize::MAX, 3], &[3; 5]] {
            for occupied in [false, true] {
                let mut journal = Journal::new(7, 4, 1).unwrap();
                let input = entry(10);
                journal.fault_enrollment_for_test_v1(free, capacity);
                journal.registration_watermark = u64::MAX;
                journal.reserved_count = usize::MAX;
                journal.member_free.clear();
                let allocation = ContextAllocationReferenceV1 {
                    slot: 3,
                    key: input.key,
                };
                journal.members[0] = Some(MemberEntryV1 {
                    writer: Reference {
                        slot: usize::MAX,
                        key: key(0),
                    },
                    allocation,
                    prior_lineage: u64::MAX,
                    attempt_epoch: 0,
                    next: Some(usize::MAX),
                });
                journal.scratch[0] = Some(BeginMemberPlanV1 {
                    member_slot: usize::MAX,
                    allocation,
                    prior_lineage: 88,
                    attempt_epoch: 99,
                });
                if occupied {
                    let stored = entry(20);
                    for value in &mut journal.allocations {
                        *value = Some(AllocationEntryV1 {
                            key: stored.key,
                            device: stored.device,
                            byte_extent: stored.byte_extent,
                            attempt_epoch: 2,
                            content_lineage: 1,
                            pending_member: None,
                        });
                    }
                }
                let (expected, accesses) = match free.last() {
                    None => (Err(Error::AllocationCapacity), 5),
                    Some(&slot) if slot >= 4 || occupied => (Err(Error::InvalidState), 6),
                    Some(&slot) => (
                        Ok(ContextAllocationReferenceV1 {
                            slot,
                            key: input.key,
                        }),
                        8,
                    ),
                };
                compare(&mut journal, input, expected, accesses);
            }
        }
    }
}

#[test]
fn scalar_enrollment_shared_empty_physical_arena() {
    for free in [&[][..], &[0], &[usize::MAX]] {
        let mut journal = Journal::new(7, 4, 1).unwrap();
        journal.allocations.clear();
        journal.fault_enrollment_for_test_v1(free, 0);
        let (error, accesses) = if free.is_empty() {
            (Error::AllocationCapacity, 1)
        } else {
            (Error::InvalidState, 2)
        };
        compare(&mut journal, entry(10), Err(error), accesses);
    }
}

#[test]
fn scalar_enrollment_shared_initializes_exact_descriptor_without_constructor_checks() {
    for context in [0, 7, u64::MAX] {
        let mut journal = Journal::new(7, 4, 1).unwrap();
        journal.context_generation = context;
        journal.fault_enrollment_for_test_v1(&[2, 0, 3], 0);
        let mut input = entry(u64::MAX - 1);
        input.key.context_generation = context;
        input.device.context_generation = context;
        input.device.local = u64::MAX - 1;
        input.byte_extent = u64::MAX;
        compare(
            &mut journal,
            input,
            Ok(ContextAllocationReferenceV1 {
                slot: 3,
                key: input.key,
            }),
            8,
        );
        assert_eq!(journal.allocation_free, [2, 0]);
        assert_eq!(
            journal.allocations[3],
            Some(AllocationEntryV1 {
                key: input.key,
                device: input.device,
                byte_extent: input.byte_extent,
                attempt_epoch: 0,
                content_lineage: 0,
                pending_member: None,
            })
        );
    }
}

#[test]
fn scalar_enrollment_shared_counts_actual_arena_scan() {
    for capacity in [1, 8, 65536] {
        let mut journal = Journal::new(7, capacity, 1).unwrap();
        journal.fault_enrollment_for_test_v1(&[capacity - 1], 0);
        let input = entry(10);
        compare(
            &mut journal,
            input,
            Ok(ContextAllocationReferenceV1 {
                slot: capacity - 1,
                key: input.key,
            }),
            capacity + 4,
        );
        compare(&mut journal, input, Err(Error::AllocationReplay), capacity);
    }
}
