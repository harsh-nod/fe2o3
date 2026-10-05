fn tile_scalar_headers_v156() -> usize {
    std::mem::size_of::<ProductionTileScalarFunctionV156<'_, '_>>()
        + std::mem::size_of::<ProductionTileScalarLoadV156<'_, '_, '_>>()
        + std::mem::size_of::<ProductionOptimizedSourceCfgRootV18<'_, '_>>()
        + tile_schedule_header_v155()
}

#[test]
fn explicit_source_tile_scalar_emits_actual_loads_with_launch_freshness_and_independent_replay() {
    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_tile_schedule_v155(&mut budget);
        with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            budget.reserve_storage(tile_scalar_headers_v156())?;
            let function = view.tile_scalar_function_v156(0, budget)?;
            let mut first = function.first_unused(budget)?;
            let mut loads = 0;
            for row in view.input_inventory(budget)?.operations() {
                if !matches!(
                    row.operation.kind,
                    OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 { .. })
                ) {
                    continue;
                }
                let schedule = view.tile_load_schedule_v155(row.coordinate, layout, budget)?;
                let load = function.load(&schedule, first, budget)?;
                assert_eq!(load.effect_sites(budget)?, schedule.effect_sites(budget)?);
                let count = load.operation_count(budget)?;
                let retained = count
                    * (std::mem::size_of::<fe2o3_kernel_ir::Operation>()
                        + 2 * std::mem::size_of::<ValueDef>()
                        + std::mem::size_of::<Type>());
                budget.reserve_storage(retained)?;
                let mut operations = Vec::with_capacity(count);
                for position in 0..count {
                    load.with_operation(position, budget, |operation, _| {
                        operations.push(operation.clone());
                        Ok(())
                    })?;
                }
                load.check_replacement(&operations, budget)?;
                let (value, mask) = load.component(0, budget)?;
                assert!(value.0 >= first.0 && mask.0 >= first.0);
                let next = load.next_value(budget)?;
                assert!(next.0 > first.0);
                first = next;
                drop(operations);
                budget.release_storage(retained)?;
                loads += 1;
            }
            assert!(loads >= 2);
            drop(function);
            budget.release_storage(tile_scalar_headers_v156())?;
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn explicit_source_tile_scalar_refuses_existing_ssa_definition_and_retains_denial() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_tile_schedule_v155(&mut budget);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        budget.reserve_storage(tile_scalar_headers_v156())?;
        let function = view.tile_scalar_function_v156(0, budget)?;
        let first = function.first_unused(budget)?;
        assert!(first.0 > 0);
        let row = view
            .input_inventory(budget)?
            .operations()
            .iter()
            .find(|row| {
                matches!(
                    row.operation.kind,
                    OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 { .. })
                )
            })
            .unwrap();
        let schedule =
            view.tile_load_schedule_v155(row.coordinate, ExecutionTileLayoutV1::Striped, budget)?;
        let error = match function.load(&schedule, ValueId(first.0 - 1), budget) {
            Ok(_) => panic!("overlapping SSA range admitted"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ProductionSourceOwnedViewErrorV18::Binding(_)
        ));
        assert!(function.first_unused(budget).is_err());
        Err::<(), _>(error)
    });
    assert!(result.is_err());
}

#[test]
fn explicit_source_tile_scalar_rejects_refunded_live_emission_storage() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_tile_schedule_v155(&mut budget);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        budget.reserve_storage(tile_scalar_headers_v156())?;
        let function = view.tile_scalar_function_v156(0, budget)?;
        let row = view
            .input_inventory(budget)?
            .operations()
            .iter()
            .find(|row| {
                matches!(
                    row.operation.kind,
                    OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 { .. })
                )
            })
            .unwrap();
        let schedule =
            view.tile_load_schedule_v155(row.coordinate, ExecutionTileLayoutV1::Blocked, budget)?;
        let load = function.load(&schedule, function.first_unused(budget)?, budget)?;
        let error = load
            .with_operation(0, budget, |_, budget| {
                budget.release_storage(1)?;
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(
            error,
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
        ));
        assert!(load.operation_count(budget).is_err());
        Err::<(), _>(error)
    });
    assert!(result.is_err());
}
