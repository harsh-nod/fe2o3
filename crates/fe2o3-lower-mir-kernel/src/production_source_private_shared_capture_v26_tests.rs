use super::*;

fn shared_dereference_v26(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap()
}

fn shared_capture_owner_v26<const MODE: u8>() -> ProductionSemanticSsaOwnerV1 {
    let base = mixed_owner(true);
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let leaf = &functions[3];
    let shared = leaf.locals()[2].ty();
    let mut tail = leaf.blocks()[1].statements().to_vec();
    if MODE != 0 {
        tail[1] = assign(
            place(3, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(shared_dereference_v26(4))),
        );
    }
    if MODE == 2 {
        tail[0] = assign(
            place(4, shared),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2, shared))),
        );
    }
    if MODE == 3 {
        tail[0] = assign(
            place(4, shared),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: shared_dereference_v26(2),
            },
        );
    }
    if MODE == 4 {
        // A source mutation during the live shared borrow must not be hidden
        // by carrying the earlier scalar snapshot to the later dereference.
        tail.insert(
            1,
            assign(place(1, U32), SemanticRvalueKindV1::Use(literal(29))),
        );
    }
    functions[3] = function(
        150,
        leaf.role(),
        leaf.abi().clone(),
        leaf.locals().to_vec(),
        vec![
            leaf.blocks()[0].clone(),
            block(171, tail, SemanticTerminatorKindV1::Return),
        ],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
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

fn shared_capture_caller_owner_v26<const MUTATE: bool>() -> ProductionSemanticSsaOwnerV1 {
    let base = private_entry_owner_v20(PrivateEntryFixtureV20::Neutral);
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let shared = reference(&mut types, U32, SemanticMutabilityV1::Immutable, false);
    let raw = semantic.functions()[3].locals()[2].ty();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    let mut borrow = vec![assign(
        place(2, shared),
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(1, U32),
        },
    )];
    if MUTATE {
        borrow.push(assign(
            place(1, U32),
            SemanticRvalueKindV1::Use(literal(31)),
        ));
    }
    functions[2] = function(
        120,
        helper.role(),
        helper.abi().clone(),
        vec![
            local(130, UNIT, SemanticLocalRoleV1::Return),
            local(131, U32, SemanticLocalRoleV1::Argument(0)),
            local(132, shared, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                140,
                borrow,
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(3),
                        vec![SemanticOperandV1::Copy(place(2, shared))],
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
                ),
            ),
            block(141, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    functions[3] = function(
        150,
        SemanticFunctionRoleV1::InternalHelper,
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([151; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                shared,
                SemanticAbiPassModeV1::Direct(
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(
                            true,
                            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                            true,
                            true,
                            false,
                            true,
                        ),
                        SemanticAbiExtensionV1::None,
                        4,
                        Some(4),
                    )
                    .unwrap(),
                ),
            ))],
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
        .unwrap(),
        vec![
            local(160, UNIT, SemanticLocalRoleV1::Return),
            local(161, shared, SemanticLocalRoleV1::Argument(0)),
            local(162, U32, SemanticLocalRoleV1::Temporary),
            local(163, raw, SemanticLocalRoleV1::Temporary),
            local(164, U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![block(
            170,
            vec![
                assign(
                    place(2, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(shared_dereference_v26(1))),
                ),
                assign(
                    place(3, raw),
                    SemanticRvalueKindV1::AddressOf {
                        place: place(2, U32),
                        mutability: SemanticMutabilityV1::Immutable,
                    },
                ),
                assign(
                    place(4, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(shared_dereference_v26(3))),
                ),
                assign(
                    place(2, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: SemanticOperandV1::Copy(place(4, U32)),
                        right: literal(0),
                    },
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
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
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
fn private_shared_capture_fixture_retains_exact_frozen_reference_abi() {
    for factory in [
        shared_capture_caller_owner_v26::<false> as fn() -> _,
        shared_capture_caller_owner_v26::<true>,
    ] {
        let owner = factory();
        let abi = owner.source_semantic().functions()[3].abi();
        assert_eq!(
            abi.source_argument_ownership(),
            &[SemanticSourceArgumentOwnershipV1::SharedBorrow]
        );
        let SemanticAbiPassModeV1::Direct(attributes) = abi.arguments()[0].mode() else {
            panic!("original shared reference must have a direct ABI");
        };
        let regular = attributes.regular();
        assert!(
            regular.no_alias() && regular.non_null() && regular.read_only() && regular.no_undef()
        );
        assert!(!regular.in_register());
        assert_eq!(
            regular.pointer_capture(),
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly)
        );
    }
}

fn shared_capture_complete_v26(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ProductionPrivateSourceHandoffErrorV20>,
    usize,
    usize,
    bool,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let completed = std::cell::Cell::new(false);
    let result = (|| -> Result<(), ProductionPrivateSourceHandoffErrorV20> {
        let projection = factory();
        let owner = factory();
        let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
        let roots = fixture.roots();
        let (_, launch) =
            with_module_fixture_view(&owner, ModuleFixture::Ordinary, &mut budget, |_, _| ())
                .map_err(ProductionSourceOwnedViewErrorV18::from)?;
        let prepared = with_module_fixture_view(
            &projection,
            ModuleFixture::Ordinary,
            &mut budget,
            |source, budget| {
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner,
                    launch,
                    source.input,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
            },
        )
        .map_err(ProductionSourceOwnedViewErrorV18::from)?
        .0?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let handoff = source.private_completed_integer_output_v20(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            let checked = (|| -> Result<(), ProductionPrivateSourceHandoffErrorV20> {
                handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                assert!(
                    census(handoff.output(budget)?.owner())
                        .into_iter()
                        .all(|count| count > 0)
                );
                assert!(!handoff.output(budget)?.grants_authority());
                Ok(())
            })();
            let settled = handoff.discard(budget).map_err(Into::into);
            checked.and(settled)?;
            assert_eq!(budget.storage(), floor);
            completed.set(true);
            Ok(())
        })
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (
        result,
        budget.work(),
        budget.peak_storage(),
        completed.get(),
    )
}

#[test]
fn private_shared_capture_follows_original_copy_move_and_reborrow_across_edges() {
    for factory in [
        shared_capture_owner_v26::<0> as fn() -> _,
        shared_capture_owner_v26::<1>,
        shared_capture_owner_v26::<2>,
        shared_capture_owner_v26::<3>,
    ] {
        let (result, _, _, completed) =
            shared_capture_complete_v26(factory, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(result.is_ok(), "{result:?}");
        assert!(completed);
    }
}

#[test]
fn private_shared_capture_never_hides_an_intervening_source_write() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let entered = std::cell::Cell::new(false);
    let replayed = private_memory_prepared_v18(shared_capture_owner_v26::<4>, &mut budget)
        .and_then(|prepared| {
            prepared.with_source_consumer_v18(&mut budget, |_, _| {
                entered.set(true);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })
        });
    assert!(
        replayed.is_err() && !entered.get(),
        "a live shared borrow survived original source replay: {replayed:?}"
    );
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn private_shared_capture_rejoins_original_helper_arguments_and_rejects_stale_caller_borrows() {
    let (result, _, _, completed) = shared_capture_complete_v26(
        shared_capture_caller_owner_v26::<false>,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(completed);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let entered = std::cell::Cell::new(false);
    let stale = private_memory_prepared_v18(shared_capture_caller_owner_v26::<true>, &mut budget)
        .and_then(|prepared| {
            prepared.with_source_consumer_v18(&mut budget, |_, _| {
                entered.set(true);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })
        });
    assert!(
        stale.is_err() && !entered.get(),
        "stale caller borrow reached a helper argument: {stale:?}"
    );
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn private_shared_caller_keeps_ordinary_activations_with_source_raw_helper() {
    run_production_optimized_consumer_v18(
        shared_capture_caller_owner_v26::<false>,
        |original, optimized, budget| {
            let semantic = original.source.source_ssa(budget)?.source_semantic();
            assert!(semantic.types().iter().any(|ty| matches!(
                ty.shape(), SemanticTypeShapeV1::Pointer(pointer)
                    if pointer.kind() == SemanticPointerKindV1::Raw
            )));
            assert!(semantic.functions().iter().any(|function| function.blocks().iter().any(|block| {
                block.statements().iter().any(|statement| matches!(
                    statement.kind(), SemanticStatementKindV1::Assign(assignment)
                        if matches!(assignment.value().kind(), SemanticRvalueKindV1::AddressOf { .. })
                ))
            })));
            // The admitted raw source use can be captured as a scalar before
            // physical emission; it need not retain a raw formation alternative.
            let [ordinary, _raw] =
                scoped_raw_admission_v29::test_mixed_scalar_activation_census_v26(
                    original, 0, budget,
                )?;
            assert!(
                ordinary > 0,
                "ordinary scalar activations were suppressed by raw memory"
            );
            original.check_optimized_source_currentness_v18(optimized, budget)?;
            Ok(())
        },
    );
}

#[test]
fn private_shared_capture_uses_exact_and_one_short_cumulative_budgets() {
    for factory in [
        shared_capture_owner_v26::<1> as fn() -> _,
        shared_capture_caller_owner_v26::<false>,
    ] {
        let (full, work, peak, completed) =
            shared_capture_complete_v26(factory, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(full.is_ok(), "{full:?}");
        assert!(completed);
        let (exact, actual_work, actual_peak, completed) =
            shared_capture_complete_v26(factory, work, peak);
        assert!(exact.is_ok(), "{exact:?}");
        assert!(completed);
        assert_eq!((actual_work, actual_peak), (work, peak));
        for (work, storage) in [(work - 1, peak), (work, peak - 1)] {
            let (short, _, _, completed) = shared_capture_complete_v26(factory, work, storage);
            assert!(short.is_err());
            assert!(!completed);
        }
    }
}

#[test]
fn private_shared_capture_rejects_copied_places_foreign_instances_roles_and_scalar_types() {
    for fault in 0..7 {
        let visited = std::cell::Cell::new(false);
        let result = with_entry_fixture_v18(
            shared_capture_owner_v26::<0>,
            |original, optimized, budget| {
                source_scalar_normalization_scratch_v18(
                    original.source.cleanup,
                    budget,
                    private_source_completion_headers_v20()?,
                    |budget| {
                        let index = OriginalEntryIndexV20::build(original, budget)?;
                        original.with_optimized_scalar_leaf_namespace_v18(
                            optimized,
                            0,
                            &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                            budget,
                            |leaves, budget| {
                                let leaves = leaves.original_leaves(budget)?;
                                let function = leaves.original_function(2, budget)?;
                                let site = EntrySiteV20::Statement {
                                    block: fe2o3_mir_model::SsaBlockIdV1::new(1),
                                    statement: 1,
                                };
                                let role = EntryOperandV20::RvalueOperand(0);
                                let original_place =
                                    scoped_object_original_place_v29(function, site, role).unwrap();
                                assert_eq!(original_place.local().index(), 2);
                                assert_eq!(original_place.projections().len(), 1);
                                let copy = original_place.clone();
                                let mut remaining = if fault == 6 {
                                    0
                                } else {
                                    fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2
                                };
                                let checked = index.captured_shared_reference_expression_v26(
                                    leaves,
                                    if fault == 3 { 1 } else { 2 },
                                    U32,
                                    ProductionSemanticScalarTypeV2::Integer {
                                        bits: 32,
                                        signed: fault == 5,
                                    },
                                    if fault == 2 {
                                        EntrySiteV20::Statement {
                                            block: fe2o3_mir_model::SsaBlockIdV1::new(0),
                                            statement: 0,
                                        }
                                    } else {
                                        site
                                    },
                                    if fault == 4 {
                                        EntryOperandV20::CallArgument(0)
                                    } else {
                                        role
                                    },
                                    if fault == 1 { &copy } else { original_place },
                                    0,
                                    &mut remaining,
                                    budget,
                                );
                                visited.set(true);
                                if fault == 0 {
                                    let actual = checked?;
                                    let borrow = scoped_object_original_place_v29(
                                        function,
                                        EntrySiteV20::Statement {
                                            block: fe2o3_mir_model::SsaBlockIdV1::new(0),
                                            statement: 0,
                                        },
                                        EntryOperandV20::RvaluePlace,
                                    )
                                    .unwrap();
                                    let expected = leaves
                                        .original_place(2, function, borrow, budget)?
                                        .unwrap();
                                    assert_eq!(actual, expected);
                                    Ok(())
                                } else {
                                    assert!(matches!(
                                        checked,
                                        Err(ProductionSourceOwnedViewErrorV18::Binding(_))
                                    ));
                                    Err(checked.unwrap_err())
                                }
                            },
                        )
                    },
                )
            },
        );
        assert!(visited.get(), "fault={fault}: {result:?}");
        assert_eq!(result.is_ok(), fault == 0, "fault={fault}: {result:?}");
    }
}
