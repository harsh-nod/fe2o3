use fe2o3_kernel_ir::{ExecutionOperationV15, ExecutionTileLayoutV1};

fn prepared_tile_schedule_result_v155(
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ProductionPreparedSourceV18> {
    let factory = || {
        kernel_argument_abi_v18::tests::fixture_descriptor_ownership_v18(
            super::super::fixtures::tile_parts_repeated_owner(),
        )
    };
    let projection = factory();
    let owner = factory();
    let semantic = projection.source_semantic();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        owner.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "lifecycle_fixture",
            [88; 32],
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
        )],
    )
    .unwrap();
    let roots = [root_input(&projection)];
    let helper = &semantic.functions()[1];
    let SemanticTerminatorKindV1::Call(derive) = helper.blocks()[0].terminator().kind() else {
        panic!("fixture workgroup derivation")
    };
    let mut classes =
        vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    classes[1] = ProductionScopeCallableCandidateV29::Provider {
        function: HELPER,
        identity: helper.identity(),
    };
    classes[derive.callee().index() as usize] = ProductionScopeCallableCandidateV29::Derive {
        binding: SemanticFunctionIdentityV1::from_sha256([121; 32]),
        operation: SemanticCompilerIntrinsicIdentityV1::from_sha256([121; 32]),
        context: CONTEXT,
        workgroup: semantic.functions()[2].abi().source_input_types()[0],
    };
    let mut events = vec![crate::ProductionScopeEventCandidateV29 {
        function: ROOT,
        block: SemanticBlockIdV1::from_index(1),
        statement_count: 0,
        kind: ProductionScopeEventKindV29::Call {
            callee: SemanticCallableIdV1::from_index(1),
            kind: ProductionScopeCallKindV29::Provider,
        },
    }];
    for (ordinal, block) in helper.blocks().iter().enumerate() {
        let kind = match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => ProductionScopeEventKindV29::Call {
                callee: call.callee(),
                kind: if call.callee() == derive.callee() {
                    ProductionScopeCallKindV29::Derive
                } else {
                    ProductionScopeCallKindV29::Ordinary
                },
            },
            SemanticTerminatorKindV1::Return => ProductionScopeEventKindV29::Return,
            _ => panic!("fixture provider control"),
        };
        events.push(crate::ProductionScopeEventCandidateV29 {
            function: HELPER,
            block: SemanticBlockIdV1::from_index(ordinal as u32),
            statement_count: block.statements().len(),
            kind,
        });
    }
    let profile = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&projection);
    let profile_roots = profile.roots();
    ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
        owner,
        launch,
        ProductionExecutionSourceInputV29 {
            semantic_sha256: projection.source_semantic_sha256(),
            roots: &roots,
            classes: &classes,
            events: &events,
        },
        ProductionKernelArgumentAbiInputV18 {
            roots: &profile_roots,
        },
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
}

fn prepared_tile_schedule_v155(budget: &mut ArgumentBudgetV1<'_>) -> ProductionPreparedSourceV18 {
    prepared_tile_schedule_result_v155(budget).unwrap()
}

fn tile_schedule_header_v155() -> usize {
    std::mem::size_of::<ProductionOptimizedTileLoadScheduleV155<'_, '_>>()
        + std::mem::size_of::<SourceOwnedResultV18<ProductionOptimizedTileLoadScheduleV155<'_, '_>>>(
        )
}

#[test]
fn explicit_source_tile_schedule_retains_each_actual_load_owner_and_layout() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_tile_schedule_v155(&mut budget);
    with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let input = view.input_inventory(budget)?;
        let owner = view.output_inventory(budget)?.owner();
        let mut count = 0;
        for row in input.operations() {
            if !matches!(
                row.operation.kind,
                OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 { .. })
            ) {
                continue;
            }
            count += 1;
            for layout in [
                ExecutionTileLayoutV1::Blocked,
                ExecutionTileLayoutV1::Striped,
            ] {
                let floor = budget.storage();
                budget.reserve_storage(tile_schedule_header_v155())?;
                let schedule = view.tile_load_schedule_v155(row.coordinate, layout, budget)?;
                schedule.check_output_owner(owner, budget)?;
                let (original, output) = schedule.effect_sites(budget)?;
                assert_eq!(original, row.coordinate);
                assert_eq!(
                    view.operation(original, budget)?,
                    ProductionOptimizedSourceOperationV18::Retained {
                        input: original,
                        output
                    }
                );
                let actual = view
                    .output_inventory(budget)?
                    .operations()
                    .iter()
                    .find(|row| row.coordinate == output)
                    .unwrap()
                    .operation;
                let OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 {
                    workgroup,
                    input,
                    base,
                    lanes,
                    elements,
                }) = actual.kind
                else {
                    panic!("actual load")
                };
                assert_eq!(schedule.operands(budget)?, (workgroup, input, base));
                let scalar = schedule.scalar_semantics(budget)?;
                assert_eq!(
                    (scalar.layout(), scalar.lanes(), scalar.elements()),
                    (layout, lanes, elements)
                );
                assert_eq!(schedule.address(0, 0, u64::MAX, u64::MAX, budget)?, None);
                assert_eq!(schedule.address(0, 0, 7, 8, budget)?, Some(7));
                drop(schedule);
                budget.release_storage(tile_schedule_header_v155())?;
                assert_eq!(budget.storage(), floor);
            }
        }
        assert!(
            count >= 2,
            "repeated source calls must retain distinct effect sites"
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn explicit_source_tile_schedule_rejects_non_tile_original_site() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_optimized_module_v18(ModuleFixture::Mixed, &mut budget);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let coordinate = view.input_inventory(budget)?.operations()[0].coordinate;
        budget.reserve_storage(tile_schedule_header_v155())?;
        let result =
            view.tile_load_schedule_v155(coordinate, ExecutionTileLayoutV1::Blocked, budget);
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "tile schedule original operation is not a tile load"
            ))
        ));
        drop(result);
        budget.release_storage(tile_schedule_header_v155())?;
        Ok(())
    });
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "tile schedule original operation is not a tile load"
            ))
        ),
        "first denial must survive a swallowed query error"
    );
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn explicit_source_tile_schedule_rejects_byte_identical_foreign_output_owner() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_tile_schedule_v155(&mut budget);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let coordinate = view
            .input_inventory(budget)?
            .operations()
            .iter()
            .find(|row| {
                matches!(
                    row.operation.kind,
                    OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 { .. })
                )
            })
            .unwrap()
            .coordinate;
        let owner = view.output_inventory(budget)?.owner();
        let (foreign, storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            owner.module(), ProductionSemanticKirLimitsV1::default().storage_layout_limits, budget,
        ).unwrap();
        budget.reserve_storage(storage.retained_storage())?;
        assert_eq!(owner.canonical_bytes(), foreign.canonical_bytes());
        budget.reserve_storage(tile_schedule_header_v155())?;
        let schedule =
            view.tile_load_schedule_v155(coordinate, ExecutionTileLayoutV1::Striped, budget)?;
        let denial = schedule.check_output_owner(&foreign, budget);
        assert!(matches!(
            denial,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "tile schedule substituted its checked output owner"
            ))
        ));
        assert!(matches!(
            schedule.scalar_semantics(budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "tile schedule substituted its checked output owner"
            ))
        ));
        drop((schedule, foreign));
        budget.release_storage(tile_schedule_header_v155() + storage.retained_storage())?;
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "tile schedule substituted its checked output owner"
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn explicit_source_tile_schedule_rejects_invalid_component_and_retains_first_denial() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_tile_schedule_v155(&mut budget);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let coordinate = view
            .input_inventory(budget)?
            .operations()
            .iter()
            .find(|row| {
                matches!(
                    row.operation.kind,
                    OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 { .. })
                )
            })
            .unwrap()
            .coordinate;
        budget.reserve_storage(tile_schedule_header_v155())?;
        let schedule =
            view.tile_load_schedule_v155(coordinate, ExecutionTileLayoutV1::Blocked, budget)?;
        assert!(matches!(
            schedule.address(u16::MAX, 0, 0, 0, budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "tile schedule component coordinate"
            ))
        ));
        assert!(matches!(
            schedule.effect_sites(budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "tile schedule component coordinate"
            ))
        ));
        drop(schedule);
        budget.release_storage(tile_schedule_header_v155())?;
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "tile schedule component coordinate"
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn explicit_source_tile_schedule_rejects_live_header_refund_and_foreign_ledger() {
    for foreign in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_tile_schedule_v155(&mut budget);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let coordinate = view
                .input_inventory(budget)?
                .operations()
                .iter()
                .find(|row| {
                    matches!(
                        row.operation.kind,
                        OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 { .. })
                    )
                })
                .unwrap()
                .coordinate;
            budget.reserve_storage(tile_schedule_header_v155())?;
            let schedule =
                view.tile_load_schedule_v155(coordinate, ExecutionTileLayoutV1::Blocked, budget)?;
            let denial = if foreign {
                let mut other_work =
                    CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                let mut other_budget = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
                other_budget.reserve_storage(budget.storage()).unwrap();
                let result = schedule.scalar_semantics(&mut other_budget);
                assert_eq!(
                    other_budget.work(),
                    0,
                    "foreign account must fail before query debits"
                );
                result
            } else {
                budget.release_storage(1)?;
                schedule.scalar_semantics(budget)
            };
            assert!(matches!(
                denial,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            drop(schedule);
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting,
            ))
        });
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert!(
            budget.storage() > MODULE_FLOOR,
            "custody loss must not manufacture a refund"
        );
    }
}

fn tile_schedule_resource_run_v155(
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ProductionSourceOptimizationErrorV18<ProductionSourceOwnedViewErrorV18>>,
    usize,
    usize,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let result = (|| {
        let prepared = prepared_tile_schedule_result_v155(&mut budget)
            .map_err(ProductionSourceOptimizationErrorV18::Source)?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let (output, (), _) =
                source.with_checked_optimization_v18(budget, |_, view, budget| {
                    let coordinate = view
                        .input_inventory(budget)?
                        .operations()
                        .iter()
                        .find(|row| {
                            matches!(
                                row.operation.kind,
                                OperationKind::Execution(
                                    ExecutionOperationV15::MaskedTileLoadU32 { .. }
                                )
                            )
                        })
                        .unwrap()
                        .coordinate;
                    budget.reserve_storage(tile_schedule_header_v155())?;
                    let queried = (|| {
                        let schedule = view.tile_load_schedule_v155(
                            coordinate,
                            ExecutionTileLayoutV1::Striped,
                            budget,
                        )?;
                        assert_eq!(schedule.address(0, 0, 7, 8, budget)?, Some(7));
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                    })();
                    budget.release_storage(tile_schedule_header_v155())?;
                    queried?;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                })?;
            drop(output);
            Ok(())
        })
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn explicit_source_tile_schedule_transaction_has_exact_and_one_short_resource_boundaries() {
    let (result, work, storage) =
        tile_schedule_resource_run_v155(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    let (exact, used, peak) = tile_schedule_resource_run_v155(work, storage);
    exact.unwrap();
    assert_eq!((used, peak), (work, storage));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, _, _) = tile_schedule_resource_run_v155(work_limit, storage_limit);
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
            ))
            | Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                    ),
                ),
            )) => error,
            other => panic!("expected exact resource refusal: {other:?}"),
        };
        assert!(if is_work {
            matches!(error, ArgumentResourceV1::Work(_))
        } else {
            matches!(error, ArgumentResourceV1::Storage(_))
        });
    }
}
