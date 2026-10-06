pub(super) fn optimizer_dead_helper_source_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticSwitchTargetV1, SemanticSwitchTargetsV1};
    let base = scalar_payload_owner_v18();
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let boolean = SemanticTypeIdV1::from_index(
        semantic
            .types()
            .iter()
            .position(|row| {
                matches!(
                    row.shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
                )
            })
            .unwrap() as u32,
    );
    let mut locals = root.locals().to_vec();
    let temporary = locals.len() as u32;
    locals.push(local(241, U32, SemanticLocalRoleV1::Temporary));
    let predicate = locals.len() as u32;
    locals.push(local(245, boolean, SemanticLocalRoleV1::Temporary));
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let compute = assign(
        place(temporary, U32),
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Add,
            left: literal(7),
            right: literal(9),
        },
    );
    // This fixture exercises conditional-branch folding; integer switches use
    // the separate gpu.switch_v3 fold interface and its occurrence checks.
    let compare = assign(
        place(predicate, boolean),
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Equal,
            left: SemanticOperandV1::Copy(place(temporary, U32)),
            right: literal(16),
        },
    );
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(2),
        vec![SemanticOperandV1::Copy(place(1, U32))],
        Some(SemanticCallDestinationV1::new(
            place(0, UNIT),
            edge(SemanticEdgeRoleV1::CallReturn, 2),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    functions[0] = function(
        60,
        root.role(),
        root.abi().clone(),
        locals,
        vec![
            block(
                242,
                vec![compute, compare],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(predicate, boolean)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            1,
                            edge(SemanticEdgeRoleV1::SwitchValue, 2),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                    )
                    .unwrap(),
                },
            ),
            block(243, vec![], SemanticTerminatorKindV1::Call(call)),
            block(
                244,
                vec![assign(
                    place(0, UNIT),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        fe2o3_mir_model::semantic_mir_v1::SemanticConstantV1::new(
                            UNIT,
                            fe2o3_mir_model::semantic_mir_v1::SemanticConstantValueV1::ZeroSized,
                        ),
                    )),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    )
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
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
fn observed_optimizer_dead_helper_keeps_original_effects_distinct_from_control_omission() {
    use ProductionOptimizedSourceSiteControlV18 as D;
    run_production_optimized_consumer_v18(
        optimizer_dead_helper_source_owner_v18,
        |original, optimized, budget| {
            let source = original.source(budget)?;
            let root = 0;
            let root_function = source.root(root, budget)?.0;
            let caller = original
                .unique_source_instance(root, root_function, budget)?
                .unwrap();
            let block = SemanticBlockIdV1::from_index(1);
            assert!(
                original
                    .source_block_entry(root, caller, block, budget)?
                    .is_some(),
                "fixture must be eliminated by actual V18 execution, not original source planning"
            );
            let child = original.defined_call_instance(root, caller, block, budget)?;
            budget.reserve_storage(size_of::<ProductionOptimizedSourceBlockControlV18<'_, '_>>())?;
            let control = optimized.source_block_control(root, caller, block, budget)?;
            assert_eq!(control.disposition(budget)?, D::RemovedUnreachable);
            assert_eq!(control.site(None, budget)?, D::RemovedUnreachable);
            drop(control);
            budget.release_storage(size_of::<ProductionOptimizedSourceBlockControlV18<'_, '_>>())?;
            original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
                analyses.with_source_projection_facts(budget,
                    |_, _, input_effects, output_effects, _, budget| {
                        let effects = optimized.instance_effects(root, child, input_effects, output_effects, budget)?;
                        assert_eq!(effects.input, fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1::CompleteNonempty);
                        // The original helper's private allocation is hoisted
                        // into the live entry. Removing its executable body
                        // must not turn that remaining operation into purity.
                        assert_eq!(effects.output, fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1::CompleteNonempty);
                        let input = optimized.input_inventory(budget)?;
                        let output = optimized.output_inventory(budget)?;
                        let (mut allocations, mut removed_accesses) = (0, 0);
                        for attachment in original.attachments {
                            budget.charge_work(1)?;
                            if attachment.key.root != root || attachment.key.instance != child {
                                continue;
                            }
                            if !matches!(attachment.location, TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(_))) {
                                continue;
                            }
                            let ProductionSourceOperationV18::Operation(coordinate) =
                                original.mapped_source_operation(attachment.location, budget)? else { continue; };
                            let row = source_operation_row_v18(input, coordinate, budget)?;
                            match optimized.operation(coordinate, budget)? {
                                ProductionOptimizedSourceOperationV18::Retained { output: actual, .. } => {
                                    if output_effects.operation_decision(actual, budget).unwrap()
                                        != fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1::CompleteEmpty {
                                        assert_eq!(output_effects.operation_decision(actual, budget).unwrap(),
                                            fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1::CompleteNonempty);
                                        assert!(matches!(row.operation.kind, OperationKind::Alloca { .. }),
                                            "only the hoisted private allocation can remain effectful: {:?}", row.operation.kind);
                                        assert!(matches!(source_operation_row_v18(output, actual, budget)?.operation.kind,
                                            OperationKind::Alloca { .. }));
                                        allocations += 1;
                                    }
                                }
                                ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {
                                    if matches!(row.operation.kind, OperationKind::Load { .. } | OperationKind::Store { .. }) {
                                        removed_accesses += 1;
                                    }
                                }
                                ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                                    assert_eq!(input_effects.operation_decision(coordinate, budget).unwrap(),
                                        fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1::CompleteEmpty);
                                }
                            }
                        }
                        assert!(allocations > 0, "genuine hoisted allocation must survive");
                        assert!(removed_accesses > 0, "genuine helper loads/stores must be unreachable");
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                    })
            })
        },
    );
}

#[test]
fn actual_source_block_control_checks_all_sites_without_repeating_original_block_scan() {
    use ProductionOptimizedSourceSiteControlV18 as D;
    for factory in [
        folding_source_owner_v18 as fn() -> _,
        division_source_owner_v18,
    ] {
        let mut sites = 0usize;
        run_production_optimized_consumer_v18(factory, |original, optimized, budget| {
            let source = original.source(budget)?;
            for root in 0..source.root_count(budget)? {
                for instance in 0..source.instance_count(root, budget)? {
                    if !source.instance_active(root, instance, budget)? {
                        continue;
                    }
                    let function = source.instance(root, instance, budget)?.0;
                    let function = &source
                        .owner
                        .inner
                        .source
                        .owner
                        .source_semantic()
                        .functions()[function.index() as usize];
                    for (block, declaration) in function.blocks().iter().enumerate() {
                        let block = SemanticBlockIdV1::from_index(block as u32);
                        let floor = budget.storage();
                        let header = size_of::<ProductionOptimizedSourceBlockControlV18<'_, '_>>();
                        budget.reserve_storage(header)?;
                        let control =
                            optimized.source_block_control(root, instance, block, budget)?;
                        let disposition = control.disposition(budget)?;
                        for statement in std::iter::once(None).chain(
                            (0..declaration.statements().len()).map(|index| Some(index as u32)),
                        ) {
                            let actual = control.site(statement, budget)?;
                            assert_ne!(actual, D::Mixed, "straight-line arithmetic fixture");
                            assert_eq!(actual, disposition);
                            if actual != D::OriginalUnmaterialized {
                                let mut entries = 0usize;
                                optimized.visit_source_operations(
                                    root,
                                    instance,
                                    block,
                                    statement,
                                    budget,
                                    |_, _| {
                                        entries += 1;
                                        Ok(())
                                    },
                                )?;
                                assert!(entries > 0);
                            }
                            // Repeated indexed site queries have the same charged
                            // cost and do not rescan the original block sidecar.
                            let before = budget.work();
                            assert_eq!(control.site(statement, budget)?, actual);
                            let first = budget.work() - before;
                            let before = budget.work();
                            assert_eq!(control.site(statement, budget)?, actual);
                            assert_eq!(budget.work() - before, first);
                            assert_eq!(budget.storage(), floor + header);
                            sites += 1;
                        }
                        drop(control);
                        budget.release_storage(header)?;
                        assert_eq!(budget.storage(), floor);
                    }
                }
            }
            Ok(())
        });
        assert!(sites > 0);
    }
}

#[test]
fn checked_site_query_rejects_bad_statement_stickily() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let reached = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let attempted =
            source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                let source = original.source(budget)?;
                let root = 0;
                let instance = (0..source.instance_count(root, budget)?)
                    .find(|&index| source.instance_active(root, index, budget).unwrap())
                    .unwrap();
                let header = size_of::<ProductionOptimizedSourceBlockControlV18<'_, '_>>();
                budget.reserve_storage(header)?;
                let control = optimized.source_block_control(
                    root,
                    instance,
                    SemanticBlockIdV1::from_index(0),
                    budget,
                )?;
                let error = control.site(Some(u32::MAX), budget).unwrap_err();
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "optimized source control statement locator"
                    )
                ));
                assert!(matches!(
                    control.site(None, budget),
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "optimized source control statement locator"
                    ))
                ));
                reached.set(true);
                drop(control);
                budget.release_storage(header)?;
                Err::<((), usize), _>(error)
            });
        assert!(attempted.is_err());
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized source control statement locator"
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_optimized_assertions_use_both_retained_source_and_fresh_output_analyses() {
    use fe2o3_kernel_analysis::{
        CanonicalKirEdgePlacementV1 as Edge, CanonicalKirSparseValueV1 as Value,
    };
    for kind in [ModuleFixture::Ordinary, ModuleFixture::LiveAssertion] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_optimized_module_v18(kind, &mut budget);
        let mut visited = 0usize;
        with_production_optimized_consumer_v18(prepared, &mut budget, |original, optimized, budget| {
            original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
                analyses.with_source_projection_facts(budget,
                    |input_sparse, output_sparse, input_effects, output_effects, cleanup, budget| {
                        let input = optimized.input_inventory(budget)?;
                        let output = optimized.output_inventory(budget)?;
                        assert!(input_sparse.belongs_to(input));
                        assert!(output_sparse.belongs_to(output));
                        assert!(input_effects.belongs_to(input));
                        assert!(output_effects.belongs_to(output));
                        assert!(!cleanup.refund_denied());
                        let source = original.source(budget)?;
                        for root in 0..source.root_count(budget)? {
                            let (owner, physical) = source.root(root, budget)?;
                            for row in &source.owner.inner.assertions {
                                if row.site.correspondence_owner != owner
                                    || row.binding.block().function.0 as usize != physical { continue; }
                                let source_assertion = original.assertion(root, row.instance.index(), row.site.semantic_block, budget)?;
                                assert_eq!(source_assertion.expected(), row.binding.expected());
                                match optimized.assertion(root, row.instance.index(), row.site.semantic_block, budget)? {
                                    SemanticKirOptimizedAssertOutcomeV1::Conditional { condition, success, failure } => {
                                        assert_ne!(success, failure);
                                        let value = output_sparse.value_at_use(condition.coordinate, budget)
                                            .map_err(|error| match error {
                                                fe2o3_kernel_analysis::CanonicalKirSparseErrorV1::Resource(error) => ProductionSourceOwnedViewErrorV18::Resource(error),
                                                _ => ProductionSourceOwnedViewErrorV18::Binding("assertion sparse fixture"),
                                            })?;
                                        if let Value::Constant(value) = value {
                                            assert_eq!(value.ty(), ScalarType::Bool);
                                            assert!(value.bits() <= 1);
                                        }
                                        assert!(output.uses().iter().any(|row| row.coordinate == condition.coordinate));
                                    }
                                    SemanticKirOptimizedAssertOutcomeV1::SelectedSuccess { success, failure } => {
                                        assert_ne!(success, Edge::Omitted);
                                        assert_eq!(failure, Edge::Omitted);
                                    }
                                    SemanticKirOptimizedAssertOutcomeV1::SelectedFailure { success, failure } => {
                                        assert_eq!(success, Edge::Omitted);
                                        assert_ne!(failure, Edge::Omitted);
                                    }
                                    SemanticKirOptimizedAssertOutcomeV1::RemovedUnreachable { .. } => {
                                        assert!(!optimized.source_block(root, row.instance.index(), row.site.semantic_block, budget)?
                                            .is_some_and(|control| control.reachable));
                                    }
                                    SemanticKirOptimizedAssertOutcomeV1::SourceRuleElision { success } => {
                                        assert_ne!(success, Edge::Omitted);
                                    }
                                }
                                visited += 1;
                            }
                        }
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                    })
            })
        }).unwrap();
        assert!(visited > 0);
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
