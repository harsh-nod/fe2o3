use super::*;

fn raw_owner<const FLAGS: u8>() -> ProductionSemanticSsaOwnerV1 {
    let root = FLAGS & 8 != 0;
    let base = typed_entry_rhs_fixture_v18(root);
    let semantic = base.source_semantic();
    // Replacing the captured pointer removes its only use. Rebuild its type
    // from the live Unit/U32 prefix rather than retaining an orphaned type.
    let mut types = semantic.types()[..2].to_vec();
    let mutable = FLAGS & 1 != 0;
    let moved = FLAGS & 2 != 0;
    let cast = FLAGS & 4 != 0;
    let mutability = if mutable {
        SemanticMutabilityV1::Mutable
    } else {
        SemanticMutabilityV1::Immutable
    };
    let pointer = reference(&mut types, U32, mutability, true);
    let mut functions = semantic.functions().to_vec();
    let at = usize::from(!root);
    let leaf = &functions[at];
    let mut locals = leaf.locals().to_vec();
    locals[2] = local(72, pointer, SemanticLocalRoleV1::Temporary);
    let mut statements = Vec::new();
    if cast {
        let borrowed = reference(&mut types, U32, mutability, false);
        locals.push(local(74, borrowed, SemanticLocalRoleV1::Temporary));
        statements.push(assign(
            place(4, borrowed),
            SemanticRvalueKindV1::Borrow {
                kind: if mutable {
                    SemanticBorrowKindV1::Mutable
                } else {
                    SemanticBorrowKindV1::Shared
                },
                place: place(1, U32),
            },
        ));
        statements.push(assign(
            place(2, pointer),
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Pointer,
                operand: if mutable {
                    SemanticOperandV1::Move(place(4, borrowed))
                } else {
                    SemanticOperandV1::Copy(place(4, borrowed))
                },
            },
        ));
    } else {
        statements.push(assign(
            place(2, pointer),
            SemanticRvalueKindV1::AddressOf {
                place: place(1, U32),
                mutability,
            },
        ));
    }
    let read = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap();
    statements.push(assign(
        place(3, U32),
        SemanticRvalueKindV1::Use(if moved {
            SemanticOperandV1::Move(read)
        } else {
            SemanticOperandV1::Copy(read)
        }),
    ));
    statements.push(leaf.blocks()[0].statements().last().unwrap().clone());
    let replacement = function(
        150,
        leaf.role(),
        leaf.abi().clone(),
        locals,
        vec![block(170, statements, SemanticTerminatorKindV1::Return)],
    );
    functions[at] = if let Some(entry) = leaf.kernel_entry() {
        replacement.with_kernel_entry(entry.clone())
    } else {
        replacement
    };
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn private_raw_fixture_has_only_its_exact_original_pointer_type_closure() {
    for (flags, factory) in [
        raw_owner::<0> as fn() -> _,
        raw_owner::<1>,
        raw_owner::<2>,
        raw_owner::<3>,
        raw_owner::<4>,
        raw_owner::<5>,
        raw_owner::<6>,
        raw_owner::<7>,
        raw_owner::<8>,
        raw_owner::<9>,
        raw_owner::<10>,
        raw_owner::<11>,
        raw_owner::<12>,
        raw_owner::<13>,
        raw_owner::<14>,
        raw_owner::<15>,
    ]
    .into_iter()
    .enumerate()
    {
        let owner = factory();
        let semantic = owner.source_semantic();
        let types = semantic.types();
        let cast = flags & 4 != 0;
        assert_eq!(types.len(), if cast { 4 } else { 3 }, "flags={flags}");
        let mutability = if flags & 1 != 0 {
            SemanticMutabilityV1::Mutable
        } else {
            SemanticMutabilityV1::Immutable
        };
        assert!(
            matches!(types[2].shape(), SemanticTypeShapeV1::Pointer(pointer)
            if pointer.kind() == SemanticPointerKindV1::Raw && pointer.mutability() == mutability)
        );
        if cast {
            assert!(
                matches!(types[3].shape(), SemanticTypeShapeV1::Pointer(pointer)
                if pointer.kind() == SemanticPointerKindV1::Reference && pointer.mutability() == mutability)
            );
        }
        let leaf = &semantic.functions()[usize::from(flags & 8 == 0)];
        assert_eq!(leaf.locals()[2].ty().index(), 2);
        if cast {
            assert_eq!(leaf.locals()[4].ty().index(), 3);
        }
    }
}

#[test]
fn private_raw_dereference_retains_original_addresses_at_roots_and_helpers() {
    for (case, factory) in [
        raw_owner::<0> as fn() -> _,
        raw_owner::<1>,
        raw_owner::<2>,
        raw_owner::<3>,
        raw_owner::<8>,
        raw_owner::<9>,
        raw_owner::<10>,
        raw_owner::<11>,
    ]
    .into_iter()
    .enumerate()
    {
        let (result, _, _, floor, counts, complete) = private_memory_run_v18(
            factory,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |memory, budget| memory.test_check_v18(budget),
        );
        result.unwrap();
        assert!(complete);
        assert_eq!(counts, [if case >= 4 { 1 } else { 2 }; 3], "case={case}");
        assert_eq!(floor, MODULE_FLOOR);
    }
}

#[test]
fn private_raw_dereference_keeps_uncompleted_parent_loan_casts_closed() {
    for factory in [
        raw_owner::<4> as fn() -> _,
        raw_owner::<5>,
        raw_owner::<6>,
        raw_owner::<7>,
        raw_owner::<12>,
        raw_owner::<13>,
        raw_owner::<14>,
        raw_owner::<15>,
    ] {
        let (result, _, _, floor, _, complete) = private_memory_run_v18(
            factory,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |_, _| {
                panic!(
                    "a raw cast with an uncompleted parent loan reached the source-memory consumer"
                )
            },
        );
        assert!(result.is_err() && !complete);
        assert_eq!(floor, MODULE_FLOOR);
    }
}

#[test]
fn private_raw_dereference_rejects_direct_pointer_and_original_slot_substitution() {
    for fault in 0..5 {
        let mut reached = false;
        let (result, _, _, floor, _, complete) = private_memory_run_v18(
            raw_owner::<0>,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |memory, budget| {
                if memory.test_counts_v18(budget)?[0] == 0 {
                    return Ok(());
                }
                reached = true;
                let error = if fault == 4 {
                    memory
                        .test_raw_pointer_substitution_v26(false, budget)
                        .unwrap_err()
                } else {
                    memory
                        .test_replaced_allocation_v18(fault, budget)
                        .unwrap_err()
                };
                assert!(
                    matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_)),
                    "{error:?}"
                );
                let stopped = (budget.work(), budget.storage());
                assert!(matches!(
                    memory.test_check_v18(budget),
                    Err(ProductionSourceOwnedViewErrorV18::Binding(_))
                ));
                assert_eq!((budget.work(), budget.storage()), stopped);
                Err(error)
            },
        );
        assert!(reached && !complete);
        assert!(result.is_err());
        assert_eq!(floor, MODULE_FLOOR);
    }
}

#[test]
fn private_raw_dereference_uses_exact_and_one_short_cumulative_budgets() {
    let run = |work, storage| private_memory_run_v18(raw_owner::<0>, work, storage, |_, _| Ok(()));
    let (result, work, storage, floor, counts, complete) =
        run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(complete);
    assert_eq!(floor, MODULE_FLOOR);
    let (result, exact_work, exact_storage, floor, exact_counts, complete) = run(work, storage);
    result.unwrap();
    assert!(complete);
    assert_eq!(
        (exact_work, exact_storage, floor, exact_counts),
        (work, storage, MODULE_FLOOR, counts)
    );
    for (limit_work, limit_storage, expect_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, _, _, floor, _, complete) = run(limit_work, limit_storage);
        assert!(!complete);
        assert_eq!(floor, MODULE_FLOOR);
        let error = match result {
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    ProductionSourceOwnedViewErrorV18::Resource(error),
                ),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(error),
            )) => error,
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                    ),
                ),
            )) => error,
            other => panic!("typed cumulative resource refusal required: {other:?}"),
        };
        match (expect_work, error) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), limit_work);
                assert!(error.actual() > error.limit());
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), limit_storage);
                assert!(error.actual() > error.limit());
            }
            _ => panic!("wrong cumulative resource boundary: {error:?}"),
        }
    }
}
