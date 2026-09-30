pub(super) fn test_source_domain_width_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    scalar: ScalarType,
    default_true: bool,
    fault: Option<(bool, u8)>,
    observed: &std::cell::Cell<[usize; 2]>,
) -> SourceOwnedResultV18<()> {
    original.with_pending_global_accesses_v18(optimized, 0, budget, |source, budget| {
        with_local_read_native_test_view_v18(original, optimized, budget, |native, budget| {
            let inventory = optimized.output_inventory(budget)?;
            let limits = fe2o3_kernel_ir::CanonicalGuardedGlobalReadLimitsV1::default();
            let formal = fe2o3_kernel_ir::with_canonical_guarded_global_reads_v18(
                inventory.owner(), limits, budget, |reads, budget| {
                    fe2o3_kernel_ir::with_canonical_guarded_global_stores_v24(
                        inventory.owner(), limits, budget, |stores, budget| {
                            Ok((|| -> SourceOwnedResultV18<()> {
                                let read = source.roles.rows.iter().find_map(|row|
                                    row.global.as_ref().filter(|pair| !pair.output.writing))
                                    .expect("genuine source read");
                                let store = source.roles.rows.iter().find_map(|row|
                                    row.global.as_ref().filter(|pair| pair.output.writing))
                                    .expect("genuine source Store");
                                let floor = budget.storage();
                                for pair in [read, store] {
                                    let condition = optimized_source_definition_row_v18(inventory, pair.output.logical.guard_condition, budget)?;
                                    assert_eq!(condition.ty, &Type::Scalar(scalar));
                                    let guard = source_block_row_v18(inventory, pair.output.logical.guard_edge.source, budget)?;
                                    let Some(Terminator::Switch { selector, cases, .. }) = guard.block.terminator.as_ref() else {
                                        panic!("source must emit existing legacy Switch, never IntegerSwitch");
                                    };
                                    assert_eq!(condition.value, Some(*selector));
                                    assert_eq!(cases.len(), 1);
                                    assert_eq!(cases[0].value, u64::from(!default_true));
                                    assert_eq!(pair.output.logical.guard_edge.successor, u32::from(default_true));
                                    let SliceDefinition::Result { operation: cast, result: 0 } = condition.coordinate else {
                                        panic!("source discriminant result");
                                    };
                                    let cast = source_operation_row_v18(inventory, cast, budget)?.operation;
                                    let OperationKind::Cast { kind: fe2o3_kernel_ir::CastKind::ZeroExtend, value, to } = &cast.kind else {
                                        panic!("source Bool zero extension");
                                    };
                                    assert_eq!(to, condition.ty);
                                    let predicate = inventory.definition_for_value(pair.output.logical.access.operation.block.function, *value, budget)
                                        .map_err(source_pointer_inventory_error_v18)?.expect("source predicate definition");
                                    assert_eq!(predicate.ty, &Type::BOOL);
                                    assert_ne!(predicate.value, condition.value);
                                    source.with_native_access_v18(native, pair.output.logical.access.operation, budget, |access, _| {
                                        assert!(std::ptr::eq(access.expect("actual native access").pair, pair));
                                        Ok(())
                                    }).map_err(slice_completion_native_error_v25)?;
                                }
                                source.with_local_read_conditions_v18(native, reads, read.output.logical.access.operation, budget, |joined, _| {
                                    assert!(std::ptr::eq(joined.expect("actual read join").pair, read));
                                    Ok(())
                                }).map_err(local_read_test_error_v18)?;
                                let fe2o3_kernel_ir::CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(store_fact) = stores
                                    .store_at(store.output.logical.access.operation, budget)
                                    .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?
                                    else { panic!("actual Store formal fact"); };
                                source.check_local_store_domain_v25(stores, store, &store_fact, budget)?;
                                assert_eq!(budget.storage(), floor);
                                observed.set([1, 1]);
                                let Some((writing, fault)) = fault else { return Ok(()); };
                                let (pair, other) = if writing { (store, read) } else { (read, store) };
                                let condition = optimized_source_definition_row_v18(inventory, pair.output.logical.guard_condition, budget)?;
                                let other_condition = optimized_source_definition_row_v18(inventory, other.output.logical.guard_condition, budget)?;
                                assert_eq!(condition.ty, other_condition.ty);
                                assert_ne!(condition.value, other_condition.value);
                                let SliceDefinition::Result { operation: cast, result: 0 } = condition.coordinate else { unreachable!(); };
                                let OperationKind::Cast { value: predicate, .. } = source_operation_row_v18(inventory, cast, budget)?.operation.kind else { unreachable!(); };
                                let predicate = inventory.definition_for_value(pair.output.logical.access.operation.block.function, predicate, budget)
                                    .map_err(source_pointer_inventory_error_v18)?.expect("actual predicate");
                                let mut changed = *pair;
                                match fault {
                                    0 => changed.output.logical.guard_condition = other_condition.coordinate,
                                    1 => changed.output.logical.guard_condition = predicate.coordinate,
                                    2 => changed.output.logical.guard_edge = other.output.logical.guard_edge,
                                    3 => changed.output.logical.guard_edge.successor = 1,
                                    _ => unreachable!(),
                                }
                                let headers = if writing { slice_store_domain_headers_v25()? } else { global_read_condition_headers_v18(0, 1)? };
                                budget.reserve_storage(headers)?;
                                let result = if writing {
                                    check_source_store_endpoint_prepaid_v30(original, inventory, stores, &changed, &store_fact, budget)
                                } else {
                                    let fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(fact) = reads.read_at(read.output.logical.access.operation, budget)
                                        .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?
                                        else { panic!("actual read formal fact"); };
                                    check_source_read_endpoint_prepaid_v30(original, inventory, reads, &changed, &fact, budget)
                                        .map_err(local_read_test_error_v18)
                                };
                                budget.release_storage(headers)?;
                                assert_eq!(budget.storage(), floor);
                                result
                            })())
                        },
                    )
                },
            );
            formal.map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?
        })
    })
}
