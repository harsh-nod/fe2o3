fn release_execution_unit_result_v18(
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    budget.release_storage(std::mem::size_of::<
        Result<(), ProductionSourceOwnedViewErrorV18>,
    >())?;
    Ok(())
}

fn execution_branch_owner_v18(constant: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = module_fixture_owner(ModuleFixture::Mixed);
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let provider = &functions[2];
    let mut blocks = provider.blocks().to_vec();
    assert_eq!(blocks.len(), 3);
    blocks[2] = block(
        92,
        vec![],
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(place(5, U32)),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::SwitchValue,
                        SemanticBlockIdV1::from_index(3),
                    ),
                )],
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchOtherwise,
                    SemanticBlockIdV1::from_index(4),
                ),
            )
            .unwrap(),
        },
    );
    blocks.push(block(93, vec![], SemanticTerminatorKindV1::Return));
    blocks.push(block(94, vec![], SemanticTerminatorKindV1::Return));
    functions[2] = function(
        100,
        provider.role(),
        provider.abi().clone(),
        provider.locals().to_vec(),
        blocks,
    );
    if constant {
        let callback = &functions[3];
        functions[3] = function(
            110,
            callback.role(),
            callback.abi().clone(),
            callback.locals().to_vec(),
            vec![block(
                115,
                vec![assign(place(0, U32), SemanticRvalueKindV1::Use(literal(0)))],
                SemanticTerminatorKindV1::Return,
            )],
        );
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
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
fn lifecycle_recipes_keep_alternative_ends_and_explicit_checked_unreachable_disposition() {
    for constant in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let projection = execution_branch_owner_v18(constant);
        let owner = execution_branch_owner_v18(constant);
        let (_, launch) =
            with_module_fixture_view(&owner, ModuleFixture::Mixed, &mut budget, |_, _| ()).unwrap();
        let prepared = with_module_fixture_view(
            &projection,
            ModuleFixture::Mixed,
            &mut budget,
            |source, budget| {
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                    owner,
                    launch,
                    source.input,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
                .unwrap()
            },
        )
        .unwrap()
        .0;
        with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            view.with_execution_recipes_v18(budget, |recipes, budget| {
                let mut ends = 0;
                let mut removed = 0;
                let mut relocated = 0;
                for ordinal in 0..recipes.len(budget)? {
                    if recipes.original_site(ordinal, budget)?.3
                        == ProductionOptimizedExecutionKindV18::ScopeEnd
                    {
                        ends += 1;
                    }
                    match recipes.operation(ordinal, budget)? {
                        ProductionOptimizedSourceOperationV18::RemovedUnreachable { input } => {
                            removed += 1;
                            assert!(matches!(
                                view.operation(input, budget)?,
                                ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. }
                            ));
                        }
                        ProductionOptimizedSourceOperationV18::Retained { input, output } => {
                            relocated += usize::from(input != output);
                        }
                        ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                            panic!("ordered execution cannot become a pure rewrite")
                        }
                    }
                }
                assert_eq!(ends, 2, "all original normal ends remain explicit recipes");
                assert_eq!(
                    removed,
                    usize::from(constant),
                    "only checked unreachable control can remove one end"
                );
                assert!(
                    relocated > 0,
                    "actual block merging must transport lifecycle coordinates"
                );
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })?;
            release_execution_unit_result_v18(budget)
        })
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn actual_lifecycle_recipes_cover_original_insertions_and_checked_output() {
    for kind in [ModuleFixture::Mixed, ModuleFixture::Array] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_optimized_module_v18(kind, &mut budget);
        with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let floor = budget.storage();
            let input_count = view
                .input_inventory(budget)?
                .operations()
                .iter()
                .filter(|row| matches!(row.operation.kind, OperationKind::Execution(_)))
                .count();
            let output_count = view
                .output_inventory(budget)?
                .operations()
                .iter()
                .filter(|row| matches!(row.operation.kind, OperationKind::Execution(_)))
                .count();
            assert!(input_count > 0);
            for _ in 0..2 {
                view.with_execution_recipes_v18(budget, |recipes, budget| {
                    assert_eq!(recipes.len(budget)?, input_count);
                    let mut retained = 0;
                    let mut roles = [0usize; 3];
                    for ordinal in 0..recipes.len(budget)? {
                        let (_, _, _, kind) = recipes.original_site(ordinal, budget)?;
                        roles[match kind {
                            ProductionOptimizedExecutionKindV18::ContextIssue => 0,
                            ProductionOptimizedExecutionKindV18::WorkgroupDerive => 1,
                            ProductionOptimizedExecutionKindV18::ScopeEnd => 2,
                        }] += 1;
                        if let ProductionOptimizedSourceOperationV18::Retained { .. } =
                            recipes.operation(ordinal, budget)?
                        {
                            retained += 1;
                        }
                    }
                    assert!(roles.iter().all(|count| *count > 0));
                    assert_eq!(retained, output_count);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                })?;
                assert_eq!(
                    budget.storage(),
                    floor + std::mem::size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
                );
                release_execution_unit_result_v18(budget)?;
                assert_eq!(
                    budget.storage(),
                    floor,
                    "same-owner repeated scopes settle exactly"
                );
            }
            view.test_execution_recipe_controls_v18(budget)?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn lifecycle_recipe_first_refusal_survives_callback_error_or_unwind() {
    for unwind in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_optimized_module_v18(ModuleFixture::Mixed, &mut budget);
        let entered = std::cell::Cell::new(false);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let floor = budget.storage();
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                view.with_execution_recipes_v18(budget, |recipes, budget| {
                    assert!(recipes.len(budget)? > 0);
                    assert!(matches!(
                        recipes.operation(usize::MAX, budget),
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "execution recipe ordinal"
                        ))
                    ));
                    entered.set(true);
                    if unwind {
                        std::panic::panic_any("execution recipe test unwind");
                    }
                    Err::<(), _>(ProductionSourceOwnedViewErrorV18::Binding(
                        "later recipe callback error",
                    ))
                })
            }));
            assert_eq!(
                budget.storage(),
                floor + std::mem::size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
            );
            // The error/panic carries no allocation. Settle the recipe's
            // transferred header before continuing the same disposition.
            match caught {
                Ok(result) => {
                    let error = result.unwrap_err();
                    release_execution_unit_result_v18(budget)?;
                    assert_eq!(budget.storage(), floor);
                    Err(error)
                }
                Err(payload) => {
                    release_execution_unit_result_v18(budget)?;
                    assert_eq!(budget.storage(), floor);
                    std::panic::resume_unwind(payload)
                }
            }
        });
        assert!(entered.get());
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "execution recipe ordinal"
            ))
        ));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn lifecycle_recipe_scope_rejects_foreign_ledger_and_reinflated_floor() {
    for foreign in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_optimized_module_v18(ModuleFixture::Mixed, &mut budget);
        let entered = std::cell::Cell::new(false);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            view.with_execution_recipes_v18(budget, |recipes, budget| {
                assert!(recipes.len(budget)? > 0);
                let refused = if foreign {
                    let mut work =
                        CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                    let mut other = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
                    other.reserve_storage(MODULE_LIMIT / 2)?;
                    recipes.len(&mut other)
                } else {
                    budget.release_storage(1)?;
                    let refused = recipes.len(budget);
                    budget.reserve_storage(1)?;
                    refused
                };
                assert!(matches!(
                    refused,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                entered.set(true);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })
        });
        assert!(entered.get());
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert!(
            budget.storage() > MODULE_FLOOR,
            "custody denial cannot be refunded"
        );
    }
}

#[test]
fn lifecycle_recipe_selected_consumer_error_precedes_later_custody_observation() {
    for undercut in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_optimized_module_v18(ModuleFixture::Mixed, &mut budget);
        let entered = std::cell::Cell::new(false);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let floor = budget.storage();
            let selected = view.with_execution_recipes_v18(budget, |recipes, budget| {
                assert!(recipes.len(budget)? > 0);
                if undercut {
                    budget.release_storage(1)?;
                }
                Err::<(), _>(ProductionSourceOwnedViewErrorV18::Binding(
                    "selected recipe consumer error",
                ))
            });
            assert!(matches!(
                selected,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "selected recipe consumer error"
                ))
            ));
            let error = selected.unwrap_err();
            if !undercut {
                assert_eq!(
                    budget.storage(),
                    floor + std::mem::size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
                );
                release_execution_unit_result_v18(budget)?;
                assert_eq!(budget.storage(), floor);
            }
            entered.set(true);
            Err(error)
        });
        assert!(entered.get());
        assert!(result.is_err());
        if undercut {
            assert!(budget.storage() > MODULE_FLOOR);
        } else {
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

#[test]
fn lifecycle_recipe_return_header_is_prepaid_before_any_consumer_or_census() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_optimized_module_v18(ModuleFixture::Mixed, &mut budget);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let header = std::mem::size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>();
        let floor = budget.storage();
        let padding = MODULE_LIMIT - floor - (header - 1);
        budget.reserve_storage(padding)?;
        let before = budget.work();
        let result = view.with_execution_recipes_v18(budget, |_, _| -> SourceOwnedResultV18<()> {
            panic!("one-short returned header cannot enter consumer")
        });
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Storage(_)
            ))
        ));
        assert_eq!(budget.failed_storage(), Some(MODULE_LIMIT + 1));
        assert_eq!(
            budget.work(),
            before + 1,
            "only the original query guard precedes the returned header"
        );
        assert_eq!(budget.storage(), MODULE_LIMIT - header + 1);
        // Only this test's synthetic padding was admitted; the returned-header
        // debit failed before the constructor or consumer could own anything.
        budget.release_storage(padding)?;
        assert_eq!(budget.storage(), floor);
        result
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn lifecycle_recipe_owned_callback_result_keeps_its_header_and_backing_live() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_optimized_module_v18(ModuleFixture::Mixed, &mut budget);
    with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let floor = budget.storage();
        let returned = std::mem::size_of::<Vec<u64>>()
            + std::mem::size_of::<Result<Vec<u64>, ProductionSourceOwnedViewErrorV18>>();
        let capture = [17u8; 4096];
        let payload = view.with_execution_recipes_v18(budget, move |recipes, budget| {
            let ordinal = recipes.len(budget)?;
            assert!(ordinal > 0 && ordinal < capture.len());
            assert!(budget.storage() >= floor + returned + std::mem::size_of_val(&capture));
            budget.reserve_storage(8 * std::mem::size_of::<u64>())?;
            let mut payload = Vec::new();
            payload
                .try_reserve_exact(8)
                .map_err(|_| ArgumentResourceV1::Allocation)?;
            budget.reserve_storage((payload.capacity() - 8) * std::mem::size_of::<u64>())?;
            for _ in 0..8 {
                payload.push(u64::from(capture[ordinal]));
            }
            Ok::<_, ProductionSourceOwnedViewErrorV18>(payload)
        })?;
        let backing = payload.capacity() * std::mem::size_of::<u64>();
        assert_eq!(payload.as_slice(), &[17u64; 8]);
        assert_eq!(budget.storage(), floor + returned + backing);
        drop(payload);
        budget.release_storage(returned + backing)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

enum ExecutionRecipeOwnedErrorV18 {
    Source(ProductionSourceOwnedViewErrorV18),
    Payload(Vec<u64>),
}

impl From<ProductionSourceOwnedViewErrorV18> for ExecutionRecipeOwnedErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}

#[test]
fn lifecycle_recipe_owned_callback_error_is_not_refunded_as_constructor_scratch() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_optimized_module_v18(ModuleFixture::Mixed, &mut budget);
    with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let floor = budget.storage();
        let returned = std::mem::size_of::<Result<(), ExecutionRecipeOwnedErrorV18>>();
        let result = view.with_execution_recipes_v18(budget, |recipes, budget| {
            assert!(recipes.len(budget)? > 0);
            let payload = (|| -> SourceOwnedResultV18<Vec<u64>> {
                budget.reserve_storage(8 * std::mem::size_of::<u64>())?;
                let mut payload = Vec::new();
                payload
                    .try_reserve_exact(8)
                    .map_err(|_| ArgumentResourceV1::Allocation)?;
                budget.reserve_storage((payload.capacity() - 8) * std::mem::size_of::<u64>())?;
                payload.extend([19u64; 8]);
                Ok(payload)
            })()?;
            Err::<(), _>(ExecutionRecipeOwnedErrorV18::Payload(payload))
        });
        let payload = match result {
            Err(ExecutionRecipeOwnedErrorV18::Payload(payload)) => payload,
            Err(ExecutionRecipeOwnedErrorV18::Source(error)) => {
                panic!("unexpected recipe refusal: {error:?}")
            }
            Ok(()) => panic!("selected owned error disappeared"),
        };
        let backing = payload.capacity() * std::mem::size_of::<u64>();
        assert_eq!(payload.as_slice(), &[19u64; 8]);
        assert_eq!(budget.storage(), floor + returned + backing);
        drop(payload);
        budget.release_storage(returned + backing)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
