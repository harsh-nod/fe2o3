fn run_scalar_range_history_v45(
    right: u64,
    reads: &[(u64, u64, bool, bool)],
    later_right: bool,
    reset: bool,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    run_scalar_range_history_tail_v45(
        right,
        reads,
        later_right,
        reset,
        0,
        work_limit,
        storage_limit,
    )
}

fn run_scalar_range_history_tail_v45(
    right: u64,
    reads: &[(u64, u64, bool, bool)],
    later_right: bool,
    reset: bool,
    tail: u8,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    use fe2o3_kernel_ir::{StorageFieldV1, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind};
    let (mut function, mut slots, mut layouts) = failure_history_fixture_v29(true);
    layouts.push(fe2o3_kernel_ir::StorageLayoutV1 {
        size: right + 4,
        alignment: 4,
        kind: Kind::Record(
            vec![
                StorageFieldV1 {
                    offset: 0,
                    layout: Id(0),
                },
                StorageFieldV1 {
                    offset: right,
                    layout: Id(0),
                },
            ]
            .into(),
        ),
    });
    slots[0].origin.source = ScopedAllocationSourceV29::OriginalObject {
        cell: 0,
        schema: Id(1),
    };
    slots[0].representation = ScopedSlotRepresentationV29::Object {
        schema: Id(1),
        bytes: right + 4,
        alignment: 4,
    };
    let pointer = Type::pointer(
        Type::StorageObject(Id(0)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    let mut root = allocation(A, Type::StorageObject(Id(1)));
    let OperationKind::Alloca { alignment, .. } = &mut root.kind else {
        unreachable!()
    };
    *alignment = 4;
    let mut operations = vec![root];
    for (field, id) in [(0, ValueId(20)), (1, ValueId(21))] {
        operations.push(Operation::effect_free(
            ValueDef::new(id, pointer.clone()),
            OperationKind::Storage(ScopedObjectOperationV29::Project {
                base: A,
                step: ScopedObjectProjectionV29::Field(field),
            }),
        ));
    }
    for address in [ValueId(20), ValueId(21)] {
        operations.push(Operation::new(
            vec![],
            OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                address,
                value: ValueId(1),
                access,
            }),
        ));
    }
    for (id, address) in [(2, ValueId(21)), (3, ValueId(20))] {
        operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
            OperationKind::Storage(ScopedObjectOperationV29::ReadValue { address, access }),
        ));
    }
    if later_right {
        operations.push(Operation::effect_free(
            ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
            OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
                address: ValueId(21),
                access,
            }),
        ));
    }
    match tail {
        0 => (),
        1 | 2 => operations.truncate(6),
        3 => {
            operations[6] = Operation::new(
                vec![],
                OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                    address: ValueId(21),
                    value: ValueId(2),
                    access,
                }),
            )
        }
        _ => panic!("range history fixture tail"),
    }
    let accesses = (3..operations.len())
        .map(|operation| SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(77),
            operation,
            slot: 0,
        })
        .collect::<Vec<_>>();
    function.body.as_mut().unwrap().blocks[0].operations = operations;
    let kills = if reset || matches!(tail, 1 | 2) {
        vec![SourceAddressKillV29 {
            source_order: match tail {
                1 => [0, 0, 1, 0, 2],
                2 => [0, 0, 1, 0, 0],
                _ => [0; 5],
            },
            block: BlockId(77),
            gap: 6,
            slot: 0,
        }]
    } else {
        vec![]
    };
    let reads = reads
        .iter()
        .enumerate()
        .map(
            |(anchor, &(start, end, moved, failure_only))| SourceIndexFailureV29 {
                source_order: [0, 0, 1, 0, anchor + 1],
                instance: ProductionCallInstanceIdV1(0),
                anchor: anchor + 1,
                slot: 0,
                block: BlockId(77),
                gap: 6,
                move_after: moved,
                range: SourceScalarByteRangeV45 { start, end },
                failure_only,
            },
        )
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
            &function, &graph, &slots, &accesses, &kills, &reads, budget,
        )
    });
    assert_eq!(budget.storage(), FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn scalar_range_history_orders_move_before_same_gap_dead_and_reinitialization() {
    let reads = [(8, 12, true, false)];
    run_scalar_range_history_tail_v45(8, &reads, false, false, 1, LIMIT, LIMIT)
        .0
        .unwrap();
    unsupported(run_scalar_range_history_tail_v45(8, &reads, false, false, 2, LIMIT, LIMIT).0);
    run_scalar_range_history_tail_v45(8, &reads, true, false, 3, LIMIT, LIMIT)
        .0
        .unwrap();
}

#[test]
fn scalar_range_moves_preserve_siblings_and_separate_failure_shadow() {
    for right in [8, 1 << 40] {
        let range = (right, right + 4);
        run_scalar_range_history_v45(
            right,
            &[(range.0, range.1, true, false)],
            false,
            false,
            LIMIT,
            LIMIT,
        )
        .0
        .unwrap();
        run_scalar_range_history_v45(
            right,
            &[(range.0, range.1, true, true), (0, 4, false, true)],
            true,
            false,
            LIMIT,
            LIMIT,
        )
        .0
        .unwrap();
        for (reads, later, reset) in [
            (vec![(range.0, range.1, true, false)], true, false),
            (
                vec![
                    (range.0, range.1, true, true),
                    (range.0, range.1, false, true),
                ],
                false,
                false,
            ),
            (
                vec![
                    (range.0, range.1, true, false),
                    (range.0, range.1, false, true),
                ],
                false,
                false,
            ),
            (vec![(4, right, false, true)], false, false),
            (vec![(right, right + 5, false, true)], false, false),
            (vec![(range.0, range.1, false, true)], false, true),
        ] {
            unsupported(run_scalar_range_history_v45(right, &reads, later, reset, LIMIT, LIMIT).0);
        }
    }
}

#[test]
fn scalar_range_history_is_sparse_and_has_independent_resource_cuts() {
    let mut measured = None;
    for right in [8, 1 << 40] {
        let reads = [(right, right + 4, true, true), (0, 4, false, true)];
        let (result, work, peak) =
            run_scalar_range_history_v45(right, &reads, true, false, LIMIT, LIMIT);
        result.unwrap();
        if let Some(previous) = measured {
            assert_eq!((work, peak), previous);
        }
        measured = Some((work, peak));
        let (result, exact_work, exact_peak) =
            run_scalar_range_history_v45(right, &reads, true, false, work, peak);
        result.unwrap();
        assert_eq!((exact_work, exact_peak), (work, peak));
        assert!(
            matches!(run_scalar_range_history_v45(right, &reads, true, false, work - 1, peak).0,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error)))
                if error.actual() == work && error.limit() == work - 1)
        );
        assert!(
            matches!(run_scalar_range_history_v45(right, &reads, true, false, work, peak - 1).0,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error)))
                if error.actual() == peak && error.limit() == peak - 1)
        );
    }
}
