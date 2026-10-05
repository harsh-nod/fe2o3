use super::*;

#[path = "canonical_predicated_slice_domains_v85_tests.rs"]
mod batch_v85;

fn predicated_fixture(axis: Axis, access: AccessMode, shape: usize) -> Module {
    let mut module = fixture(axis, access, false);
    let function = &mut module.functions[0];
    function.signature.parameters.push(Type::BOOL);
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(10));
    let mut store_operations = std::mem::take(&mut body.blocks[1].operations);
    body.blocks.truncate(1);
    let entry = &mut body.blocks[0];
    let mut predicate = ValueId(4);
    if shape != 0 {
        entry.operations.push(op(
            9,
            Type::BOOL,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(10),
                rhs: predicate,
            },
        ));
        predicate = ValueId(9);
    }
    if shape == 2 {
        entry.operations.push(op(
            11,
            Type::BOOL,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: predicate,
                rhs: predicate,
            },
        ));
        predicate = ValueId(11);
    }
    if shape == 3 {
        entry.operations.extend([
            op(
                11,
                Type::BOOL,
                OperationKind::Unary {
                    op: crate::UnaryOp::Not,
                    operand: predicate,
                },
            ),
            op(
                12,
                Type::BOOL,
                OperationKind::Unary {
                    op: crate::UnaryOp::Not,
                    operand: ValueId(11),
                },
            ),
        ]);
        predicate = ValueId(12);
    }
    let carried = (shape == 4).then_some(predicate);
    if carried.is_some() {
        predicate = ValueId(30);
    }
    let mut tail = vec![
        op(7, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            8,
            Type::INDEX,
            OperationKind::Select {
                condition: predicate,
                true_value: ValueId(2),
                false_value: ValueId(7),
            },
        ),
    ];
    let OperationKind::GetElementPointer { offset, .. } = &mut store_operations[1].kind else {
        panic!()
    };
    *offset = ValueId(8);
    store_operations[2].kind = OperationKind::GuardedStore {
        pointer: ValueId(6),
        predicate,
        value: ValueId(1),
        access: MemoryAccess::new(AddressSpace::Global, 4),
    };
    tail.extend(store_operations);
    if let Some(original) = carried {
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(20),
            arguments: vec![original],
        });
        let mut next = BasicBlock::new(BlockId(20));
        next.parameters.push(ValueDef::new(ValueId(30), Type::BOOL));
        next.operations = tail;
        next.terminator = Some(Terminator::Return { values: vec![] });
        body.blocks.push(next);
    } else {
        entry.operations.extend(tail);
        entry.terminator = Some(Terminator::Return { values: vec![] });
    }
    module
}

fn predicated_coordinate(module: &Module) -> Coordinate {
    let blocks = &module.functions[0].body.as_ref().unwrap().blocks;
    Coordinate {
        block: BlockCoordinate {
            function: FunctionCoordinate(0),
            block: (blocks.len() - 1) as u32,
        },
        operation: (blocks.last().unwrap().operations.len() - 1) as u32,
    }
}

fn run_predicated<T>(
    graph: &VerifiedCanonicalKernelIrModuleV18,
    credit: usize,
    consume: impl for<'s> FnOnce(
        &CheckedCanonicalGuardedGlobalStoresV24<'s, '_>,
        &mut Budget<'_>,
    ) -> Result<T>,
) -> Result<T> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 100_000_000);
    budget.reserve_storage(credit + 17).unwrap();
    let result = with_canonical_predicated_global_stores_v84(
        graph,
        Default::default(),
        &mut budget,
        consume,
    );
    assert_eq!(budget.storage(), credit + 17);
    result
}

#[test]
fn predicated_store_v84_direct_conjunct_and_carrier_keep_exact_domain_without_cfg_truth() {
    for shape in 0..5 {
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            for access in [AccessMode::WriteOnly, AccessMode::ReadWrite] {
                let module = predicated_fixture(axis, access, shape);
                let at = predicated_coordinate(&module);
                let OperationKind::GuardedStore { predicate, .. } = module.functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .last()
                    .unwrap()
                    .operations
                    .last()
                    .unwrap()
                    .kind
                else {
                    panic!()
                };
                let (graph, credit) = owner(&module);
                run_predicated(&graph, credit, |view, budget| {
                    assert!(std::ptr::eq(view.owner(budget)?, &graph));
                    assert_eq!(
                        view.function_effects(FunctionCoordinate(0), budget)?,
                        (1, 0, 0)
                    );
                    let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(store) =
                        view.store_at(at, budget)?
                    else {
                        panic!("shape {shape}")
                    };
                    assert_eq!(
                        store.domain().path(),
                        FormalGuardedPathV1::ExplicitPredicate
                    );
                    assert_eq!(store.domain().predicate(), predicate);
                    assert_eq!(store.domain().index(), ValueId(2));
                    assert_eq!(store.domain().slice(), ValueId(0));
                    assert_eq!(store.domain().allocation().parameter_index(), 0);
                    assert_eq!(store.domain().pointer(), ValueId(6));
                    assert_eq!(store.domain().element_bytes(), 4);
                    assert_eq!(store.comparison_operands(), (ValueId(2), ValueId(3)));
                    assert_eq!(store.normalized_length_origin(), ValueId(3));
                    assert_eq!(
                        store.invocation_projection(budget)?,
                        Some((axis, ValueId(2)))
                    );
                    assert!(view.true_at(at, ValueId(4), budget)?.is_none());
                    assert!(view.true_at(at, predicate, budget)?.is_none());
                    let mut extents = [1; 3];
                    extents[match axis {
                        Axis::X => 0,
                        Axis::Y => 1,
                        Axis::Z => 2,
                    }] = 256;
                    assert!(
                        store
                            .distinct_invocations(
                                ExplicitLaunchExtent::Exact { rank: 3, extents },
                                FormalIndexWidth::Bits64,
                                budget
                            )?
                            .is_some()
                    );
                    Ok(())
                })
                .unwrap();
            }
        }
    }
}

#[test]
fn predicated_store_v84_rejects_predicate_address_length_root_and_access_substitutions() {
    for mutation in 0..12 {
        let mut module = predicated_fixture(Axis::X, AccessMode::WriteOnly, 1);
        let function = &mut module.functions[0];
        if mutation == 6 {
            function
                .signature
                .parameters
                .push(function.signature.parameters[0].clone());
            function.body.as_mut().unwrap().parameters.push(ValueId(20));
        }
        let ops = &mut function.body.as_mut().unwrap().blocks[0].operations;
        match mutation {
            0 => {
                let OperationKind::Select { condition, .. } = &mut ops[5].kind else {
                    panic!()
                };
                *condition = ValueId(4);
            }
            1 => {
                let OperationKind::GuardedStore { predicate, .. } = &mut ops[8].kind else {
                    panic!()
                };
                *predicate = ValueId(4);
            }
            2 => ops[4].kind = OperationKind::Constant(Constant::Index(1)),
            3 => {
                let OperationKind::Binary { op, .. } = &mut ops[3].kind else {
                    panic!()
                };
                *op = BinaryOp::BitOr;
            }
            4 => {
                let OperationKind::Compare { predicate, .. } = &mut ops[2].kind else {
                    panic!()
                };
                *predicate = ComparePredicate::GreaterThan;
            }
            5 => ops[1].kind = OperationKind::Constant(Constant::Index(4096)),
            6 => ops[1].kind = OperationKind::SliceLength { slice: ValueId(20) },
            7 => {
                let OperationKind::Select { true_value, .. } = &mut ops[5].kind else {
                    panic!()
                };
                *true_value = ValueId(7);
            }
            8 => {
                let OperationKind::GuardedStore { access, .. } = &mut ops[8].kind else {
                    panic!()
                };
                access.volatile = true;
            }
            9 => {
                let OperationKind::GuardedStore { access, .. } = &mut ops[8].kind else {
                    panic!()
                };
                access.alignment = 8;
            }
            10 => {
                ops[3].kind = OperationKind::Unary {
                    op: crate::UnaryOp::Not,
                    operand: ValueId(4),
                };
            }
            _ => {
                let OperationKind::Select { condition, .. } = &mut ops[5].kind else {
                    panic!()
                };
                *condition = ValueId(10);
                let OperationKind::GuardedStore { predicate, .. } = &mut ops[8].kind else {
                    panic!()
                };
                *predicate = ValueId(10);
            }
        }
        let at = predicated_coordinate(&module);
        let (graph, credit) = owner(&module);
        run_predicated(&graph, credit, |view, budget| {
            assert!(
                matches!(
                    view.store_at(at, budget)?,
                    CanonicalGuardedGlobalStoreOutcomeV24::NotProved(_)
                ),
                "mutation {mutation}"
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn predicated_store_v84_legacy_entry_remains_closed_and_ordinary_edges_unchanged() {
    let module = predicated_fixture(Axis::X, AccessMode::WriteOnly, 1);
    let at = predicated_coordinate(&module);
    let (graph, credit) = owner(&module);
    run(&graph, credit, |view, budget| {
        assert!(matches!(
            view.store_at(at, budget)?,
            CanonicalGuardedGlobalStoreOutcomeV24::NotProved(_)
        ));
        Ok(())
    })
    .unwrap();
    let (ordinary, credit) = owner(&fixture(Axis::X, AccessMode::WriteOnly, false));
    run_predicated(&ordinary, credit, |view, budget| {
        let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(store) =
            view.store_at(coordinate(2), budget)?
        else {
            panic!()
        };
        assert!(matches!(
            store.domain().path(),
            FormalGuardedPathV1::TrueEdge { .. }
        ));
        assert!(view.true_at(coordinate(2), ValueId(4), budget)?.is_some());
        Ok(())
    })
    .unwrap();
}

#[test]
fn predicated_store_v84_local_bound_does_not_supply_injectivity_or_erase_other_effects() {
    let mut module = predicated_fixture(Axis::X, AccessMode::ReadWrite, 0);
    let at = predicated_coordinate(&module);
    let ops = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    ops[0].kind = OperationKind::Constant(Constant::Index(0));
    ops.push(op(
        25,
        Type::Scalar(ScalarType::U32),
        OperationKind::Load {
            pointer: ValueId(6),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    ops.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(6),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    let (graph, credit) = owner(&module);
    run_predicated(&graph, credit, |view, budget| {
        assert_eq!(
            view.function_effects(FunctionCoordinate(0), budget)?,
            (2, 1, 0)
        );
        let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(store) =
            view.store_at(at, budget)?
        else {
            panic!()
        };
        assert_eq!(store.invocation_projection(budget)?, None);
        assert!(
            store
                .distinct_invocations(
                    ExplicitLaunchExtent::Exact {
                        rank: 1,
                        extents: [256, 1, 1]
                    },
                    FormalIndexWidth::Bits64,
                    budget
                )?
                .is_none()
        );
        for operation in at.operation + 1..=at.operation + 2 {
            assert!(matches!(
                view.store_at(Coordinate { operation, ..at }, budget)?,
                CanonicalGuardedGlobalStoreOutcomeV24::NotProved(_)
            ));
        }
        Ok(())
    })
    .unwrap();
}

fn measured_predicated(
    graph: &VerifiedCanonicalKernelIrModuleV18,
    credit: usize,
    at: Coordinate,
    work: usize,
    storage: usize,
) -> (Result<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = Budget::new(&mut work, storage);
    budget.reserve_storage(credit + 17).unwrap();
    let result = with_canonical_predicated_global_stores_v84(
        graph,
        Default::default(),
        &mut budget,
        |view, budget| {
            let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(store) =
                view.store_at(at, budget)?
            else {
                panic!()
            };
            assert!(
                store
                    .distinct_invocations(
                        ExplicitLaunchExtent::Exact {
                            rank: 1,
                            extents: [256, 1, 1]
                        },
                        FormalIndexWidth::Bits64,
                        budget
                    )?
                    .is_some()
            );
            Ok(())
        },
    );
    assert_eq!(budget.storage(), credit + 17);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn predicated_store_v84_exact_and_one_short_transaction_resources_preserve_owner_floor() {
    for shape in 0..5 {
        let module = predicated_fixture(Axis::X, AccessMode::WriteOnly, shape);
        let at = predicated_coordinate(&module);
        let (graph, credit) = owner(&module);
        let (result, work, peak) =
            measured_predicated(&graph, credit, at, 100_000_000, 100_000_000);
        result.unwrap();
        measured_predicated(&graph, credit, at, work, peak)
            .0
            .unwrap();
        assert!(matches!(
            measured_predicated(&graph, credit, at, work - 1, peak).0,
            Err(Failure::Resource(ResourceError::Work(_)))
        ));
        assert!(matches!(
            measured_predicated(&graph, credit, at, work, peak - 1).0,
            Err(Failure::Resource(ResourceError::Storage { .. }))
        ));
    }
}

#[test]
fn predicated_store_v84_foreign_refunded_and_exhausted_queries_are_sticky_before_retry_debit() {
    let module = predicated_fixture(Axis::X, AccessMode::WriteOnly, 2);
    let at = predicated_coordinate(&module);
    let (graph, credit) = owner(&module);
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget = Budget::new(&mut work, 100_000_000);
        budget.reserve_storage(credit + 17).unwrap();
        let retained = Cell::new(0);
        let error = with_canonical_predicated_global_stores_v84(
            &graph,
            Default::default(),
            &mut budget,
            |view, budget| {
                retained.set(budget.storage());
                let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(store) =
                    view.store_at(at, budget)?
                else {
                    panic!()
                };
                let first = match mode {
                    0 => {
                        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
                        let mut foreign = Budget::new(&mut work, 100_000_000);
                        foreign.reserve_storage(budget.storage())?;
                        let error = store.invocation_projection(&mut foreign).unwrap_err();
                        assert_eq!(foreign.work(), 0);
                        error
                    }
                    1 => {
                        budget.release_storage(1)?;
                        let before = budget.work();
                        let error = store.invocation_projection(budget).unwrap_err();
                        assert_eq!(budget.work(), before);
                        budget.reserve_storage(1)?;
                        error
                    }
                    _ => {
                        budget.charge_work(100_000_000 - budget.work() - 5)?;
                        store.invocation_projection(budget).unwrap_err()
                    }
                };
                let after = budget.work();
                assert_eq!(store.invocation_projection(budget).unwrap_err(), first);
                assert_eq!(budget.work(), after);
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(
            budget.storage(),
            if mode == 1 {
                retained.get()
            } else {
                credit + 17
            }
        );
        assert!(matches!(
            error,
            Failure::Resource(ResourceError::Accounting | ResourceError::Work(_))
        ));
    }
}

#[test]
fn predicated_store_v84_callback_error_unwind_and_leak_preserve_scope_cleanup() {
    let module = predicated_fixture(Axis::X, AccessMode::WriteOnly, 1);
    let at = predicated_coordinate(&module);
    let (graph, credit) = owner(&module);
    for mode in 0..3 {
        let result = run_predicated(&graph, credit, |view, budget| {
            assert!(matches!(
                view.store_at(at, budget)?,
                CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(_)
            ));
            match mode {
                0 => Err(Failure::Coordinate(at)),
                1 => panic!("predicated-store callback"),
                _ => {
                    budget.reserve_storage(13)?;
                    Ok(())
                }
            }
        });
        assert_eq!(
            result,
            Err(match mode {
                0 => Failure::Coordinate(at),
                1 => Failure::Panicked,
                _ => Failure::Resource(ResourceError::Accounting),
            })
        );
    }
}
