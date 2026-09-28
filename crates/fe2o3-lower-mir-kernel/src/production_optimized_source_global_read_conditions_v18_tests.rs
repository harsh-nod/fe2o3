fn run_local_read_condition_fixture_v18(
    mode: slice_view_v1::GlobalReadConditionTestV18,
    source: DescriptorRoleSourceV18,
    work: usize,
    storage: usize,
    observed: &std::cell::Cell<[usize; 3]>,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    let owner = match source {
        DescriptorRoleSourceV18::ReadValue => global_native_cross_descriptor_copy_owner_v18(),
        _ => issued_descriptor_role_owner_v18(source),
    };
    let abi = issued_descriptor_role_abi_v18(&owner);
    run_descriptor_role_owner_with_abi_v18(
        owner,
        abi,
        work,
        storage,
        |original, optimized, budget| {
            slice_view_v1::test_pending_global_read_conditions_v18(
                original, optimized, budget, work, mode, observed,
            )
        },
    )
}

#[test]
fn pending_global_read_conditions_observe_exact_issued_bool_switch_transport() {
    let observed = std::cell::Cell::new([0; 3]);
    run_local_read_condition_fixture_v18(
        slice_view_v1::GlobalReadConditionTestV18::IssuedGuardShape,
        DescriptorRoleSourceV18::ReadValue,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        &observed,
    )
    .0
    .unwrap();
    assert_eq!(observed.get(), [1, 1, 1]);
}

#[test]
fn pending_global_read_conditions_reject_actual_selector_operand_and_edge_substitutions() {
    for fault in 0..5 {
        let observed = std::cell::Cell::new([0; 3]);
        let result = run_local_read_condition_fixture_v18(
            slice_view_v1::GlobalReadConditionTestV18::ControlTransport(fault),
            DescriptorRoleSourceV18::ReadValue,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            &observed,
        )
        .0;
        assert_eq!(
            observed.get(),
            [1, 1, 1],
            "genuine positive must precede fault {fault}: {result:?}"
        );
        let expected = if fault == 3 {
            "pending global read changed exact local domain"
        } else {
            "pending global read changed exact control transport"
        };
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(detail)) if detail == expected),
            "fault {fault}: {result:?}"
        );
    }
}

#[test]
fn pending_global_read_conditions_join_genuine_issued_reads_and_leave_stores_pending() {
    for source in [
        DescriptorRoleSourceV18::ReadValue,
        DescriptorRoleSourceV18::Arithmetic,
    ] {
        let observed = std::cell::Cell::new([0; 3]);
        run_local_read_condition_fixture_v18(
            slice_view_v1::GlobalReadConditionTestV18::Positive,
            source,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            &observed,
        )
        .0
        .unwrap();
        assert_eq!(observed.get()[0], 4, "four repeated exact read queries");
        assert!(
            observed.get()[1] > 0,
            "genuine Store is not a completed read"
        );
    }
}

#[test]
fn pending_global_read_conditions_fresh_owners_have_equal_complete_transaction_work() {
    let mut previous: Option<(usize, usize, [usize; 3])> = None;
    for _ in 0..8 {
        let observed = std::cell::Cell::new([0; 3]);
        let (result, work, peak) = run_local_read_condition_fixture_v18(
            slice_view_v1::GlobalReadConditionTestV18::Positive,
            DescriptorRoleSourceV18::ReadValue,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            &observed,
        );
        result.unwrap();
        assert_eq!(observed.get(), [4, 1, 0]);
        let actual = (work, peak, observed.get());
        if let Some(expected) = previous {
            assert_eq!(actual, expected);
        }
        previous = Some(actual);
    }
}

#[test]
fn pending_global_read_conditions_preserve_exact_full_transaction_limits() {
    let run = |work, storage| {
        let observed = std::cell::Cell::new([0; 3]);
        let result = run_local_read_condition_fixture_v18(
            slice_view_v1::GlobalReadConditionTestV18::Positive,
            DescriptorRoleSourceV18::ReadValue,
            work,
            storage,
            &observed,
        );
        (result, observed.get())
    };
    let ((result, work, peak), count) = run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert_eq!(count[0], 4);
    let ((result, exact_work, exact_peak), exact_count) = run(work, peak);
    result.unwrap();
    assert_eq!((exact_work, exact_peak, exact_count), (work, peak, count));
    let ((result, accepted, _accepted_peak), short_count) = run(work - 1, peak);
    let resource = source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err());
    assert!(
        matches!(resource, ArgumentResourceV1::Work(error)
        if error.limit() == work - 1 && error.actual() == work),
        "work={work} peak={peak} accepted={accepted} observed={short_count:?}: {resource:?}"
    );
    assert!(accepted <= work - 1);
    let ((result, _accepted, accepted_peak), short_count) = run(work, peak - 1);
    let resource = source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err());
    assert!(
        matches!(resource, ArgumentResourceV1::Storage(error)
        if error.limit() == peak - 1 && error.actual() == peak),
        "work={work} peak={peak} accepted_peak={accepted_peak} observed={short_count:?}: {resource:?}"
    );
    assert!(accepted_peak <= peak - 1);
}

#[test]
fn pending_global_read_conditions_join_real_assertion_representation_chain() {
    let observed = std::cell::Cell::new([0; 3]);
    let result = run_descriptor_role_owner_v18(
        descriptor_source_owner(DescriptorCase::READ),
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            slice_view_v1::test_pending_global_read_conditions_v18(
                original,
                optimized,
                budget,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                slice_view_v1::GlobalReadConditionTestV18::NormalizedPositive,
                &observed,
            )
        },
    )
    .0;
    assert_eq!(
        observed.get(),
        [1, 0, 0],
        "authentic assertion, native transport and normalized formal fact must join"
    );
    result.unwrap();
}

#[test]
fn pending_global_read_conditions_normalized_complete_scope_exact_and_one_short_limits() {
    let run = |work_limit, storage_limit| {
        let observed = std::cell::Cell::new([0; 3]);
        let result = run_descriptor_role_owner_v18(
            descriptor_source_owner(DescriptorCase::READ),
            work_limit,
            storage_limit,
            |original, optimized, budget| {
                slice_view_v1::test_pending_global_read_conditions_v18(
                    original,
                    optimized,
                    budget,
                    work_limit,
                    slice_view_v1::GlobalReadConditionTestV18::NormalizedPositive,
                    &observed,
                )
            },
        );
        (result, observed.get())
    };
    let ((result, work, peak), observed) = run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert_eq!(observed, [1, 0, 0]);
    let ((result, exact_work, exact_peak), exact_observed) = run(work, peak);
    result.unwrap();
    assert_eq!(
        (exact_work, exact_peak, exact_observed),
        (work, peak, observed)
    );
    let ((result, accepted, _), _) = run(work - 1, peak);
    assert!(
        matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error)))
        if error.limit() == work - 1 && error.actual() == work)
    );
    assert!(accepted <= work - 1);
    let ((result, _, accepted_peak), _) = run(work, peak - 1);
    assert!(
        matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)))
        if error.limit() == peak - 1 && error.actual() == peak)
    );
    assert!(accepted_peak <= peak - 1);
}

#[test]
fn pending_global_read_conditions_normalized_same_type_index_pointer_and_guard_substitutions() {
    for fault in 0..5 {
        let observed = std::cell::Cell::new([0; 3]);
        let result = run_descriptor_role_owner_v18(
            global_native_two_index_shared_owner_v18(),
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_pending_global_read_conditions_v18(
                    original,
                    optimized,
                    budget,
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                    slice_view_v1::GlobalReadConditionTestV18::NormalizedDomain(fault),
                    &observed,
                )
            },
        )
        .0;
        assert_eq!(
            observed.get(),
            [2, 1, 0],
            "both genuine reads precede fault {fault}: {result:?}"
        );
        let expected = if matches!(fault, 2 | 3) {
            "pending global read changed exact control transport"
        } else {
            "pending global read changed exact local domain"
        };
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(detail)) if detail == expected),
            "fault {fault}: {result:?}"
        );
    }
}

fn local_read_two_length_occurrences_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let base = global_native_two_index_shared_owner_v18();
    let original = &base.source_semantic().functions()[0];
    assert_eq!(original.blocks().len(), 4);
    let length = original.blocks()[0].statements()[0].clone();
    let SemanticStatementKindV1::Assign(assignment) = length.kind() else {
        panic!("genuine original length assignment");
    };
    assert_eq!(
        assignment.destination().local(),
        SemanticLocalIdV1::from_index(4)
    );
    assert!(matches!(
        assignment.value().kind(),
        SemanticRvalueKindV1::Length(_)
            | SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                ..
            }
    ));
    let mut blocks = original.blocks().to_vec();
    let mut statements = vec![length];
    statements.extend_from_slice(blocks[2].statements());
    blocks[2] = SemanticBasicBlockV1::new(
        blocks[2].identity(),
        blocks[2].source(),
        statements,
        blocks[2].terminator().clone(),
    )
    .unwrap();
    global_native_rebuild_owner_v18(&base, original.locals().to_vec(), blocks)
}

#[test]
fn pending_global_read_conditions_require_the_actual_same_root_length_occurrence() {
    let observed = std::cell::Cell::new([0; 3]);
    let result = run_descriptor_role_owner_v18(
        local_read_two_length_occurrences_owner_v18(),
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            slice_view_v1::test_pending_global_read_conditions_v18(
                original,
                optimized,
                budget,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                slice_view_v1::GlobalReadConditionTestV18::OriginalLengthOccurrence,
                &observed,
            )
        },
    )
    .0;
    assert_eq!(
        observed.get(),
        [1, 1, 1],
        "genuine native/output positive plus exact original-input local fact: {result:?}"
    );
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "pending global read changed exact local domain"
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn pending_global_read_conditions_refuse_same_type_roots_indices_pointers_and_other_domain_changes()
{
    for fault in 0..12 {
        let observed = std::cell::Cell::new([0; 3]);
        let result = run_local_read_condition_fixture_v18(
            slice_view_v1::GlobalReadConditionTestV18::ChangedDomain(fault),
            DescriptorRoleSourceV18::Arithmetic,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            &observed,
        )
        .0;
        assert_eq!(
            observed.get()[0],
            1,
            "authentic positive precedes copied row fault {fault}"
        );
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(_))),
            "fault {fault}: {result:?}"
        );
    }
}

#[test]
fn pending_global_read_conditions_reject_exact_foreign_formal_owner() {
    let observed = std::cell::Cell::new([0; 3]);
    let result = run_local_read_condition_fixture_v18(
        slice_view_v1::GlobalReadConditionTestV18::ForeignFacts,
        DescriptorRoleSourceV18::ReadValue,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        &observed,
    )
    .0;
    assert_eq!(observed.get(), [1, 0, 0]);
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "pending global read changed formal owner"
        ))
    ));
}

#[test]
fn pending_global_read_conditions_retain_header_and_work_refusals_without_retry_debit() {
    for mode in [
        slice_view_v1::GlobalReadConditionTestV18::HeaderCut,
        slice_view_v1::GlobalReadConditionTestV18::WorkCut,
    ] {
        let observed = std::cell::Cell::new([0; 3]);
        let result = run_local_read_condition_fixture_v18(
            mode,
            DescriptorRoleSourceV18::ReadValue,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            &observed,
        )
        .0;
        assert_eq!(observed.get(), [1, 0, 0]);
        match mode {
            slice_view_v1::GlobalReadConditionTestV18::HeaderCut => assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Storage(_)
                ))
            )),
            _ => assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Work(_)
                ))
            )),
        }
    }
}

#[test]
fn pending_global_read_conditions_preserve_selected_error_unwind_and_first_error_before_drop() {
    use slice_view_v1::GlobalReadConditionTestV18 as Mode;
    for mode in [Mode::SelectedError, Mode::Unwind, Mode::ErrorThenDropPanic] {
        let observed = std::cell::Cell::new([0; 3]);
        let result = run_local_read_condition_fixture_v18(
            mode,
            DescriptorRoleSourceV18::ReadValue,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            &observed,
        )
        .0;
        assert_eq!(observed.get(), [1, 0, 0]);
        if matches!(mode, Mode::ErrorThenDropPanic) {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Arithmetic
                ))
            ));
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn pending_global_read_conditions_keep_genuine_query_error_before_callback_drop_or_later_error() {
    use slice_view_v1::GlobalReadConditionTestV18 as Mode;
    for mode in [Mode::QueryThenDropPanic, Mode::SwallowedQueryThenError] {
        let observed = std::cell::Cell::new([0; 3]);
        let result = run_local_read_condition_fixture_v18(
            mode,
            DescriptorRoleSourceV18::ReadValue,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            &observed,
        )
        .0;
        assert_eq!(observed.get(), [1, 0, 0]);
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Work(_)
            ))
        ));
    }
}

#[test]
fn pending_global_read_conditions_deny_refund_after_higher_floor_loss_on_every_disposition() {
    for disposition in 0..3 {
        let observed = std::cell::Cell::new([0; 3]);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_local_read_condition_fixture_v18(
                slice_view_v1::GlobalReadConditionTestV18::HigherFloor(disposition),
                DescriptorRoleSourceV18::ReadValue,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                &observed,
            )
        }));
        assert_eq!(
            observed.get(),
            [1, 0, 0],
            "inner exact error/panic and denied refund assertions must run"
        );
        assert!(
            result.is_err(),
            "fixture's outer trusted floor cannot settle an intentionally damaged ledger"
        );
    }
}

#[test]
fn pending_global_read_conditions_never_refund_same_or_higher_foreign_ledger_credits() {
    for extra in [0, 17] {
        for disposition in 0..3 {
            let observed = std::cell::Cell::new([0; 3]);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_local_read_condition_fixture_v18(
                    slice_view_v1::GlobalReadConditionTestV18::ForeignLedger { extra, disposition },
                    DescriptorRoleSourceV18::ReadValue,
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                    MODULE_LIMIT,
                    &observed,
                )
            }));
            assert_eq!(
                observed.get(),
                [1, 0, 0],
                "no-debit/no-refund must be checked before fixture teardown"
            );
            assert!(result.is_err());
        }
    }
}

#[test]
fn pending_global_read_conditions_foreign_query_is_no_debit_and_denies_original_refund() {
    for extra in [0, 17] {
        let observed = std::cell::Cell::new([0; 3]);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_local_read_condition_fixture_v18(
                slice_view_v1::GlobalReadConditionTestV18::ForeignQuery(extra),
                DescriptorRoleSourceV18::ReadValue,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                &observed,
            )
        }));
        assert_eq!(observed.get(), [1, 0, 0]);
        assert!(
            result.is_err(),
            "the original scope's denied refund remains observable at its trusted floor"
        );
    }
}
