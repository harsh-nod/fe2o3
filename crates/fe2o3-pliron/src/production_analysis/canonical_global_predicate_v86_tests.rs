use super::super::tests::{LAYOUTS, coordinate, fixture, with_checked};
use super::*;
use fe2o3_kernel_ir::{ComparePredicate, Module, OperationKind, ValueId};

fn predicate_fixture() -> Module {
    let mut module = fixture(1);
    let operations = &mut module.functions[1].body.as_mut().unwrap().blocks[0].operations;
    for (result, predicate) in [
        (30, ComparePredicate::LessThan),
        (31, ComparePredicate::Equal),
    ] {
        operations.push(Operation::new(
            vec![ValueDef::new(ValueId(result), Type::BOOL)],
            OperationKind::Compare {
                predicate,
                lhs: ValueId(1),
                rhs: ValueId(5),
            },
        ));
    }
    module
}

#[test]
fn pending_global_predicate_v86_preserves_real_definitions_and_no_edge_authority() {
    with_checked(&predicate_fixture(), |checked, budget| {
        with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                pending.with_pending_global_accesses_v18(pending.owner, budget, |view, budget| {
                    let full = Definition::Result {
                        operation: coordinate(1, 6),
                        result: 0,
                    };
                    let bound = Definition::Result {
                        operation: coordinate(1, 5),
                        result: 0,
                    };
                    let floor = budget.storage();
                    let before = budget.work();
                    for pair in [[full, bound], [bound, bound], [bound, full]] {
                        let rows = view
                            .explicit_guard_definitions_v86(
                                pending.owner,
                                pair[0],
                                pair[1],
                                budget,
                            )?
                            .unwrap();
                        for (row, expected) in rows.iter().zip(pair) {
                            let Definition::Result { operation, .. } = expected else {
                                unreachable!()
                            };
                            let actual = &pending.owner.module().functions[1]
                                .body
                                .as_ref()
                                .unwrap()
                                .blocks[0]
                                .operations[operation.operation as usize];
                            assert!(std::ptr::eq(row.0, actual));
                            assert!(std::ptr::eq(row.1, &actual.results[0]));
                        }
                        assert_eq!(budget.storage(), floor);
                    }
                    assert_eq!(budget.work() - before, 3 * 64);
                    assert!(
                        view.explicit_guard_definitions_v86(
                            pending.owner,
                            Definition::Result {
                                operation: coordinate(1, 1),
                                result: 0
                            },
                            bound,
                            budget
                        )?
                        .is_none()
                    );
                    assert!(
                        view.explicit_guard_definitions_v86(
                            pending.owner,
                            Definition::FunctionArgument {
                                function: coordinate(1, 0).block.function,
                                argument: 3
                            },
                            bound,
                            budget
                        )?
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
fn pending_global_predicate_v86_headers_have_independent_exact_storage_bound() {
    // The retained tuple layout and result-discriminant padding are independent
    // of the implementation's Frame alias and contain every coexisting slot.
    type Shape<'a> = (
        &'a PendingCanonicalGlobalAccessesV18<'a, 'a>,
        &'a VerifiedCanonicalKernelIrModuleV18,
        &'a mut Budget<'a>,
        [Definition; 2],
        [Option<(&'a Operation, &'a ValueDef)>; 2],
        Option<(&'a Operation, &'a ValueDef)>,
        Result<Option<[(&'a Operation, &'a ValueDef); 2]>, Failure>,
        Result<(), Failure>,
        [Coordinate; 2],
        [u32; 2],
        [usize; 4],
        [&'a Operation; 2],
        [&'a ValueDef; 2],
        std::iter::Enumerate<std::array::IntoIter<Definition, 2>>,
        Option<(usize, Definition)>,
        Option<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>,
        (
            &'a PendingCanonicalGlobalAccessesV18<'a, 'a>,
            &'a VerifiedCanonicalKernelIrModuleV18,
            &'a mut Budget<'a>,
            Definition,
            Definition,
        ),
    );
    let expected = size_of::<Shape<'_>>() + 2 * size_of::<Result<Shape<'_>, Failure>>();
    assert_eq!(headers(), expected);
    for limit in [expected, expected - 1] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(64);
        let mut budget = Budget::new(&mut work, limit);
        let result = budget.reserve_storage(headers());
        if limit == expected {
            result.unwrap();
            assert_eq!(budget.storage(), expected);
        } else {
            assert!(
                matches!(result, Err(Resource::Storage(error)) if error.actual() == expected && error.limit() == limit)
            );
            assert_eq!(budget.storage(), 0);
        }
        assert_eq!(budget.work(), 0);
    }
}

#[test]
fn pending_global_predicate_v86_exact_work_storage_and_sticky_denials() {
    const AMPLE: usize = 1 << 40;
    for storage_short in [false, true] {
        for remaining in [64, 63] {
            with_checked(&predicate_fixture(), |checked, budget| {
                let result = with_pending_canonical_ranked_source_roles_v18(
                    checked,
                    LAYOUTS,
                    budget,
                    |pending, budget| {
                        pending.with_pending_global_accesses_v18(pending.owner, budget, |view, budget| {
                        let definition = Definition::Result { operation: coordinate(1, 5), result: 0 };
                        let base = budget.storage();
                        let extra = if storage_short { AMPLE - base - headers() + 1 } else { 0 };
                        budget.reserve_storage(extra)?;
                        budget.charge_work(AMPLE - budget.work() - remaining)?;
                        let before = budget.work();
                        let result = view.explicit_guard_definitions_v86(pending.owner, definition, definition, budget);
                        assert_eq!(budget.storage(), base + extra);
                        if storage_short {
                            assert!(matches!(result, Err(Failure::Resource(Resource::Storage(error))) if error.actual() == AMPLE + 1));
                            assert_eq!(budget.work(), before);
                        } else if remaining == 63 {
                            assert!(matches!(result, Err(Failure::Resource(Resource::Work(error))) if error.actual() == AMPLE + 1));
                            assert_eq!(budget.work(), before);
                        } else {
                            assert!(result.unwrap().is_some());
                            assert_eq!(budget.work() - before, 64);
                        }
                        if storage_short || remaining == 63 {
                            let before = budget.work();
                            for _ in 0..2 { assert!(view.explicit_guard_definitions_v86(pending.owner, definition, definition, budget).is_err()); }
                            assert_eq!(budget.work(), before);
                        }
                        budget.release_storage(extra)?;
                        Ok(())
                    })
                    },
                );
                if !storage_short && remaining == 64 {
                    result.unwrap();
                } else {
                    assert!(matches!(result, Err(Failure::Resource(_))));
                }
            });
        }
    }
}

#[test]
fn pending_global_predicate_v86_foreign_and_changed_coordinates_refuse_before_retry() {
    with_checked(&predicate_fixture(), |checked, budget| {
        with_checked(&predicate_fixture(), |foreign, other_budget| {
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
                            let definition = Definition::Result {
                                operation: coordinate(1, 5),
                                result: 0,
                            };
                            let before = (budget.work(), budget.storage());
                            assert!(matches!(
                                view.explicit_guard_definitions_v86(
                                    other, definition, definition, budget
                                ),
                                Err(Failure::ExactGraph)
                            ));
                            assert_eq!((budget.work(), budget.storage()), before);
                            assert!(matches!(
                                view.explicit_guard_definitions_v86(
                                    pending.owner,
                                    definition,
                                    definition,
                                    budget
                                ),
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
    for mutation in 0..3 {
        with_checked(&predicate_fixture(), |checked, budget| {
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    pending.with_pending_global_accesses_v18(
                        pending.owner,
                        budget,
                        |view, budget| {
                            let definition = Definition::Result {
                                operation: coordinate(1, 5),
                                result: 0,
                            };
                            let wrong = match mutation {
                                0 => Definition::Result {
                                    operation: coordinate(99, 5),
                                    result: 0,
                                },
                                1 => Definition::Result {
                                    operation: coordinate(1, 5),
                                    result: 1,
                                },
                                _ => Definition::Result {
                                    operation: coordinate(0, 0),
                                    result: 0,
                                },
                            };
                            let floor = budget.storage();
                            assert!(matches!(
                                view.explicit_guard_definitions_v86(
                                    pending.owner,
                                    definition,
                                    wrong,
                                    budget
                                ),
                                Err(Failure::ExactGraph)
                            ));
                            assert_eq!(budget.storage(), floor);
                            let before = budget.work();
                            assert!(matches!(
                                view.explicit_guard_definitions_v86(
                                    pending.owner,
                                    definition,
                                    definition,
                                    budget
                                ),
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
