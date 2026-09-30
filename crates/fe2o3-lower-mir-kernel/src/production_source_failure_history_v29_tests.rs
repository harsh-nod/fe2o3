fn failure_history_fixture_v29(
    typed: bool,
) -> (
    Function,
    Vec<ScopedSourceSlotV29>,
    Vec<fe2o3_kernel_ir::StorageLayoutV1>,
) {
    let mut entry = block(77);
    entry.operations = vec![
        allocation(A, Type::Scalar(ScalarType::U32)),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: A,
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: A,
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ];
    let mut function = Function::internal_helper(
        "failure-history",
        fe2o3_kernel_ir::Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(1)],
        vec![entry],
    );
    let mut slots = candidates(&function);
    let mut layouts = Vec::new();
    if typed {
        let schema = fe2o3_kernel_ir::StorageLayoutIdV1(0);
        layouts.push(fe2o3_kernel_ir::StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(ScalarType::U32),
        });
        let slot = &mut slots[0];
        slot.origin.identity = ScopedAllocationIdentityV29::OriginalObject {
            local: 0,
            generation: 0,
        };
        slot.origin.source = ScopedAllocationSourceV29::OriginalObject { cell: 0, schema };
        slot.representation = ScopedSlotRepresentationV29::Object {
            schema,
            bytes: 4,
            alignment: 4,
        };
        let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
        operations[0] = allocation(A, Type::StorageObject(schema));
        let OperationKind::Alloca { alignment, .. } = &mut operations[0].kind else {
            unreachable!()
        };
        *alignment = 4;
        operations[1].kind = OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
            address: A,
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        });
        operations[2].kind = OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
            address: A,
            access: MemoryAccess::new(AddressSpace::Private, 4),
        });
    }
    (function, slots, layouts)
}

fn run_unprojected_object_history_v29(
    bytes: u64,
    fault: u8,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    use fe2o3_kernel_ir::{StorageFieldV1, StorageLayoutIdV1 as Id, StorageLayoutKindV1};
    let (mut function, mut slots, mut layouts) = failure_history_fixture_v29(true);
    layouts.push(fe2o3_kernel_ir::StorageLayoutV1 {
        size: bytes,
        alignment: 4,
        kind: StorageLayoutKindV1::Record(
            if bytes == 0 {
                vec![]
            } else {
                vec![StorageFieldV1 {
                    offset: 0,
                    layout: Id(0),
                }]
            }
            .into(),
        ),
    });
    let mut slot = slots[0];
    slot.origin.identity = ScopedAllocationIdentityV29::OriginalObject {
        local: 1,
        generation: 0,
    };
    slot.origin.source = ScopedAllocationSourceV29::OriginalObject {
        cell: 1,
        schema: Id(1),
    };
    slot.origin.pointer = ValueId(99);
    slot.representation = ScopedSlotRepresentationV29::Object {
        schema: Id(1),
        bytes,
        alignment: 4,
    };
    slot.allocation.operation = 3;
    slots.push(slot);
    let mut object = allocation(ValueId(99), Type::StorageObject(Id(1)));
    let OperationKind::Alloca { alignment, .. } = &mut object.kind else {
        unreachable!()
    };
    *alignment = 4;
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .push(object);
    let accesses = [
        SourceAddressAccessV29 {
            block: BlockId(77),
            operation: 1,
            slot: 0,
        },
        SourceAddressAccessV29 {
            block: BlockId(77),
            operation: 2,
            slot: 0,
        },
    ];
    let kills = if fault == 1 {
        vec![SourceAddressKillV29 {
            block: BlockId(77),
            gap: 2,
            slot: 0,
        }]
    } else {
        vec![]
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let mut graph = SourceAddressMemoryV29::prepare_with_layouts(
            &function, &slots, None, &accesses, &layouts, budget,
        )?
        .solve(&slots, &accesses, &kills, budget)?;
        assert!(graph.projections.is_empty());
        if fault == 2 {
            graph.object_layouts.pop();
        }
        scoped_slot_uses_v29::check_expanded_scalar_addresses_v29(
            &function, &graph, &slots, &accesses, &kills, budget,
        )
    });
    assert_eq!(budget.storage(), FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn unprojected_object_history_preserves_scalar_initialization_and_exact_schemas() {
    for bytes in [0, 4] {
        run_unprojected_object_history_v29(bytes, 0, LIMIT, LIMIT)
            .0
            .unwrap();
        unsupported(run_unprojected_object_history_v29(bytes, 1, LIMIT, LIMIT).0);
        unsupported(run_unprojected_object_history_v29(bytes, 2, LIMIT, LIMIT).0);
    }
}

#[test]
fn unprojected_object_history_has_exact_and_one_short_cumulative_limits() {
    for bytes in [0, 4] {
        let (result, work, peak) = run_unprojected_object_history_v29(bytes, 0, LIMIT, LIMIT);
        result.unwrap();
        let (result, exact_work, exact_peak) =
            run_unprojected_object_history_v29(bytes, 0, work, peak);
        result.unwrap();
        assert_eq!((exact_work, exact_peak), (work, peak));
        for (work_limit, storage_limit) in [(work - 1, peak), (work, peak - 1)] {
            assert!(
                run_unprojected_object_history_v29(bytes, 0, work_limit, storage_limit)
                    .0
                    .is_err()
            );
        }
    }
}

fn run_failure_history_v29(
    typed: bool,
    failures: &[(usize, bool)],
    killed: bool,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    // Inert physical history equations only; full source tests independently
    // authenticate these diagnostic positions, local identities and move roles.
    let (function, slots, layouts) = failure_history_fixture_v29(typed);
    let accesses = [
        SourceAddressAccessV29 {
            block: BlockId(77),
            operation: 1,
            slot: 0,
        },
        SourceAddressAccessV29 {
            block: BlockId(77),
            operation: 2,
            slot: 0,
        },
    ];
    let kills = if killed {
        vec![SourceAddressKillV29 {
            block: BlockId(77),
            gap: 2,
            slot: 0,
        }]
    } else {
        vec![]
    };
    let failures = failures
        .iter()
        .enumerate()
        .map(|(anchor, &(gap, move_after))| SourceIndexFailureV29 {
            instance: ProductionCallInstanceIdV1(0),
            anchor,
            slot: 0,
            block: BlockId(77),
            gap,
            move_after,
        })
        .collect::<Vec<_>>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_with_layouts(
            &function, &slots, None, &accesses, &layouts, budget,
        )?
        .solve(&slots, &accesses, &kills, budget)?;
        scoped_slot_uses_v29::check_expanded_scalar_addresses_with_failures_v29(
            &function, &graph, &slots, &accesses, &kills, &failures, budget,
        )
    });
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn failure_moves_preserve_success_reads_but_invalidate_later_ordered_diagnostics() {
    for typed in [false, true] {
        for failures in [
            vec![(2, false)],
            vec![(2, true)],
            vec![(2, false), (2, true)],
        ] {
            run_failure_history_v29(typed, &failures, false, LIMIT, LIMIT)
                .0
                .unwrap();
        }
        for (failures, killed) in [
            (vec![(1, false)], false),
            (vec![(2, true), (2, false)], false),
            (vec![(2, true), (2, true)], false),
            (vec![(2, false)], true),
        ] {
            unsupported(run_failure_history_v29(typed, &failures, killed, LIMIT, LIMIT).0);
        }
    }
}

#[test]
fn expanded_failure_history_preserves_exact_limits_and_original_caller_floor() {
    for typed in [false, true] {
        let failures = [(2, false), (2, true)];
        let (result, work, storage) =
            run_failure_history_v29(typed, &failures, false, LIMIT, LIMIT);
        result.unwrap();
        run_failure_history_v29(typed, &failures, false, work, storage)
            .0
            .unwrap();
        assert!(matches!(
            run_failure_history_v29(typed, &failures, false, work - 1, storage).0,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                )
            )
        ));
        assert!(matches!(
            run_failure_history_v29(typed, &failures, false, work, storage - 1).0,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                )
            )
        ));
    }
}
