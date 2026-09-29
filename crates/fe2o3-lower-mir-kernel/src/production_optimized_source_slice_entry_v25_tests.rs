fn run_slice_entry_fixture_v25(
    exclusive: bool,
    check_shapes: bool,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, [usize; 2], bool) {
    let counts = std::cell::Cell::new([0; 2]);
    let completed = std::cell::Cell::new(false);
    let consume = |original: &ProductionSourceCorrespondenceV18<'_>,
                   optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                   budget: &mut ArgumentBudgetV1<'_>| {
        slice_view_v1::test_slice_entry_regions_v25(
            original,
            optimized,
            budget,
            exclusive,
            check_shapes,
            &counts,
            &completed,
        )
    };
    let (result, used, peak) = if exclusive {
        let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic);
        let abi = issued_descriptor_role_abi_v18(&owner);
        run_descriptor_role_owner_with_abi_v18(owner, abi, work, storage, consume)
    } else {
        run_descriptor_role_owner_v18(
            descriptor_source_owner(DescriptorCase::READ),
            work,
            storage,
            consume,
        )
    };
    (result, used, peak, counts.get(), completed.get())
}

#[test]
fn slice_entry_contract_joins_actual_shared_and_disjoint_source_native_accesses() {
    for exclusive in [false, true] {
        let (result, _, _, counts, completed) = run_slice_entry_fixture_v25(
            exclusive,
            false,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(result.is_ok(), "exclusive={exclusive}: {result:?}");
        assert!(completed);
        assert_eq!(counts, if exclusive { [1, 1] } else { [1, 0] });
    }
}

#[test]
fn slice_entry_contract_refuses_wrong_type_ordinal_address_space_and_access() {
    for exclusive in [false, true] {
        let (result, _, _, counts, completed) = run_slice_entry_fixture_v25(
            exclusive,
            true,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(result.is_ok(), "exclusive={exclusive}: {result:?}");
        assert!(completed);
        assert_eq!(counts, if exclusive { [1, 1] } else { [1, 0] });
    }
}

#[test]
fn slice_entry_contract_complete_scope_preserves_exact_and_one_short_limits() {
    for exclusive in [false, true] {
        let (result, used, peak, counts, completed) = run_slice_entry_fixture_v25(
            exclusive,
            false,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        result.unwrap();
        assert!(completed);
        let (result, exact_used, exact_peak, exact_counts, completed) =
            run_slice_entry_fixture_v25(exclusive, false, used, peak);
        result.unwrap();
        assert!(completed);
        assert_eq!((exact_used, exact_peak, exact_counts), (used, peak, counts));
        let (result, accepted, _, _, _) =
            run_slice_entry_fixture_v25(exclusive, false, used - 1, peak);
        let resource =
            source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err());
        assert!(
            matches!(resource, ArgumentResourceV1::Work(error)
            if error.actual() == used && error.limit() == used - 1),
            "{resource:?}"
        );
        assert!(accepted < used);
        let (result, _, accepted_peak, _, _) =
            run_slice_entry_fixture_v25(exclusive, false, used, peak - 1);
        let resource =
            source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err());
        assert!(
            matches!(resource, ArgumentResourceV1::Storage(error)
            if error.actual() == peak && error.limit() == peak - 1),
            "{resource:?}"
        );
        assert!(accepted_peak < peak);
    }
}

#[test]
fn slice_entry_scoped_index_reuses_actual_source_nodes_for_all_accesses() {
    for exclusive in [false, true] {
        let counts = std::cell::Cell::new([0; 2]);
        let completed = std::cell::Cell::new(false);
        let consume = |original: &ProductionSourceCorrespondenceV18<'_>,
                       optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                       budget: &mut ArgumentBudgetV1<'_>| {
            slice_view_v1::test_slice_entry_regions_indexed_v26(
                original, optimized, budget, exclusive, &counts, &completed,
            )
        };
        let result = if exclusive {
            let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic);
            let abi = issued_descriptor_role_abi_v18(&owner);
            run_descriptor_role_owner_with_abi_v18(
                owner,
                abi,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                consume,
            )
            .0
        } else {
            run_descriptor_role_owner_v18(
                descriptor_source_owner(DescriptorCase::READ),
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                consume,
            )
            .0
        };
        result.unwrap();
        assert!(completed.get());
        assert_eq!(counts.get(), if exclusive { [1, 1] } else { [1, 0] });
    }
}

fn run_slice_index_refusal_v26(fault: u8) -> SourceOwnedResultV18<()> {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(self.0);
        }
    }
    let _restore = Restore(DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.replace(None));
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = run_descriptor_role_owner_v18(
        descriptor_source_owner(DescriptorCase::READ),
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            let result = slice_view_v1::test_slice_entry_index_refusal_v26(
                original, optimized, budget, fault, &reached,
            );
            if original.source.cleanup.is_denied() {
                DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(Some(budget.storage()));
            }
            result
        },
    );
    assert!(reached.get(), "fault={fault}: {result:?}");
    result
}

#[test]
fn slice_entry_scoped_index_refuses_missing_slot_changed_slot_and_value() {
    for fault in 0..3 {
        assert!(matches!(
            run_slice_index_refusal_v26(fault),
            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
        ));
    }
}

#[test]
fn slice_entry_scoped_index_retains_ignored_foreign_budget_and_callback_refund_denials() {
    for fault in 3..5 {
        let error = run_slice_index_refusal_v26(fault).unwrap_err();
        assert!(matches!(
            source_slot_tests::original_repeated_source_resource_v29(error),
            ArgumentResourceV1::Accounting
        ));
    }
}

fn run_slice_completion_fixture_v25(
    exclusive: bool,
    extents: [u64; 3],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, [usize; 3]) {
    let observed = std::cell::Cell::new([0; 3]);
    let consume = |original: &ProductionSourceCorrespondenceV18<'_>,
                   optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                   budget: &mut ArgumentBudgetV1<'_>| {
        slice_view_v1::test_source_slice_completion_v25(
            original,
            optimized,
            budget,
            fe2o3_kernel_ir::ExplicitLaunchExtent::Exact { rank: 3, extents },
            width,
            &observed,
        )
    };
    let (result, used, peak) = if exclusive {
        let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic);
        let abi = issued_descriptor_role_abi_v18(&owner);
        run_descriptor_role_owner_with_abi_v18(owner, abi, work, storage, consume)
    } else {
        run_descriptor_role_owner_v18(
            descriptor_source_owner(DescriptorCase::READ),
            work,
            storage,
            consume,
        )
    };
    (result, used, peak, observed.get())
}

#[test]
fn slice_family_completion_consumes_actual_shared_reads_and_disjoint_read_modify_write() {
    for exclusive in [false, true] {
        let (result, _, _, observed) = run_slice_completion_fixture_v25(
            exclusive,
            [64, 1, 1],
            fe2o3_kernel_ir::FormalIndexWidth::Bits64,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(result.is_ok(), "exclusive={exclusive}: {result:?}");
        assert_eq!(observed, if exclusive { [1, 1, 1] } else { [1, 1, 0] });
    }
}

#[test]
fn slice_family_completion_refuses_unbound_width_and_nonsingleton_orthogonal_launch() {
    for (extents, width) in [
        ([64, 1, 1], fe2o3_kernel_ir::FormalIndexWidth::Unknown),
        ([64, 2, 1], fe2o3_kernel_ir::FormalIndexWidth::Bits64),
        ([64, 1, 2], fe2o3_kernel_ir::FormalIndexWidth::Bits64),
        (
            [(1_u64 << 32) + 1, 1, 1],
            fe2o3_kernel_ir::FormalIndexWidth::Bits32,
        ),
    ] {
        let (result, _, _, observed) = run_slice_completion_fixture_v25(
            true,
            extents,
            width,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(
            matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "slice Store invocation injectivity remains unproved"
                ))
            ),
            "{result:?}"
        );
        assert_eq!(observed, [0; 3]);
    }
}

#[test]
fn slice_family_completion_preserves_exact_and_one_short_whole_transaction_budgets() {
    let run = |work, storage| {
        run_slice_completion_fixture_v25(
            true,
            [64, 1, 1],
            fe2o3_kernel_ir::FormalIndexWidth::Bits64,
            work,
            storage,
        )
    };
    let (result, work, peak, observed) = run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert_eq!(observed, [1, 1, 1]);
    let (result, exact_work, exact_peak, exact_observed) = run(work, peak);
    result.unwrap();
    assert_eq!(
        (exact_work, exact_peak, exact_observed),
        (work, peak, observed)
    );
    let (result, accepted, _, _) = run(work - 1, peak);
    let resource = source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err());
    assert!(
        matches!(resource, ArgumentResourceV1::Work(error)
        if error.actual() == work && error.limit() == work - 1),
        "{resource:?}"
    );
    assert!(accepted < work);
    let (result, _, accepted_peak, _) = run(work, peak - 1);
    let resource = source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err());
    assert!(
        matches!(resource, ArgumentResourceV1::Storage(error)
        if error.actual() == peak && error.limit() == peak - 1),
        "{resource:?}"
    );
    assert!(accepted_peak < peak);
}
