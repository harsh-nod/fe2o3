fn two_descriptor_reads_at_one_source_site_v18() -> ProductionSemanticSsaOwnerV1 {
    let base = descriptor_source_owner(DescriptorCase::READ);
    let semantic = base.source_semantic();
    let original = &semantic.functions()[0];
    let SemanticStatementKindV1::Assign(assignment) = original.blocks()[1].statements()[0].kind()
    else {
        panic!("descriptor fixture read assignment");
    };
    let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
        panic!("descriptor fixture ordinary source operand");
    };
    let effect = assign(
        assignment.destination().clone(),
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Add,
            left: operand.clone(),
            right: operand.clone(),
        },
    );
    let mut blocks = original.blocks().to_vec();
    blocks[1] = block(212, vec![effect], SemanticTerminatorKindV1::Return);
    let function = function(
        200,
        original.role(),
        original.abi().clone(),
        original.locals().to_vec(),
        blocks,
    )
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
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
fn actual_source_access_ordinals_survive_optimization_and_reject_all_site_substitutions() {
    for fault in 0..7 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(
            two_descriptor_reads_at_one_source_site_v18,
            &mut budget,
        );
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let attempted = source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                original.with_optimized_analysis_v18(optimized, budget, |analysis, budget| {
                    analysis.with_memory_versions(budget, |input_memory, output_memory, budget| {
                        scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                            original, optimized, 0, input_memory, output_memory, budget, |memory, budget| {
                                let floor = budget.storage();
                                scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
                                    let input_function = original.inventory.functions()[original.source.root(0, budget)?.1].coordinate;
                                    let output_function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
                                    let input_spaces = SourcePointerSpacesV18::derive(original,
                                        original.inventory, input_function, budget)?;
                                    let output_spaces = SourcePointerSpacesV18::derive_optimized(original, optimized,
                                        optimized.output_inventory(budget)?, output_function.coordinate, budget)?;
                                    let sites = optimized_source_access_sites_v18(original, optimized, 0,
                                        &input_spaces, &output_spaces, memory, budget)?;
                                    let mut first = None;
                                    let mut ordinals = Vec::new();
                                    for (effect, binding) in sites.census.effects.iter().zip(&sites.census.bindings) {
                                        if let OptimizedSourceAccessOriginV18::SourceSpan(site) = binding.origin {
                                            let location = SourceEffectLocationV18 { site, physical: effect.coordinate,
                                                access: effect.ordinal, ranked: (0, 0) };
                                            sites.check_location(original, 0, &location, budget)?;
                                            if site.block.index() == 1 && site.statement == Some(0) {
                                                ordinals.push(site.ordinal);
                                                first.get_or_insert(location);
                                            }
                                        }
                                    }
                                    ordinals.sort_unstable();
                                    assert_eq!(ordinals, [0, 1], "two original source operands remain distinct");
                                    reached.set(true);
                                    let mut wrong = first.unwrap();
                                    match fault {
                                        0 => return Ok(()),
                                        1 => wrong.site.ordinal += 1,
                                        2 => wrong.site.instance = usize::MAX,
                                        3 => wrong.site.function = SemanticFunctionIdV1::from_index(u32::MAX),
                                        4 => wrong.site.block = SemanticBlockIdV1::from_index(u32::MAX),
                                        5 => wrong.site.statement = None,
                                        6 => wrong.access = u32::MAX,
                                        _ => unreachable!(),
                                    }
                                    let error = sites.check_location(original, 0, &wrong, budget).unwrap_err();
                                    assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_)));
                                    Err(error)
                                })?;
                                budget.release_storage(budget.storage() - floor)?;
                                Ok::<(), ProductionSourceOwnedViewErrorV18>(())
                            })
                    })
                })?;
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            });
            if fault == 0 { attempted.expect("actual source ordinal/currentness composition"); }
            else { assert!(attempted.is_err()); }
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(
            reached.get(),
            "the source and actual optimizer must reach the ordinal consumer"
        );
        assert_eq!(result.is_ok(), fault == 0);
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn actual_optimized_effect_order_uses_the_complete_bound_census_and_real_cfg() {
    for fault in 0..5 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let attempted =
                source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                    let floor = budget.storage();
                    scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
                        let input_function = original.inventory.functions()
                            [original.source.root(0, budget)?.1]
                            .coordinate;
                        let output_function =
                            optimized_source_root_function_v18(original, optimized, 0, budget)?;
                        let input_spaces = SourcePointerSpacesV18::derive(
                            original,
                            original.inventory,
                            input_function,
                            budget,
                        )?;
                        let output_spaces = SourcePointerSpacesV18::derive_optimized(
                            original,
                            optimized,
                            optimized.output_inventory(budget)?,
                            output_function.coordinate,
                            budget,
                        )?;
                        let census = optimized_source_effect_census_v18(
                            original,
                            optimized,
                            0,
                            &input_spaces,
                            &output_spaces,
                            budget,
                        )?;
                        let mut rows = original_store_order_rows_v18(original, budget)?;
                        for row in &mut rows {
                            let ProductionOptimizedSourceOperationV18::Retained { output, .. } =
                                optimized.operation(row.physical, budget)?
                            else {
                                panic!("real ordered Store must have an exact retained occurrence");
                            };
                            row.physical = output;
                        }
                        let recipe =
                            effect_order_recipe_v18(output_function.function, rows.len(), false);
                        let before_order = budget.storage();
                        source_ranked_effect_order_endpoint_v18(
                            original,
                            SourceEffectEndpointV18::Optimized(&census),
                            0,
                            &recipe,
                            &rows,
                            budget,
                        )?;
                        assert_eq!(budget.storage(), before_order);
                        reached.set(true);
                        match fault {
                            0 => return Ok(()),
                            1 => {
                                let first = rows[0].ranked;
                                rows[0].ranked = rows[1].ranked;
                                rows[1].ranked = first;
                            }
                            2 => rows[0].site.instance = rows[2].site.instance,
                            3 => rows[0].access = u32::MAX,
                            4 => budget.release_storage(1)?,
                            _ => unreachable!(),
                        }
                        let before_negative = (budget.storage(), budget.work());
                        let error = source_ranked_effect_order_endpoint_v18(
                            original,
                            SourceEffectEndpointV18::Optimized(&census),
                            0,
                            &recipe,
                            &rows,
                            budget,
                        )
                        .unwrap_err();
                        if fault == 4 {
                            assert!(matches!(
                                error,
                                ProductionSourceOwnedViewErrorV18::Resource(
                                    ArgumentResourceV1::Accounting
                                )
                            ));
                            assert_eq!(
                                (budget.storage(), budget.work()),
                                before_negative,
                                "lost census custody precedes new work or credits"
                            );
                        } else {
                            assert!(matches!(
                                error,
                                ProductionSourceOwnedViewErrorV18::Binding(_)
                            ));
                            assert_eq!(
                                budget.storage(),
                                before_order,
                                "drop only the CFG scratch before refund"
                            );
                        }
                        let stopped = budget.work();
                        assert!(census.check(original, 0, budget).is_err());
                        assert_eq!(
                            budget.work(),
                            stopped,
                            "caught source refusal remains first"
                        );
                        Err(error)
                    })?;
                    budget.release_storage(budget.storage() - floor)?;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                });
            if fault == 0 {
                attempted.expect("actual optimized effect order");
            } else {
                assert!(attempted.is_err());
            }
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(
            reached.get(),
            "each negative first checks the exact real-source CFG"
        );
        assert_eq!(result.is_ok(), fault == 0);
        if fault == 4 {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert!(budget.storage() > MODULE_FLOOR);
        } else {
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

#[test]
fn typed_effect_pointer_roles_follow_operand_slots_even_when_values_alias() {
    use fe2o3_kernel_ir::{
        MemoryAccess, StorageCopyOverlapV1, StorageOperationV1 as S, StorageProjectionV1 as P,
    };
    let pointer = ValueId(37);
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    let operations = [
        S::Project {
            base: pointer,
            step: P::Variant { index: 0, access },
        },
        S::ReadValue {
            address: pointer,
            access,
        },
        S::ReadDiscriminant {
            address: pointer,
            access,
        },
        S::WriteValue {
            address: pointer,
            value: pointer,
            access,
        },
        S::SetDiscriminant {
            address: pointer,
            variant: 0,
            access,
        },
        S::CopyObject {
            source: pointer,
            destination: pointer,
            source_access: access,
            destination_access: access,
            overlap: StorageCopyOverlapV1::MayOverlap,
        },
    ];
    // Isolated role decoding is not prepared-source or typed-currentness proof.
    for operation in operations {
        let kind = OperationKind::Storage(operation);
        let mut operands = Vec::new();
        kind.visit_operands(|value| operands.push(value));
        let mut effects = 0u32;
        operation
            .try_visit_memory_accesses(|value, _, _| {
                let (slot, actual) = optimized_effect_pointer_role_v18(&kind, effects).unwrap();
                assert_eq!(actual, value);
                assert_eq!(operands[slot as usize], value);
                assert_eq!(
                    slot, effects,
                    "typed access slots stay distinct despite equal values"
                );
                effects += 1;
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
        assert_eq!(optimized_effect_pointer_role_v18(&kind, effects), None);
        assert_eq!(optimized_effect_pointer_role_v18(&kind, u32::MAX), None);
    }
    for step in [
        P::Field(0),
        P::ArrayIndex(pointer),
        P::VariantForWrite { index: 0 },
    ] {
        assert_eq!(
            optimized_effect_pointer_role_v18(
                &OperationKind::Storage(S::Project {
                    base: pointer,
                    step
                }),
                0
            ),
            None
        );
    }
    assert_eq!(
        optimized_effect_pointer_role_v18(
            &OperationKind::Execution(fe2o3_kernel_ir::ExecutionOperationV15::ContextIssue),
            0
        ),
        None
    );
    assert_eq!(
        optimized_effect_pointer_role_v18(
            &OperationKind::Store {
                pointer,
                value: pointer,
                access,
            },
            0
        ),
        Some((0, pointer))
    );
}

#[test]
fn optimized_formal_facts_borrow_the_actual_output_owner() {
    run_production_optimized_consumer_v18(
        folding_source_owner_v18,
        |original, optimized, budget| {
            let output = optimized.output_inventory(budget)?;
            let floor = budget.storage();
            original
                .with_optimized_guarded_reads_v18(
                    optimized,
                    fe2o3_kernel_ir::CanonicalGuardedGlobalReadLimitsV1::default(),
                    budget,
                    |facts, budget| {
                        assert!(std::ptr::eq(facts.owner(budget)?, output.owner()));
                        assert!(!std::ptr::eq(
                            facts.owner(budget)?,
                            original.inventory.owner()
                        ));
                        assert_eq!(facts.function_count(budget)?, output.functions().len());
                        Ok::<_, fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1>(())
                    },
                )
                .expect("actual output guarded-read facts");
            assert_eq!(budget.storage(), floor);
            Ok(())
        },
    );
}
include!("production_optimized_source_alias_generality_v18_tests.rs");

#[test]
fn optimized_typed_source_roles_recheck_folded_alias_births_and_fresh_restarts() {
    for (case, factory) in [
        (
            PhysicalAddressCase::Stored,
            stored_physical_owner_v29 as fn() -> ProductionSemanticSsaOwnerV1,
        ),
        (PhysicalAddressCase::FreshRestart, || {
            physical_address_owner(PhysicalAddressCase::FreshRestart)
        }),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
        let joined = std::cell::Cell::new(false);
        let completed = std::cell::Cell::new(false);
        let result = with_production_optimized_consumer_v18(
            prepared,
            &mut budget,
            |original, optimized, budget| {
                let (retained, unreachable) =
                    scoped_raw_admission_v29::test_optimized_whole_value_roles_v18(
                        original, optimized, 0, budget,
                    )?;
                assert!(retained > 0);
                assert_eq!(unreachable, 0);
                let input = original.inventory;
                let input_function = &input.functions()[original.source.root(0, budget)?.1];
                let mut erased_aliases = 0;
                for operation in &input.operations()[input_function.operations.clone()] {
                    budget.charge_work(1)?;
                    if let OperationKind::Select {
                        true_value,
                        false_value,
                        ..
                    } = operation.operation.kind
                        && true_value == false_value
                        && matches!(operation.operation.results.as_slice(), [result] if matches!(result.ty, Type::Pointer(_)))
                    {
                        assert!(matches!(
                            optimized.operation(operation.coordinate, budget)?,
                            ProductionOptimizedSourceOperationV18::Rewritten { .. }
                        ));
                        erased_aliases += 1;
                    }
                }
                assert!(
                    erased_aliases >= 2,
                    "real same-arm folding must have occurred"
                );
                joined.set(true);
                original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
                    analyses.with_memory_versions(budget, |input, output, budget| {
                        scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                            original,
                            optimized,
                            0,
                            input,
                            output,
                            budget,
                            |checked, budget| -> SourceOwnedResultV18<()> {
                                let mut actual_accesses = 0;
                                checked.visit_accesses(budget, |_, _, _, actual, budget| {
                                    budget.charge_work(1)?;
                                    assert!(
                                        actual.is_some(),
                                        "this fixture has no unreachable memory effects"
                                    );
                                    actual_accesses += 1;
                                    Ok(())
                                })?;
                                assert!(actual_accesses >= retained);
                                completed.set(true);
                                Ok(())
                            },
                        )
                    })
                })
            },
        );
        assert!(joined.get(), "{case:?}: {result:?}");
        assert!(completed.get(), "{case:?}: {result:?}");
        result.unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

fn optimized_alias_currentness_resource_run_v18(
    work_limit: usize,
    storage_limit: usize,
) -> (ProductionOptimizerTestResultV18, usize, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut count = 0;
    let result = (|| {
        let prepared = physical_prepared_result_v29(&mut budget)?;
        with_production_optimizer_result_v18(
            prepared,
            &mut budget,
            |original, optimized, budget| {
                original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
                    analyses.with_memory_versions(budget, |input, output, budget| {
                        let floor = budget.storage();
                        scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                            original,
                            optimized,
                            0,
                            input,
                            output,
                            budget,
                            |checked, budget| {
                                checked.visit_accesses(budget, |_, _, _, actual, budget| {
                                    budget.charge_work(1)?;
                                    assert!(actual.is_some());
                                    count += 1;
                                    Ok(())
                                })
                            },
                        )?;
                        assert_eq!(
                            budget.storage(),
                            floor,
                            "alias transport is lexical scratch"
                        );
                        Ok(())
                    })
                })
            },
        )
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), count)
}

#[test]
fn optimized_folded_alias_currentness_has_exact_and_one_short_owned_resource_boundaries() {
    let (result, work, storage, count) =
        optimized_alias_currentness_resource_run_v18(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(count > 0);
    let (exact, used, peak, retained) = optimized_alias_currentness_resource_run_v18(work, storage);
    exact.unwrap();
    assert_eq!((used, peak, retained), (work, storage, count));
    for (work_limit, storage_limit, expect_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, _, _, _) =
            optimized_alias_currentness_resource_run_v18(work_limit, storage_limit);
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
            other => panic!("exact original resource denial required: {other:?}"),
        };
        if expect_work {
            assert!(matches!(error, ArgumentResourceV1::Work(_)));
        } else {
            assert!(matches!(error, ArgumentResourceV1::Storage(_)));
        }
    }
}

fn optimized_loop_alias_source_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let owner = physical_address_owner(PhysicalAddressCase::Stored);
    let semantic = owner.source_semantic();
    let original = &semantic.functions()[0];
    let source_statements = original.blocks()[0].statements();
    assert_eq!(source_statements.len(), 8);
    assert!(
        matches!(source_statements[0].kind(), SemanticStatementKindV1::StorageLive(local)
        if local.index() == 2)
    );
    let raw = original.locals()[3].ty();
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let statements = vec![
        source_statements[0].clone(), // The only static activation of object 2.
        source_statements[1].clone(),
        source_statements[4].clone(), // Fresh alias A for this iteration.
        source_statements[6].clone(), // Retained holder write of A.
        source_statements[7].clone(), // Actual holder load and dereference.
        assign(
            place(4, raw),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place(2, U32),
            },
        ),
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(3, raw),
                SemanticOperandV1::Copy(place(4, raw)),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ),
    ];
    let blocks = vec![
        // Keep the original true assertion: its Bool is part of the exact
        // retained type closure, and its success edge enters the loop.
        block(64, vec![], original.blocks()[0].terminator().kind().clone()),
        block(
            65,
            statements,
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(1, U32)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(66, vec![], SemanticTerminatorKindV1::Return),
    ];
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let mut functions = semantic.functions().to_vec();
    functions[0] = root;
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
        ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn optimized_loop_restart_generated_holder_role_rejects_an_inert_stale_alias_substitution() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared =
        scalar_payload_prepared_from_v18(optimized_loop_alias_source_owner_v18, &mut budget);
    let completed = std::cell::Cell::new(false);
    let result = with_production_optimized_consumer_v18(
        prepared,
        &mut budget,
        |original, optimized, budget| {
            original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
            analyses.with_memory_versions(budget, |input, output, budget| {
                let challenged = scoped_raw_admission_v29::test_optimized_alias_fresh_holder_equations_v18(
                    original, optimized, input, output, budget)?;
                assert!(challenged > 0, "actual loop-restart pointer-holder payload role and stale backedge witness must be tested");
                completed.set(true);
                Ok::<(), ProductionSourceOwnedViewErrorV18>(())
            })
        })
        },
    );
    assert!(completed.get(), "{result:?}");
    result.unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn optimized_saved_restart_is_rejected_by_original_admission_before_the_optimizer_consumer() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let entered = std::cell::Cell::new(false);
    let result = (|| -> SourceOwnedResultV18<()> {
        let projection = physical_address_owner(PhysicalAddressCase::SavedRestart);
        let owner = physical_address_owner(PhysicalAddressCase::SavedRestart);
        let (_, launch) =
            with_module_fixture_view(&owner, ModuleFixture::Ordinary, &mut budget, |_, _| ())?;
        let prepared = with_module_fixture_view(
            &projection,
            ModuleFixture::Ordinary,
            &mut budget,
            |source, budget| {
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                    owner,
                    launch,
                    source.input,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
            },
        )?
        .0?;
        with_production_optimized_consumer_v18(prepared, &mut budget, |_, _, _| {
            entered.set(true);
            Ok(())
        })
    })();
    assert!(
        !entered.get(),
        "invalid original source must not reach optimized admission"
    );
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        detail: "source raw pointer outlived its storage activation",
                        ..
                    }
                )
            ))
        ),
        "{result:?}"
    );
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
fn optimized_whole_value_role_resource_run_v18(
    work_limit: usize,
    storage_limit: usize,
) -> (ProductionOptimizerTestResultV18, usize, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut count = 0;
    let result = (|| {
        let prepared = physical_prepared_result_v29(&mut budget)?;
        with_production_optimizer_result_v18(
            prepared,
            &mut budget,
            |original, optimized, budget| {
                let floor = budget.storage();
                let (retained, unreachable) =
                    scoped_raw_admission_v29::test_optimized_whole_value_roles_v18(
                        original, optimized, 0, budget,
                    )?;
                assert_eq!(
                    budget.storage(),
                    floor,
                    "typed role census must refund its lexical scratch"
                );
                assert_eq!(unreachable, 0);
                assert!(retained > 0);
                count = retained;
                Ok(())
            },
        )
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), count)
}

#[test]
fn optimized_typed_source_role_census_has_exact_and_one_short_owned_resource_boundaries() {
    let (result, work, storage, count) =
        optimized_whole_value_role_resource_run_v18(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(count > 0);
    let (exact, used, peak, retained) = optimized_whole_value_role_resource_run_v18(work, storage);
    exact.unwrap();
    assert_eq!((used, peak, retained), (work, storage, count));
    for (work_limit, storage_limit, expect_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, _, _, _) =
            optimized_whole_value_role_resource_run_v18(work_limit, storage_limit);
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
            other => panic!("exact original resource denial required: {other:?}"),
        };
        if expect_work {
            assert!(matches!(error, ArgumentResourceV1::Work(_)));
        } else {
            assert!(matches!(error, ArgumentResourceV1::Storage(_)));
        }
    }
}
#[test]
fn optimized_typed_source_role_census_rejects_missing_duplicate_and_substituted_use_roles() {
    use TileAttachmentFieldV29 as Field;
    let other_singletons = [
        (Field::MemoryPosition, 0),
        (Field::MemoryPointer, 0),
        (Field::MemoryLoadResult, 0),
        (Field::MemoryStoreValue, 0),
        (Field::MemoryStoreUse, 0),
        (Field::ObjectOperand, 0),
        (Field::ObjectOperand, 1),
        (Field::ObjectOperandUse, 1),
        (Field::ObjectResult, 0),
    ];
    for fault in 0..4 + other_singletons.len() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = physical_prepared_result_v29(&mut budget).unwrap();
        let rejected = std::cell::Cell::new(false);
        let expected = if !matches!(fault, 0 | 2) {
            "typed object attachment census"
        } else {
            "typed object changed exact operand use"
        };
        let result = with_production_optimizer_result_v18(
            prepared,
            &mut budget,
            |original, optimized, budget| {
                assert!(
                    scoped_raw_admission_v29::test_optimized_whole_value_roles_v18(
                        original, optimized, 0, budget
                    )?
                    .0 > 0
                );
                let floor = budget.storage();
                budget.reserve_storage(
                    std::mem::size_of::<Vec<SourceAttachmentV18>>()
                        + std::mem::size_of::<ProductionSourceCorrespondenceV18<'_>>(),
                )?;
                let mut rows = source_attachments_v18(original.source, original.inventory, budget)?;
                let at = rows
                    .iter()
                    .position(|row| {
                        row.key.root == 0
                            && row.key.family == TileAttachmentFamilyV29::MemoryAnchor
                            && row.key.field == TileAttachmentFieldV29::ObjectOperandUse
                            && row.key.component == 0
                            && row.location != TileAttachmentLocationV29::NoOutput
                    })
                    .unwrap();
                match fault {
                    0 => rows[at].location = TileAttachmentLocationV29::NoOutput,
                    1 => rows[at].key.part = 1,
                    2 => {
                        let replacement = rows
                            .iter()
                            .find(|row| {
                                row.key.family == TileAttachmentFamilyV29::MemoryAnchor
                                    && row.key.field == TileAttachmentFieldV29::ObjectOperandUse
                                    && row.location != TileAttachmentLocationV29::NoOutput
                                    && row.location != rows[at].location
                            })
                            .unwrap()
                            .location;
                        rows[at].location = replacement;
                    }
                    3 => {
                        let key = rows[at].key;
                        let extra = rows
                            .iter()
                            .position(|row| {
                                row.key.root == key.root
                                    && row.key.family == key.family
                                    && row.key.instance == key.instance
                                    && row.key.row == key.row
                                    && row.key.field == key.field
                                    && row.key.component == 1
                            })
                            .unwrap();
                        rows[extra] = rows[at];
                        rows[extra].key.part = 1;
                    }
                    other => {
                        let key = rows[at].key;
                        let (field, component) = other_singletons[other - 4];
                        let singleton = rows
                            .iter()
                            .position(|row| {
                                row.key.root == key.root
                                    && row.key.family == key.family
                                    && row.key.instance == key.instance
                                    && row.key.row == key.row
                                    && row.key.field == field
                                    && row.key.component == component
                            })
                            .unwrap();
                        assert_eq!(rows[singleton].key.part, 0);
                        rows[singleton].key.part = 1;
                    }
                }
                private_array_heapsort_v1(
                    &mut rows,
                    |row| source_attachment_key_v18(row.key),
                    &mut SourceCorrespondenceWorkV18(budget),
                    || ArgumentResourceV1::Arithmetic.into(),
                )?;
                let tampered = ProductionSourceCorrespondenceV18 {
                    source: original.source,
                    inventory: original.inventory,
                    attachments: &rows,
                    slot: std::ptr::from_ref(budget) as usize,
                    ledger: budget.work_ledger_identity_v1(),
                    floor: budget.storage(),
                };
                let error = scoped_raw_admission_v29::test_optimized_whole_value_roles_v18(
                    &tampered, optimized, 0, budget,
                )
                .expect_err("typed use role substitution must refuse");
                assert!(
                    matches!(error, ProductionSourceOwnedViewErrorV18::Binding(detail) if detail == expected),
                    "fault {fault}: {error:?}"
                );
                let stopped = budget.work();
                assert!(
                    scoped_raw_admission_v29::test_optimized_whole_value_roles_v18(
                        original, optimized, 0, budget
                    )
                    .is_err()
                );
                assert_eq!(
                    budget.work(),
                    stopped,
                    "the first typed role refusal remains owned"
                );
                rejected.set(true);
                drop(tampered);
                drop(rows);
                budget.release_storage(budget.storage() - floor)?;
                Err(error)
            },
        );
        assert!(rejected.get(), "fault {fault}: {result:?}");
        assert!(
            matches!(result, Err(ProductionSourceOptimizationErrorV18::Source(
            ProductionSourceOwnedViewErrorV18::Binding(detail))) if detail == expected),
            "fault {fault}: {result:?}"
        );
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn optimized_formal_local_storage_refusal_is_exact_and_does_not_fail_shared_storage() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let observed = std::cell::Cell::new(None);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let attempted =
                source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                    let limits = fe2o3_kernel_ir::CanonicalGuardedGlobalReadLimitsV1 {
                        per_function_new_bytes: 0,
                        ..Default::default()
                    };
                    let before = budget.failed_storage();
                    let error = original
                        .with_optimized_guarded_reads_v18(
                            optimized,
                            limits,
                            budget,
                            |_, _| -> Result<(), std::convert::Infallible> {
                                panic!("zero local capacity admitted")
                            },
                        )
                        .unwrap_err();
                    let ProductionOptimizedSourceFormalErrorV18::Formal {
                        error:
                            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Resource(
                                fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1::Storage {
                                    actual,
                                    limit,
                                },
                            ),
                        source_refusal:
                            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                                source,
                            )),
                    } = error
                    else {
                        panic!("local refusal lost its exact formal error: {error:?}")
                    };
                    assert!(actual > 0);
                    assert_eq!(limit, 0);
                    assert_eq!((source.actual(), source.limit()), (actual, limit));
                    assert_eq!(budget.failed_storage(), before);
                    observed.set(Some((actual, limit)));
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                });
        assert!(attempted.is_err());
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    let expected = observed.get().expect("formal checker reached");
    assert!(
        matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)))
        if (error.actual(), error.limit()) == expected)
    );
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn caught_formal_coordinate_refusal_remains_a_source_refusal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let reached = std::cell::Cell::new(false);
    let coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
        block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(u32::MAX),
            block: u32::MAX,
        },
        operation: u32::MAX,
    };
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let attempted = source.with_checked_optimization_v18(budget, |original, optimized, budget| {
            let error = original.with_optimized_guarded_reads_v18(optimized, Default::default(), budget,
                |facts, budget| {
                    assert!(matches!(facts.read_at(coordinate, budget),
                        Err(fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Coordinate(actual)) if actual == coordinate));
                    Ok::<_, std::convert::Infallible>(())
                }).unwrap_err();
            assert!(matches!(error, ProductionOptimizedSourceFormalErrorV18::Formal {
                error: fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Coordinate(actual),
                source_refusal: ProductionSourceOwnedViewErrorV18::Binding("optimized formal query used a foreign output coordinate"),
            } if actual == coordinate));
            reached.set(true);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
        });
        assert!(attempted.is_err());
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized formal query used a foreign output coordinate"
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
#[test]
fn optimized_memory_footprint_census_covers_actual_output_and_original_instances() {
    use fe2o3_kernel_ir::KirLocalMemoryEffectRefV1 as Effect;
    run_production_optimized_consumer_v18(
        scalar_payload_owner_v18,
        |original, optimized, budget| {
            let source = original.source(budget)?;
            let input = original.inventory(budget)?;
            let output = optimized.output_inventory(budget)?;
            let mut accesses = 0usize;
            for root in 0..source.root_count(budget)? {
                let floor = budget.storage();
                budget
                    .reserve_storage(2 * std::mem::size_of::<SourcePointerSpacesV18<'_, '_>>())?;
                let input_function = input.functions()[source.root(root, budget)?.1].coordinate;
                let output_function =
                    optimized_source_root_function_v18(original, optimized, root, budget)?;
                let before =
                    SourcePointerSpacesV18::derive(original, input, input_function, budget)?;
                let after = SourcePointerSpacesV18::derive_optimized(
                    original,
                    optimized,
                    output,
                    output_function.coordinate,
                    budget,
                )?;
                let census = optimized_source_effect_census_v18(
                    original, optimized, root, &before, &after, budget,
                )?;
                let expected = output.effects()[output_function.effects.clone()]
                    .iter()
                    .filter(|row| {
                        matches!(
                            row.effect,
                            Effect::Read(_)
                                | Effect::Write(_)
                                | Effect::VolatileRead(_)
                                | Effect::VolatileWrite(_)
                                | Effect::Atomic { .. }
                        )
                    })
                    .count();
                assert_eq!(census.effects.len(), expected);
                assert_eq!(census.bindings.len(), expected);
                assert_eq!(census.removed_unreachable, 0);
                for (effect, binding) in census.effects.iter().zip(&census.bindings) {
                    assert_eq!(effect.coordinate.block.function, output_function.coordinate);
                    let instance = binding
                        .original
                        .instance
                        .expect("actual output retained its original source invocation");
                    assert!(instance < source.instance_count(root, budget)?);
                    assert!(
                        binding.original.span.is_some()
                            || binding.original.lifecycle.is_some()
                            || binding.original.failure.is_some()
                            || binding.original.terminal.is_some()
                    );
                }
                accesses += census.effects.len();
                drop((census, after, before));
                let credit = budget.storage().checked_sub(floor).unwrap();
                budget.release_storage(credit)?;
            }
            assert!(accesses > 0);
            Ok(())
        },
    );
}
