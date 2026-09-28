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
