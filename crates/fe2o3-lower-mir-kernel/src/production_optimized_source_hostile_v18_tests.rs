#[test]
fn caught_invalid_source_site_remains_a_refusal_after_nominal_callback_success() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let called = std::cell::Cell::new(false);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        assert!(
            view.source_block(usize::MAX, 0, SemanticBlockIdV1::from_index(0), budget)
                .is_err()
        );
        assert!(view.output_inventory(budget).is_err());
        called.set(true);
        Ok(())
    });
    assert!(called.get());
    assert!(result.is_err());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn foreign_budget_slot_never_grants_optimized_source_authority() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let called = std::cell::Cell::new(false);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, _budget| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut foreign = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        foreign.reserve_storage(MODULE_LIMIT / 2)?;
        assert!(matches!(
            view.output_inventory(&mut foreign),
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        called.set(true);
        Ok(())
    });
    assert!(called.get());
    assert!(result.is_err());
    assert!(
        budget.storage() > MODULE_FLOOR,
        "foreign custody must deny refund"
    );
}
#[test]
fn same_owner_rederived_inventory_is_not_the_checked_input_inventory() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let denied = std::cell::Cell::new(false);
    let result =
        with_actual_optimized_transition_v18(prepared, &mut budget, |source, checked, budget| {
            let (foreign, receipt) = fe2o3_kernel_analysis::CanonicalKirInventoryV18::derive_v18(
                &source.owner.inner.pending.graph,
                budget,
            )
            .unwrap();
            budget.reserve_storage(receipt.retained_storage())?;
            assert!(foreign.belongs_to(checked.input().owner()));
            assert!(!std::ptr::eq(&foreign, checked.input()));
            let result =
                source.with_ranked_correspondence_v18(&foreign, budget, |original, budget| {
                    let result = original.with_optimized_correspondence_v18(
                        checked,
                        budget,
                        |_, _| -> SourceOwnedResultV18<()> {
                            panic!("foreign inventory must refuse before callback")
                        },
                    );
                    assert!(matches!(
                        result,
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "optimized source substituted its original inventory"
                        ))
                    ));
                    denied.set(true);
                    result
                });
            drop(foreign);
            budget.release_storage(receipt.retained_storage())?;
            result
        });
    assert!(denied.get());
    assert!(result.is_err());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_successor_cannot_replace_the_original_source_inventory() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let denied = std::cell::Cell::new(false);
    let result =
        with_actual_optimized_transition_v18(prepared, &mut budget, |source, checked, budget| {
            let result = source.with_ranked_correspondence_v18(
                checked.output(),
                budget,
                |_, _| -> SourceOwnedResultV18<()> {
                    panic!("successor must not acquire original source ownership")
                },
            );
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "foreign canonical inventory"
                ))
            ));
            denied.set(true);
            result
        });
    assert!(denied.get());
    assert!(result.is_err());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
#[test]
fn first_source_refusal_survives_selected_error_and_raw_unwind() {
    for panic_after in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let attempted = std::cell::Cell::new(false);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let invalid = OptimizedDefinition::FunctionArgument {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(u32::MAX),
                argument: 0,
            };
            assert!(matches!(
                view.definition_descendants(invalid, budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized source argument function"
                ))
            ));
            attempted.set(true);
            if panic_after {
                std::panic::panic_any(Box::new([7u8, 11, 13]));
            }
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "later selected callback error",
            ))
        });
        assert!(attempted.get());
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized source argument function"
            ))
        ));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
