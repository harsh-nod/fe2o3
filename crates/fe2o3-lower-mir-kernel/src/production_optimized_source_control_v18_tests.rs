struct OutputCfgMeterV18<'a, 'g, 'b, 'w> {
    source: &'a ProductionSourceOwnedViewV18<'g>,
    budget: &'b mut ArgumentBudgetV1<'w>,
}

impl fe2o3_mir_model::SemanticAssertionMeterV1 for OutputCfgMeterV18<'_, '_, '_, '_> {
    type Error = ProductionSourceOwnedViewErrorV18;
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.source.check_query_v18(self.budget)?;
        self.budget
            .charge_work(amount)
            .map_err(|error| self.source.retain_query_resource_error_v18(error))
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.source.check_query_v18(self.budget)?;
        self.budget
            .reserve_storage(amount)
            .map_err(|error| self.source.retain_query_resource_error_v18(error))
    }
}

#[test]
fn output_cfg_streams_actual_order_merge_connectors_effects_and_edge_roles() {
    use ProductionOptimizedSourceCfgEventV18 as Event;
    use fe2o3_kernel_ir::CanonicalKirOperationOriginV1 as Origin;
    let merged = std::cell::Cell::new(0usize);
    for factory in [active_shared_target_owner_v18, folding_source_owner_v18] {
        run_optimized_source_v18(factory, |view, budget| {
            let source = view.original_source(budget)?;
            for root in 0..source.root_count(budget)? {
                let floor = budget.storage();
                budget.reserve_storage(std::mem::size_of::<
                    ProductionOptimizedSourceCfgRootV18<'_, '_>,
                >())?;
                let cfg = view.output_root_cfg_v18(root, budget)?;
                assert_eq!(cfg.root(), root);
                assert!(std::ptr::eq(
                    cfg.inventory(),
                    view.output_inventory(budget)?
                ));
                let inventory = cfg.inventory();
                let input_inventory = view.input_inventory(budget)?;
                let expected = cfg.function();
                let mut blocks = expected.blocks.start;
                let mut operations = expected.operations.start;
                let mut effects = expected.effects.start;
                let mut edges = expected.edges.start;
                let mut terminators = 0;
                let mut open = None;
                cfg.visit(&mut OutputCfgMeterV18 { source, budget }, |event, meter| {
                    match event {
                        Event::Block { actual, segments } => {
                            assert!(open.replace(actual.coordinate).is_none());
                            assert!(std::ptr::eq(actual, &inventory.blocks()[blocks]));
                            assert!(!segments.is_empty());
                            meter.charge_work(segments.len())?;
                            for (position, segment) in segments.iter().enumerate() {
                                assert_eq!(
                                    segment.connector.is_some(),
                                    position + 1 < segments.len()
                                );
                                if let Some(connector) = segment.connector {
                                    assert_eq!(connector.source, segment.input);
                                    merged.set(merged.get() + 1);
                                }
                            }
                            blocks += 1;
                        }
                        Event::Operation {
                            actual,
                            origin,
                            operands,
                            operand_origins,
                            effects: actual_effects,
                        } => {
                            assert_eq!(open, Some(actual.coordinate.block));
                            assert!(std::ptr::eq(actual, &inventory.operations()[operations]));
                            assert_eq!(operands.len(), operand_origins.len());
                            meter.charge_work(operands.len() + actual_effects.len())?;
                            for (operand, origin) in operands.iter().zip(operand_origins) {
                                assert_eq!(operand.coordinate, origin.output);
                            }
                            for effect in actual_effects {
                                assert!(std::ptr::eq(effect, &inventory.effects()[effects]));
                                effects += 1;
                            }
                            match origin {
                                Origin::Retained(input) => {
                                    meter.charge_work(input_inventory.operations().len())?;
                                    assert!(
                                        input_inventory
                                            .operations()
                                            .iter()
                                            .any(|row| row.coordinate == input)
                                    );
                                }
                                Origin::ConstantFrom(_) => assert!(actual_effects.is_empty()),
                            }
                            operations += 1;
                        }
                        Event::Terminator {
                            actual,
                            operands,
                            operand_origins,
                            edges: actual_edges,
                            edge_origins,
                            arguments,
                            argument_origins,
                        } => {
                            assert_eq!(open.take(), Some(actual.coordinate));
                            assert_eq!(operands.len(), operand_origins.len());
                            assert_eq!(actual_edges.len(), edge_origins.len());
                            assert_eq!(arguments.len(), argument_origins.len());
                            meter.charge_work(
                                operands.len() + actual_edges.len() + arguments.len(),
                            )?;
                            for (operand, origin) in operands.iter().zip(operand_origins) {
                                assert_eq!(operand.coordinate, origin.output);
                            }
                            for (edge, origin) in actual_edges.iter().zip(edge_origins) {
                                assert!(std::ptr::eq(edge, &inventory.edges()[edges]));
                                assert_eq!(edge.coordinate, origin.output);
                                edges += 1;
                            }
                            for (argument, origin) in arguments.iter().zip(argument_origins) {
                                assert_eq!(argument.coordinate, origin.output);
                            }
                            terminators += 1;
                        }
                    }
                    Ok(())
                })?;
                assert_eq!(
                    (blocks, operations, effects, edges),
                    (
                        expected.blocks.end,
                        expected.operations.end,
                        expected.effects.end,
                        expected.edges.end
                    )
                );
                assert_eq!(terminators, expected.blocks.len());
                assert!(open.is_none());
                drop(cfg);
                budget.release_storage(budget.storage() - floor)?;
            }
            Ok(())
        });
    }
    assert!(
        merged.get() > 0,
        "fixture must execute a checked block merge"
    );
}

#[test]
fn output_cfg_refuses_missing_source_root_and_retains_first_refusal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let source = view.original_source(budget)?;
        let count = source.root_count(budget)?;
        let missing = view.output_root_cfg_v18(count, budget).err().unwrap();
        assert!(matches!(
            missing,
            ProductionSourceOwnedViewErrorV18::Binding(_)
        ));
        let stopped = budget.work();
        let repeated = view.output_root_cfg_v18(0, budget).err().unwrap();
        assert_eq!(format!("{repeated:?}"), format!("{missing:?}"));
        assert_eq!(
            budget.work(),
            stopped,
            "missing-root refusal precedes new work"
        );
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(_))
    ));
}

#[test]
fn output_cfg_retains_swallowed_meter_refusal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let source = view.original_source(budget)?;
        budget.reserve_storage(std::mem::size_of::<
            ProductionOptimizedSourceCfgRootV18<'_, '_>,
        >())?;
        let cfg = view.output_root_cfg_v18(0, budget)?;
        let mut called = 0;
        let refusal = cfg.visit(&mut OutputCfgMeterV18 { source, budget }, |_, meter| {
            called += 1;
            assert!(meter.charge_work(usize::MAX).is_err());
            Ok(())
        });
        assert_eq!(called, 1);
        assert!(matches!(
            refusal,
            Err(ProductionSourceOwnedViewErrorV18::Resource(_))
        ));
        // The raw ledger is intentionally nonsticky, but the source scope must
        // not publish after a callback swallowed its actual owned refusal.
        budget.charge_work(1)?;
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(_))
    ));
}

#[test]
fn source_control_queries_use_actual_checked_block_and_operand_coordinates() {
    run_optimized_source_v18(active_shared_target_owner_v18, |view, budget| {
        let source = view.original_source(budget)?;
        for root in 0..source.root_count(budget)? {
            for instance in 0..source.instance_count(root, budget)? {
                let function = source.instance(root, instance, budget)?.0;
                for (block, _) in source
                    .owner
                    .inner
                    .source
                    .owner
                    .source_semantic()
                    .functions()[function.index() as usize]
                    .blocks()
                    .iter()
                    .enumerate()
                {
                    if let Some(control) = view.source_block(
                        root,
                        instance,
                        SemanticBlockIdV1::from_index(block as u32),
                        budget,
                    )? {
                        if let Some(placement) = control.placement {
                            assert!(
                                view.output_inventory(budget)?
                                    .blocks()
                                    .iter()
                                    .any(|row| row.coordinate == placement.output)
                            );
                        } else {
                            assert!(!control.reachable);
                        }
                    }
                }
            }
        }
        for operand in view.input_inventory(budget)?.uses() {
            if let Some(output) = view.operand(operand.coordinate, budget)? {
                let inventory = view.output_inventory(budget)?;
                let actual = inventory
                    .uses()
                    .iter()
                    .find(|row| row.coordinate == output.coordinate)
                    .unwrap();
                assert_eq!(
                    inventory.definitions()[actual.definition].coordinate,
                    output.definition
                );
            }
        }
        for edge in view.input_inventory(budget)?.edge_arguments() {
            if let Some(output) = view.edge_argument(edge.coordinate, budget)? {
                assert!(
                    view.output_inventory(budget)?
                        .edge_arguments()
                        .iter()
                        .any(|row| row.coordinate == output)
                );
            }
        }
        Ok(())
    });
}

#[test]
fn instance_effects_require_both_actual_analysis_endpoints() {
    run_optimized_source_v18(scalar_payload_owner_v18, |view, budget| {
        let input = view.input_inventory(budget)?;
        let output = view.output_inventory(budget)?;
        let (before, before_storage) =
            fe2o3_kernel_analysis::CanonicalKirCallEffectsV18::derive_v18(input, budget).unwrap();
        budget.reserve_storage(before_storage.retained_storage())?;
        let (after, after_storage) =
            fe2o3_kernel_analysis::CanonicalKirCallEffectsV18::derive_v18(output, budget).unwrap();
        budget.reserve_storage(after_storage.retained_storage())?;
        let source = view.original_source(budget)?;
        for root in 0..source.root_count(budget)? {
            for instance in 0..source.instance_count(root, budget)? {
                if source.instance_active(root, instance, budget)? {
                    let decisions =
                        view.instance_effects(root, instance, &before, &after, budget)?;
                    assert_ne!(
                        decisions.input,
                        fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1::Incomplete
                    );
                    assert_ne!(
                        decisions.output,
                        fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1::Incomplete
                    );
                }
            }
        }
        drop(after);
        drop(before);
        budget.release_storage(
            after_storage.retained_storage() + before_storage.retained_storage(),
        )?;
        Ok(())
    });
}
#[test]
fn actual_original_assertions_keep_source_polarity_and_checked_outcomes() {
    for kind in [ModuleFixture::Ordinary, ModuleFixture::LiveAssertion] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_optimized_module_v18(kind, &mut budget);
        let visited = std::cell::Cell::new(0usize);
        with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let source = view.original_source(budget)?;
            for root in 0..source.root_count(budget)? {
                let (owner, physical) = source.root(root, budget)?;
                for row in &source.owner.inner.assertions {
                    if row.site.correspondence_owner != owner
                        || row.binding.block().function.0 as usize != physical
                    {
                        continue;
                    }
                    let outcome = view.assertion(
                        root,
                        row.instance.index(),
                        row.site.semantic_block,
                        budget,
                    )?;
                    match outcome {
                        SemanticKirOptimizedAssertOutcomeV1::Conditional {
                            condition,
                            success,
                            failure,
                        } => {
                            assert_ne!(success, failure);
                            assert!(
                                view.output_inventory(budget)?
                                    .uses()
                                    .iter()
                                    .any(|row| row.coordinate == condition.coordinate)
                            );
                        }
                        SemanticKirOptimizedAssertOutcomeV1::SelectedSuccess {
                            success,
                            failure,
                        } => {
                            assert_ne!(
                                success,
                                fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Omitted
                            );
                            assert_eq!(
                                failure,
                                fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Omitted
                            );
                        }
                        SemanticKirOptimizedAssertOutcomeV1::SelectedFailure {
                            success,
                            failure,
                        } => {
                            assert_eq!(
                                success,
                                fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Omitted
                            );
                            assert_ne!(
                                failure,
                                fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Omitted
                            );
                        }
                        SemanticKirOptimizedAssertOutcomeV1::RemovedUnreachable {
                            block, ..
                        } => {
                            if let Some(block) = block {
                                assert!(
                                    view.output_inventory(budget)?
                                        .blocks()
                                        .iter()
                                        .any(|row| row.coordinate == block.output)
                                );
                            }
                        }
                        SemanticKirOptimizedAssertOutcomeV1::SourceRuleElision { success } => {
                            assert_ne!(
                                success,
                                fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Omitted
                            )
                        }
                    }
                    visited.set(visited.get() + 1);
                }
            }
            Ok(())
        })
        .unwrap();
        assert!(visited.get() > 0);
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
