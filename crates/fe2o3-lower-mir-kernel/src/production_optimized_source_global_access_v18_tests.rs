#[test]
fn pending_global_accesses_authenticate_exact_correspondence_before_new_debits() {
    let completed = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(
        DescriptorRoleEntranceV18::IssuedDisjointSlice,
        DescriptorRoleSourceV18::Constant,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            original.with_pending_global_accesses_v18(optimized, 0, budget, |view, budget| {
                assert!(view.operation_count(budget)? > 0);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })?;
            original.source.with_ranked_correspondence_v18(
                original.inventory,
                budget,
                |other, budget| {
                    assert!(!std::ptr::eq(original, other));
                    let before = (budget.work(), budget.storage());
                    let refused = other.with_pending_global_accesses_v18(
                        optimized,
                        0,
                        budget,
                        |_, _| -> SourceOwnedResultV18<()> {
                            panic!("foreign correspondence reached pending Global consumer");
                        },
                    );
                    assert!(matches!(
                        refused,
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "optimized source substituted its exact correspondence"
                        ))
                    ));
                    assert_eq!((budget.work(), budget.storage()), before);
                    completed.set(true);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )
        },
    )
    .0;
    assert!(completed.get(), "{result:?}");
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized source substituted its exact correspondence"
        ))
    ));
}

#[test]
fn pending_global_accesses_deny_refund_after_higher_floor_loss_on_all_dispositions() {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(self.0);
        }
    }
    let _restore = Restore(DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.replace(None));
    for mode in 0..3 {
        DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(None);
        let completed = std::cell::Cell::new(false);
        let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Constant);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let result = run_descriptor_role_owner_result_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                original.with_pending_global_accesses_v18(
                    optimized,
                    0,
                    budget,
                    |view, budget| {
                        assert!(view.operation_count(budget)? > 0);
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                    },
                )?;
                let parent_floor = budget.storage();
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    original.with_pending_global_accesses_v18(
                        optimized,
                        0,
                        budget,
                        |view, budget| -> SourceOwnedResultV18<()> {
                            assert!(view.operation_count(budget)? > 0);
                            let required = budget.storage();
                            assert!(required > parent_floor);
                            if mode == 2 {
                                budget.reserve_storage(size_of::<&'static str>())?;
                            }
                            // The extra panic payload is included in this hostile
                            // loss; the test must not hide any owned allocation.
                            budget.release_storage(budget.storage() - required + 1)?;
                            DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(Some(budget.storage()));
                            match mode {
                                0 => Ok(()),
                                1 => Err(ProductionSourceOwnedViewErrorV18::Binding(
                                    "pending Global callback sentinel",
                                )),
                                _ => std::panic::resume_unwind(Box::new(
                                    "pending Global callback panic",
                                )),
                            }
                        },
                    )
                }));
                assert_eq!(
                    Some(budget.storage()),
                    DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.get()
                );
                assert!(original.source.cleanup.is_denied());
                let selected = match (mode, caught) {
                    (
                        0,
                        Ok(Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Accounting,
                        ))),
                    ) => {
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                    }
                    (
                        1,
                        Ok(Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "pending Global callback sentinel",
                        ))),
                    ) => ProductionSourceOwnedViewErrorV18::Binding(
                        "pending Global callback sentinel",
                    ),
                    (2, Err(payload)) => {
                        assert_eq!(
                            *payload.downcast::<&'static str>().unwrap(),
                            "pending Global callback panic"
                        );
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "caught pending Global callback panic",
                        )
                    }
                    _ => panic!("pending Global custody disposition changed"),
                };
                completed.set(true);
                Err(selected)
            },
        )
        .0;
        assert!(completed.get(), "{result:?}");
        assert!(DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.get().unwrap() > MODULE_FLOOR);
        match (mode, result) {
            (
                0,
                Err(ProductionSourceOptimizationErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting),
                )),
            ) => {}
            (
                1,
                Err(ProductionSourceOptimizationErrorV18::Adoption(
                    fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "pending Global callback sentinel",
                        ),
                    ),
                )),
            ) => {}
            (
                2,
                Err(ProductionSourceOptimizationErrorV18::Adoption(
                    fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "caught pending Global callback panic",
                        ),
                    ),
                )),
            ) => {}
            (_, result) => panic!("pending Global outer custody disposition: {result:?}"),
        }
    }
}

fn pending_global_boundary_v18(
    issued: bool,
    disposition: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(self.0);
        }
    }
    let _restore = Restore(DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.replace(None));
    let completed = std::cell::Cell::new(false);
    let retained_floor = std::cell::Cell::new(None);
    let consume = |original: &ProductionSourceCorrespondenceV18<'_>,
                   optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                   budget: &mut ArgumentBudgetV1<'_>| {
        let result = slice_view_v1::test_pending_global_accesses_v18(
            original,
            optimized,
            issued,
            if issued { [1, 1] } else { [1, 0] },
            0,
            disposition,
            &completed,
            &retained_floor,
            budget,
        );
        if let Some(floor) = retained_floor.get() {
            DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(Some(floor));
        }
        result
    };
    let (result, work, peak) = if issued {
        run_descriptor_roles_v18(
            DescriptorRoleEntranceV18::IssuedDisjointSlice,
            DescriptorRoleSourceV18::Constant,
            work_limit,
            storage_limit,
            consume,
        )
    } else {
        run_descriptor_role_owner_v18(
            descriptor_source_owner(DescriptorCase::READ),
            work_limit,
            storage_limit,
            consume,
        )
    };
    (result, work, peak, completed.get())
}

#[test]
fn pending_global_accesses_retain_actual_source_output_guard_and_rhs_without_native_authority() {
    for issued in [false, true] {
        let result =
            pending_global_boundary_v18(issued, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(
            result.0.is_ok() && result.3,
            "issued={issued}: {:?}",
            result.0
        );
    }
}

#[test]
fn pending_global_accesses_keep_complete_transaction_exact_and_one_short_boundaries() {
    for issued in [false, true] {
        let measured =
            pending_global_boundary_v18(issued, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        measured.0.unwrap();
        assert!(measured.3);
        let exact = pending_global_boundary_v18(issued, 0, measured.1, measured.2);
        exact.0.unwrap();
        assert!(exact.3);
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        for (work, storage, is_work) in [
            (measured.1 - 1, measured.2, true),
            (measured.1, measured.2 - 1, false),
        ] {
            let short = pending_global_boundary_v18(issued, 0, work, storage);
            match (
                is_work,
                source_slot_tests::original_repeated_source_resource_v29(short.0.unwrap_err()),
            ) {
                (true, ArgumentResourceV1::Work(error)) => {
                    assert_eq!(error.limit(), work);
                    assert_eq!(error.actual(), work + 1);
                }
                (false, ArgumentResourceV1::Storage(error)) => {
                    assert_eq!(error.limit(), storage);
                    assert_eq!(error.actual(), storage + 1);
                }
                other => panic!("pending global resource boundary: {other:?}"),
            }
            assert!(short.1 <= work && short.2 <= storage);
        }
    }
}

#[test]
fn pending_global_accesses_preserve_same_candidate_descriptor_refusals_before_publication() {
    for fault in [
        DescriptorFault::Data,
        DescriptorFault::Extent,
        DescriptorFault::Guard,
    ] {
        let positive =
            pending_global_boundary_v18(false, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
        let _restore = DescriptorObservers::install(fault);
        let refused =
            pending_global_boundary_v18(false, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(!refused.3);
        assert_eq!(DESCRIPTOR_TAMPERED.get(), 1);
        assert!(
            matches!(refused.0, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })))
            if detail == "source runtime slice descriptor/index/extent correspondence differs")
        );
    }
}

#[test]
fn pending_global_accesses_preserve_selected_error_and_foreign_ledger_custody() {
    for issued in [false, true] {
        let positive =
            pending_global_boundary_v18(issued, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
        let owner = if issued {
            issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Constant)
        } else {
            descriptor_source_owner(DescriptorCase::READ)
        };
        let abi = if issued {
            issued_descriptor_role_abi_v18(&owner)
        } else {
            kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner)
        };
        let completed = std::cell::Cell::new(false);
        let retained = std::cell::Cell::new(None);
        let selected = run_descriptor_role_owner_result_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_pending_global_accesses_v18(
                    original,
                    optimized,
                    issued,
                    if issued { [1, 1] } else { [1, 0] },
                    0,
                    1,
                    &completed,
                    &retained,
                    budget,
                )
            },
        );
        assert!(completed.get());
        assert!(retained.get().is_none());
        assert!(matches!(
            selected.0,
            Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    ProductionSourceOwnedViewErrorV18::Binding("selected pending global callback")
                )
            ))
        ));
        let foreign =
            pending_global_boundary_v18(issued, 3, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(foreign.3);
        assert!(matches!(
            foreign.0,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        let coordinate =
            pending_global_boundary_v18(issued, 4, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(coordinate.3);
        assert!(matches!(
            coordinate.0,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "pending global access changed actual occurrence"
            ))
        ));
    }
}

#[test]
fn pending_global_accesses_unwind_drops_rows_before_restoring_the_original_floor() {
    let completed = std::cell::Cell::new(false);
    let retained = std::cell::Cell::new(None);
    let result = run_descriptor_role_owner_v18(
        descriptor_source_owner(DescriptorCase::READ),
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            let parent = budget.storage();
            let payload = size_of::<&'static str>();
            budget.reserve_storage(payload)?;
            let floor = budget.storage();
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                slice_view_v1::test_pending_global_accesses_v18(
                    original,
                    optimized,
                    false,
                    [1, 0],
                    0,
                    2,
                    &completed,
                    &retained,
                    budget,
                )
            }));
            let panic = caught.expect_err("selected callback must unwind");
            assert_eq!(
                panic.downcast_ref::<&'static str>(),
                Some(&"selected pending global panic")
            );
            assert_eq!(budget.storage(), floor);
            assert!(retained.get().is_none());
            drop(panic);
            budget.release_storage(payload)?;
            assert_eq!(budget.storage(), parent);
            Ok(())
        },
    );
    result.0.unwrap();
    assert!(completed.get());
}

#[test]
fn pending_global_accesses_leave_unused_issuers_and_ordered_stores_unclaimed() {
    for used in [false, true] {
        let owner = if used {
            issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::VolatileOnly)
        } else {
            issued_descriptor_role_owner_with_access_v18(DescriptorRoleSourceV18::Constant, false)
        };
        let abi = issued_descriptor_role_abi_v18(&owner);
        let completed = std::cell::Cell::new(false);
        let retained = std::cell::Cell::new(None);
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_pending_global_accesses_v18(
                    original,
                    optimized,
                    true,
                    [0, 0],
                    usize::from(used),
                    0,
                    &completed,
                    &retained,
                    budget,
                )
            },
        );
        result.0.unwrap();
        assert!(completed.get());
    }
}

#[test]
fn pending_global_accesses_do_not_bypass_volatile_load_namespace_refusal() {
    let positive =
        pending_global_boundary_v18(true, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
    let completed = std::cell::Cell::new(false);
    let retained = std::cell::Cell::new(None);
    let reached = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(
        DescriptorRoleEntranceV18::IssuedDisjointSlice,
        DescriptorRoleSourceV18::VolatileRead,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            reached.set(true);
            slice_view_v1::test_pending_global_accesses_v18(
                original,
                optimized,
                true,
                [0, 0],
                1,
                0,
                &completed,
                &retained,
                budget,
            )
        },
    );
    assert!(reached.get());
    assert!(!completed.get());
    assert!(matches!(
        result.0,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "scalar leaf physical type or Load changed"
        ))
    ));
}

#[test]
fn pending_global_accesses_keep_same_type_wrong_store_value_refusal_before_publication() {
    let positive =
        pending_global_boundary_v18(true, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let _restore =
        Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(descriptor_role_changed_store_v18)));
    DESCRIPTOR_ROLE_CHANGED_STORE_V18.set(0);
    let refused =
        pending_global_boundary_v18(true, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    assert!(!refused.3);
    assert!(DESCRIPTOR_ROLE_CHANGED_STORE_V18.get() > 0);
    assert!(matches!(
        refused.0,
        Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(
                ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source issued pointer differs from its original issuer or actual guard",
                    ..
                }
            )
        ))
    ));
}

#[test]
fn pending_global_accesses_use_the_existing_logarithmic_index_without_query_storage_growth() {
    for vary_issuers in [false, true] {
        let mut previous_operations = 0;
        for size in [1, 2, 8, 16] {
            let (issuers, accesses) = if vary_issuers { (size, 1) } else { (1, size) };
            let captured =
                source_issued_pointer_source_tests_v29::owner_with_shape(issuers, accesses);
            let owner = source_issued_pointer_source_tests_v29::owner_with_shape_uncaptured(
                issuers, accesses,
            );
            assert!(captured.occurrence_storage().is_some());
            assert!(owner.occurrence_storage().is_none());
            assert_eq!(
                owner.source_semantic_sha256(),
                captured.source_semantic_sha256(),
                "capture timing must not alter the genuine source"
            );
            drop(captured);
            let abi = issued_descriptor_role_abi_v18(&owner);
            let observed = std::cell::Cell::new(None);
            let stage = std::cell::Cell::new("before optimized consumer");
            let result = run_descriptor_role_owner_with_abi_v18(
                owner,
                abi,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                |original, optimized, budget| {
                    stage.set("optimized consumer");
                    slice_view_v1::test_pending_global_growth_v18(
                        original,
                        optimized,
                        issuers * accesses,
                        &observed,
                        &stage,
                        budget,
                    )
                },
            );
            assert!(
                result.0.is_ok(),
                "issuers={issuers} accesses={accesses} stage={} observed={:?} work={} peak={}: {:?}",
                stage.get(),
                observed.get(),
                result.1,
                result.2,
                result.0
            );
            result.0.unwrap();
            let [operations, lookup_work] =
                observed.get().expect("genuine source consumer completed");
            assert!(operations > previous_operations);
            assert!(lookup_work <= 1 + 10 * (operations.ilog2() as usize + 1));
            previous_operations = operations;
        }
    }
}

#[test]
fn pending_global_source_construction_destroys_only_temporary_capability_credits() {
    for issuers in [1, 2, 4] {
        let owner = source_issued_pointer_source_tests_v29::owner_with_shape_uncaptured(issuers, 1);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let observed = std::cell::Cell::new(None);
        let stage = std::cell::Cell::new("before optimized consumer");
        CAPABILITY_ORIGIN_SETTLEMENT_V29.set([0; 5]);
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_pending_global_growth_v18(
                    original, optimized, issuers, &observed, &stage, budget,
                )
            },
        );
        result.0.unwrap();
        assert!(observed.get().is_some());
        let [count, total, before, after, own] = CAPABILITY_ORIGIN_SETTLEMENT_V29.get();
        assert!(
            count > 0,
            "authentic source planning must reach resolver destruction"
        );
        assert!(
            total > count * capability_origin_storage_headers_v29().unwrap(),
            "at least one real query/map reservation must be reclaimed"
        );
        assert_eq!(before - after, own);
        assert!(
            after > 0,
            "unrelated retained source/output storage remains paid"
        );
    }
}

#[test]
fn pending_global_commoned_issuer_metadata_has_exact_original_use_and_native_store_joins() {
    for native in [false, true] {
        let owner = source_issued_pointer_source_tests_v29::owner_with_shape_uncaptured(2, 1);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let reached = std::cell::Cell::new(false);
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                if native {
                    slice_view_v1::test_issued_commoned_native_stores_v18(
                        original, optimized, &reached, budget,
                    )
                } else {
                    slice_view_v1::test_issued_commoned_metadata_v18(
                        original, optimized, None, &reached, budget,
                    )
                }
            },
        )
        .0;
        assert!(reached.get(), "native={native}: {result:?}");
        result.unwrap();
    }
}

#[test]
fn pending_global_commoned_issuer_metadata_rejects_substituted_source_use_receiver_and_kind() {
    for fault in 0..8 {
        let owner = source_issued_pointer_source_tests_v29::owner_with_shape_uncaptured(2, 1);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let reached = std::cell::Cell::new(false);
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_issued_commoned_metadata_v18(
                    original,
                    optimized,
                    Some(fault),
                    &reached,
                    budget,
                )
            },
        )
        .0;
        assert!(reached.get(), "fault={fault}: {result:?}");
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(_))),
            "fault={fault}: {result:?}"
        );
    }
}

#[test]
fn pending_global_commoned_issuer_metadata_preserves_whole_exact_and_short_resource_cuts() {
    let run = |work, storage| {
        let owner = source_issued_pointer_source_tests_v29::owner_with_shape_uncaptured(2, 1);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let reached = std::cell::Cell::new(false);
        run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            work,
            storage,
            |original, optimized, budget| {
                slice_view_v1::test_issued_commoned_metadata_v18(
                    original, optimized, None, &reached, budget,
                )
            },
        )
    };
    let (result, work, storage) = run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    let (result, exact_work, exact_storage) = run(work, storage);
    result.unwrap();
    assert_eq!((exact_work, exact_storage), (work, storage));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, accepted_work, accepted_storage) = run(work_limit, storage_limit);
        match (
            is_work,
            source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err()),
        ) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work_limit);
                assert_eq!(error.actual(), work);
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage_limit);
                assert_eq!(error.actual(), storage);
            }
            other => panic!("commoned metadata exact resource cause: {other:?}"),
        }
        assert!(accepted_work <= work_limit && accepted_storage <= storage_limit);
    }
}

fn issued_metadata_closed_consumer_owner_v18(mode: u8) -> ProductionSemanticSsaOwnerV1 {
    let base = source_issued_pointer_source_tests_v29::owner_with_shape_uncaptured(2, 1);
    let function = &base.source_semantic().functions()[0];
    let mut blocks = function.blocks().to_vec();
    assert_eq!(blocks.len(), 9);
    if mode == 0 {
        // Both first-issuer outcomes reach this real source branch. Its constant
        // successor makes the second complete issuer unreachable only in O.
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = blocks[2].terminator().kind()
        else {
            panic!("first issuer guard");
        };
        blocks[2] = SemanticBasicBlockV1::new(
            blocks[2].identity(),
            blocks[2].source(),
            blocks[2].statements().to_vec(),
            SemanticTerminatorV1::new(
                blocks[2].terminator().source(),
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: discriminant.clone(),
                    targets: SemanticSwitchTargetsV1::new(
                        targets.values().to_vec(),
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            SemanticBlockIdV1::from_index(9),
                        ),
                    )
                    .unwrap(),
                },
            ),
        )
        .unwrap();
        blocks[3] = SemanticBasicBlockV1::new(
            blocks[3].identity(),
            blocks[3].source(),
            blocks[3].statements().to_vec(),
            SemanticTerminatorV1::new(
                blocks[3].terminator().source(),
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(9),
                )),
            ),
        )
        .unwrap();
        blocks.push(block(
            91,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Constant(SemanticConstantV1::new(
                    SemanticTypeIdV1::from_index(1),
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap()),
                )),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchValue,
                            SemanticBlockIdV1::from_index(8),
                        ),
                    )],
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::SwitchOtherwise,
                        SemanticBlockIdV1::from_index(4),
                    ),
                )
                .unwrap(),
            },
        ));
    } else {
        assert_eq!(mode, 1);
        // One actual witness definition feeds both issuer calls. The receiver
        // loans and distinct Option producers remain separate source events.
        for index in [1, 5] {
            let SemanticTerminatorKindV1::Call(call) = blocks[index].terminator().kind() else {
                panic!("original issuer call");
            };
            let mut arguments = call.arguments().to_vec();
            let SemanticOperandV1::Move(witness) = &arguments[1] else {
                panic!("original witness operand");
            };
            arguments[1] = SemanticOperandV1::Copy(witness.clone());
            blocks[index] = SemanticBasicBlockV1::new(
                blocks[index].identity(),
                blocks[index].source(),
                blocks[index].statements().to_vec(),
                SemanticTerminatorV1::new(
                    blocks[index].terminator().source(),
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            call.callee(),
                            arguments,
                            call.destination().cloned(),
                            call.unwind(),
                        )
                        .unwrap(),
                    ),
                ),
            )
            .unwrap();
        }
        blocks[4] = SemanticBasicBlockV1::new(
            blocks[4].identity(),
            blocks[4].source(),
            blocks[4].statements().to_vec(),
            SemanticTerminatorV1::new(
                blocks[4].terminator().source(),
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(5),
                )),
            ),
        )
        .unwrap();
    }
    global_native_rebuild_owner_v18(&base, function.locals().to_vec(), blocks)
}

fn issued_metadata_two_root_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let base = source_issued_pointer_source_tests_v29::owner_with_shape_uncaptured(1, 1);
    let source = base.source_semantic();
    let first = &source.functions()[0];
    // Defined callables are a dense prefix. Inserting the second root shifts
    // both intrinsic declarations and every original call to those declarations.
    let blocks: Vec<_> = first
        .blocks()
        .iter()
        .map(|block| {
            let terminator = match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => {
                    assert!([1, 2].contains(&call.callee().index()));
                    SemanticTerminatorV1::new(
                        block.terminator().source(),
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new_callable(
                                SemanticCallableIdV1::from_index(call.callee().index() + 1),
                                call.arguments().to_vec(),
                                call.destination().cloned(),
                                call.unwind(),
                            )
                            .unwrap(),
                        ),
                    )
                }
                _ => block.terminator().clone(),
            };
            SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                block.statements().to_vec(),
                terminator,
            )
            .unwrap()
        })
        .collect();
    let initial = SemanticFunctionDeclV1::new(
        first.identity(),
        first.role(),
        first.item_definition_identity(),
        first.monomorphization_identity(),
        first.generic_type_arguments_identity(),
        first.const_generic_arguments_identity(),
        first.source(),
        first.abi().clone(),
        first.locals().to_vec(),
        first.entry(),
        blocks.clone(),
    )
    .unwrap()
    .with_kernel_entry(first.kernel_entry().unwrap().clone());
    let second = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([110; 32]),
        first.role(),
        SemanticItemDefinitionIdentityV1::from_sha256([111; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([112; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([113; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([114; 32]),
        first.source(),
        first.abi().clone(),
        first.locals().to_vec(),
        first.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"issued_metadata_second_root".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([115; 32]),
        first.kernel_entry().unwrap().source_contract(),
    ));
    let mut callables = source.callables().to_vec();
    callables.insert(
        1,
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        vec![initial, second],
        callables,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn pending_global_commoned_metadata_keeps_unreachable_and_rewritten_consumers_closed() {
    for mode in [0, 1] {
        let owner = issued_metadata_closed_consumer_owner_v18(mode);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_issued_metadata_closed_consumer_v18(
                    original, optimized, mode, &reached, budget,
                )
            },
        );
        result.unwrap();
        assert!(reached.get(), "authentic closed consumer mode {mode}");
    }
}

#[test]
fn pending_global_generic_helper_accesses_preserve_actual_space_and_exact_issuer() {
    for nested in [false, true] {
        let owner = global_expression_helper_owner_v23(nested);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let count = std::cell::Cell::new(0);
        let (result, _, _) = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                count.set(slice_view_v1::test_issued_generic_global_domains_v26(
                    original, optimized, budget,
                )?);
                Ok(())
            },
        );
        result.unwrap();
        assert!(
            count.get() > 0,
            "nested={nested}: genuine Generic helper accesses"
        );
    }
}

#[test]
fn pending_global_generic_helper_domains_have_exact_and_one_short_transaction_limits() {
    let run = |work, storage| {
        let owner = global_expression_helper_owner_v23(true);
        let abi = issued_descriptor_role_abi_v18(&owner);
        run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            work,
            storage,
            |original, optimized, budget| {
                assert!(
                    slice_view_v1::test_issued_generic_global_domains_v26(
                        original, optimized, budget
                    )? > 0
                );
                Ok(())
            },
        )
    };
    let (result, work, peak) = run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(work > 0 && peak > MODULE_FLOOR);
    let (result, exact_work, exact_peak) = run(work, peak);
    result.unwrap();
    assert_eq!((exact_work, exact_peak), (work, peak));
    assert!(matches!(
        source_slot_tests::original_repeated_source_resource_v29(
            run(work - 1, peak).0.unwrap_err()
        ),
        ArgumentResourceV1::Work(_)
    ));
    assert!(matches!(
        source_slot_tests::original_repeated_source_resource_v29(
            run(work, peak - 1).0.unwrap_err()
        ),
        ArgumentResourceV1::Storage(_)
    ));
}

#[test]
fn pending_global_commoned_metadata_rejects_genuine_foreign_function_coordinates() {
    for mode in [2, 3] {
        let owner = issued_metadata_two_root_owner_v18();
        let mut abi = issued_descriptor_role_abi_v18(&owner);
        let first = abi
            .arguments_mut(0)
            .iter()
            .map(|argument| {
                let ProductionKernelArgumentAbiKindV18::Descriptor {
                    source,
                    argument: descriptor,
                } = &argument.kind
                else {
                    panic!("genuine issued descriptor input");
                };
                ProductionKernelArgumentAbiArgumentV18 {
                    semantic_type_identity: argument.semantic_type_identity,
                    kind: ProductionKernelArgumentAbiKindV18::Descriptor {
                        source: source.clone(),
                        argument: descriptor.clone(),
                    },
                }
            })
            .collect();
        *abi.arguments_mut(1) = first;
        assert_eq!(abi.roots().len(), 2);
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |original, optimized, budget| {
                slice_view_v1::test_issued_metadata_closed_consumer_v18(
                    original, optimized, mode, &reached, budget,
                )
            },
        );
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
        ));
        assert!(reached.get(), "genuine foreign-function mode {mode}");
    }
}
