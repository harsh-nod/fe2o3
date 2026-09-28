include!("production_optimized_source_global_native_fixture_sources_v18_tests.rs");

#[test]
fn pending_global_source_native_genuine_optimizer_load_store_and_rhs_join() {
    for entrance in [
        DescriptorRoleEntranceV18::OriginalUniqueBorrow,
        DescriptorRoleEntranceV18::IssuedDisjointSlice,
    ] {
        for mode in [
            DescriptorRoleSourceV18::Constant,
            DescriptorRoleSourceV18::ReadValue,
            DescriptorRoleSourceV18::Arithmetic,
        ] {
            let counts = std::cell::Cell::new([0usize; 2]);
            let entered = std::cell::Cell::new(false);
            let result =
                run_global_native_join_source_v18(entrance, mode, |original, optimized, budget| {
                    entered.set(true);
                    slice_view_v1::test_pending_global_native_positive_v18(
                        original,
                        optimized,
                        budget,
                        &counts,
                        matches!(mode, DescriptorRoleSourceV18::ReadValue),
                        matches!(mode, DescriptorRoleSourceV18::Arithmetic),
                    )
                })
                .0;
            if matches!(entrance, DescriptorRoleEntranceV18::OriginalUniqueBorrow) {
                assert!(!entered.get(), "{entrance:?}/{mode:?}: {result:?}");
                assert_eq!(counts.get(), [0, 0]);
                assert!(
                    matches!(
                        result,
                        Err(ProductionSourceOwnedViewErrorV18::Source(
                            ProductionPendingScopedSourceErrorV29::Source(
                                ProductionSemanticKirErrorV1::Unsupported {
                                    function: 0,
                                    block: None,
                                    statement: None,
                                    detail: "kernel argument ABI profile differs from the complete original descriptor/source contract",
                                }
                            )
                        ))
                    ),
                    "{entrance:?}/{mode:?}: {result:?}"
                );
                continue;
            }
            assert!(entered.get(), "{entrance:?}/{mode:?}: {result:?}");
            assert!(result.is_ok(), "{entrance:?}/{mode:?}: {result:?}");
            result.as_ref().unwrap();
            let expected = if matches!(mode, DescriptorRoleSourceV18::Arithmetic) {
                [1, 0] // The actual Store remains; its arithmetic RHS recipe is pending.
            } else {
                [1, 1]
            };
            assert_eq!(counts.get(), expected, "{entrance:?}/{mode:?}: {result:?}");
        }
    }
}

#[test]
fn pending_global_source_native_conservatively_preserves_genuine_load_store_back() {
    let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::ReadValue);
    let statements = owner.source_semantic().functions()[0].blocks()[3].statements();
    assert_eq!(statements.len(), 3);
    let SemanticStatementKindV1::Assign(read) = statements[1].kind() else {
        panic!("original load assignment");
    };
    let SemanticRvalueKindV1::Load(load) = read.value().kind() else {
        panic!("original explicit load");
    };
    let SemanticStatementKindV1::Store(store) = statements[2].kind() else {
        panic!("original explicit store");
    };
    assert_eq!(load.source(), store.destination());
    assert_eq!(
        store.value(),
        &SemanticOperandV1::Copy(read.destination().clone())
    );
    let abi = issued_descriptor_role_abi_v18(&owner);
    let counts = std::cell::Cell::new([0usize; 2]);
    let entered = std::cell::Cell::new(false);
    let result = run_descriptor_role_owner_with_abi_v18(
        owner,
        abi,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            entered.set(true);
            slice_view_v1::test_pending_global_native_positive_v18(
                original, optimized, budget, &counts, false, false,
            )
        },
    )
    .0;
    assert!(entered.get(), "{result:?}");
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        counts.get(),
        [1, 1],
        "current optimizer conservatively retains the actual Load and same-pointer Store"
    );
}

#[test]
fn pending_global_source_native_rejects_changed_rhs_pointer_index_root_guard_and_direction() {
    for fault in 0..7 {
        let reached = std::cell::Cell::new(false);
        let result = run_descriptor_roles_v18(
            DescriptorRoleEntranceV18::IssuedDisjointSlice,
            DescriptorRoleSourceV18::Arithmetic,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_pending_global_native_mutations_v18(
                    original, optimized, budget, fault, &reached,
                )
            },
        )
        .0;
        assert!(reached.get(), "fault {fault}: {result:?}");
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
        ));
    }
}

#[test]
fn pending_global_source_native_no_debit_accessor_rejects_equal_but_foreign_correspondence() {
    let reached = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(
        DescriptorRoleEntranceV18::IssuedDisjointSlice,
        DescriptorRoleSourceV18::Constant,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            slice_view_v1::test_pending_global_native_foreign_owner_v18(
                original, optimized, budget, &reached,
            )
        },
    )
    .0;
    assert!(reached.get(), "{result:?}");
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(_))
    ));
}

#[test]
fn pending_global_source_native_successful_custody_accessor_is_zero_debit() {
    run_descriptor_roles_v18(
        DescriptorRoleEntranceV18::IssuedDisjointSlice,
        DescriptorRoleSourceV18::Constant,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            slice_view_v1::test_pending_global_native_accessor_v18(original, optimized, budget)
        },
    )
    .0
    .unwrap();
}

#[test]
fn pending_global_source_native_frame_refusal_survives_released_padding_and_retry() {
    let observed = std::cell::Cell::new(false);
    let entered = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(
        DescriptorRoleEntranceV18::IssuedDisjointSlice,
        DescriptorRoleSourceV18::ReadValue,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            slice_view_v1::test_pending_global_native_frame_retry_v18(
                original, optimized, budget, &observed, &entered,
            )
        },
    )
    .0;
    assert!(observed.get(), "genuine composed scope must run");
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
}

#[test]
fn pending_global_source_native_foreign_replacement_never_refunds_same_or_higher_credits() {
    for extra in [0, 17] {
        for disposition in 0..3 {
            let observed = std::cell::Cell::new(false);
            let outer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = run_descriptor_roles_v18(
                    DescriptorRoleEntranceV18::IssuedDisjointSlice,
                    DescriptorRoleSourceV18::ReadValue,
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                    MODULE_LIMIT,
                    |original, optimized, budget| {
                        slice_view_v1::test_pending_global_native_foreign_budget_v18(
                            MODULE_LIMIT,
                            original,
                            optimized,
                            budget,
                            extra,
                            disposition,
                            &observed,
                        )
                    },
                );
            }));
            assert!(outer.is_err());
            assert!(
                observed.get(),
                "child custody assertions must execute before fixture teardown"
            );
        }
    }
}

#[test]
fn pending_global_source_native_genuine_shared_slice_read_joins_without_write_authority() {
    let reads = std::cell::Cell::new(0usize);
    run_descriptor_role_owner_v18(
        descriptor_source_owner(DescriptorCase::READ),
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            slice_view_v1::test_pending_global_native_shared_read_v18(
                original, optimized, budget, &reads,
            )
        },
    )
    .0
    .unwrap();
    assert!(reads.get() > 0);
}

#[test]
fn pending_global_source_native_keeps_both_normalized_and_emitted_index_identity() {
    for physical in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result = run_descriptor_role_owner_v18(
            global_native_two_index_shared_owner_v18(),
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_pending_global_native_shared_index_mutation_v18(
                    original, optimized, budget, physical, &reached,
                )
            },
        ).0;
        assert!(reached.get(), "physical={physical}: {result:?}");
        assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(
            "pending global native substituted checked source pair"
        ))), "physical={physical}: {result:?}");
    }
}
