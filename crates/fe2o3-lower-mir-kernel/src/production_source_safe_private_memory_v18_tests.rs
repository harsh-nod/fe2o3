fn safe_private_root_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    safe_private_owner_v18(true, false, false, false, false)
}

fn safe_private_helper_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    safe_private_owner_v18(false, false, false, false, false)
}

fn safe_private_nested_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    safe_private_owner_v18(false, true, false, false, false)
}

fn safe_private_mutable_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    safe_private_owner_v18(false, true, true, false, false)
}

fn safe_private_nonentry_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    safe_private_owner_v18(true, false, true, true, false)
}

fn safe_private_move_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    safe_private_owner_v18(true, false, false, false, true)
}

fn safe_private_owner_v18(
    root: bool,
    nested: bool,
    mutable: bool,
    nonentry_write: bool,
    move_read: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let base = typed_entry_rhs_fixture_v18(root);
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let kind = if mutable {
        SemanticBorrowKindV1::Mutable
    } else {
        SemanticBorrowKindV1::Shared
    };
    let shared = reference(
        &mut types,
        U32,
        if mutable {
            SemanticMutabilityV1::Mutable
        } else {
            SemanticMutabilityV1::Immutable
        },
        false,
    );
    let mut functions = semantic.functions().to_vec();
    let at = usize::from(!root);
    let prior = &functions[at];
    assert_eq!(prior.blocks().len(), 1);
    assert_eq!(prior.locals().len(), 4);
    assert_eq!(prior.blocks()[0].statements().len(), 3);
    let mut locals = prior.locals().to_vec();
    locals.extend([
        local(74, shared, SemanticLocalRoleV1::Temporary),
        local(75, shared, SemanticLocalRoleV1::Temporary),
        local(76, U32, SemanticLocalRoleV1::Temporary),
    ]);
    let deref = |local| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
            U32,
        )
        .unwrap()
    };
    let mut statements = prior.blocks()[0].statements()[..2].to_vec();
    statements.extend([
        assign(
            place(4, shared),
            SemanticRvalueKindV1::Borrow {
                kind,
                place: place(1, U32),
            },
        ),
        assign(
            place(5, shared),
            SemanticRvalueKindV1::Borrow {
                kind,
                place: deref(4),
            },
        ),
        assign(
            place(6, U32),
            SemanticRvalueKindV1::Use(if move_read {
                SemanticOperandV1::Move(deref(5))
            } else {
                SemanticOperandV1::Copy(deref(5))
            }),
        ),
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(5)),
        ),
    ]);
    if nonentry_write {
        assert!(mutable);
        statements.insert(4, assign(deref(5), SemanticRvalueKindV1::Use(literal(23))));
    }
    let blocks = if nested {
        let call = SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(2),
                vec![if mutable {
                    SemanticOperandV1::Move(place(4, shared))
                } else {
                    SemanticOperandV1::Copy(place(4, shared))
                }],
                Some(SemanticCallDestinationV1::new(
                    place(0, UNIT),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        );
        vec![
            block(110, statements, call),
            block(
                111,
                vec![
                    SemanticStatementV1::new(
                        source(),
                        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(4)),
                    ),
                    prior.blocks()[0].statements()[2].clone(),
                ],
                SemanticTerminatorKindV1::Return,
            ),
        ]
    } else {
        statements.push(SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(4)),
        ));
        statements.push(prior.blocks()[0].statements()[2].clone());
        vec![
            SemanticBasicBlockV1::new(
                prior.blocks()[0].identity(),
                prior.blocks()[0].source(),
                statements,
                prior.blocks()[0].terminator().clone(),
            )
            .unwrap(),
        ]
    };
    let mut replacement = SemanticFunctionDeclV1::new(
        prior.identity(),
        prior.role(),
        prior.item_definition_identity(),
        prior.monomorphization_identity(),
        prior.generic_type_arguments_identity(),
        prior.const_generic_arguments_identity(),
        prior.source(),
        prior.abi().clone(),
        locals,
        prior.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = prior.kernel_entry() {
        replacement = replacement.with_kernel_entry(entry.clone());
    }
    functions[at] = replacement;
    if nested {
        assert!(!root);
        let helper_abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([121; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(value_abi(&types, shared))],
            ignored(UNIT),
        )
        .unwrap()
        .with_source_argument_ownership(vec![if mutable {
            SemanticSourceArgumentOwnershipV1::UniqueBorrow
        } else {
            SemanticSourceArgumentOwnershipV1::SharedBorrow
        }])
        .unwrap();
        functions.push(function(
            120,
            SemanticFunctionRoleV1::InternalHelper,
            helper_abi,
            vec![
                local(130, UNIT, SemanticLocalRoleV1::Return),
                local(131, shared, SemanticLocalRoleV1::Argument(0)),
                local(132, U32, SemanticLocalRoleV1::Temporary),
                local(133, shared, SemanticLocalRoleV1::Temporary),
            ],
            vec![block(
                140,
                vec![
                    assign(
                        place(3, shared),
                        SemanticRvalueKindV1::Borrow {
                            kind,
                            place: deref(1),
                        },
                    ),
                    assign(
                        place(2, U32),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(deref(3))),
                    ),
                    SemanticStatementV1::new(
                        source(),
                        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(3)),
                    ),
                    assign(
                        place(0, UNIT),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                            SemanticConstantV1::new(UNIT, SemanticConstantValueV1::ZeroSized),
                        )),
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            )],
        ));
    }
    let callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
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
fn safe_private_source_mutable_reads_keep_exact_referent_and_nonentry_writes_pending() {
    let mut seen = [0; 3];
    let (result, _, _, floor, counts, complete) = private_memory_run_v18(
        safe_private_mutable_owner_v18,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |memory, budget| {
            let observed = memory.test_safe_counts_v18(budget)?;
            for (all, count) in seen.iter_mut().zip(observed) {
                *all += count;
            }
            Ok(())
        },
    );
    result.unwrap();
    assert!(complete && counts[2] >= 6 && seen[0] >= 4 && seen[1] >= 2);
    assert_eq!(floor, MODULE_FLOOR);
    assert_eq!(
        seen[2], 0,
        "mutable reads use the exact original RW address"
    );
    let (result, _, _, floor, _, complete) = private_memory_run_v18(
        safe_private_nonentry_owner_v18,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |_, _| panic!("non-entry safe write was completed"),
    );
    assert!(!complete);
    assert_eq!(floor, MODULE_FLOOR);
    assert!(
        matches!(
            result,
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "source private non-entry write remains unresolved"
                )
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn safe_private_source_composition_requires_real_root_helper_and_cross_instance_loans() {
    for (factory, allocations, cross) in [
        (safe_private_root_owner_v18 as fn() -> _, 1, false),
        (safe_private_helper_owner_v18, 2, false),
        (safe_private_nested_owner_v18, 2, true),
        (safe_private_move_owner_v18, 1, false),
    ] {
        let mut safe_counts = [0; 3];
        let ((result, _, _, floor, counts, complete), observed) =
            scoped_raw_admission_v29::with_safe_object_activation_test_v29(0, || {
                private_memory_run_v18(
                    factory,
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                    MODULE_LIMIT,
                    |memory, budget| {
                        let counts = memory.test_safe_counts_v18(budget)?;
                        for (all, count) in safe_counts.iter_mut().zip(counts) {
                            *all += count;
                        }
                        Ok(())
                    },
                )
            });
        result.unwrap();
        assert!(complete);
        assert_eq!(floor, MODULE_FLOOR);
        assert_eq!((counts[0], counts[1]), (allocations, allocations));
        assert!(
            counts[2] >= allocations * if cross { 3 } else { 2 },
            "{counts:?}"
        );
        assert!(observed[0] > 0 && observed[2] > 0, "{observed:?}");
        assert_eq!(observed[1] > 0, cross, "{observed:?}");
        assert!(
            safe_counts[0] >= allocations && safe_counts[2] >= allocations,
            "{safe_counts:?}"
        );
        assert_eq!(safe_counts[1] > 0, cross, "{safe_counts:?}");
    }
}

#[test]
fn safe_private_source_original_borrow_role_origin_and_instance_queries_remain_exact() {
    let ((result, _, _, floor, counts, complete), observed) =
        scoped_raw_admission_v29::with_safe_object_activation_test_v29(1, || {
            private_memory_run_v18(
                safe_private_nested_owner_v18,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                |_, _| Ok(()),
            )
        });
    result.unwrap();
    assert!(complete && counts[2] >= 6);
    assert_eq!(floor, MODULE_FLOOR);
    assert!(observed[0] > 0 && observed[1] > 0 && observed[2] > 0 && observed[3] >= 9);
}

#[test]
fn safe_private_source_exact_and_one_short_transactions_preserve_required_coverage() {
    let run = |work, storage| {
        private_memory_run_v18(safe_private_nested_owner_v18, work, storage, |_, _| Ok(()))
    };
    let (result, work, storage, floor, counts, complete) =
        run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(complete && counts[2] >= 6);
    assert_eq!(floor, MODULE_FLOOR);
    let (result, used, peak, floor, actual, complete) = run(work, storage);
    result.unwrap();
    assert!(complete);
    assert_eq!(
        (used, peak, floor, actual),
        (work, storage, MODULE_FLOOR, counts)
    );
    for (wl, sl, expected_work) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let (result, _, _, floor, _, complete) = run(wl, sl);
        assert!(!complete);
        assert_eq!(floor, MODULE_FLOOR);
        let error = match result {
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(error),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    ProductionSourceOwnedViewErrorV18::Resource(error),
                ),
            )) => error,
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                    ),
                ),
            )) => error,
            other => panic!("exact resource cause required: {other:?}"),
        };
        match (expected_work, error) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), wl);
                assert!(error.actual() > wl);
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), sl);
                assert!(error.actual() > sl);
            }
            _ => panic!("wrong resource boundary: {error:?}"),
        }
    }
}

#[test]
fn safe_private_source_alias_equivalence_cannot_replace_the_exact_actual_pointer_use() {
    let positive = private_memory_run_v18(
        safe_private_nested_owner_v18,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |_, _| Ok(()),
    );
    positive.0.unwrap();
    assert!(positive.5 && positive.4[2] >= 6);
    for other_allocation in [false, true] {
        let mut entered = false;
        let (result, _, _, floor, _, complete) = private_memory_run_v18(
            safe_private_nested_owner_v18,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |memory, budget| {
                entered = true;
                memory.test_safe_pointer_substitution_v18(other_allocation, budget)
            },
        );
        assert!(entered && !complete);
        assert_eq!(floor, MODULE_FLOOR);
        assert!(
            matches!(
                result,
                Err(ProductionSourceOptimizationErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "retained footprint changed source occurrence or lacks activation evidence"
                    )
                ))
            ),
            "{result:?}"
        );
    }
}

#[test]
fn safe_private_source_activation_foreign_ledger_and_first_header_short_are_sticky() {
    for mode in [2, 3] {
        let ((result, _, _, floor, _, complete), observed) =
            scoped_raw_admission_v29::with_safe_object_activation_test_v29(mode, || {
                private_memory_run_v18(
                    safe_private_root_owner_v18,
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                    MODULE_LIMIT,
                    |_, _| panic!("failed source activation reached completed memory"),
                )
            });
        assert!(!complete && observed[0] > 0);
        assert_eq!(floor, MODULE_FLOOR);
        let resource = match result {
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                    ),
                ),
            )) => error,
            other => panic!("exact original resource refusal required: {other:?}"),
        };
        match (mode, resource) {
            (2, ArgumentResourceV1::Accounting) => {}
            (3, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), MODULE_LIMIT);
                assert_eq!(error.actual(), MODULE_LIMIT + 1);
            }
            _ => panic!("wrong resource cause: {resource:?}"),
        }
    }
}
