#[test]
fn actual_store_rhs_selects_constant_descendant_and_keeps_zero_use_loads() {
    run_optimized_source_v18(division_source_owner_v18, |view, budget| {
        let cleanup = view.original_source(budget)?.cleanup;
        let floor = budget.storage();
        // Borrowed allocation rows must die before their scratch is refunded.
        source_scalar_normalization_scratch_v18(cleanup, budget, 0, |budget| {
            let mut loads = 0;
            let mut stores = 0;
            let mut folded_stores = 0;
            let mut allocations = 0;
            let source = view.original_source(budget)?;
            for root in 0..source.root_count(budget)? {
                let (_, function) = source.root(root, budget)?;
                let input = view.input_inventory(budget)?;
                for operation in &input.operations()[input.functions()[function].operations.clone()]
                {
                    if let Some(allocation) = view.allocation(root, operation.coordinate, budget)? {
                        allocations += 1;
                        assert_eq!(allocation.input(), operation.coordinate);
                        assert!(allocation.output().is_some());
                        assert!(allocation.pointer().is_some());
                    }
                    let Some(access) =
                        view.scalar_memory_access(root, operation.coordinate, budget)?
                    else {
                        continue;
                    };
                    assert!(access.output().is_some());
                    assert!(access.pointer().is_some());
                    match access.payload() {
                        Some(ProductionOptimizedSourcePayloadV18::Load { output }) => {
                            assert!(matches!(output, OptimizedDefinition::Result { .. }));
                            loads += 1;
                        }
                        Some(ProductionOptimizedSourcePayloadV18::Store { output }) => {
                            let OptimizedUse::OperationOperand {
                                operation: actual_operation,
                                ..
                            } = output.coordinate
                            else {
                                panic!("not a Store use")
                            };
                            assert_eq!(Some(actual_operation), access.output());
                            let value = match operation.operation.kind {
                                OperationKind::Store { value, .. }
                                | OperationKind::GuardedStore { value, .. } => value,
                                _ => panic!("original payload is not a Store"),
                            };
                            let input_definition = input
                                .definition_index_for_value(
                                    operation.coordinate.block.function,
                                    value,
                                    budget,
                                )
                                .unwrap()
                                .unwrap();
                            let descendants = view.definition_descendants(
                                input.definitions()[input_definition].coordinate,
                                budget,
                            )?;
                            if descendants.len() > 1 {
                                let OptimizedDefinition::Result {
                                    operation: definition,
                                    result: 0,
                                } = output.definition
                                else {
                                    panic!("folded RHS must be the actual synthesized constant")
                                };
                                let actual = view
                                    .output_inventory(budget)?
                                    .operations()
                                    .iter()
                                    .find(|row| row.coordinate == definition)
                                    .unwrap();
                                assert!(matches!(
                                    actual.operation.kind,
                                    OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(16))
                                ));
                                folded_stores += 1;
                            }
                            stores += 1;
                        }
                        Some(ProductionOptimizedSourcePayloadV18::RemovedUnreachable) => {
                            panic!("live memory disappeared")
                        }
                        None => {}
                    }
                }
            }
            assert!(loads > 0 && stores > 0 && allocations > 0 && folded_stores > 0);
            Ok(())
        })?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn scalar_none_does_not_erase_actual_execution_payloads_or_instance_effects() {
    for kind in [ModuleFixture::Mixed, ModuleFixture::Array] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_optimized_module_v18(kind, &mut budget);
        with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let input = view.input_inventory(budget)?;
            let output = view.output_inventory(budget)?;
            let source = view.original_source(budget)?;
            let (before, before_storage) = fe2o3_kernel_analysis::CanonicalKirCallEffectsV18::derive_v18(input, budget).unwrap();
            budget.reserve_storage(before_storage.retained_storage())?;
            let (after, after_storage) = fe2o3_kernel_analysis::CanonicalKirCallEffectsV18::derive_v18(output, budget).unwrap();
            budget.reserve_storage(after_storage.retained_storage())?;
            let mut execution = 0;
            for root in 0..source.root_count(budget)? {
                let (_, physical) = source.root(root, budget)?;
                for operation in &input.operations()[input.functions()[physical].operations.clone()] {
                    if let OperationKind::Execution(before_payload) = &operation.operation.kind {
                        assert!(view.scalar_memory_access(root, operation.coordinate, budget)?.is_none());
                        let ProductionOptimizedSourceOperationV18::Retained { output: retained, .. } = view.operation(operation.coordinate, budget)?
                            else { panic!("ordered lifecycle operation disappeared"); };
                        let actual = output.operations().iter().find(|row| row.coordinate == retained).unwrap();
                        let OperationKind::Execution(after_payload) = &actual.operation.kind else { panic!("lifecycle opcode changed") };
                        assert_eq!(std::mem::discriminant(before_payload), std::mem::discriminant(after_payload));
                        assert_eq!(operation.operands.len(), actual.operands.len());
                        for use_row in &input.uses()[operation.operands.clone()] {
                            let mapped = view.operand(use_row.coordinate, budget)?.expect("ordered operand retained");
                            assert!(matches!(mapped.coordinate, OptimizedUse::OperationOperand { operation, .. } if operation == retained));
                        }
                        assert_eq!(before.operation_decision(operation.coordinate, budget).unwrap(),
                            fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1::CompleteNonempty);
                        execution += 1;
                    }
                }
                let decision = view.instance_effects(root, 0, &before, &after, budget)?;
                assert_eq!(decision.input, decision.output);
            }
            assert!(execution > 0, "real source lifecycle must emit Execution operations");
            let actual = output.operations().iter().filter(|operation| matches!(operation.operation.kind, OperationKind::Execution(_))).count();
            assert_eq!(actual, execution, "scalar None cannot authorize removal of ordered lifecycle operations");
            drop(after);
            drop(before);
            budget.release_storage(before_storage.retained_storage() + after_storage.retained_storage())?;
            Ok(())
        }).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
#[test]
fn private_array_allocations_join_actual_count_uses_instead_of_choosing_descendants() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_optimized_module_v18(ModuleFixture::Array, &mut budget);
    let counts = std::cell::Cell::new(0usize);
    with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        let source = view.original_source(budget)?;
        let floor = budget.storage();
        source_scalar_normalization_scratch_v18(source.cleanup, budget, 0, |budget| {
        for root in 0..source.root_count(budget)? {
            let (_, function) = source.root(root, budget)?;
            let input = view.input_inventory(budget)?;
            for row in &input.operations()[input.functions()[function].operations.clone()] {
                let Some(allocation) = view.allocation(root, row.coordinate, budget)? else { continue; };
                if let Some(count) = allocation.count() {
                    assert!(matches!(count.coordinate, OptimizedUse::OperationOperand { operation, operand: 0 }
                        if Some(operation) == allocation.output()));
                    assert_eq!(view.operand(OptimizedUse::OperationOperand { operation: row.coordinate, operand: 0 }, budget)?, Some(count));
                    counts.set(counts.get() + 1);
                }
            }
        }
        Ok(())
        })?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    }).unwrap();
    assert!(counts.get() > 0);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
