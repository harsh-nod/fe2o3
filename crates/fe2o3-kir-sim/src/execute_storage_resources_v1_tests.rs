use super::storage_tests_v1::*;
use super::*;
use crate::SimulationScheduleDecisionV1;

#[test]
fn schedule_growth_exact_and_one_short_preserve_old_buffer_and_semantic_work() {
    type Decision = SimulationScheduleDecisionV1;
    let decision = Decision::new([3, 5, 7], 11, [13, 17, 19]);
    let headers = storage_growth_headers_v1::<Decision>();
    assert!(size_of::<SchedulePrepareErrorV1>() <= size_of::<SimulationExecutionErrorKindV1>());
    for short in [false, true] {
        let mut rows = Vec::new();
        rows.try_reserve_exact(2).unwrap();
        rows.resize(rows.capacity(), decision);
        let old_capacity = rows.capacity();
        let old_bytes = old_capacity * size_of::<Decision>();
        let mut measured = Vec::<Decision>::new();
        measured.try_reserve_exact(old_capacity * 2).unwrap();
        let new_capacity = measured.capacity();
        let new_bytes = new_capacity * size_of::<Decision>();
        drop(measured);
        let accounting = StorageAccountingV1::new(SimulationLimitsV1 {
            max_resident_bytes: 37 + old_bytes + headers + new_bytes - usize::from(short),
            ..SimulationLimitsV1::default()
        });
        accounting.hold(37 + old_bytes).unwrap();
        let original_pointer = rows.as_ptr();
        let result = reserve_schedule_decisions_v1(&mut rows, old_capacity * 2, &accounting);
        if short {
            assert!(matches!(
                result,
                Err(SchedulePrepareErrorV1::ResidentLimit { .. })
            ));
            assert_eq!(rows.capacity(), old_capacity);
            assert_eq!(rows.as_ptr(), original_pointer);
            assert_eq!(accounting.held(), 37 + old_bytes);
        } else {
            assert!(result.is_ok());
            assert_eq!(rows.capacity(), new_capacity);
            assert_eq!(accounting.held(), 37 + new_bytes);
            assert_eq!(accounting.peak(), 37 + old_bytes + headers + new_bytes);
        }
        assert_eq!(rows.len(), old_capacity);
        assert!(rows.iter().all(|value| *value == decision));
        assert_eq!(accounting.steps(), 0);
    }
}

#[test]
fn schedule_and_storage_growth_compete_for_one_resident_ledger() {
    type Decision = SimulationScheduleDecisionV1;
    let mut rows = Vec::new();
    rows.try_reserve_exact(1).unwrap();
    rows.resize(rows.capacity(), Decision::new([0; 3], 0, [0; 3]));
    let old_capacity = rows.capacity();
    let old_bytes = old_capacity * size_of::<Decision>();
    let mut measured = Vec::<Decision>::new();
    measured.try_reserve_exact(old_capacity * 2).unwrap();
    let new_bytes = measured.capacity() * size_of::<Decision>();
    drop(measured);
    let maximum = 23 + old_bytes + storage_growth_headers_v1::<Decision>() + new_bytes;
    let accounting = StorageAccountingV1::new(SimulationLimitsV1 {
        max_resident_bytes: maximum,
        ..SimulationLimitsV1::default()
    });
    accounting.hold(23 + old_bytes).unwrap();
    let mut storage = Vec::<u8>::new();
    storage_reserve_v1(&mut storage, 1, &accounting).unwrap();
    let storage_bytes = storage.capacity();
    assert!(matches!(
        reserve_schedule_decisions_v1(&mut rows, old_capacity * 2, &accounting),
        Err(SchedulePrepareErrorV1::ResidentLimit { .. })
    ));
    assert_eq!(rows.capacity(), old_capacity);
    assert_eq!(accounting.held(), 23 + old_bytes + storage_bytes);
    drop(storage);
    accounting.release(storage_bytes);
    assert!(reserve_schedule_decisions_v1(&mut rows, old_capacity * 2, &accounting).is_ok());
    assert_eq!(accounting.held(), 23 + new_bytes);

    let held = accounting.held();
    let mut storage = Vec::<u8>::new();
    let request = maximum - held - storage_growth_headers_v1::<u8>() + 1;
    assert!(matches!(
        storage_reserve_v1(&mut storage, request, &accounting),
        Err(SimulationExecutionErrorKindV1::StorageResidentLimit { .. })
    ));
    assert_eq!(storage.capacity(), 0);
    assert_eq!(accounting.held(), held);
    assert_eq!(accounting.steps(), 0);
}

#[test]
fn schedule_growth_keeps_logical_limit_refusal_before_reservation() {
    let mut rows = Vec::new();
    rows.try_reserve_exact(1).unwrap();
    rows.resize(
        rows.capacity(),
        SimulationScheduleDecisionV1::new([0; 3], 0, [0; 3]),
    );
    let maximum = rows.len();
    let accounting = StorageAccountingV1::new(SimulationLimitsV1::default());
    accounting
        .hold(rows.capacity() * size_of::<SimulationScheduleDecisionV1>())
        .unwrap();
    let held = accounting.held();
    assert!(
        matches!(reserve_schedule_decisions_v1(&mut rows, maximum, &accounting),
        Err(SchedulePrepareErrorV1::DecisionLimit { actual, limit })
            if actual == maximum + 1 && limit == maximum)
    );
    assert_eq!(rows.len(), maximum);
    assert_eq!(accounting.held(), held);
    assert_eq!(accounting.steps(), 0);
}

#[test]
fn storage_unidentified_schedule_completes_coverage_only_after_all_workgroups() {
    let mut schedule = StorageExecutionScheduleV1::Unidentified {
        decisions: 0,
        workgroups: 0,
        barrier_releases: 0,
    };
    schedule.begin_workgroup();
    assert!(
        schedule
            .selected(
                SimulationInvocationV1 {
                    global: [0; 3],
                    workgroup: [0; 3],
                    local: [0; 3],
                    workgroup_size: [1; 3],
                    workgroup_count: [1; 3],
                    launch_extent: [1; 3],
                },
                0,
                &StorageAccountingV1::new(SimulationLimitsV1::default()),
            )
            .is_ok()
    );
    schedule.barrier_released();
    let completion = schedule
        .finish(
            1,
            None,
            &request(),
            None,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(completion.coverage.workgroups(), 1);
    assert_eq!(completion.coverage.decisions(), 1);
    assert_eq!(completion.coverage.barrier_releases(), 1);
    assert!(completion.coverage.is_complete());
    assert_eq!(completion.transcript_identity, None);
    assert!(completion.records.is_empty());
    let unfinished = StorageExecutionScheduleV1::Unidentified {
        decisions: 0,
        workgroups: 0,
        barrier_releases: 0,
    };
    assert!(matches!(
        unfinished.finish(
            1,
            None,
            &request(),
            None,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        ),
        Err(SimulationScheduleReplayErrorV1::CoverageMismatch)
    ));
}

#[test]
fn storage_whole_entry_allocation_exact_and_one_short_work_are_source_derived() {
    let module = module(rows(), vec![allocate(1, 0, AddressSpace::Private)]);
    // One operation step, one allocation-row lookup, one result-binding row
    // lookup, one Return step. No storage mutation, scan, or second interpreter.
    const WORK: u64 = 1 + 1 + 1 + 1;
    for short in [false, true] {
        let limits = SimulationLimitsV1 {
            max_steps: WORK - u64::from(short),
            ..SimulationLimitsV1::default()
        };
        let mut events = Events::default();
        let result = simulate_storage_view_v1(
            verified(&module),
            &request(),
            None,
            SimulationTargetV1::amdgpu_64(),
            limits,
            &mut events,
        );
        if short {
            assert!(matches!(
                result,
                Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::StepLimit { limit: 3 },
                    ..
                }))
            ));
        } else {
            let result = result.unwrap();
            assert_eq!(result.steps_executed, WORK);
            assert_eq!(result.invocations_executed, 1);
        }
        assert!(events.0.iter().any(|e| matches!(
            e.kind,
            SimulationEventKindV1::AllocationCreated {
                address_space: AddressSpace::Private,
                bytes: 4,
                ..
            }
        )));
    }
}

#[test]
fn storage_snapshot_full_typed_header_exact_and_one_short_preserve_nonzero_floor() {
    let expected = 4 * size_of::<StorageSnapshotV1>()
        + 4 * size_of::<Result<StorageSnapshotV1, SimulationExecutionErrorKindV1>>()
        + 2 * size_of::<Result<StorageSnapshotV1, SimulationExecutionErrorV1>>()
        + 2 * size_of::<Result<(), SimulationExecutionErrorKindV1>>()
        + 2 * size_of::<Result<(), SimulationExecutionErrorV1>>();
    assert_eq!(storage_snapshot_headers_v1(), expected);
    for short in [false, true] {
        let limits = SimulationLimitsV1 {
            max_resident_bytes: 17 + expected - usize::from(short),
            ..SimulationLimitsV1::default()
        };
        let accounting = StorageAccountingV1::new(limits);
        accounting.hold(17).unwrap();
        let result = StorageSnapshotV1::new(&accounting);
        if short {
            assert!(matches!(
                result,
                Err(SimulationExecutionErrorKindV1::StorageResidentLimit { .. })
            ));
            assert_eq!(accounting.held(), 17);
        } else {
            let snapshot = result.unwrap();
            assert_eq!(accounting.held(), 17 + expected);
            snapshot.release(&accounting);
            assert_eq!(accounting.held(), 17);
        }
    }
}

#[test]
fn storage_growth_header_refusal_happens_before_backing_or_work() {
    let expected = 3 * size_of::<Vec<u64>>()
        + 2 * size_of::<Result<(), std::collections::TryReserveError>>()
        + 6 * size_of::<Result<usize, SimulationExecutionErrorKindV1>>()
        + 8 * size_of::<Result<(), SimulationExecutionErrorKindV1>>()
        + 3 * size_of::<StorageReservationV1<'_>>()
        + 2 * size_of::<Result<StorageReservationV1<'_>, SimulationExecutionErrorKindV1>>();
    assert_eq!(storage_growth_headers_v1::<u64>(), expected);
    let accounting = StorageAccountingV1::new(SimulationLimitsV1 {
        max_resident_bytes: 19 + expected - 1,
        ..SimulationLimitsV1::default()
    });
    accounting.hold(19).unwrap();
    let mut rows = Vec::<u64>::new();
    assert!(matches!(
        storage_reserve_v1(&mut rows, 1, &accounting),
        Err(SimulationExecutionErrorKindV1::StorageResidentLimit { .. })
    ));
    assert_eq!(rows.capacity(), 0);
    assert_eq!(accounting.held(), 19);
    assert_eq!(accounting.steps(), 0);
}

#[test]
fn storage_failed_growth_debit_drops_new_backing_and_preserves_old_credit() {
    let accounting = StorageAccountingV1::new(SimulationLimitsV1 {
        max_steps: 1,
        ..SimulationLimitsV1::default()
    });
    let mut rows = Vec::new();
    storage_reserve_v1(&mut rows, 2, &accounting).unwrap();
    rows.extend([3_u64, 5]);
    let held = accounting.held();
    let capacity = rows.capacity();
    let result = storage_reserve_v1(&mut rows, capacity + 1, &accounting);
    assert!(matches!(
        result,
        Err(SimulationExecutionErrorKindV1::StepLimit { limit: 1 })
    ));
    assert_eq!(rows, [3, 5]);
    assert_eq!(rows.capacity(), capacity);
    assert_eq!(accounting.held(), held);
    assert_eq!(accounting.steps(), 0);
    drop(rows);
    accounting.release(held);
    assert_eq!(accounting.held(), 0);
}

#[test]
fn storage_whole_entry_write_read_work_and_first_denial_cuts_are_source_derived() {
    let module = simple(AddressSpace::Private);
    // Seven ordinary operation/terminator steps. Alloca + binding use two row
    // lookups; Project uses three row lookups and one child-view charge.
    // Write: row + validate + 2W initialization + 2W encoding + 4W commit.
    // Read: row + two validations + W initialization scan + W scalar decode.
    const W: u64 = 4;
    const WORK: u64 = 7 + 2 + 4 + (2 + 8 * W) + (3 + 2 * W);
    assert_eq!(WORK, 58);
    let mut events = Events::default();
    let result = simulate_storage_view_v1(
        verified(&module),
        &request(),
        None,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1 {
            max_steps: WORK,
            ..SimulationLimitsV1::default()
        },
        &mut events,
    )
    .unwrap();
    assert_eq!(result.steps_executed, WORK);
    assert_eq!(output_bits(&result), 0x1234_5678);
    // The fixed source debit boundaries are 20/28/44/56/58, respectively.
    // They are computed above/below without running a successful-limit search.
    for (limit, writes, reads) in [(19, 0, 0), (27, 0, 0), (43, 0, 0), (55, 1, 0), (57, 2, 1)] {
        let mut events = Events::default();
        let result = simulate_storage_view_v1(
            verified(&module),
            &request(),
            None,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_steps: limit,
                ..SimulationLimitsV1::default()
            },
            &mut events,
        );
        assert!(
            matches!(result, Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::StepLimit { limit: actual }, ..
        })) if actual == limit)
        );
        assert_eq!(
            events
                .0
                .iter()
                .filter(|event| matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. }))
                .count(),
            writes
        );
        assert_eq!(
            events
                .0
                .iter()
                .filter(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
                .count(),
            reads
        );
    }
}

#[test]
fn storage_vector_constructor_headers_are_full_typed_and_floor_preserving() {
    let expected = size_of::<Vec<ScalarBitsV1>>()
        + size_of::<StorageVectorV1>()
        + 2 * size_of::<Result<usize, SimulationExecutionErrorV1>>()
        + 2 * size_of::<Result<(), SimulationExecutionErrorKindV1>>();
    assert_eq!(storage_vector_headers_v1(), expected);
    let accounting = StorageAccountingV1::new(SimulationLimitsV1 {
        max_resident_bytes: 23 + expected,
        ..SimulationLimitsV1::default()
    });
    accounting.hold(23).unwrap();
    let held = accounting.temporary(expected).unwrap();
    assert_eq!(accounting.held(), 23 + expected);
    assert!(matches!(
        accounting.hold(1),
        Err(SimulationExecutionErrorKindV1::StorageResidentLimit { .. })
    ));
    drop(held);
    assert_eq!(accounting.held(), 23);
}

#[test]
fn storage_enum_whole_entry_construction_noop_and_tag_write_work_are_source_derived() {
    use super::storage_views_tests_v1::{construction_module, niche, set_discriminant};
    for (case, work) in [(0, 9_u64), (1, 7), (2, 34)] {
        let mut module = construction_module(0, false);
        if case == 0 {
            // Alloca 3; construction operation/base row/child row/child/bind 5;
            // Return 1. No tag read, write, guard, or snapshot allocation.
            module.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .truncate(2);
        } else {
            if case == 1 {
                niche(
                    &mut module,
                    StorageFieldV1 {
                        offset: 0,
                        layout: StorageLayoutIdV1(0),
                    },
                    0,
                    1,
                );
            }
            module.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
                allocate(1, 4, AddressSpace::Private),
                set_discriminant(1, 0),
            ];
            // Untagged: Alloca 3 + Set step/row/holder-check 3 + Return 1.
            // Direct: Alloca 3 + Set six fixed units + 2W snapshot + 4W
            // prepared commit + Return 1 = 34 for W=4, no prior metadata.
        }
        for short in [false, true] {
            let mut events = Events::default();
            let result = simulate_storage_view_v1(
                verified(&module),
                &request(),
                None,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1 {
                    max_steps: work - u64::from(short),
                    ..SimulationLimitsV1::default()
                },
                &mut events,
            );
            if short {
                assert!(matches!(
                    result,
                    Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                        kind: SimulationExecutionErrorKindV1::StepLimit { .. },
                        ..
                    }))
                ));
            } else {
                assert_eq!(result.unwrap().steps_executed, work);
            }
            assert!(
                !events
                    .0
                    .iter()
                    .any(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
            );
            assert_eq!(
                events
                    .0
                    .iter()
                    .filter(|event| matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. }))
                    .count(),
                usize::from(case == 2)
            );
        }
        if case == 2 {
            let mut events = Events::default();
            assert!(
                simulate_storage_view_v1(
                    verified(&module),
                    &request(),
                    None,
                    SimulationTargetV1::amdgpu_64(),
                    SimulationLimitsV1 {
                        max_steps: 32,
                        ..SimulationLimitsV1::default()
                    },
                    &mut events
                )
                .is_err()
            );
            assert!(
                !events
                    .0
                    .iter()
                    .any(|event| matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. }))
            );
        }
    }
}
