#[test]
fn original_activation_unit_queries_reuse_constant_scratch_without_exporting_borrows() {
    with_address_plan(AddressFlow::Read, |plan, budget| {
        scoped_raw_admission_v29::check_object_activation_scratch_scope_v29(plan, budget, 0)
    })
    .unwrap();
}

#[test]
fn original_activation_unit_query_semantic_error_and_panic_restore_exact_floor() {
    for mode in [1, 2] {
        with_address_plan(AddressFlow::Read, |plan, budget| {
            scoped_raw_admission_v29::check_object_activation_scratch_scope_v29(plan, budget, mode)
        })
        .unwrap();
    }
}

#[test]
fn original_activation_unit_query_partial_denials_and_foreign_owner_replay_first_resource() {
    for mode in [3, 4, 5] {
        let entered = std::cell::Cell::new(false);
        let error = with_address_plan(AddressFlow::Read, |plan, budget| {
            entered.set(true);
            scoped_raw_admission_v29::check_object_activation_scratch_scope_v29(plan, budget, mode)
        })
        .unwrap_err();
        assert!(entered.get());
        assert!(matches!(
            (mode, error),
            (
                3,
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                )
            ) | (
                4,
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                )
            ) | (
                5,
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
    }
}

#[test]
fn original_activation_unit_query_higher_floor_loss_denies_refund_after_error_or_panic() {
    for mode in [6, 7] {
        let entered = std::cell::Cell::new(false);
        let error = with_address_plan(AddressFlow::Read, |plan, budget| {
            entered.set(true);
            scoped_raw_admission_v29::check_object_activation_scratch_scope_v29(plan, budget, mode)
        })
        .unwrap_err();
        assert!(entered.get());
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        ));
    }
}
