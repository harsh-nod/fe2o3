use super::super::super::tests::pointer_flow;
pub(super) use super::super::super::tests::{LAYOUTS, with_checked};
use super::*;
#[path = "canonical_global_pending_generic_v18_tests.rs"]
mod generic_carriers;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirFunctionCoordinateV1, Function, MemoryAccess, Module, OperationKind, ScalarType,
    Signature, SliceType, StorageLayoutIdV1, StorageOperationV1, Type, ValueDef, ValueId,
};

const AMPLE: usize = 1 << 40;

pub(super) fn fixture(count: usize) -> Module {
    let mut module = pointer_flow(false);
    let private = &mut module.functions[0];
    private
        .signature
        .parameters
        .push(Type::Scalar(ScalarType::U64));
    private.body.as_mut().unwrap().parameters.push(ValueId(10));
    private.body.as_mut().unwrap().blocks[0].operations = vec![
        Operation::new(
            vec![ValueDef::new(
                ValueId(11),
                Type::pointer(
                    Type::StorageObject(StorageLayoutIdV1(0)),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            )],
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(0)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(11),
                value: ValueId(10),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            }),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(12), Type::Scalar(ScalarType::U64))],
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(11),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            }),
        ),
    ];
    let scalar = Type::Scalar(ScalarType::U32);
    let slice = Type::Slice(SliceType::new(
        scalar.clone(),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    for ordinal in 0..count {
        let mut block = BasicBlock::new(BlockId(4));
        block.operations = vec![
            Operation::new(
                vec![ValueDef::new(ValueId(4), pointer.clone())],
                OperationKind::SliceData { slice: ValueId(0) },
            ),
            Operation::new(
                vec![ValueDef::new(ValueId(5), Type::INDEX)],
                OperationKind::SliceLength { slice: ValueId(0) },
            ),
            Operation::new(
                vec![ValueDef::new(ValueId(6), pointer.clone())],
                OperationKind::GetElementPointer {
                    base: ValueId(4),
                    offset: ValueId(1),
                },
            ),
            Operation::new(
                vec![ValueDef::new(ValueId(7), scalar.clone())],
                OperationKind::Load {
                    pointer: ValueId(6),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(6),
                    value: ValueId(2),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ];
        block.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(3),
            then_target: BlockId(9),
            then_arguments: vec![],
            else_target: BlockId(9),
            else_arguments: vec![],
        });
        let mut end = BasicBlock::new(BlockId(9));
        end.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::definition(
            format!("global_{ordinal}"),
            Signature::new(
                vec![
                    slice.clone(),
                    Type::INDEX,
                    scalar.clone(),
                    Type::BOOL,
                    slice.clone(),
                    Type::BOOL,
                ],
                vec![],
            ),
            vec![
                ValueId(0),
                ValueId(1),
                ValueId(2),
                ValueId(3),
                ValueId(20),
                ValueId(21),
            ],
            vec![block, end],
        ));
    }
    module
}

pub(super) fn coordinate(function: u32, operation: u32) -> Coordinate {
    Coordinate {
        block: Block {
            function: CanonicalKirFunctionCoordinateV1(function),
            block: 0,
        },
        operation,
    }
}

#[test]
fn pending_global_native_foreign_replacement_never_refunds_same_or_higher_credits() {
    for extra in [0, 17] {
        for disposition in 0..4 {
            let observed = Cell::new(false);
            let caught = catch_unwind(AssertUnwindSafe(|| {
                with_checked(&fixture(1), |checked, budget| {
                    let _ = with_pending_canonical_ranked_source_roles_v18(
                        checked,
                        LAYOUTS,
                        budget,
                        |pending, budget| {
                            let foreign_floor = Cell::new(0);
                            let result = pending.with_pending_global_accesses_v18(
                                pending.owner,
                                budget,
                                |view, budget| {
                                    if disposition == 3 {
                                        assert!(matches!(
                                            view.operation(
                                                pending.owner,
                                                coordinate(99, 0),
                                                budget
                                            ),
                                            Err(Failure::ExactGraph)
                                        ));
                                    }
                                    let expected = budget.storage() + extra;
                                    // A process-lifetime test meter can inhabit the callback's
                                    // universally borrowed Work lifetime; it grants no authority.
                                    let meter = Box::leak(Box::new(Work::new(AMPLE)));
                                    let mut foreign = Budget::new(meter, AMPLE);
                                    foreign.reserve_storage(expected)?;
                                    foreign_floor.set(expected);
                                    let old = std::mem::replace(budget, foreign);
                                    assert_eq!(old.storage() + extra, expected);
                                    drop(old);
                                    match disposition {
                                        0 => Ok(()),
                                        1 => Err(Failure::Callback("foreign replacement sentinel")),
                                        _ => panic!("foreign replacement panic"),
                                    }
                                },
                            );
                            assert_eq!(budget.storage(), foreign_floor.get());
                            assert_eq!(budget.work(), 0);
                            if disposition == 3 {
                                assert!(matches!(result, Err(Failure::ExactGraph)));
                            } else {
                                assert!(matches!(
                                    result,
                                    Err(Failure::Resource(Resource::Accounting))
                                ));
                            }
                            observed.set(true);
                            result
                        },
                    );
                });
            }));
            // The positive-only outer fixture also refuses the replaced ledger.
            assert!(caught.is_err());
            assert!(
                observed.get(),
                "child custody assertions must execute before fixture teardown"
            );
        }
    }
}

#[test]
fn pending_global_native_real_mixed_graph_preserves_all_roles_and_has_no_authority() {
    with_checked(&fixture(2), |checked, budget| {
        let floor = budget.storage();
        with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                let owner = pending.owner;
                let obligations = pending.obligations.len();
                let before_native = NATIVE_STARTS.with(Cell::get);
                pending.with_pending_global_accesses_v18(owner, budget, |view, budget| {
                    assert!(!view.memory_safety_is_complete());
                    assert!(!view.native_stage_coverage_is_complete());
                    assert!(!view.grants_artifact_or_launch_authority());
                    for operation in 0..3 {
                        assert!(
                            view.operation(owner, coordinate(0, operation), budget)?
                                .is_none(),
                            "private operations must not become Global coverage"
                        );
                    }
                    for function in 1..=2 {
                        for operation in 0..5 {
                            let coordinate = coordinate(function, operation);
                            let actual = view.operation(owner, coordinate, budget)?.unwrap();
                            assert!(std::ptr::eq(
                                actual,
                                &owner.module().functions[function as usize]
                                    .body
                                    .as_ref()
                                    .unwrap()
                                    .blocks[0]
                                    .operations[operation as usize]
                            ));
                        }
                        let guard = view
                            .guard_terminator(owner, coordinate(function, 0).block, budget)?
                            .unwrap();
                        assert!(matches!(
                            guard,
                            Terminator::ConditionalBranch {
                                condition: ValueId(3),
                                ..
                            }
                        ));
                    }
                    Ok(())
                })?;
                assert_eq!(pending.obligations.len(), obligations);
                assert_eq!(NATIVE_STARTS.with(Cell::get), before_native);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn pending_global_native_broader_candidates_remain_correspondence_only() {
    let mut module = fixture(1);
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let pointer_to_pointer =
        Type::pointer(pointer.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let private_slice = Type::Slice(SliceType::new(
        scalar.clone(),
        AddressSpace::Private,
        AccessMode::ReadOnly,
    ));
    let private_pointer = Type::pointer(scalar, AddressSpace::Private, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::new(
            vec![ValueDef::new(ValueId(3), pointer.clone())],
            OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, 8),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 8),
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(4), Type::INDEX)],
            OperationKind::SliceLength { slice: ValueId(2) },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(5), private_pointer)],
            OperationKind::SliceData { slice: ValueId(2) },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::definition(
        "non_scalar_or_non_global_candidates",
        Signature::new(vec![pointer_to_pointer, pointer, private_slice], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    with_checked(&module, |checked, budget| {
        with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                pending.with_pending_global_accesses_v18(pending.owner, budget, |view, budget| {
                    for operation in 0..3 {
                        assert!(
                            view.operation(pending.owner, coordinate(2, operation), budget)?
                                .is_some()
                        );
                    }
                    assert!(
                        view.operation(pending.owner, coordinate(2, 3), budget)?
                            .is_none()
                    );
                    assert!(!view.memory_safety_is_complete());
                    assert!(!view.native_stage_coverage_is_complete());
                    assert!(!view.grants_artifact_or_launch_authority());
                    Ok(())
                })
            },
        )
        .unwrap();
    });
}

#[test]
fn pending_global_native_query_cost_does_not_grow_with_other_functions_or_accesses() {
    for count in [1, 8, 32] {
        with_checked(&fixture(count), |checked, budget| {
            with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    pending.with_pending_global_accesses_v18(
                        pending.owner,
                        budget,
                        |view, budget| {
                            let floor = budget.storage();
                            for function in 1..=count {
                                let before = budget.work();
                                assert!(
                                    view.operation(
                                        pending.owner,
                                        coordinate(function as u32, 4),
                                        budget
                                    )?
                                    .is_some()
                                );
                                assert_eq!(budget.work() - before, 16);
                                assert_eq!(budget.storage(), floor);
                            }
                            Ok(())
                        },
                    )
                },
            )
            .unwrap();
        });
    }
}

#[test]
fn pending_global_native_foreign_owner_is_zero_debit_and_latches_exact_first_error() {
    for changed_target in [false, true] {
        with_checked(&fixture(1), |checked, budget| {
            let mut foreign_module = fixture(1);
            if changed_target {
                foreign_module
                    .required_capabilities
                    .insert(fe2o3_kernel_ir::gfx950_xnack_minus_target_capability());
            }
            with_checked(&foreign_module, |foreign, other_budget| {
                let other = foreign.inventory(other_budget).unwrap().owner();
                let result = with_pending_canonical_ranked_source_roles_v18(
                    checked,
                    LAYOUTS,
                    budget,
                    |pending, budget| {
                        pending.with_pending_global_accesses_v18(
                            pending.owner,
                            budget,
                            |view, budget| {
                                let before = (budget.work(), budget.storage());
                                assert!(matches!(
                                    view.operation(other, coordinate(1, 4), budget),
                                    Err(Failure::ExactGraph)
                                ));
                                assert_eq!((budget.work(), budget.storage()), before);
                                assert!(matches!(
                                    view.operation(pending.owner, coordinate(1, 4), budget),
                                    Err(Failure::ExactGraph)
                                ));
                                assert_eq!((budget.work(), budget.storage()), before);
                                Ok(())
                            },
                        )
                    },
                );
                assert!(matches!(result, Err(Failure::ExactGraph)));
            });
        });
    }
}

#[test]
fn pending_global_native_nested_higher_floor_loss_is_not_refunded_by_parent() {
    for disposition in 0..4 {
        with_checked(&fixture(1), |checked, budget| {
            let floor = budget.storage();
            let lost = Cell::new(None);
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    let parent = budget.storage();
                    pending.with_pending_global_accesses_v18(
                        pending.owner,
                        budget,
                        |view, budget| {
                            if disposition == 3 {
                                assert!(matches!(
                                    view.operation(pending.owner, coordinate(90, 0), budget),
                                    Err(Failure::ExactGraph)
                                ));
                            }
                            assert!(budget.storage() > parent);
                            budget.release_storage(1)?;
                            assert!(budget.storage() > parent);
                            lost.set(Some(budget.storage()));
                            match disposition {
                                0 => Ok(()),
                                1 => Err(Failure::Callback("higher floor sentinel")),
                                _ => panic!("higher floor panic"),
                            }
                        },
                    )
                },
            );
            if disposition == 3 {
                assert!(matches!(result, Err(Failure::ExactGraph)));
            } else {
                assert!(matches!(
                    result,
                    Err(Failure::Resource(Resource::Accounting))
                ));
            }
            assert_eq!(Some(budget.storage()), lost.get());
            // Every graph/view in the failed nested scope has now been dropped.
            // The test, not a production scope, drains its denied fixture credits.
            budget.release_storage(budget.storage() - floor).unwrap();
        });
    }
}

#[test]
fn pending_global_native_first_query_error_survives_later_callback_panic() {
    with_checked(&fixture(1), |checked, budget| {
        let result = with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                pending.with_pending_global_accesses_v18(pending.owner, budget, |view, budget| {
                    assert!(matches!(
                        view.operation(pending.owner, coordinate(90, 0), budget),
                        Err(Failure::ExactGraph)
                    ));
                    panic!("after selected exact graph error");
                })
            },
        );
        assert!(matches!(result, Err(Failure::ExactGraph)));
    });
}

#[test]
fn pending_global_native_fixed_headers_have_an_independent_exact_and_one_short_equation() {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T, Failure>>()
    }
    let expected = h::<PendingCanonicalGlobalAccessesV18<'_, '_>>()
        + h::<Guard>()
        + h::<&VerifiedCanonicalKernelIrModuleV18>()
        + h::<&crate::KirPlironGraphV18<'_>>()
        + h::<&Guard>()
        + h::<&mut Budget<'_>>()
        + h::<Coordinate>()
        + h::<Block>()
        + h::<&Operation>()
        + h::<Option<&Operation>>()
        + h::<&Terminator>()
        + h::<Option<&Terminator>>()
        + h::<OperationKind>()
        + h::<Type>()
        + h::<ValueId>()
        + h::<[usize; 8]>()
        + h::<u64>()
        + h::<()>()
        + h::<Result<(), Failure>>()
        + h::<std::thread::Result<Result<(), Failure>>>()
        + 2 * h::<Option<Failure>>()
        + h::<(Ledger, usize, usize, usize)>()
        + h::<(
            &PendingCanonicalRankedSourceRolesV18<'_, '_>,
            &mut Option<Failure>,
        )>()
        + crate::KirPlironGraphV18::pending_global_scan_headers_v18().unwrap()
        + size_of::<std::thread::Result<()>>()
        + h::<(&PendingCanonicalGlobalAccessesV18<'_, '_>, &mut Budget<'_>)>();
    assert_eq!(headers(0, 1).unwrap(), expected + 2);
    for short in [false, true] {
        let mut work = Work::new(1);
        let mut budget = Budget::new(&mut work, expected + 2 - usize::from(short));
        let result = budget.reserve_storage(headers(0, 1).unwrap());
        if short {
            assert!(
                matches!(result, Err(Resource::Storage(error)) if error.actual() == error.limit() + 1)
            );
            assert_eq!(budget.storage(), 0);
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), expected + 2);
        }
    }
}

#[test]
fn pending_global_native_census_work_refusal_precedes_uninvoked_consumer_drop_panic() {
    struct Capture<'a> {
        drops: &'a Cell<usize>,
        panic: bool,
    }
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
            if self.panic {
                panic!("uninvoked census consumer capture");
            }
        }
    }
    for panic in [false, true] {
        with_checked(&fixture(1), |checked, budget| {
            let drops = Cell::new(0);
            let entered = Cell::new(false);
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    let floor = budget.storage();
                    let before = budget.work();
                    pending
                        .with_pending_global_accesses_v18(pending.owner, budget, |_, _| Ok(()))?;
                    let required = budget.work() - before;
                    assert!(required > 2);
                    // Cut the authentic carrier census, after all wrapper preflight.
                    budget.charge_work(AMPLE - budget.work() - (required - 1))?;
                    let capture = Capture {
                        drops: &drops,
                        panic,
                    };
                    let error = pending
                        .with_pending_global_accesses_v18(pending.owner, budget, |_, _| {
                            entered.set(true);
                            drop(capture);
                            Ok(())
                        })
                        .unwrap_err();
                    let Failure::Resource(Resource::Work(first)) = error else {
                        panic!("census resource must precede capture panic: {error:?}");
                    };
                    assert_eq!(budget.failed_work(), Some(first.actual()));
                    assert_eq!(first.limit(), AMPLE);
                    assert_eq!(first.actual(), AMPLE + 1);
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(drops.get(), 1);
                    assert!(!entered.get());
                    let before = (budget.work(), budget.storage());
                    assert!(matches!(pending.with_pending_global_accesses_v18(
                        pending.owner, budget, |_, _| { entered.set(true); Ok(()) }
                    ), Err(Failure::Resource(Resource::Work(error))) if error == first));
                    assert_eq!((budget.work(), budget.storage()), before);
                    assert!(!entered.get());
                    Ok(())
                },
            );
            assert!(matches!(result, Err(Failure::Resource(Resource::Work(_)))));
        });
    }
}

#[test]
fn pending_global_native_census_exact_work_enters_consumer_without_extra_debit() {
    with_checked(&fixture(2), |checked, budget| {
        with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                let before = budget.work();
                pending.with_pending_global_accesses_v18(pending.owner, budget, |_, _| Ok(()))?;
                let required = budget.work() - before;
                budget.charge_work(AMPLE - budget.work() - required)?;
                let entered = Cell::new(false);
                pending.with_pending_global_accesses_v18(pending.owner, budget, |_, budget| {
                    assert_eq!(budget.work(), AMPLE);
                    entered.set(true);
                    Ok(())
                })?;
                assert!(entered.get());
                Ok(())
            },
        )
        .unwrap();
    });
}

#[test]
fn pending_global_native_roster_probe_growth_has_an_independent_equation() {
    for extra_functions in [0, 3] {
        for blocks_per_function in [1, 5] {
            for declarations in [0, 7] {
                for short in [false, true] {
                    let mut module = fixture(1);
                    for function in 0..extra_functions {
                        let blocks = (0..blocks_per_function)
                            .map(|ordinal| {
                                let mut block = BasicBlock::new(BlockId(ordinal));
                                block.terminator = Some(if ordinal + 1 == blocks_per_function {
                                    Terminator::Return { values: vec![] }
                                } else {
                                    Terminator::Branch {
                                        target: BlockId(ordinal + 1),
                                        arguments: vec![],
                                    }
                                });
                                block
                            })
                            .collect();
                        module.functions.push(Function::definition(
                            format!("empty_body_{function}"),
                            Signature::new(vec![], vec![]),
                            vec![],
                            blocks,
                        ));
                    }
                    for ordinal in 0..declarations {
                        module.functions.push(Function::declaration(
                            format!("declaration_{ordinal}"),
                            Signature::new(vec![], vec![]),
                        ));
                    }
                    let functions = module.functions.len();
                    let defined = module.functions.iter().filter(|f| f.body.is_some()).count();
                    let blocks: usize = module
                        .functions
                        .iter()
                        .filter_map(|f| f.body.as_ref())
                        .map(|body| body.blocks.len())
                        .sum();
                    let coordinates: usize = module
                        .functions
                        .iter()
                        .filter_map(|f| f.body.as_ref())
                        .flat_map(|body| &body.blocks)
                        .map(|block| block.operations.len() + 1)
                        .sum();
                    with_checked(&module, |checked, budget| {
                        let result = with_pending_canonical_ranked_source_roles_v18(
                            checked,
                            LAYOUTS,
                            budget,
                            |pending, budget| {
                                pending.with_pending_global_accesses_v18(pending.owner, budget, |view, budget| {
                                    let [session, actual_functions, actual_defined, actual_blocks, actual_coordinates] =
                                        view.graph.test_pending_global_roster_counts_v18();
                                    assert_eq!([actual_functions, actual_defined, actual_blocks, actual_coordinates],
                                        [functions, defined, blocks, coordinates]);
                                    // Independent source cardinalities, including declarations and
                                    // bodies with no operations. Coordinate work cannot pay these probes.
                                    let expected = session + 4
                                        + functions * (defined + 4)
                                        + blocks * (blocks + 4)
                                        + coordinates * (coordinates + 4);
                                    let before = (budget.work(), budget.storage());
                                    view.graph.test_pending_global_roster_work_v18(budget)?;
                                    assert_eq!(budget.work() - before.0, expected);
                                    assert_eq!(budget.storage(), before.1);
                                    budget.charge_work(AMPLE - budget.work() - expected + usize::from(short))?;
                                    let result = view.graph.test_pending_global_roster_work_v18(budget);
                                    if short {
                                        assert!(matches!(result, Err(Failure::Resource(Resource::Work(error)))
                                            if error.actual() == AMPLE + 1 && error.limit() == AMPLE));
                                    } else {
                                        assert!(result.is_ok());
                                        assert_eq!(budget.work(), AMPLE);
                                    }
                                    result
                                })
                            },
                        );
                        if short {
                            assert!(matches!(result, Err(Failure::Resource(Resource::Work(_)))));
                        } else {
                            result.unwrap();
                        }
                    });
                }
            }
        }
    }
}

#[test]
fn pending_global_native_parent_refund_header_delta_is_explicit_and_independent() {
    let expected = size_of::<Cell<bool>>() + size_of::<(&Cell<bool>, Ledger, usize, usize)>();
    assert_eq!(pending_refund_headers_v18().unwrap(), expected);
    let mut work = Work::new(1);
    let mut budget = Budget::new(&mut work, expected - 1);
    assert!(
        matches!(budget.reserve_storage(pending_refund_headers_v18().unwrap()),
        Err(Resource::Storage(error)) if error.actual() == expected && error.limit() == expected - 1)
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn pending_global_native_foreign_ledger_is_zero_debit_and_not_recoverable_by_retry() {
    with_checked(&fixture(1), |checked, budget| {
        let result = with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                pending.with_pending_global_accesses_v18(pending.owner, budget, |view, budget| {
                    let mut work = Work::new(AMPLE);
                    let mut foreign = Budget::new(&mut work, AMPLE);
                    foreign.reserve_storage(budget.storage())?;
                    let before = foreign.work();
                    assert!(matches!(
                        view.operation(pending.owner, coordinate(1, 4), &mut foreign),
                        Err(Failure::Resource(Resource::Accounting))
                    ));
                    assert_eq!(foreign.work(), before);
                    assert!(matches!(
                        view.operation(pending.owner, coordinate(1, 4), budget),
                        Err(Failure::Resource(Resource::Accounting))
                    ));
                    Ok(())
                })
            },
        );
        assert!(matches!(
            result,
            Err(Failure::Resource(Resource::Accounting))
        ));
    });
}

#[test]
fn pending_global_native_invalid_occurrence_latches_before_later_valid_query() {
    for coordinate in [
        coordinate(9, 0),
        coordinate(1, 90),
        Coordinate {
            block: Block {
                function: CanonicalKirFunctionCoordinateV1(1),
                block: 90,
            },
            operation: 0,
        },
    ] {
        with_checked(&fixture(1), |checked, budget| {
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    pending.with_pending_global_accesses_v18(
                        pending.owner,
                        budget,
                        |view, budget| {
                            assert!(matches!(
                                view.operation(pending.owner, coordinate, budget),
                                Err(Failure::ExactGraph)
                            ));
                            let before = budget.work();
                            assert!(matches!(
                                view.check_owner(pending.owner, budget),
                                Err(Failure::ExactGraph)
                            ));
                            assert_eq!(budget.work(), before);
                            Ok(())
                        },
                    )
                },
            );
            assert!(matches!(result, Err(Failure::ExactGraph)));
        });
    }
}

#[test]
fn pending_global_native_mutation_and_restore_is_rejected_before_query_work() {
    with_checked(&fixture(1), |checked, budget| {
        let result = with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                pending.with_pending_global_accesses_v18(pending.owner, budget, |view, budget| {
                    view.graph.test_ranked_mutate_and_restore_v18();
                    let before = budget.work();
                    assert!(matches!(
                        view.operation(pending.owner, coordinate(1, 4), budget),
                        Err(Failure::Mutation)
                    ));
                    assert_eq!(budget.work(), before);
                    Ok(())
                })
            },
        );
        assert!(matches!(result, Err(Failure::Mutation)));
    });
}

#[test]
fn pending_global_native_real_same_typed_rhs_base_index_descriptor_and_guard_mutations_are_stale() {
    for fault in 0..5 {
        with_checked(&fixture(1), |checked, budget| {
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    pending.with_pending_global_accesses_v18(
                        pending.owner,
                        budget,
                        |view, budget| {
                            view.graph.test_pending_global_operand_v18(fault);
                            let before = (budget.work(), budget.storage());
                            assert!(matches!(
                                view.operation(pending.owner, coordinate(1, 4), budget),
                                Err(Failure::Mutation)
                            ));
                            assert_eq!((budget.work(), budget.storage()), before);
                            Ok(())
                        },
                    )
                },
            );
            assert!(matches!(result, Err(Failure::Mutation)));
        });
    }
}

#[test]
fn pending_global_native_query_exact_and_one_short_work_cannot_be_suppressed() {
    for remaining in [16, 15] {
        with_checked(&fixture(1), |checked, budget| {
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    pending.with_pending_global_accesses_v18(
                        pending.owner,
                        budget,
                        |view, budget| {
                            budget.charge_work(AMPLE - budget.work() - remaining)?;
                            let result = view.operation(pending.owner, coordinate(1, 4), budget);
                            if remaining == 16 {
                                assert!(result.unwrap().is_some());
                            } else {
                                assert!(matches!(
                                    result,
                                    Err(Failure::Resource(Resource::Work(_)))
                                ));
                                assert_eq!(budget.failed_work(), Some(AMPLE + 1));
                                let before = budget.work();
                                assert!(matches!(
                                    view.operation(pending.owner, coordinate(1, 3), budget),
                                    Err(Failure::Resource(Resource::Work(_)))
                                ));
                                assert_eq!(budget.work(), before);
                            }
                            Ok(())
                        },
                    )
                },
            );
            if remaining == 16 {
                result.unwrap();
            } else {
                assert!(matches!(result, Err(Failure::Resource(Resource::Work(_)))));
            }
        });
    }
}

#[test]
fn pending_global_native_callback_error_and_panic_preserve_floor_and_disposition() {
    for panic in [false, true] {
        with_checked(&fixture(1), |checked, budget| {
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    let floor = budget.storage();
                    let result =
                        pending.with_pending_global_accesses_v18(pending.owner, budget, |_, _| {
                            if panic {
                                panic!("pending global native callback");
                            }
                            Err(Failure::Callback("pending global native callback"))
                        });
                    assert_eq!(budget.storage(), floor);
                    if panic {
                        assert!(matches!(result, Err(Failure::Panicked)));
                    } else {
                        assert!(matches!(
                            result,
                            Err(Failure::Callback("pending global native callback"))
                        ));
                    }
                    Ok(())
                },
            );
            result.unwrap();
        });
    }
}

#[test]
fn pending_global_native_complete_scope_exact_and_one_short_storage() {
    for short in [false, true] {
        with_checked(&fixture(1), |checked, budget| {
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    let callback =
                        |_: &PendingCanonicalGlobalAccessesV18<'_, '_>, _: &mut Budget<'_>| Ok(());
                    let required = headers(
                        std::mem::size_of_val(&callback),
                        std::mem::align_of_val(&callback),
                    )?;
                    let filler = AMPLE - budget.storage() - required + usize::from(short);
                    budget.reserve_storage(filler)?;
                    let before = budget.storage();
                    let result =
                        pending.with_pending_global_accesses_v18(pending.owner, budget, callback);
                    assert_eq!(budget.storage(), before);
                    budget.release_storage(filler)?;
                    if short {
                        assert!(
                            matches!(result, Err(Failure::Resource(Resource::Storage(error)))
                        if error.actual() == error.limit() + 1)
                        );
                        Err(Failure::Callback("expected storage boundary"))
                    } else {
                        result
                    }
                },
            );
            if short {
                assert!(matches!(
                    result,
                    Err(Failure::Resource(Resource::Storage(_)))
                ));
            } else {
                result.unwrap();
            }
        });
    }
}
