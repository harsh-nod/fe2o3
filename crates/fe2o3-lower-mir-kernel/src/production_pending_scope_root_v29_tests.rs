use super::*;

fn with_emitted(
    nested: bool,
    test: impl FnOnce(
        &ProductionCallInstancePlanV1<'_>,
        &mut Vec<Option<LoweredFunctionResultV1>>,
        &mut ArgumentBudgetV1<'_>,
    ),
) {
    with_plan_owner(scalar_calls_owner(nested), |instances, budget| {
        let floor = budget.storage();
        let lowered = lower_scalar_instances(instances, budget);
        let call_storage: usize = lowered
            .iter()
            .map(|row| row.call_returns.requested_bytes().unwrap())
            .sum();
        let mut slots = lowered.into_iter().map(Some).collect();
        test(instances, &mut slots, budget);
        drop(slots);
        budget.release_storage(call_storage).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

fn slot_snapshot(
    slots: &[Option<LoweredFunctionResultV1>],
) -> Vec<Option<(Option<ProductionCallInstanceIdV1>, usize)>> {
    slots
        .iter()
        .map(|slot| {
            slot.as_ref().map(|row| {
                (
                    row.source_call_instance,
                    row.function.body.as_ref().unwrap().blocks.as_ptr() as usize,
                )
            })
        })
        .collect()
}

fn drop_pending(pending: PendingScopedRootEmissionV29, budget: &mut ArgumentBudgetV1<'_>) {
    let storage = pending.additional_storage_bytes;
    drop(pending);
    budget.release_storage(storage).unwrap();
}

#[test]
fn pending_root_retains_scalar_instance_sidecars_and_nested_capabilities() {
    for nested in [false, true] {
        with_emitted(nested, |instances, slots, budget| {
            let last = slots.last_mut().unwrap().as_mut().unwrap();
            last.function
                .required_capabilities
                .insert(fe2o3_kernel_ir::TargetCapability::Float64);
            let before: Vec<_> = slots
                .iter()
                .map(|row| {
                    let row = row.as_ref().unwrap();
                    (
                        row.call_returns.requested_bytes().unwrap(),
                        row.statement_operation_spans.as_ptr(),
                        row.parameter_bindings.as_ptr(),
                        row.source_call_instance,
                        row.next_value,
                    )
                })
                .collect();
            let floor = budget.storage();
            let pending = assemble_pending_scoped_root_v29(
                instances,
                slots,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
            .unwrap();
            assert!(slots.iter().all(Option::is_none));
            assert_eq!(pending.sidecars.rows.len(), before.len());
            assert!(
                pending
                    .function
                    .required_capabilities
                    .contains(&fe2o3_kernel_ir::TargetCapability::Float64,)
            );
            assert!(
                !pending
                    .function
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| matches!(operation.kind, OperationKind::Call { .. }))
            );
            for (row, old) in pending.sidecars.rows.iter().zip(before) {
                assert_eq!(row.call_returns.requested_bytes().unwrap(), old.0);
                assert_eq!(row.statement_operation_spans.as_ptr(), old.1);
                assert_eq!(row.parameter_bindings.as_ptr(), old.2);
                assert_eq!(row.source_call_instance, old.3);
                assert_eq!(row.next_value, old.4);
            }
            pending
                .coordinates
                .check_source_plan(instances, budget)
                .unwrap();
            assert_eq!(budget.storage(), floor + pending.additional_storage_bytes);
            drop_pending(pending, budget);
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn pending_root_preflight_does_not_consume_missing_swapped_or_aliased_inputs() {
    for fault in 0..5 {
        with_emitted(true, |instances, slots, budget| {
            match fault {
                0 => {
                    slots[1].take();
                }
                1 => slots.swap(1, 2),
                2 => {
                    slots[2].as_mut().unwrap().source_call_instance =
                        slots[1].as_ref().unwrap().source_call_instance
                }
                3 => {
                    let block = slots[1]
                        .as_ref()
                        .unwrap()
                        .function
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks[0]
                        .id;
                    slots[2]
                        .as_mut()
                        .unwrap()
                        .function
                        .body
                        .as_mut()
                        .unwrap()
                        .blocks[0]
                        .id = block;
                }
                4 => slots[0].as_mut().unwrap().source_call_instance = None,
                _ => unreachable!(),
            }
            let before = slot_snapshot(slots);
            let floor = budget.storage();
            assert!(matches!(
                assemble_pending_scoped_root_v29(
                    instances,
                    slots,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                ),
                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
            ));
            assert_eq!(slot_snapshot(slots), before);
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn pending_root_enforces_all_four_size_limits_before_consuming_payloads() {
    for axis in 0..4 {
        for short in [false, true] {
            with_emitted(true, |instances, slots, budget| {
                let mut limits = ProductionSemanticKirLimitsV1::default();
                let exact = match axis {
                    0 => slots.len(),
                    1 => {
                        slots
                            .iter()
                            .map(|row| {
                                row.as_ref()
                                    .unwrap()
                                    .function
                                    .body
                                    .as_ref()
                                    .unwrap()
                                    .blocks
                                    .len()
                            })
                            .sum::<usize>()
                            + 2 * (slots.len() - 1)
                    }
                    2 => slots
                        .iter()
                        .map(|row| row.as_ref().unwrap().statement_operation_spans.len())
                        .sum(),
                    3 => slots
                        .iter()
                        .flat_map(|row| {
                            &row.as_ref().unwrap().function.body.as_ref().unwrap().blocks
                        })
                        .map(|block| block.operations.len())
                        .sum(),
                    _ => unreachable!(),
                };
                assert!(exact > 0);
                let limit = exact - usize::from(short);
                let resource = match axis {
                    0 => {
                        limits.max_functions = limit;
                        ProductionSemanticKirResourceV1::Functions
                    }
                    1 => {
                        limits.max_blocks = limit;
                        ProductionSemanticKirResourceV1::Blocks
                    }
                    2 => {
                        limits.max_statements = limit;
                        ProductionSemanticKirResourceV1::Statements
                    }
                    3 => {
                        limits.max_operations = limit;
                        ProductionSemanticKirResourceV1::Operations
                    }
                    _ => unreachable!(),
                };
                let before = slot_snapshot(slots);
                let floor = budget.storage();
                match assemble_pending_scoped_root_v29(instances, slots, limits, budget) {
                    Ok(pending) => {
                        assert!(!short);
                        drop_pending(pending, budget);
                    }
                    Err(ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: actual_resource,
                        actual,
                        limit: actual_limit,
                    }) => {
                        assert!(short);
                        assert_eq!(actual_resource, resource);
                        assert_eq!(actual, exact);
                        assert_eq!(actual_limit, limit);
                        assert_eq!(slot_snapshot(slots), before);
                    }
                    Err(error) => panic!("unexpected limit result: {error:?}"),
                }
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn pending_root_work_and_storage_failures_restore_the_preexisting_floor() {
    let mut exact_work = 0;
    let mut retained = 0;
    with_emitted(true, |instances, slots, budget| {
        let work = budget.work();
        let pending = assemble_pending_scoped_root_v29(
            instances,
            slots,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )
        .unwrap();
        exact_work = budget.work() - work;
        retained = pending.additional_storage_bytes;
        drop_pending(pending, budget);
    });
    let mut saw_destructive_refusal = false;
    for allowed in [0, 1, exact_work / 2, exact_work - 1, exact_work] {
        with_emitted(true, |instances, slots, budget| {
            budget.charge_work(LIMIT - budget.work() - allowed).unwrap();
            let floor = budget.storage();
            match assemble_pending_scoped_root_v29(
                instances,
                slots,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            ) {
                Ok(pending) => {
                    assert_eq!(allowed, exact_work);
                    drop_pending(pending, budget);
                }
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_),
                )) => {
                    assert!(allowed < exact_work);
                    saw_destructive_refusal |= slots.iter().any(Option::is_none);
                }
                Err(error) => panic!("unexpected work result: {error:?}"),
            }
            assert_eq!(budget.storage(), floor);
        });
    }
    assert!(saw_destructive_refusal);
    for allowed in [0, retained - 1] {
        with_emitted(true, |instances, slots, budget| {
            let reserve = LIMIT - budget.storage() - allowed;
            budget.reserve_storage(reserve).unwrap();
            let floor = budget.storage();
            assert!(matches!(
                assemble_pending_scoped_root_v29(
                    instances,
                    slots,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                ),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_),
                    )
                )
            ));
            assert_eq!(budget.storage(), floor);
            budget.release_storage(reserve).unwrap();
        });
    }
}

#[test]
fn pending_capability_union_prepays_variable_width_comparisons() {
    use fe2o3_kernel_ir::TargetCapability;
    let run = |work_limit, storage_limit| {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        let mut caller = BTreeSet::from([TargetCapability::Extension {
            namespace: "scope-test".repeat(32),
            name: "caller".into(),
        }]);
        let mut callee = BTreeSet::from([TargetCapability::Extension {
            namespace: "scope-test".repeat(32),
            name: "callee".into(),
        }]);
        let before = (caller.clone(), callee.clone());
        let result = merge_pending_scope_capabilities_v29(&mut caller, &mut callee, &mut budget);
        if result.is_err() {
            assert_eq!((caller, callee), before);
        } else {
            assert!(callee.is_empty());
            assert_eq!(caller.len(), 2);
        }
        (result, budget.work(), budget.storage())
    };
    let (result, work, storage) = run(LIMIT, LIMIT);
    result.unwrap();
    run(work, storage).0.unwrap();
    assert!(matches!(
        run(work - 1, storage).0,
        Err(ArgumentResourceV1::Work(_))
    ));
    assert!(matches!(
        run(work, storage - 1).0,
        Err(ArgumentResourceV1::Storage(_))
    ));
}
