use super::super::storage_inputs_tests_v29::*;
use super::*;

fn import_scalar(
    limit: SimulationLimitsV1,
) -> (
    Result<Vec<RuntimeValue>, SimulationExecutionErrorKindV1>,
    Memory,
) {
    let owner = admit(&graph(vec![scalar()], vec![inline_type(0)], vec![]));
    let verified = owner.verified_storage_module_ref_v1();
    let request = request(vec![SimulationStorageArgumentV29::InlineObject(image(
        0,
        4,
        vec![0; 4],
        vec![true; 4],
        vec![],
    ))]);
    let mut memory = Memory::new(1, 0, limit).unwrap();
    memory.storage_accounting.hold(29).unwrap();
    let result = storage_import_inputs_v29(
        &mut StorageInputContextV29 {
            memory: &mut memory,
            target: targets()[0],
            limits: limit,
        },
        &verified,
        &request,
        &owner.module().functions[0],
    );
    (result, memory)
}

fn independent_input_headers() -> usize {
    size_of::<InputScratchV29<'_>>()
        + 2 * size_of::<PreparedInputRelocationV29<'_>>()
        + 2 * size_of::<PreparedInputGuardV29>()
        + 2 * size_of::<PreparedInputArgumentV29>()
        + 4 * size_of::<Result<StorageInputPositionV29, SimulationExecutionErrorKindV1>>()
        + 4 * size_of::<Result<StorageInputReferentV29, SimulationExecutionErrorKindV1>>()
        + 2 * size_of::<Result<Vec<RuntimeValue>, SimulationExecutionErrorKindV1>>()
        + size_of::<
            Result<
                Result<Vec<RuntimeValue>, SimulationExecutionErrorKindV1>,
                Box<dyn std::any::Any + Send>,
            >,
        >()
}

fn independent_growth_headers<T>() -> usize {
    3 * size_of::<Vec<T>>()
        + 2 * size_of::<Result<(), std::collections::TryReserveError>>()
        + 6 * size_of::<Result<usize, SimulationExecutionErrorKindV1>>()
        + 8 * size_of::<Result<(), SimulationExecutionErrorKindV1>>()
        + 3 * size_of::<StorageReservationV1<'_>>()
        + 2 * size_of::<Result<StorageReservationV1<'_>, SimulationExecutionErrorKindV1>>()
}

#[test]
fn input_work_is_consumed_once_and_exact_one_short_preserves_the_caller_floor() {
    // Four input bytes and four mask entries + registration = 9; original
    // image lookup = 1; argument preparation = 1; parameter + origin = 2.
    const WORK: u64 = (4 * 2 + 1) + 1 + 1 + 2;
    for limit in [WORK, WORK - 1] {
        let (result, memory) = import_scalar(SimulationLimitsV1 {
            max_steps: limit,
            ..limits()
        });
        if limit == WORK {
            assert!(matches!(
                result.unwrap().as_slice(),
                [RuntimeValue::StoragePointer(_)]
            ));
            assert_eq!(memory.storage_accounting.steps(), WORK);
        } else {
            assert!(matches!(
                result,
                Err(SimulationExecutionErrorKindV1::StepLimit { limit: 12 })
            ));
            assert_eq!(memory.storage_accounting.steps(), WORK - 1);
        }
        assert_eq!(memory.storage_accounting.held(), 29);
        assert_eq!(memory.allocations.len(), 1);
        assert!(
            memory
                .allocations
                .values()
                .all(|allocation| allocation.storage.relocations.is_empty())
        );
        memory.storage_accounting.release(29);
    }
}

#[test]
fn complete_execution_does_not_reset_the_pre_engine_step_counter() {
    let owner = admit(&graph(vec![scalar()], vec![inline_type(0)], vec![]));
    let request = request(vec![SimulationStorageArgumentV29::InlineObject(image(
        0,
        4,
        vec![0; 4],
        vec![true; 4],
        vec![],
    ))]);
    // Import 13, Return dispatch 1, output original lookup 1 + byte/mask copy 8.
    const WORK: u64 = 13 + 1 + 1 + 8;
    for target in targets() {
        for limit in [WORK, WORK - 1] {
            let result = simulate_canonical_storage_inputs_v29(
                &owner,
                &request,
                target,
                SimulationLimitsV1 {
                    max_steps: limit,
                    ..limits()
                },
            );
            if limit == WORK {
                assert_eq!(result.unwrap().steps_executed(), WORK);
            } else {
                assert!(matches!(
                    result,
                    Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                        kind: SimulationExecutionErrorKindV1::StepLimit { limit: 22 },
                        ..
                    }))
                ));
            }
        }
    }
}

#[test]
fn import_fixed_headers_and_argument_growth_have_independent_exact_resident_boundaries() {
    let headers = independent_input_headers();
    assert_eq!(storage_input_headers_v29(), headers);
    let peak = 29
        + headers
        + independent_growth_headers::<Option<PreparedInputArgumentV29>>()
        + size_of::<Option<PreparedInputArgumentV29>>();
    for bytes in [29 + headers - 1, peak - 1, peak] {
        let (result, memory) = import_scalar(SimulationLimitsV1 {
            max_resident_bytes: bytes,
            ..limits()
        });
        if bytes == peak {
            assert!(result.is_ok());
        } else {
            assert!(
                matches!(result, Err(SimulationExecutionErrorKindV1::StorageResidentLimit { limit, .. }) if limit == bytes)
            );
        }
        assert_eq!(memory.storage_accounting.held(), 29);
        assert_eq!(memory.allocations.len(), usize::from(bytes >= 29 + headers));
        memory.storage_accounting.release(29);
    }
}

#[test]
fn deep_input_paths_are_iterative_and_charge_each_exact_containment_edge() {
    const DEPTH: usize = 96;
    let mut rows = vec![scalar()];
    for id in 0..DEPTH {
        rows.push(StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(id as u32),
                }]
                .into_boxed_slice(),
            ),
        });
    }
    let owner = admit(&graph(rows, vec![], vec![]));
    let verified = owner.verified_storage_module_ref_v1();
    let path = vec![SimulationObjectComponentV29::Field(0); DEPTH];
    for limit in [3 * DEPTH, 3 * DEPTH - 1] {
        let accounting = StorageAccountingV1::new(SimulationLimitsV1 {
            max_steps: limit as u64,
            ..limits()
        });
        accounting.hold(37).unwrap();
        let result = storage_input_path_v29(
            &verified,
            &accounting,
            StorageInputPositionV29 {
                allocation: 7,
                layout: StorageLayoutIdV1(DEPTH as u32),
                start: 0,
                end: 4,
            },
            &path,
            |_| panic!("no variant in exact record chain"),
        );
        if limit == 3 * DEPTH {
            assert_eq!(result.unwrap().layout, StorageLayoutIdV1(0));
        } else {
            assert!(matches!(
                result,
                Err(SimulationExecutionErrorKindV1::StepLimit { .. })
            ));
        }
        assert_eq!(accounting.steps(), limit as u64);
        assert_eq!(accounting.held(), 37);
    }
}

#[test]
fn partial_registration_respects_actual_allocation_and_live_byte_limits() {
    let owner = admit(&graph(vec![scalar()], vec![], vec![]));
    let verified = owner.verified_storage_module_ref_v1();
    let mut request = request(vec![]);
    for id in [1, 2] {
        request.shared_storage.push(SimulationSharedStorageV29 {
            id: BufferBackingIdV1(id),
            storage: SimulationStorageBackingV29::Object {
                image: image(0, 4, vec![0; 4], vec![true; 4], vec![]),
                access: AccessMode::ReadWrite,
            },
        });
    }
    for (limit, retained, expected) in [
        (
            SimulationLimitsV1 {
                max_allocations: 1,
                ..limits()
            },
            1,
            Some(SimulationExecutionErrorKindV1::AllocationLimit { limit: 1 }),
        ),
        (
            SimulationLimitsV1 {
                max_total_bytes: 7,
                ..limits()
            },
            1,
            Some(SimulationExecutionErrorKindV1::TotalBytesLimit {
                actual: 8,
                limit: 7,
            }),
        ),
        (
            SimulationLimitsV1 {
                max_allocation_bytes: 3,
                ..limits()
            },
            0,
            Some(SimulationExecutionErrorKindV1::AllocationBytesLimit {
                actual: 4,
                limit: 3,
            }),
        ),
        (
            SimulationLimitsV1 {
                max_allocations: 2,
                max_total_bytes: 8,
                max_allocation_bytes: 4,
                ..limits()
            },
            2,
            None,
        ),
    ] {
        let mut memory = Memory::new(0, 2, limit).unwrap();
        memory.storage_accounting.hold(43).unwrap();
        let result = storage_import_inputs_v29(
            &mut StorageInputContextV29 {
                memory: &mut memory,
                target: targets()[0],
                limits: limit,
            },
            &verified,
            &request,
            &owner.module().functions[0],
        );
        match expected {
            Some(error) => assert_eq!(result.unwrap_err(), error),
            None => assert!(result.is_ok()),
        }
        assert_eq!(memory.allocations.len(), retained);
        assert_eq!(memory.live_bytes, retained * 4);
        assert_eq!(memory.storage_accounting.held(), 43);
        assert!(
            memory
                .allocations
                .values()
                .all(|allocation| allocation.storage.relocations.is_empty())
        );
    }
}

#[test]
fn pointer_value_endpoint_is_representable_but_width_aware_operations_stay_strict() {
    let target = targets()[0];
    let mut memory = Memory::new(0, 1, limits()).unwrap();
    let backing = buffer(7, target);
    let id = storage_register_input_v29(
        &mut StorageInputContextV29 {
            memory: &mut memory,
            target,
            limits: limits(),
        },
        SimulationInputOriginV29::Backing(BufferBackingIdV1(1)),
        SimulationBackingRefV29::Scalar(&backing),
        AddressSpace::Global,
    )
    .unwrap();
    let representation = StoragePointerV1 {
        pointee: StorageLayoutIdV1(0),
        value_space: AddressSpace::Global,
        encoded_space: AddressSpace::Global,
        access: AccessMode::ReadWrite,
        stored_bits: 64,
    };
    let pointer = PointerValue {
        allocation: id,
        byte_offset: 4,
        element: ScalarType::U32,
        address_space: AddressSpace::Global,
        access: AccessMode::ReadWrite,
        lower_bound: 0,
        upper_bound: 4,
        abi_argument_ordinal: 0,
        storage_guard: None,
        generic_exposed: false,
    };
    let address = StorageAddressV1 {
        pointer,
        layout: StorageLayoutIdV1(0),
    };
    memory
        .storage_validate_input_pointer_v29(
            &StoragePointerPayloadV1::Object(address.clone()),
            representation,
        )
        .unwrap();
    let invocation = super::super::storage_views_tests_v1::invocation();
    for write in [false, true] {
        assert!(
            memory
                .storage_validate_v1(
                    &address,
                    MemoryAccess::new(AddressSpace::Global, 4),
                    4,
                    write,
                    invocation
                )
                .is_err()
        );
    }
    assert!(
        memory
            .storage_child_v1(
                &address,
                0,
                StorageLayoutIdV1(0),
                &scalar(),
                AccessMode::ReadWrite
            )
            .is_err()
    );
    let mut outside = address;
    outside.pointer.byte_offset = 5;
    assert!(
        memory
            .storage_validate_input_pointer_v29(
                &StoragePointerPayloadV1::Object(outside),
                representation
            )
            .is_err()
    );
}

#[test]
fn prepared_pointer_and_publication_growth_share_one_ledger_with_exact_persistent_credit() {
    let owner = admit(&graph(
        vec![scalar(), pointer(0)],
        vec![inline_type(1)],
        vec![],
    ));
    let verified = owner.verified_storage_module_ref_v1();
    let target = targets()[0];
    let mut request = request(vec![SimulationStorageArgumentV29::InlineObject(image(
        1,
        8,
        vec![0; 8],
        vec![true; 8],
        vec![relocation(1, vec![], view(3, 0, vec![]))],
    ))]);
    request.shared_storage.push(SimulationSharedStorageV29 {
        id: BufferBackingIdV1(3),
        storage: SimulationStorageBackingV29::Scalar(buffer(9, target)),
    });
    let prepared = size_of::<PreparedInputRelocationV29<'_>>();
    let argument = size_of::<Option<PreparedInputArgumentV29>>();
    let published = size_of::<StorageRelocationV1>();
    let transient_peak = (independent_growth_headers::<PreparedInputRelocationV29<'_>>()
        + prepared)
        .max(prepared + independent_growth_headers::<Option<PreparedInputArgumentV29>>() + argument)
        .max(prepared + argument + independent_growth_headers::<StorageRelocationV1>() + published);
    let required = 53 + independent_input_headers() + transient_peak;
    for bytes in [required, required - 1] {
        let limit = SimulationLimitsV1 {
            max_resident_bytes: bytes,
            ..limits()
        };
        let mut memory = Memory::new(1, 1, limit).unwrap();
        memory.storage_accounting.hold(53).unwrap();
        let result = storage_import_inputs_v29(
            &mut StorageInputContextV29 {
                memory: &mut memory,
                target,
                limits: limit,
            },
            &verified,
            &request,
            &owner.module().functions[0],
        );
        if bytes == required {
            assert!(result.is_ok());
            assert_eq!(memory.storage_accounting.held(), 53 + published);
            assert_eq!(
                memory
                    .allocations
                    .values()
                    .map(|allocation| allocation.storage.relocations.len())
                    .sum::<usize>(),
                1
            );
        } else {
            assert!(
                matches!(result, Err(SimulationExecutionErrorKindV1::StorageResidentLimit { limit, .. }) if limit == bytes)
            );
            assert_eq!(memory.storage_accounting.held(), 53);
            assert!(
                memory
                    .allocations
                    .values()
                    .all(|allocation| allocation.storage.relocations.is_empty())
            );
        }
        assert_eq!(memory.allocations.len(), 2);
    }
}

#[test]
fn active_guard_publication_keeps_only_live_guard_credit_on_late_work_failure() {
    let rows = vec![
        scalar(),
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 4,
                    layout: StorageLayoutIdV1(0),
                }]
                .into_boxed_slice(),
            ),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: StorageLayoutKindV1::Variants {
                encoding: StorageVariantEncodingV1::Direct {
                    tag: StorageFieldV1 {
                        offset: 0,
                        layout: StorageLayoutIdV1(0),
                    },
                },
                variants: [0, 1]
                    .map(|tag| StorageVariantV1 {
                        discriminant: tag,
                        direct_tag_bits: Some(tag),
                        uninhabited: false,
                        layout: StorageLayoutIdV1(1),
                    })
                    .into(),
            },
        },
    ];
    let owner = admit(&graph(
        rows,
        vec![object_pointer(
            0,
            AddressSpace::Global,
            AccessMode::ReadWrite,
        )],
        vec![],
    ));
    let verified = owner.verified_storage_module_ref_v1();
    let mut request = request(vec![SimulationStorageArgumentV29::ObjectView(view(
        1,
        0,
        vec![
            SimulationObjectComponentV29::Variant(0),
            SimulationObjectComponentV29::Field(0),
        ],
    ))]);
    request.shared_storage.push(SimulationSharedStorageV29 {
        id: BufferBackingIdV1(1),
        storage: SimulationStorageBackingV29::Object {
            image: image(2, 4, vec![0; 8], vec![true; 8], vec![]),
            access: AccessMode::ReadWrite,
        },
    });
    // Register 17, image lookup 1, argument 1; referent: lookup 1,
    // two path edges 6, initialized/raw tag bytes 8, selector rows 2,
    // variant roster 2, result row + endpoint check 2; publish guard 1, parameter 1.
    const WORK: u64 = 17 + 1 + 1 + 1 + 6 + 8 + 2 + 2 + 2 + 1 + 1;
    for steps in [WORK, WORK - 1] {
        let limit = SimulationLimitsV1 {
            max_steps: steps,
            ..limits()
        };
        let mut memory = Memory::new(1, 1, limit).unwrap();
        memory.storage_accounting.hold(61).unwrap();
        let result = storage_import_inputs_v29(
            &mut StorageInputContextV29 {
                memory: &mut memory,
                target: targets()[0],
                limits: limit,
            },
            &verified,
            &request,
            &owner.module().functions[0],
        );
        if steps == WORK {
            assert!(result.is_ok());
        } else {
            assert!(
                matches!(result, Err(SimulationExecutionErrorKindV1::StepLimit { limit }) if limit == steps)
            );
        }
        assert_eq!(memory.storage_accounting.steps(), steps);
        assert_eq!(
            memory.storage_accounting.held(),
            61 + size_of::<StorageGuardV1>()
        );
        assert_eq!(
            memory
                .allocations
                .values()
                .map(|allocation| allocation.storage.guards.len())
                .sum::<usize>(),
            1
        );
        assert!(
            memory
                .allocations
                .values()
                .all(|allocation| allocation.storage.relocations.is_empty())
        );
    }
    let argument = size_of::<Option<PreparedInputArgumentV29>>();
    let prepared = size_of::<PreparedInputGuardV29>();
    let published = size_of::<StorageGuardV1>();
    let peak = (independent_growth_headers::<Option<PreparedInputArgumentV29>>() + argument)
        .max(argument + independent_growth_headers::<PreparedInputGuardV29>() + prepared)
        .max(argument + prepared + independent_growth_headers::<StorageGuardV1>() + published);
    let required = 61 + independent_input_headers() + peak;
    for bytes in [required, required - 1] {
        let limit = SimulationLimitsV1 {
            max_resident_bytes: bytes,
            ..limits()
        };
        let mut memory = Memory::new(1, 1, limit).unwrap();
        memory.storage_accounting.hold(61).unwrap();
        let result = storage_import_inputs_v29(
            &mut StorageInputContextV29 {
                memory: &mut memory,
                target: targets()[0],
                limits: limit,
            },
            &verified,
            &request,
            &owner.module().functions[0],
        );
        if bytes == required {
            assert!(result.is_ok());
            assert_eq!(memory.storage_accounting.held(), 61 + published);
            assert_eq!(
                memory
                    .allocations
                    .values()
                    .map(|allocation| allocation.storage.guards.len())
                    .sum::<usize>(),
                1
            );
        } else {
            assert!(
                matches!(result, Err(SimulationExecutionErrorKindV1::StorageResidentLimit { limit, .. }) if limit == bytes)
            );
            assert_eq!(memory.storage_accounting.held(), 61);
            assert!(
                memory
                    .allocations
                    .values()
                    .all(|allocation| allocation.storage.guards.is_empty())
            );
        }
    }
}

#[test]
fn explicit_input_view_cannot_substitute_a_legacy_schedule_request() {
    let owner = admit(&graph(vec![], vec![], vec![]));
    let request = request(vec![]);
    let verified = owner.verified_storage_module_ref_v1();
    let target = targets()[0];
    let plan = crate::preflight::preflight_storage_inputs_v29(
        &verified,
        0,
        &request,
        None,
        target,
        limits(),
    )
    .unwrap();
    assert!(
        SimulationRequestRefV29::Storage(&request)
            .legacy()
            .is_none()
    );
    assert!(
        StorageExecutionScheduleV1::prepare_inputs_v29(
            None,
            None,
            None,
            None,
            target,
            limits(),
            &plan,
            1,
            0
        )
        .is_ok()
    );
    for schedule in [
        crate::SimulationScheduleRequestV1::RecordCanonical { max_decisions: 1 },
        crate::SimulationScheduleRequestV1::RecordSeeded {
            seed: 9,
            max_decisions: 1,
        },
    ] {
        assert!(matches!(
            StorageExecutionScheduleV1::prepare_inputs_v29(
                Some(ExecutionScheduleRequestV1::Public(schedule)),
                None,
                None,
                None,
                target,
                limits(),
                &plan,
                1,
                0
            ),
            Err(SchedulePrepareErrorV1::Replay(
                crate::SimulationScheduleReplayErrorV1::CoverageMismatch
            ))
        ));
    }
}

#[test]
fn repeated_runs_and_callback_errors_or_panics_do_not_mutate_original_inputs() {
    struct Sink {
        panic: bool,
        calls: usize,
    }
    impl SimulationEventSinkV1 for Sink {
        fn record(&mut self, _: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
            self.calls += 1;
            if self.panic {
                panic!("input callback sentinel");
            }
            Err(SimulationEventSinkErrorV1 {
                detail: "input callback sentinel".into(),
            })
        }
    }
    let owner = admit(&graph(vec![scalar()], vec![inline_type(0)], vec![]));
    let request = request(vec![SimulationStorageArgumentV29::InlineObject(image(
        0,
        4,
        vec![0; 4],
        vec![true; 4],
        vec![],
    ))]);
    let original = request.clone();
    let original_bytes = owner.canonical_bytes().to_vec();
    for panic in [false, true] {
        let mut sink = Sink { panic, calls: 0 };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            simulate_canonical_storage_inputs_with_sinks_v29(
                &owner,
                &request,
                None,
                targets()[0],
                limits(),
                SimulationDebugCaptureLimitsV1::disabled(),
                &mut sink,
                &mut NoopSimulationDebugSinkV1,
            )
        }));
        if panic {
            assert!(outcome.is_err());
        } else {
            assert!(outcome.unwrap().is_err());
        }
        assert!(sink.calls > 0);
        assert_eq!(request, original);
        assert_eq!(owner.canonical_bytes(), original_bytes);
        for _ in 0..2 {
            let result =
                simulate_canonical_storage_inputs_v29(&owner, &request, targets()[0], limits())
                    .unwrap();
            assert_eq!(result.steps_executed(), 23);
        }
    }
}
